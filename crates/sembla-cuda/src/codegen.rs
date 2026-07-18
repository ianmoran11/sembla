use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sembla_ir::{
    AggOp, AttrType, ClaimOrdering, Effect, Expr, ParamType, Table, ValidatedModel,
};
use sha2::{Digest, Sha256};

use crate::CudaError;

pub const DUMP_ENV: &str = "SEMBLA_CUDA_DUMP_DIR";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedCuda {
    pub source: String,
    pub source_sha256: String,
    pub transition_kernels: Vec<String>,
    /// Global table index supplying the result length of each generated
    /// group aggregate, in generated aggregate order.
    pub aggregate_group_tables: Vec<usize>,
}

impl GeneratedCuda {
    /// Dumps deterministic source to a content-addressed file when
    /// `SEMBLA_CUDA_DUMP_DIR` is set. An existing identical file is reused.
    pub fn dump_if_requested(&self) -> Result<Option<PathBuf>, CudaError> {
        let Some(directory) = std::env::var_os(DUMP_ENV) else {
            return Ok(None);
        };
        dump_source(Path::new(&directory), &self.source_sha256, &self.source).map(Some)
    }
}

fn dump_source(directory: &Path, hash: &str, source: &str) -> Result<PathBuf, CudaError> {
    std::fs::create_dir_all(directory).map_err(|error| {
        CudaError::Dump(format!("cannot create '{}': {error}", directory.display()))
    })?;
    let path = directory.join(format!("{hash}.cu"));
    if path.exists() {
        let existing = std::fs::read(&path).map_err(|error| {
            CudaError::Dump(format!("cannot read '{}': {error}", path.display()))
        })?;
        if existing == source.as_bytes() {
            return Ok(path);
        }
        return Err(CudaError::Dump(format!(
            "content-addressed path '{}' contains different bytes",
            path.display()
        )));
    }
    let temporary = directory.join(format!(".{hash}.{}.tmp", std::process::id()));
    std::fs::write(&temporary, source).map_err(|error| {
        CudaError::Dump(format!("cannot write '{}': {error}", temporary.display()))
    })?;
    std::fs::rename(&temporary, &path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        CudaError::Dump(format!("cannot install '{}': {error}", path.display()))
    })?;
    Ok(path)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Ty {
    Real,
    Int,
    Bool,
    Enum(Vec<String>),
    Ref(String),
}

impl Ty {
    fn cuda(&self) -> &'static str {
        match self {
            Self::Real => "double",
            Self::Int => "long long",
            Self::Bool => "int",
            Self::Enum(_) => "unsigned short",
            Self::Ref(_) => "unsigned int",
        }
    }

    fn numeric(&self) -> bool {
        matches!(self, Self::Real | Self::Int)
    }
}

impl From<&AttrType> for Ty {
    fn from(value: &AttrType) -> Self {
        match value {
            AttrType::Real => Self::Real,
            AttrType::Int => Self::Int,
            AttrType::Enum { variants } => Self::Enum(variants.clone()),
            AttrType::Ref { table } => Self::Ref(table.clone()),
        }
    }
}

#[derive(Clone, Copy)]
enum Rows {
    State { box_index: usize, table_index: usize },
    Input { box_index: usize, port_index: usize },
}

#[derive(Clone)]
struct AggSpec {
    key: String,
    box_index: usize,
    target_table_index: usize,
    group_table_index: usize,
    target_fk_column: usize,
    self_fk_column: usize,
    op: AggOp,
    filter: Expr,
    ty: Ty,
}

#[derive(Clone)]
struct InputSpec {
    key: String,
    box_index: usize,
    port_index: usize,
    agg: sembla_ir::Aggregate,
    ty: Ty,
}

struct Generator<'a> {
    model: &'a ValidatedModel,
    global_tables: Vec<(usize, usize)>,
    columns: Vec<(usize, usize, usize)>,
    ports: Vec<(usize, usize)>,
    input_fields: Vec<(usize, usize, usize)>,
    params: BTreeMap<String, usize>,
    aggs: Vec<AggSpec>,
    inputs: Vec<InputSpec>,
}

impl<'a> Generator<'a> {
    fn new(model: &'a ValidatedModel) -> Result<Self, CudaError> {
        let mut global_tables = Vec::new();
        let mut columns = Vec::new();
        let mut ports = Vec::new();
        let mut input_fields = Vec::new();
        for (box_index, model_box) in model.model().boxes.iter().enumerate() {
            for (table_index, table) in model_box.tables.iter().enumerate() {
                global_tables.push((box_index, table_index));
                for attr_index in 0..table.attrs.len() {
                    columns.push((box_index, table_index, attr_index));
                }
            }
            for (port_index, port) in model_box.inputs.iter().enumerate() {
                ports.push((box_index, port_index));
                for field_index in 0..port.schema.len() {
                    input_fields.push((box_index, port_index, field_index));
                }
            }
        }
        let params = model
            .model()
            .params
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.name.clone(), index))
            .collect();
        let mut this = Self {
            model,
            global_tables,
            columns,
            ports,
            input_fields,
            params,
            aggs: Vec::new(),
            inputs: Vec::new(),
        };
        this.collect_all()?;
        Ok(this)
    }

    fn collect_all(&mut self) -> Result<(), CudaError> {
        for (box_index, model_box) in self.model.model().boxes.iter().enumerate() {
            for transition in &model_box.transitions {
                let table_index = self.table_index(box_index, &transition.table)?;
                self.collect_expr(box_index, table_index, &transition.guard)?;
                self.collect_expr(box_index, table_index, &transition.hazard)?;
                for effect in &transition.effects {
                    let Effect::SetAttr { value, .. } = effect;
                    self.collect_expr(box_index, table_index, value)?;
                }
                for claim in &transition.contests {
                    self.collect_expr(box_index, table_index, &claim.resource)?;
                    if let ClaimOrdering::Key { expr } = &claim.ordering {
                        self.collect_expr(box_index, table_index, expr)?;
                    }
                }
            }
            for output in &model_box.outputs {
                let sembla_ir::OutputBuilder::PerTable { table, fields } = &output.builder;
                let table_index = self.table_index(box_index, table)?;
                for field in fields {
                    if let Some(filter) = &field.filter {
                        self.collect_expr(box_index, table_index, filter)?;
                    }
                    if let AggOp::Sum { value } = &field.op {
                        self.collect_expr(box_index, table_index, value)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn collect_expr(
        &mut self,
        box_index: usize,
        query_table_index: usize,
        expr: &Expr,
    ) -> Result<(), CudaError> {
        match expr {
            Expr::Add { lhs, rhs }
            | Expr::Sub { lhs, rhs }
            | Expr::Mul { lhs, rhs }
            | Expr::Div { lhs, rhs }
            | Expr::Eq { lhs, rhs }
            | Expr::Ne { lhs, rhs }
            | Expr::Lt { lhs, rhs }
            | Expr::Le { lhs, rhs }
            | Expr::Gt { lhs, rhs }
            | Expr::Ge { lhs, rhs }
            | Expr::And { lhs, rhs }
            | Expr::Or { lhs, rhs } => {
                self.collect_expr(box_index, query_table_index, lhs)?;
                self.collect_expr(box_index, query_table_index, rhs)?;
            }
            Expr::Not { expr } => self.collect_expr(box_index, query_table_index, expr)?,
            Expr::Input { port, agg } => {
                if let Some(filter) = &agg.filter {
                    let port_index = self.port_index(box_index, port)?;
                    self.collect_input_expr(box_index, port_index, filter)?;
                }
                if let AggOp::Sum { value } = &agg.op {
                    let port_index = self.port_index(box_index, port)?;
                    self.collect_input_expr(box_index, port_index, value)?;
                }
                let key = format!("{box_index}:{port}:{agg:?}");
                if !self.inputs.iter().any(|entry| entry.key == key) {
                    let port_index = self.port_index(box_index, port)?;
                    let ty = match &agg.op {
                        AggOp::Count => Ty::Int,
                        AggOp::Sum { value } => {
                            self.infer(value, Rows::Input { box_index, port_index }, None)?
                        }
                    };
                    self.inputs.push(InputSpec {
                        key,
                        box_index,
                        port_index,
                        agg: agg.clone(),
                        ty,
                    });
                }
            }
            Expr::Agg {
                op,
                table,
                on,
                filter,
            } => {
                let target_table_index = self.table_index(box_index, table)?;
                self.collect_expr(box_index, target_table_index, filter)?;
                if let AggOp::Sum { value } = op {
                    self.collect_expr(box_index, target_table_index, value)?;
                }
                let key = format!("{box_index}:{query_table_index}:{expr:?}");
                if !self.aggs.iter().any(|entry| entry.key == key) {
                    let query_table = &self.model.model().boxes[box_index].tables[query_table_index];
                    let target_table = &self.model.model().boxes[box_index].tables[target_table_index];
                    let self_fk_column = attr_index(query_table, &on.self_fk_attr)?;
                    let target_fk_column = attr_index(target_table, &on.fk_attr)?;
                    let group_name = match &query_table.attrs[self_fk_column].ty {
                        AttrType::Ref { table } => table,
                        _ => return Err(codegen("aggregate self key is not Ref")),
                    };
                    let group_table_index = self.table_index(box_index, group_name)?;
                    let ty = match op {
                        AggOp::Count => Ty::Int,
                        AggOp::Sum { value } => self.infer(
                            value,
                            Rows::State {
                                box_index,
                                table_index: target_table_index,
                            },
                            None,
                        )?,
                    };
                    self.aggs.push(AggSpec {
                        key,
                        box_index,
                        target_table_index,
                        group_table_index,
                        target_fk_column,
                        self_fk_column,
                        op: op.clone(),
                        filter: (**filter).clone(),
                        ty,
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn collect_input_expr(
        &mut self,
        box_index: usize,
        port_index: usize,
        expr: &Expr,
    ) -> Result<(), CudaError> {
        match expr {
            Expr::Add { lhs, rhs }
            | Expr::Sub { lhs, rhs }
            | Expr::Mul { lhs, rhs }
            | Expr::Div { lhs, rhs }
            | Expr::Eq { lhs, rhs }
            | Expr::Ne { lhs, rhs }
            | Expr::Lt { lhs, rhs }
            | Expr::Le { lhs, rhs }
            | Expr::Gt { lhs, rhs }
            | Expr::Ge { lhs, rhs }
            | Expr::And { lhs, rhs }
            | Expr::Or { lhs, rhs } => {
                self.collect_input_expr(box_index, port_index, lhs)?;
                self.collect_input_expr(box_index, port_index, rhs)?;
            }
            Expr::Not { expr } => self.collect_input_expr(box_index, port_index, expr)?,
            Expr::Input { .. } | Expr::Agg { .. } => {
                return Err(codegen("nested Input/Agg inside an input aggregate is unsupported"));
            }
            _ => {}
        }
        Ok(())
    }

    fn table_index(&self, box_index: usize, name: &str) -> Result<usize, CudaError> {
        self.model.model().boxes[box_index]
            .tables
            .iter()
            .position(|table| table.name == name)
            .ok_or_else(|| codegen(format!("unknown table '{name}'")))
    }

    fn global_table(&self, box_index: usize, table_index: usize) -> usize {
        self.global_tables
            .iter()
            .position(|entry| *entry == (box_index, table_index))
            .expect("validated table is indexed")
    }

    fn column(&self, box_index: usize, table_index: usize, attr_index: usize) -> usize {
        self.columns
            .iter()
            .position(|entry| *entry == (box_index, table_index, attr_index))
            .expect("validated column is indexed")
    }

    fn port_index(&self, box_index: usize, name: &str) -> Result<usize, CudaError> {
        self.model.model().boxes[box_index]
            .inputs
            .iter()
            .position(|port| port.name == name)
            .ok_or_else(|| codegen(format!("unknown input port '{name}'")))
    }

    fn port(&self, box_index: usize, port_index: usize) -> usize {
        self.ports
            .iter()
            .position(|entry| *entry == (box_index, port_index))
            .expect("validated input port is indexed")
    }

    fn input_field(&self, box_index: usize, port_index: usize, field_index: usize) -> usize {
        self.input_fields
            .iter()
            .position(|entry| *entry == (box_index, port_index, field_index))
            .expect("validated input field is indexed")
    }

    fn infer(&self, expr: &Expr, rows: Rows, expected: Option<&Ty>) -> Result<Ty, CudaError> {
        match expr {
            Expr::Real { .. } => Ok(Ty::Real),
            Expr::Int { .. } => Ok(Ty::Int),
            Expr::Bool { .. } => Ok(Ty::Bool),
            Expr::Enum { variant } => match expected {
                Some(Ty::Enum(variants)) if variants.iter().any(|item| item == variant) => {
                    Ok(Ty::Enum(variants.clone()))
                }
                _ => Err(codegen(format!(
                    "enum literal '{variant}' lacks destination enum context"
                ))),
            },
            Expr::Param { name } => self
                .model
                .model()
                .params
                .iter()
                .find(|parameter| parameter.name == *name)
                .map(|parameter| match parameter.ty {
                    ParamType::Real => Ty::Real,
                    ParamType::Int => Ty::Int,
                })
                .ok_or_else(|| codegen(format!("unknown parameter '{name}'"))),
            Expr::SelfAttr { name } => self.row_attr(rows, name).map(|attr| Ty::from(&attr.ty)),
            Expr::Add { lhs, rhs } | Expr::Sub { lhs, rhs } | Expr::Mul { lhs, rhs } => {
                let left = self.infer(lhs, rows, None)?;
                let right = self.infer(rhs, rows, None)?;
                if !left.numeric() || !right.numeric() {
                    return Err(codegen("arithmetic operand is not numeric"));
                }
                if left == Ty::Real || right == Ty::Real {
                    Ok(Ty::Real)
                } else {
                    Ok(Ty::Int)
                }
            }
            Expr::Div { lhs, rhs } => {
                if !self.infer(lhs, rows, None)?.numeric()
                    || !self.infer(rhs, rows, None)?.numeric()
                {
                    return Err(codegen("division operand is not numeric"));
                }
                Ok(Ty::Real)
            }
            Expr::Eq { .. }
            | Expr::Ne { .. }
            | Expr::Lt { .. }
            | Expr::Le { .. }
            | Expr::Gt { .. }
            | Expr::Ge { .. }
            | Expr::And { .. }
            | Expr::Or { .. }
            | Expr::Not { .. }
            | Expr::EnumIs { .. } => Ok(Ty::Bool),
            Expr::Input { port, agg } => {
                let key = format!("{}:{port}:{agg:?}", rows_box(rows));
                self.inputs
                    .iter()
                    .find(|entry| entry.key == key)
                    .map(|entry| entry.ty.clone())
                    .ok_or_else(|| codegen("input aggregate was not collected"))
            }
            Expr::Agg { .. } => {
                let (box_index, table_index) = rows_state(rows)?;
                let key = format!("{box_index}:{table_index}:{expr:?}");
                self.aggs
                    .iter()
                    .find(|entry| entry.key == key)
                    .map(|entry| entry.ty.clone())
                    .ok_or_else(|| codegen("group aggregate was not collected"))
            }
        }
    }

    fn row_attr(&self, rows: Rows, name: &str) -> Result<&sembla_ir::Attr, CudaError> {
        match rows {
            Rows::State {
                box_index,
                table_index,
            } => self.model.model().boxes[box_index].tables[table_index]
                .attrs
                .iter()
                .find(|attr| attr.name == name)
                .ok_or_else(|| codegen(format!("unknown state attribute '{name}'"))),
            Rows::Input {
                box_index,
                port_index,
            } => self.model.model().boxes[box_index].inputs[port_index]
                .schema
                .iter()
                .find(|attr| attr.name == name)
                .ok_or_else(|| codegen(format!("unknown input attribute '{name}'"))),
        }
    }

    fn render(
        &self,
        expr: &Expr,
        rows: Rows,
        expected: Option<&Ty>,
        state_name: &str,
        row_name: &str,
    ) -> Result<(String, Ty), CudaError> {
        let result = match expr {
            Expr::Real { value } => (f64_literal(*value), Ty::Real),
            Expr::Int { value } => (format!("{value}LL"), Ty::Int),
            Expr::Bool { value } => ((if *value { "1" } else { "0" }).to_owned(), Ty::Bool),
            Expr::Enum { variant } => {
                let Ty::Enum(variants) = expected.ok_or_else(|| {
                    codegen(format!("enum literal '{variant}' lacks destination context"))
                })? else {
                    return Err(codegen("enum literal destination is not Enum"));
                };
                let index = variants
                    .iter()
                    .position(|item| item == variant)
                    .ok_or_else(|| codegen(format!("unknown enum variant '{variant}'")))?;
                (format!("{index}U"), Ty::Enum(variants.clone()))
            }
            Expr::Param { name } => {
                let index = *self
                    .params
                    .get(name)
                    .ok_or_else(|| codegen(format!("unknown parameter '{name}'")))?;
                let ty = self.infer(expr, rows, expected)?;
                (
                    format!(
                        "(*((const {}*)(params + {}ULL)))",
                        ty.cuda(),
                        index * 8
                    ),
                    ty,
                )
            }
            Expr::SelfAttr { name } => self.render_attr(rows, name, state_name, row_name)?,
            Expr::Add { lhs, rhs } | Expr::Sub { lhs, rhs } | Expr::Mul { lhs, rhs } => {
                let (left, left_ty) = self.render(lhs, rows, None, state_name, row_name)?;
                let (right, right_ty) = self.render(rhs, rows, None, state_name, row_name)?;
                let ty = if left_ty == Ty::Real || right_ty == Ty::Real {
                    Ty::Real
                } else {
                    Ty::Int
                };
                if ty == Ty::Int {
                    let helper = match expr {
                        Expr::Add { .. } => "sembla_add_i64",
                        Expr::Sub { .. } => "sembla_sub_i64",
                        Expr::Mul { .. } => "sembla_mul_i64",
                        _ => unreachable!(),
                    };
                    (format!("{helper}({left}, {right}, error)"), ty)
                } else {
                    let operator = match expr {
                        Expr::Add { .. } => "+",
                        Expr::Sub { .. } => "-",
                        Expr::Mul { .. } => "*",
                        _ => unreachable!(),
                    };
                    (
                        format!(
                            "((double)({left}) {operator} (double)({right}))"
                        ),
                        ty,
                    )
                }
            }
            Expr::Div { lhs, rhs } => {
                let (left, _) = self.render(lhs, rows, None, state_name, row_name)?;
                let (right, _) = self.render(rhs, rows, None, state_name, row_name)?;
                (
                    format!("((double)({left}) / (double)({right}))"),
                    Ty::Real,
                )
            }
            Expr::Eq { lhs, rhs }
            | Expr::Ne { lhs, rhs }
            | Expr::Lt { lhs, rhs }
            | Expr::Le { lhs, rhs }
            | Expr::Gt { lhs, rhs }
            | Expr::Ge { lhs, rhs } => {
                let left_hint = self.infer(lhs, rows, None).ok();
                let right_hint = self.infer(rhs, rows, left_hint.as_ref()).ok();
                let (left, left_ty) = self.render(
                    lhs,
                    rows,
                    right_hint.as_ref(),
                    state_name,
                    row_name,
                )?;
                let (right, right_ty) =
                    self.render(rhs, rows, Some(&left_ty), state_name, row_name)?;
                let numeric_mixed = left_ty.numeric()
                    && right_ty.numeric()
                    && (left_ty == Ty::Real || right_ty == Ty::Real);
                let left = if numeric_mixed {
                    format!("(double)({left})")
                } else {
                    left
                };
                let right = if numeric_mixed {
                    format!("(double)({right})")
                } else {
                    right
                };
                let operator = match expr {
                    Expr::Eq { .. } => "==",
                    Expr::Ne { .. } => "!=",
                    Expr::Lt { .. } => "<",
                    Expr::Le { .. } => "<=",
                    Expr::Gt { .. } => ">",
                    Expr::Ge { .. } => ">=",
                    _ => unreachable!(),
                };
                (format!("(({left}) {operator} ({right}))"), Ty::Bool)
            }
            Expr::And { lhs, rhs } | Expr::Or { lhs, rhs } => {
                let (left, _) = self.render(lhs, rows, Some(&Ty::Bool), state_name, row_name)?;
                let (right, _) = self.render(rhs, rows, Some(&Ty::Bool), state_name, row_name)?;
                let operator = if matches!(expr, Expr::And { .. }) { "&" } else { "|" };
                (format!("(({left}) {operator} ({right}))"), Ty::Bool)
            }
            Expr::Not { expr } => {
                let (value, _) = self.render(expr, rows, Some(&Ty::Bool), state_name, row_name)?;
                (format!("(!({value}))"), Ty::Bool)
            }
            Expr::EnumIs { attr, variant } => {
                let attr_decl = self.row_attr(rows, attr)?;
                let AttrType::Enum { variants } = &attr_decl.ty else {
                    return Err(codegen("enum_is attribute is not Enum"));
                };
                let index = variants
                    .iter()
                    .position(|item| item == variant)
                    .ok_or_else(|| codegen(format!("unknown enum variant '{variant}'")))?;
                let (value, _) = self.render_attr(rows, attr, state_name, row_name)?;
                (format!("(({value}) == {index}U)"), Ty::Bool)
            }
            Expr::Input { port, agg } => {
                let box_index = rows_box(rows);
                let key = format!("{box_index}:{port}:{agg:?}");
                let index = self
                    .inputs
                    .iter()
                    .position(|entry| entry.key == key)
                    .ok_or_else(|| codegen("input aggregate was not collected"))?;
                (
                    format!(
                        "sembla_input_{index}(inputs, input_offsets, input_counts, params, error)"
                    ),
                    self.inputs[index].ty.clone(),
                )
            }
            Expr::Agg { .. } => {
                let (box_index, table_index) = rows_state(rows)?;
                let key = format!("{box_index}:{table_index}:{expr:?}");
                let index = self
                    .aggs
                    .iter()
                    .position(|entry| entry.key == key)
                    .ok_or_else(|| codegen("group aggregate was not collected"))?;
                let spec = &self.aggs[index];
                let query = &self.model.model().boxes[box_index].tables[table_index];
                let attr = &query.attrs[spec.self_fk_column];
                let (group, _) = self.render_attr(rows, &attr.name, state_name, row_name)?;
                (
                    format!(
                        "(*((const {}*)(aggs + agg_offsets[{index}]) + (unsigned long long)({group})))",
                        spec.ty.cuda()
                    ),
                    spec.ty.clone(),
                )
            }
        };
        Ok(result)
    }

    fn render_attr(
        &self,
        rows: Rows,
        name: &str,
        state_name: &str,
        row_name: &str,
    ) -> Result<(String, Ty), CudaError> {
        match rows {
            Rows::State {
                box_index,
                table_index,
            } => {
                let table = &self.model.model().boxes[box_index].tables[table_index];
                let index = attr_index(table, name)?;
                let ty = Ty::from(&table.attrs[index].ty);
                let column = self.column(box_index, table_index, index);
                Ok((
                    format!(
                        "(*((const {}*)({state_name} + column_offsets[{column}]) + (unsigned long long)({row_name})))",
                        ty.cuda()
                    ),
                    ty,
                ))
            }
            Rows::Input {
                box_index,
                port_index,
            } => {
                let port = &self.model.model().boxes[box_index].inputs[port_index];
                let index = port
                    .schema
                    .iter()
                    .position(|attr| attr.name == name)
                    .ok_or_else(|| codegen(format!("unknown input attribute '{name}'")))?;
                let ty = Ty::from(&port.schema[index].ty);
                let field = self.input_field(box_index, port_index, index);
                Ok((
                    format!(
                        "(*((const {}*)(inputs + input_offsets[{field}]) + (unsigned long long)({row_name})))",
                        ty.cuda()
                    ),
                    ty,
                ))
            }
        }
    }

    fn emit(self) -> Result<GeneratedCuda, CudaError> {
        let mut out = String::new();
        writeln!(out, "// Generated by sembla-cuda {}. DO NOT EDIT.", env!("CARGO_PKG_VERSION")).unwrap();
        writeln!(out, "// model: {}", self.model.model().name).unwrap();
        out.push_str(PRELUDE);
        self.emit_input_helpers(&mut out)?;
        self.emit_aggregate_kernel(&mut out)?;
        let transition_kernels = self.emit_transition_kernels(&mut out)?;
        self.emit_error_check_kernel(&mut out);
        self.emit_resolve_kernel(&mut out)?;
        self.emit_apply_kernel(&mut out)?;
        self.emit_output_kernel(&mut out)?;
        out.push_str(PHILOX_TEST_KERNEL);
        let source_sha256 = hex(Sha256::digest(out.as_bytes()).as_slice());
        let aggregate_group_tables = self
            .aggs
            .iter()
            .map(|spec| self.global_table(spec.box_index, spec.group_table_index))
            .collect();
        Ok(GeneratedCuda {
            source: out,
            source_sha256,
            transition_kernels,
            aggregate_group_tables,
        })
    }

    fn emit_input_helpers(&self, out: &mut String) -> Result<(), CudaError> {
        for (index, spec) in self.inputs.iter().enumerate() {
            writeln!(out, "__device__ __forceinline__ {} sembla_input_{index}(const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, unsigned char* error) {{", spec.ty.cuda()).unwrap();
            let port = self.port(spec.box_index, spec.port_index);
            match &spec.agg.op {
                AggOp::Count => {
                    writeln!(out, "  long long result = 0LL;").unwrap();
                    writeln!(out, "  for (unsigned long long row = 0; row < input_counts[{port}]; ++row) {{").unwrap();
                    let selected = if let Some(filter) = &spec.agg.filter {
                        self.render(filter, Rows::Input { box_index: spec.box_index, port_index: spec.port_index }, Some(&Ty::Bool), "state", "row")?.0
                    } else { "1".to_owned() };
                    writeln!(out, "    if ({selected}) result = sembla_add_i64(result, 1LL, error);").unwrap();
                    writeln!(out, "  }}\n  return result;\n}}").unwrap();
                }
                AggOp::Sum { value } => {
                    writeln!(out, "  {} result = ({})(0);", spec.ty.cuda(), spec.ty.cuda()).unwrap();
                    writeln!(out, "  for (unsigned long long row = 0; row < input_counts[{port}]; ++row) {{").unwrap();
                    let selected = if let Some(filter) = &spec.agg.filter {
                        self.render(filter, Rows::Input { box_index: spec.box_index, port_index: spec.port_index }, Some(&Ty::Bool), "state", "row")?.0
                    } else { "1".to_owned() };
                    let rendered = self.render(value, Rows::Input { box_index: spec.box_index, port_index: spec.port_index }, Some(&spec.ty), "state", "row")?.0;
                    if spec.ty == Ty::Int {
                        writeln!(out, "    if ({selected}) result = sembla_add_i64(result, {rendered}, error);").unwrap();
                    } else {
                        writeln!(out, "    if ({selected}) result = result + (double)({rendered});").unwrap();
                    }
                    writeln!(out, "  }}\n  return result;\n}}").unwrap();
                }
            }
        }
        Ok(())
    }

    fn emit_aggregate_kernel(&self, out: &mut String) -> Result<(), CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_build_aggregates(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, unsigned char* aggs, const unsigned long long* agg_offsets, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0) return;\n  status[0] = 0ULL; status[1] = 0ULL; status[2] = 0ULL; status[3] = 0ULL;\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");
        for (index, spec) in self.aggs.iter().enumerate() {
            let groups = self.global_table(spec.box_index, spec.group_table_index);
            let target = self.global_table(spec.box_index, spec.target_table_index);
            writeln!(out, "  {}* agg_{index} = ({}*)(aggs + agg_offsets[{index}]);", spec.ty.cuda(), spec.ty.cuda()).unwrap();
            writeln!(out, "  for (unsigned long long group = 0; group < row_counts[{groups}]; ++group) agg_{index}[group] = ({})0;", spec.ty.cuda()).unwrap();
            writeln!(out, "  for (unsigned long long row = 0; row < row_counts[{target}]; ++row) {{").unwrap();
            let rows = Rows::State { box_index: spec.box_index, table_index: spec.target_table_index };
            let filter = self.render(&spec.filter, rows, Some(&Ty::Bool), "state", "row")?.0;
            let fk_attr = &self.model.model().boxes[spec.box_index].tables[spec.target_table_index].attrs[spec.target_fk_column].name;
            let group = self.render_attr(rows, fk_attr, "state", "row")?.0;
            writeln!(out, "    if ({filter}) {{ unsigned int group = {group}; if ((unsigned long long)group >= row_counts[{groups}]) {{ status[0] = 1ULL; status[1] = {index}ULL; return; }}").unwrap();
            match &spec.op {
                AggOp::Count => writeln!(out, "      agg_{index}[group] = sembla_add_i64(agg_{index}[group], 1LL, error);").unwrap(),
                AggOp::Sum { value } => {
                    let rendered = self.render(value, rows, Some(&spec.ty), "state", "row")?.0;
                    if spec.ty == Ty::Int {
                        writeln!(out, "      agg_{index}[group] = sembla_add_i64(agg_{index}[group], {rendered}, error);").unwrap();
                    } else {
                        writeln!(out, "      agg_{index}[group] = agg_{index}[group] + (double)({rendered});").unwrap();
                    }
                }
            }
            writeln!(out, "      if (local_error) {{ status[0] = 2ULL; status[1] = {index}ULL; return; }} }}\n  }}").unwrap();
        }
        out.push_str("}\n");
        Ok(())
    }

    fn emit_transition_kernels(&self, out: &mut String) -> Result<Vec<String>, CudaError> {
        let mut names = Vec::new();
        for validated in self.model.transitions() {
            let transition = &self.model.model().boxes[validated.box_index].transitions[validated.transition_index];
            let table_index = self.table_index(validated.box_index, &transition.table)?;
            let global_table = self.global_table(validated.box_index, table_index);
            let name = format!("sembla_transition_{:08x}", validated.rule_id);
            names.push(name.clone());
            writeln!(out, "\nextern \"C\" __global__ void {name}(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, unsigned long long seed, unsigned int tick, double dt, unsigned char* enabled, double* times, unsigned char* errors) {{").unwrap();
            out.push_str("  unsigned long long row = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n");
            writeln!(out, "  if (row >= row_counts[{global_table}]) return;\n  unsigned long long candidate = candidate_offsets[{}] + row;\n  unsigned char local_error = 0; unsigned char* error = &local_error;", validated.rule_id).unwrap();
            let rows = Rows::State { box_index: validated.box_index, table_index };
            let guard = self.render(&transition.guard, rows, Some(&Ty::Bool), "state", "row")?.0;
            let hazard = self.render(&transition.hazard, rows, Some(&Ty::Real), "state", "row")?.0;
            writeln!(out, "  int guard = {guard};\n  double lambda = (double)({hazard});\n  double time = sembla_exp(seed, tick, {}U, (unsigned int)row, 0U, lambda);\n  errors[candidate] = local_error;\n  times[candidate] = time;\n  enabled[candidate] = (unsigned char)(!local_error && guard && lambda > 0.0 && time < dt);\n}}", validated.rule_id).unwrap();
        }
        Ok(names)
    }

    fn emit_error_check_kernel(&self, out: &mut String) {
        out.push_str("\nextern \"C\" __global__ void sembla_check_candidate_errors(const unsigned char* errors, unsigned long long candidate_count, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  for (unsigned long long i = 0; i < candidate_count; ++i) { if (errors[i]) { status[0] = 3ULL; status[1] = i; return; } }\n}\n");
    }

    fn emit_resolve_kernel(&self, out: &mut String) -> Result<(), CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_resolve_conflicts(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, unsigned long long candidate_count, const unsigned char* enabled, const double* times, unsigned char* wins, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  for (unsigned long long i = 0; i < candidate_count; ++i) wins[i] = enabled[i];\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");
        for validated in self.model.transitions() {
            let transition = &self.model.model().boxes[validated.box_index].transitions[validated.transition_index];
            if transition.contests.is_empty() { continue; }
            let table_index = self.table_index(validated.box_index, &transition.table)?;
            let table_global = self.global_table(validated.box_index, table_index);
            let rows = Rows::State { box_index: validated.box_index, table_index };
            writeln!(out, "  for (unsigned long long row = 0; row < row_counts[{table_global}]; ++row) {{ unsigned long long self_candidate = candidate_offsets[{}] + row; if (!enabled[self_candidate]) continue;", validated.rule_id).unwrap();
            for (claim_index, claim) in transition.contests.iter().enumerate() {
                let resource_ty = self.infer(&claim.resource, rows, None)?;
                let Ty::Ref(target_name) = resource_ty else { return Err(codegen("claim resource is not Ref")); };
                let resource = self.render(&claim.resource, rows, Some(&Ty::Ref(target_name.clone())), "state", "row")?.0;
                writeln!(out, "    unsigned int resource_{claim_index} = {resource};").unwrap();
                let (self_key, self_key_ty) = self.claim_key(claim, rows, validated.rule_id, "row", "self_candidate")?;
                writeln!(out, "    {} best_key_{claim_index} = {self_key}; unsigned int best_rule_{claim_index} = {}U; unsigned int best_entity_{claim_index} = (unsigned int)row;", self_key_ty.cuda(), validated.rule_id).unwrap();
                for other in self.model.transitions() {
                    let other_transition = &self.model.model().boxes[other.box_index].transitions[other.transition_index];
                    let other_table_index = self.table_index(other.box_index, &other_transition.table)?;
                    let other_rows = Rows::State { box_index: other.box_index, table_index: other_table_index };
                    let other_global = self.global_table(other.box_index, other_table_index);
                    for other_claim in &other_transition.contests {
                        let other_ty = self.infer(&other_claim.resource, other_rows, None)?;
                        if other_ty != Ty::Ref(target_name.clone()) || other.box_index != validated.box_index { continue; }
                        let other_resource = self.render(&other_claim.resource, other_rows, Some(&other_ty), "state", "other_row")?.0;
                        let compatible = claim_ordering_type(self, claim, rows)? == claim_ordering_type(self, other_claim, other_rows)?;
                        writeln!(out, "    for (unsigned long long other_row = 0; other_row < row_counts[{other_global}]; ++other_row) {{ unsigned long long other_candidate = candidate_offsets[{}] + other_row; if (!enabled[other_candidate] || (unsigned int)({other_resource}) != resource_{claim_index}) continue;", other.rule_id).unwrap();
                        if !compatible {
                            out.push_str("      status[0] = 4ULL; status[1] = self_candidate; status[2] = other_candidate; return;\n    }\n");
                            continue;
                        }
                        let (other_key, _) = self.claim_key(other_claim, other_rows, other.rule_id, "other_row", "other_candidate")?;
                        let better = if self_key_ty == Ty::Real {
                            format!("sembla_total_less({other_key}, best_key_{claim_index}) || (sembla_total_equal({other_key}, best_key_{claim_index}) && ({}U < best_rule_{claim_index} || ({}U == best_rule_{claim_index} && (unsigned int)other_row < best_entity_{claim_index})))", other.rule_id, other.rule_id)
                        } else {
                            format!("({other_key}) < best_key_{claim_index} || (({other_key}) == best_key_{claim_index} && ({}U < best_rule_{claim_index} || ({}U == best_rule_{claim_index} && (unsigned int)other_row < best_entity_{claim_index})))", other.rule_id, other.rule_id)
                        };
                        writeln!(out, "      if ({better}) {{ best_key_{claim_index} = {other_key}; best_rule_{claim_index} = {}U; best_entity_{claim_index} = (unsigned int)other_row; }}\n    }}", other.rule_id).unwrap();
                    }
                }
                writeln!(out, "    if (best_rule_{claim_index} != {}U || best_entity_{claim_index} != (unsigned int)row) wins[self_candidate] = 0;", validated.rule_id).unwrap();
            }
            out.push_str("  }\n");
        }
        out.push_str("}\n");
        Ok(())
    }

    fn claim_key(&self, claim: &sembla_ir::ResourceClaim, rows: Rows, _rule: u32, row: &str, candidate: &str) -> Result<(String, Ty), CudaError> {
        match &claim.ordering {
            ClaimOrdering::RaceTime => Ok((format!("times[{candidate}]"), Ty::Real)),
            ClaimOrdering::Key { expr } => self.render(expr, rows, None, "state", row),
        }
    }

    fn emit_apply_kernel(&self, out: &mut String) -> Result<(), CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_apply_effects(const unsigned char* state, unsigned char* next_state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, const unsigned char* wins, const unsigned long long* write_offsets, int* owners, unsigned long long owner_count, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  for (unsigned long long i = 0; i < owner_count; ++i) owners[i] = -1;\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");
        for validated in self.model.transitions() {
            let transition = &self.model.model().boxes[validated.box_index].transitions[validated.transition_index];
            let table_index = self.table_index(validated.box_index, &transition.table)?;
            let table = &self.model.model().boxes[validated.box_index].tables[table_index];
            let global_table = self.global_table(validated.box_index, table_index);
            let rows = Rows::State { box_index: validated.box_index, table_index };
            for effect in &transition.effects {
                let Effect::SetAttr { attr, value } = effect;
                let attr_index = attr_index(table, attr)?;
                let ty = Ty::from(&table.attrs[attr_index].ty);
                let column = self.column(validated.box_index, table_index, attr_index);
                let rendered = self.render(value, rows, Some(&ty), "state", "row")?.0;
                writeln!(out, "  for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) {{ unsigned long long candidate = candidate_offsets[{}] + row; if (!wins[candidate]) continue; local_error = 0; {} value = ({} )({rendered}); if (local_error) {{ status[0] = 5ULL; status[1] = candidate; return; }}", validated.rule_id, ty.cuda(), ty.cuda()).unwrap();
                match &ty {
                    Ty::Enum(variants) => writeln!(out, "    if ((unsigned long long)value >= {}ULL) {{ status[0] = 6ULL; status[1] = candidate; return; }}", variants.len()).unwrap(),
                    Ty::Ref(target) => {
                        let target_index = self.table_index(validated.box_index, target)?;
                        let target_global = self.global_table(validated.box_index, target_index);
                        writeln!(out, "    if ((unsigned long long)value >= row_counts[{target_global}]) {{ status[0] = 7ULL; status[1] = candidate; return; }}").unwrap();
                    }
                    _ => {}
                }
                writeln!(out, "    unsigned long long owner = write_offsets[{column}] + row; if (owners[owner] != -1) {{ status[0] = 8ULL; status[1] = owner; status[2] = (unsigned long long)owners[owner]; status[3] = {}ULL; return; }} owners[owner] = (int){}U; *(({}*)(next_state + column_offsets[{column}]) + row) = value;\n  }}", validated.rule_id, validated.rule_id, ty.cuda()).unwrap();
            }
        }
        out.push_str("}\n");
        Ok(())
    }

    fn emit_output_kernel(&self, out: &mut String) -> Result<(), CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_build_outputs(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, unsigned char* next_inputs, const unsigned long long* next_input_offsets, unsigned long long* next_input_counts, unsigned long long port_count, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  for (unsigned long long i = 0; i < port_count; ++i) next_input_counts[i] = 0ULL;\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");
        for wire in &self.model.model().wires {
            let from_box = self.model.model().boxes.iter().position(|entry| entry.name == wire.from.r#box).ok_or_else(|| codegen("wire source box disappeared"))?;
            let to_box = self.model.model().boxes.iter().position(|entry| entry.name == wire.to.r#box).ok_or_else(|| codegen("wire target box disappeared"))?;
            let output = self.model.model().boxes[from_box].outputs.iter().find(|entry| entry.name == wire.from.port).ok_or_else(|| codegen("wire output disappeared"))?;
            let to_port_index = self.port_index(to_box, &wire.to.port)?;
            let to_port = self.port(to_box, to_port_index);
            let sembla_ir::OutputBuilder::PerTable { table, fields } = &output.builder;
            let table_index = self.table_index(from_box, table)?;
            let global_table = self.global_table(from_box, table_index);
            let rows = Rows::State { box_index: from_box, table_index };
            writeln!(out, "  next_input_counts[{to_port}] = 1ULL;").unwrap();
            for (field_index, field) in fields.iter().enumerate() {
                let target_field = self.input_field(to_box, to_port_index, field_index);
                let ty = Ty::from(&output.schema[field_index].ty);
                writeln!(out, "  {{ {} result = ({})0;", ty.cuda(), ty.cuda()).unwrap();
                writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) {{").unwrap();
                let selected = if let Some(filter) = &field.filter {
                    self.render(filter, rows, Some(&Ty::Bool), "state", "row")?.0
                } else { "1".to_owned() };
                match &field.op {
                    AggOp::Count => writeln!(out, "      if ({selected}) result = sembla_add_i64(result, 1LL, error);").unwrap(),
                    AggOp::Sum { value } => {
                        let value = self.render(value, rows, Some(&ty), "state", "row")?.0;
                        if ty == Ty::Int {
                            writeln!(out, "      if ({selected}) result = sembla_add_i64(result, {value}, error);").unwrap();
                        } else {
                            writeln!(out, "      if ({selected}) result = result + (double)({value});").unwrap();
                        }
                    }
                }
                writeln!(out, "    }}\n    if (local_error) {{ status[0] = 9ULL; status[1] = {target_field}ULL; return; }} *(({}*)(next_inputs + next_input_offsets[{target_field}])) = result;\n  }}", ty.cuda()).unwrap();
            }
        }
        out.push_str("}\n");
        Ok(())
    }
}

fn rows_box(rows: Rows) -> usize {
    match rows {
        Rows::State { box_index, .. } | Rows::Input { box_index, .. } => box_index,
    }
}

fn rows_state(rows: Rows) -> Result<(usize, usize), CudaError> {
    match rows {
        Rows::State { box_index, table_index } => Ok((box_index, table_index)),
        Rows::Input { .. } => Err(codegen("state aggregate used in input-row context")),
    }
}

fn attr_index(table: &Table, name: &str) -> Result<usize, CudaError> {
    table.attrs.iter().position(|attr| attr.name == name).ok_or_else(|| {
        codegen(format!("table '{}' has no attribute '{name}'", table.name))
    })
}

fn claim_ordering_type(generator: &Generator<'_>, claim: &sembla_ir::ResourceClaim, rows: Rows) -> Result<Ty, CudaError> {
    match &claim.ordering {
        ClaimOrdering::RaceTime => Ok(Ty::Real),
        ClaimOrdering::Key { expr } => generator.infer(expr, rows, None),
    }
}

fn f64_literal(value: f64) -> String {
    format!("sembla_f64(0x{:016x}ULL)", value.to_bits())
}

fn codegen(message: impl Into<String>) -> CudaError {
    CudaError::Codegen(message.into())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

/// Generates one deterministic NVRTC translation unit for a validated model.
pub fn generate(model: &ValidatedModel) -> Result<GeneratedCuda, CudaError> {
    Generator::new(model)?.emit()
}

const PRELUDE: &str = r#"
// No result-bearing atomics are used. State writes are sorted by generated
// rule/effect/row order and conflicts are resolved lexicographically.
__device__ __forceinline__ double sembla_f64(unsigned long long bits) {
  return __longlong_as_double((long long)bits);
}
__device__ __forceinline__ long long sembla_total_key(double value) {
  long long bits = __double_as_longlong(value);
  return bits ^ (long long)(((unsigned long long)(bits >> 63)) >> 1);
}
__device__ __forceinline__ int sembla_total_less(double left, double right) {
  return sembla_total_key(left) < sembla_total_key(right);
}
__device__ __forceinline__ int sembla_total_equal(double left, double right) {
  return sembla_total_key(left) == sembla_total_key(right);
}
__device__ __forceinline__ long long sembla_add_i64(long long a, long long b, unsigned char* error) {
  if ((b > 0 && a > 0x7fffffffffffffffLL - b) ||
      (b < 0 && a < (-0x7fffffffffffffffLL - 1LL) - b)) { *error = 1; return 0; }
  return a + b;
}
__device__ __forceinline__ long long sembla_sub_i64(long long a, long long b, unsigned char* error) {
  if ((b < 0 && a > 0x7fffffffffffffffLL + b) ||
      (b > 0 && a < (-0x7fffffffffffffffLL - 1LL) + b)) { *error = 1; return 0; }
  return a - b;
}
__device__ __forceinline__ long long sembla_mul_i64(long long a, long long b, unsigned char* error) {
  if (a == 0 || b == 0) return 0;
  if ((a == (-0x7fffffffffffffffLL - 1LL) && b == -1) ||
      (b == (-0x7fffffffffffffffLL - 1LL) && a == -1)) { *error = 1; return 0; }
  long long result = a * b;
  if (result / b != a) { *error = 1; return 0; }
  return result;
}
__device__ __forceinline__ void sembla_philox(unsigned int counter[4], unsigned int key[2]) {
  const unsigned int M0 = 0xD2511F53U, M1 = 0xCD9E8D57U;
  const unsigned int W0 = 0x9E3779B9U, W1 = 0xBB67AE85U;
  #pragma unroll
  for (int round = 0; round < 10; ++round) {
    unsigned long long p0 = (unsigned long long)M0 * counter[0];
    unsigned long long p1 = (unsigned long long)M1 * counter[2];
    unsigned int next0 = (unsigned int)(p1 >> 32) ^ counter[1] ^ key[0];
    unsigned int next1 = (unsigned int)p1;
    unsigned int next2 = (unsigned int)(p0 >> 32) ^ counter[3] ^ key[1];
    unsigned int next3 = (unsigned int)p0;
    counter[0] = next0; counter[1] = next1; counter[2] = next2; counter[3] = next3;
    if (round != 9) { key[0] += W0; key[1] += W1; }
  }
}
__device__ __forceinline__ double sembla_uniform(unsigned long long seed, unsigned int tick,
                                                  unsigned int rule, unsigned int entity,
                                                  unsigned int draw) {
  unsigned int counter[4] = {tick, rule, entity, draw};
  unsigned int key[2] = {(unsigned int)seed, (unsigned int)(seed >> 32)};
  sembla_philox(counter, key);
  unsigned long long mantissa = ((unsigned long long)counter[0] << 21) |
                                ((unsigned long long)counter[1] >> 11);
  double sample = ((double)mantissa + 0.5) * (1.0 / 9007199254740992.0);
  return sample == 1.0 ? sembla_f64(0x3fefffffffffffffULL) : sample;
}
__device__ __forceinline__ double sembla_exp(unsigned long long seed, unsigned int tick,
                                              unsigned int rule, unsigned int entity,
                                              unsigned int draw, double lambda) {
  return lambda <= 0.0 ? sembla_f64(0x7ff0000000000000ULL)
                       : -log(sembla_uniform(seed, tick, rule, entity, draw)) / lambda;
}
"#;

const PHILOX_TEST_KERNEL: &str = r#"
extern "C" __global__ void sembla_philox_vectors(const unsigned long long* seeds,
                                                   const unsigned int* ticks,
                                                   const unsigned int* rules,
                                                   const unsigned int* entities,
                                                   const unsigned int* draws,
                                                   unsigned int* output,
                                                   unsigned int count) {
  unsigned int i = blockIdx.x * blockDim.x + threadIdx.x;
  if (i >= count) return;
  unsigned int counter[4] = {ticks[i], rules[i], entities[i], draws[i]};
  unsigned long long seed = seeds[i];
  unsigned int key[2] = {(unsigned int)seed, (unsigned int)(seed >> 32)};
  sembla_philox(counter, key);
  output[i * 4U + 0U] = counter[0];
  output[i * 4U + 1U] = counter[1];
  output[i * 4U + 2U] = counter[2];
  output[i * 4U + 3U] = counter[3];
}
"#;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{generate, DUMP_ENV};

    fn sir_model() -> sembla_ir::ValidatedModel {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sir.json");
        let source = std::fs::read_to_string(path).unwrap();
        sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
    }

    #[test]
    fn generation_is_deterministic_and_has_one_kernel_per_transition() {
        let model = sir_model();
        let first = generate(&model).unwrap();
        let second = generate(&model).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.transition_kernels,
            ["sembla_transition_00000000", "sembla_transition_00000001"]
        );
        assert!(first.source.contains("sembla_resolve_conflicts"));
        assert!(first.source.contains("sembla_apply_effects"));
        assert!(!first.source.contains("atomicAdd"));
        assert!(!first.source.contains("atomicMin"));
    }

    #[test]
    fn sir_source_matches_checked_in_golden() {
        let generated = generate(&sir_model()).unwrap();
        assert_eq!(
            generated.source,
            include_str!("../tests/fixtures/sir.generated.cu")
        );
    }

    #[test]
    fn dump_is_content_addressed_and_repeatable() {
        let generated = generate(&sir_model()).unwrap();
        let directory = std::env::temp_dir().join(format!(
            "sembla-cuda-dump-{}-{}",
            std::process::id(),
            generated.source_sha256
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::env::set_var(DUMP_ENV, &directory);
        let first = generated.dump_if_requested().unwrap().unwrap();
        let second = generated.dump_if_requested().unwrap().unwrap();
        std::env::remove_var(DUMP_ENV);
        assert_eq!(first, second);
        assert_eq!(std::fs::read_to_string(first).unwrap(), generated.source);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
