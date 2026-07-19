use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sembla_ir::{AggOp, AttrType, ClaimOrdering, Effect, Expr, ParamType, Table, ValidatedModel};
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
    /// Aggregate indices evaluated against tick-start state. Error facts are
    /// recorded eagerly but surfaced only at first semantic use.
    pub state_aggregate_indices: Vec<usize>,
    /// Aggregate indices reachable from scheduling expressions.
    pub schedule_aggregate_indices: Vec<usize>,
    /// First-use aggregate indices for each rule, in global rule order.
    pub schedule_aggregate_indices_by_rule: Vec<Vec<usize>>,
    /// Effect-only aggregate indices evaluated after conflict resolution.
    pub effect_aggregate_indices: Vec<usize>,
    /// Aggregate indices evaluated against prospective state for wired outputs.
    pub output_aggregate_indices: Vec<usize>,
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
    State {
        box_index: usize,
        table_index: usize,
    },
    Input {
        box_index: usize,
        port_index: usize,
    },
}

#[derive(Clone, Copy)]
enum AggUse {
    Schedule(u32),
    Effect(u32),
    Output,
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
    schedule_rules: Vec<u32>,
    effect_rules: Vec<u32>,
    output_use: bool,
}

impl AggSpec {
    fn record_use(&mut self, usage: AggUse) {
        match usage {
            AggUse::Schedule(rule_id) => {
                if !self.schedule_rules.contains(&rule_id) {
                    self.schedule_rules.push(rule_id);
                }
            }
            AggUse::Effect(rule_id) => {
                if !self.effect_rules.contains(&rule_id) {
                    self.effect_rules.push(rule_id);
                }
            }
            AggUse::Output => self.output_use = true,
        }
    }
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

#[derive(Clone, Copy)]
enum ValidationTarget<'a> {
    /// Validation while constructing an aggregate error fact. Nested
    /// aggregate facts are propagated without committing global status.
    AggregateFact,
    /// Validation in CPU semantic order. `identity` is a CUDA expression
    /// identifying the transition candidate or output field.
    Status { code: u64, identity: &'a str },
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
        for validated in self.model.transitions() {
            let box_index = validated.box_index;
            let transition =
                &self.model.model().boxes[box_index].transitions[validated.transition_index];
            let table_index = self.table_index(box_index, &transition.table)?;
            self.collect_expr(
                box_index,
                table_index,
                &transition.guard,
                AggUse::Schedule(validated.rule_id),
            )?;
            self.collect_expr(
                box_index,
                table_index,
                &transition.hazard,
                AggUse::Schedule(validated.rule_id),
            )?;
            for effect in &transition.effects {
                let Effect::SetAttr { value, .. } = effect;
                self.collect_expr(
                    box_index,
                    table_index,
                    value,
                    AggUse::Effect(validated.rule_id),
                )?;
            }
            for claim in &transition.contests {
                self.collect_expr(
                    box_index,
                    table_index,
                    &claim.resource,
                    AggUse::Schedule(validated.rule_id),
                )?;
                if let ClaimOrdering::Key { expr } = &claim.ordering {
                    self.collect_expr(
                        box_index,
                        table_index,
                        expr,
                        AggUse::Schedule(validated.rule_id),
                    )?;
                }
            }
        }

        // Only wired outputs are observable and evaluated by the CPU oracle.
        for wire in &self.model.model().wires {
            let box_index = self
                .model
                .model()
                .boxes
                .iter()
                .position(|model_box| model_box.name == wire.from.r#box)
                .expect("validated output box is indexed");
            let model_box = &self.model.model().boxes[box_index];
            let output = model_box
                .outputs
                .iter()
                .find(|output| output.name == wire.from.port)
                .expect("validated output is indexed");
            let sembla_ir::OutputBuilder::PerTable { table, fields } = &output.builder;
            let table_index = self.table_index(box_index, table)?;
            for field in fields {
                if let Some(filter) = &field.filter {
                    self.collect_expr(box_index, table_index, filter, AggUse::Output)?;
                }
                if let AggOp::Sum { value } = &field.op {
                    self.collect_expr(box_index, table_index, value, AggUse::Output)?;
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
        usage: AggUse,
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
                self.collect_expr(box_index, query_table_index, lhs, usage)?;
                self.collect_expr(box_index, query_table_index, rhs, usage)?;
            }
            Expr::Not { expr } => self.collect_expr(box_index, query_table_index, expr, usage)?,
            Expr::Input { port, agg } => {
                if let Some(filter) = &agg.filter {
                    Self::validate_input_expr(filter)?;
                }
                if let AggOp::Sum { value } = &agg.op {
                    Self::validate_input_expr(value)?;
                }
                let key = format!("{box_index}:{port}:{agg:?}");
                if !self.inputs.iter().any(|entry| entry.key == key) {
                    let port_index = self.port_index(box_index, port)?;
                    let ty = match &agg.op {
                        AggOp::Count => Ty::Int,
                        AggOp::Sum { value } => self.infer(
                            value,
                            Rows::Input {
                                box_index,
                                port_index,
                            },
                            None,
                        )?,
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
                self.collect_expr(box_index, target_table_index, filter, usage)?;
                if let AggOp::Sum { value } = op {
                    self.collect_expr(box_index, target_table_index, value, usage)?;
                }
                let key = format!("{box_index}:{query_table_index}:{expr:?}");
                if let Some(existing) = self.aggs.iter_mut().find(|entry| entry.key == key) {
                    existing.record_use(usage);
                } else {
                    let query_table =
                        &self.model.model().boxes[box_index].tables[query_table_index];
                    let target_table =
                        &self.model.model().boxes[box_index].tables[target_table_index];
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
                    let mut spec = AggSpec {
                        key,
                        box_index,
                        target_table_index,
                        group_table_index,
                        target_fk_column,
                        self_fk_column,
                        op: op.clone(),
                        filter: (**filter).clone(),
                        ty,
                        schedule_rules: Vec::new(),
                        effect_rules: Vec::new(),
                        output_use: false,
                    };
                    spec.record_use(usage);
                    self.aggs.push(spec);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_input_expr(expr: &Expr) -> Result<(), CudaError> {
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
                Self::validate_input_expr(lhs)?;
                Self::validate_input_expr(rhs)?;
            }
            Expr::Not { expr } => Self::validate_input_expr(expr)?,
            Expr::Input { .. } | Expr::Agg { .. } => {
                return Err(codegen(
                    "nested Input/Agg inside an input aggregate is unsupported",
                ));
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

    fn emit_scalar_validation_failure(&self, out: &mut String, target: ValidationTarget<'_>) {
        match target {
            ValidationTarget::AggregateFact => {
                out.push_str("      aggregate_errors[0] = 2U; return;\n");
            }
            ValidationTarget::Status { code, identity } => {
                writeln!(out, "      status[0] = {code}ULL; status[1] = (unsigned long long)({identity}); return;").unwrap();
            }
        }
    }

    fn emit_aggregate_validation_failure(
        &self,
        out: &mut String,
        target: ValidationTarget<'_>,
        aggregate_index: usize,
    ) -> Result<(), CudaError> {
        match target {
            ValidationTarget::AggregateFact => {
                writeln!(
                    out,
                    "      aggregate_errors[0] = aggregate_facts[{aggregate_index}]; return;"
                )
                .unwrap();
            }
            ValidationTarget::Status { .. } => {
                writeln!(out, "      status[0] = (unsigned long long)aggregate_facts[{aggregate_index}]; status[1] = {aggregate_index}ULL; return;").unwrap();
            }
        }
        Ok(())
    }

    /// Emits validation in the CPU evaluator's recursive column order. Child
    /// expressions are completely validated before their sibling and checked
    /// integer operations scan rows in ascending order. Value kernels may then
    /// recompute with the compact expression renderer without discovering a
    /// new error or relying on C++ operand evaluation order.
    #[allow(clippy::too_many_arguments)]
    fn emit_expr_validation(
        &self,
        out: &mut String,
        expr: &Expr,
        rows: Rows,
        expected: Option<&Ty>,
        state_name: &str,
        row_count: &str,
        target: ValidationTarget<'_>,
    ) -> Result<(), CudaError> {
        match expr {
            Expr::Add { lhs, rhs } | Expr::Sub { lhs, rhs } | Expr::Mul { lhs, rhs } => {
                self.emit_expr_validation(out, lhs, rows, None, state_name, row_count, target)?;
                self.emit_expr_validation(out, rhs, rows, None, state_name, row_count, target)?;
                let left_ty = self.infer(lhs, rows, None)?;
                let right_ty = self.infer(rhs, rows, None)?;
                if left_ty != Ty::Real && right_ty != Ty::Real {
                    let left = self.render(lhs, rows, Some(&Ty::Int), state_name, "row")?.0;
                    let right = self.render(rhs, rows, Some(&Ty::Int), state_name, "row")?.0;
                    let helper = match expr {
                        Expr::Add { .. } => "sembla_add_i64",
                        Expr::Sub { .. } => "sembla_sub_i64",
                        Expr::Mul { .. } => "sembla_mul_i64",
                        _ => unreachable!(),
                    };
                    writeln!(
                        out,
                        "    for (unsigned long long row = 0; row < {row_count}; ++row) {{"
                    )
                    .unwrap();
                    writeln!(out, "      local_error = 0U; long long validation_left = (long long)({left}); if (local_error) {{").unwrap();
                    self.emit_scalar_validation_failure(out, target);
                    out.push_str("      }\n");
                    writeln!(out, "      local_error = 0U; long long validation_right = (long long)({right}); if (local_error) {{").unwrap();
                    self.emit_scalar_validation_failure(out, target);
                    out.push_str("      }\n      local_error = 0U; (void)");
                    writeln!(
                        out,
                        "{helper}(validation_left, validation_right, error); if (local_error) {{"
                    )
                    .unwrap();
                    self.emit_scalar_validation_failure(out, target);
                    out.push_str("      }\n    }\n");
                }
            }
            Expr::Div { lhs, rhs }
            | Expr::Lt { lhs, rhs }
            | Expr::Le { lhs, rhs }
            | Expr::Gt { lhs, rhs }
            | Expr::Ge { lhs, rhs }
            | Expr::And { lhs, rhs }
            | Expr::Or { lhs, rhs } => {
                self.emit_expr_validation(out, lhs, rows, None, state_name, row_count, target)?;
                self.emit_expr_validation(out, rhs, rows, None, state_name, row_count, target)?;
            }
            Expr::Eq { lhs, rhs } | Expr::Ne { lhs, rhs } => {
                // Enum literals need their sibling's type, but literals cannot
                // fail. The only observable ordering is therefore the same
                // left-then-right recursion used by the CPU evaluator.
                let left_hint = self.infer(lhs, rows, None).ok();
                let right_hint = self.infer(rhs, rows, left_hint.as_ref()).ok();
                self.emit_expr_validation(
                    out,
                    lhs,
                    rows,
                    right_hint.as_ref(),
                    state_name,
                    row_count,
                    target,
                )?;
                self.emit_expr_validation(
                    out,
                    rhs,
                    rows,
                    left_hint.as_ref(),
                    state_name,
                    row_count,
                    target,
                )?;
            }
            Expr::Not { expr } => {
                self.emit_expr_validation(
                    out,
                    expr,
                    rows,
                    Some(&Ty::Bool),
                    state_name,
                    row_count,
                    target,
                )?;
            }
            Expr::Input { port, agg } => {
                let key = format!("{}:{port}:{agg:?}", rows_box(rows));
                let index = self
                    .inputs
                    .iter()
                    .position(|entry| entry.key == key)
                    .ok_or_else(|| codegen("input aggregate was not collected"))?;
                writeln!(out, "    {{ unsigned long long row = 0ULL; local_error = 0U; (void)sembla_input_{index}(inputs, input_offsets, input_counts, params, error); if (local_error) {{").unwrap();
                self.emit_scalar_validation_failure(out, target);
                out.push_str("      }\n    }\n");
            }
            Expr::Agg { .. } => {
                let (box_index, table_index) = rows_state(rows)?;
                let key = format!("{box_index}:{table_index}:{expr:?}");
                let index = self
                    .aggs
                    .iter()
                    .position(|entry| entry.key == key)
                    .ok_or_else(|| codegen("group aggregate was not collected"))?;
                writeln!(out, "    if (aggregate_facts[{index}] != 0U) {{").unwrap();
                self.emit_aggregate_validation_failure(out, target, index)?;
                out.push_str("    }\n");
            }
            Expr::Real { .. }
            | Expr::Int { .. }
            | Expr::Bool { .. }
            | Expr::Enum { .. }
            | Expr::Param { .. }
            | Expr::SelfAttr { .. }
            | Expr::EnumIs { .. } => {
                let _ = expected;
            }
        }
        Ok(())
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
            Expr::Int { value } => (i64_literal(*value), Ty::Int),
            Expr::Bool { value } => ((if *value { "1" } else { "0" }).to_owned(), Ty::Bool),
            Expr::Enum { variant } => {
                let Ty::Enum(variants) = expected.ok_or_else(|| {
                    codegen(format!(
                        "enum literal '{variant}' lacks destination context"
                    ))
                })?
                else {
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
                    format!("(*((const {}*)(params + {}ULL)))", ty.cuda(), index * 8),
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
                    (
                        format!("([&]() {{ long long sembla_left = (long long)({left}); if (*error) return 0LL; long long sembla_right = (long long)({right}); if (*error) return 0LL; return {helper}(sembla_left, sembla_right, error); }}())"),
                        ty,
                    )
                } else {
                    let operator = match expr {
                        Expr::Add { .. } => "+",
                        Expr::Sub { .. } => "-",
                        Expr::Mul { .. } => "*",
                        _ => unreachable!(),
                    };
                    (
                        format!("([&]() {{ double sembla_left = (double)({left}); if (*error) return 0.0; double sembla_right = (double)({right}); if (*error) return 0.0; return sembla_left {operator} sembla_right; }}())"),
                        ty,
                    )
                }
            }
            Expr::Div { lhs, rhs } => {
                let (left, _) = self.render(lhs, rows, None, state_name, row_name)?;
                let (right, _) = self.render(rhs, rows, None, state_name, row_name)?;
                (
                    format!("([&]() {{ double sembla_left = (double)({left}); if (*error) return 0.0; double sembla_right = (double)({right}); if (*error) return 0.0; return sembla_left / sembla_right; }}())"),
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
                let (left, left_ty) =
                    self.render(lhs, rows, right_hint.as_ref(), state_name, row_name)?;
                let (right, right_ty) =
                    self.render(rhs, rows, Some(&left_ty), state_name, row_name)?;
                let input_ordering = matches!(rows, Rows::Input { .. })
                    && matches!(
                        expr,
                        Expr::Lt { .. } | Expr::Le { .. } | Expr::Gt { .. } | Expr::Ge { .. }
                    );
                let promote_numeric = left_ty.numeric()
                    && right_ty.numeric()
                    && (left_ty == Ty::Real || right_ty == Ty::Real || input_ordering);
                let left = if promote_numeric {
                    format!("(double)({left})")
                } else {
                    left
                };
                let right = if promote_numeric {
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
                (
                    format!("([&]() -> int {{ auto sembla_left = ({left}); if (*error) return 0; auto sembla_right = ({right}); if (*error) return 0; return sembla_left {operator} sembla_right; }}())"),
                    Ty::Bool,
                )
            }
            Expr::And { lhs, rhs } | Expr::Or { lhs, rhs } => {
                let (left, _) = self.render(lhs, rows, Some(&Ty::Bool), state_name, row_name)?;
                let (right, _) = self.render(rhs, rows, Some(&Ty::Bool), state_name, row_name)?;
                let operator = if matches!(expr, Expr::And { .. }) {
                    "&"
                } else {
                    "|"
                };
                (
                    format!("([&]() -> int {{ int sembla_left = (int)({left}); if (*error) return 0; int sembla_right = (int)({right}); if (*error) return 0; return sembla_left {operator} sembla_right; }}())"),
                    Ty::Bool,
                )
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
        writeln!(
            out,
            "// Generated by sembla-cuda {}. DO NOT EDIT.",
            env!("CARGO_PKG_VERSION")
        )
        .unwrap();
        let model_name_sha256 = hex(Sha256::digest(self.model.model().name.as_bytes()).as_slice());
        writeln!(out, "// model-name-sha256: {model_name_sha256}").unwrap();
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
        let state_aggregate_indices = self
            .aggs
            .iter()
            .enumerate()
            .filter_map(|(index, spec)| {
                (!spec.schedule_rules.is_empty() || !spec.effect_rules.is_empty()).then_some(index)
            })
            .collect();
        let schedule_aggregate_indices = self
            .aggs
            .iter()
            .enumerate()
            .filter_map(|(index, spec)| (!spec.schedule_rules.is_empty()).then_some(index))
            .collect();
        let mut schedule_aggregate_indices_by_rule =
            vec![Vec::new(); self.model.transitions().len()];
        for (index, spec) in self.aggs.iter().enumerate() {
            if let Some(rule_id) = spec.schedule_rules.first() {
                let rule_index = usize::try_from(*rule_id)
                    .map_err(|_| codegen("rule id exceeds host index width"))?;
                schedule_aggregate_indices_by_rule[rule_index].push(index);
            }
        }
        let effect_aggregate_indices = self
            .aggs
            .iter()
            .enumerate()
            .filter_map(|(index, spec)| {
                (spec.schedule_rules.is_empty() && !spec.effect_rules.is_empty()).then_some(index)
            })
            .collect();
        let output_aggregate_indices = self
            .aggs
            .iter()
            .enumerate()
            .filter_map(|(index, spec)| spec.output_use.then_some(index))
            .collect();
        Ok(GeneratedCuda {
            source: out,
            source_sha256,
            transition_kernels,
            aggregate_group_tables,
            state_aggregate_indices,
            schedule_aggregate_indices,
            schedule_aggregate_indices_by_rule,
            effect_aggregate_indices,
            output_aggregate_indices,
        })
    }

    fn emit_input_helpers(&self, out: &mut String) -> Result<(), CudaError> {
        for (index, spec) in self.inputs.iter().enumerate() {
            writeln!(out, "__device__ __forceinline__ {} sembla_input_{index}(const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, unsigned char* error) {{", spec.ty.cuda()).unwrap();
            let port = self.port(spec.box_index, spec.port_index);
            let selected = if let Some(filter) = &spec.agg.filter {
                self.render(
                    filter,
                    Rows::Input {
                        box_index: spec.box_index,
                        port_index: spec.port_index,
                    },
                    Some(&Ty::Bool),
                    "state",
                    "row",
                )?
                .0
            } else {
                "1".to_owned()
            };
            // The CPU evaluates the complete filter column before reducing.
            writeln!(out, "  for (unsigned long long row = 0; row < input_counts[{port}]; ++row) {{ (void)({selected}); if (*error) return ({})0; }}", spec.ty.cuda()).unwrap();
            match &spec.agg.op {
                AggOp::Count => {
                    writeln!(out, "  long long result = 0LL;\n  for (unsigned long long row = 0; row < input_counts[{port}]; ++row) {{ int selected = {selected}; if (*error) return 0LL; if (selected) {{ result = sembla_add_i64(result, 1LL, error); if (*error) return 0LL; }} }}\n  return result;\n}}").unwrap();
                }
                AggOp::Sum { value } => {
                    let rendered = self
                        .render(
                            value,
                            Rows::Input {
                                box_index: spec.box_index,
                                port_index: spec.port_index,
                            },
                            Some(&spec.ty),
                            "state",
                            "row",
                        )?
                        .0;
                    writeln!(out, "  {} result = ({})0;\n  for (unsigned long long row = 0; row < input_counts[{port}]; ++row) {{ int selected = {selected}; if (*error) return ({})0; if (selected) {{ {} value = ({})({rendered}); if (*error) return ({})0;", spec.ty.cuda(), spec.ty.cuda(), spec.ty.cuda(), spec.ty.cuda(), spec.ty.cuda(), spec.ty.cuda()).unwrap();
                    if spec.ty == Ty::Int {
                        out.push_str(" result = sembla_add_i64(result, value, error);");
                    } else {
                        out.push_str(" result = result + (double)value;");
                    }
                    writeln!(
                        out,
                        " if (*error) return ({})0; }} }}\n  return result;\n}}",
                        spec.ty.cuda()
                    )
                    .unwrap();
                }
            }
        }
        Ok(())
    }

    fn emit_aggregate_kernel(&self, out: &mut String) -> Result<(), CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_reset_status(unsigned long long* status, unsigned char* aggregate_errors, unsigned long long error_count) {\n  if (blockIdx.x != 0 || threadIdx.x != 0) return;\n  status[0] = 0ULL; status[1] = 0ULL; status[2] = 0ULL; status[3] = 0ULL;\n  for (unsigned long long i = 0; i < error_count; ++i) aggregate_errors[i] = 0U;\n}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_mark_effect_aggregates(const unsigned long long* row_counts, const unsigned long long* candidate_offsets, const unsigned char* wins, unsigned char* active, unsigned long long aggregate_count) {\n  if (blockIdx.x != 0 || threadIdx.x != 0) return;\n  for (unsigned long long i = 0; i < aggregate_count; ++i) active[i] = 0U;\n");
        for (index, spec) in self.aggs.iter().enumerate() {
            if !spec.schedule_rules.is_empty() {
                continue;
            }
            for rule_id in &spec.effect_rules {
                let validated = self
                    .model
                    .transitions()
                    .iter()
                    .find(|transition| transition.rule_id == *rule_id)
                    .expect("aggregate effect rule is validated");
                let transition = &self.model.model().boxes[validated.box_index].transitions
                    [validated.transition_index];
                let table_index = self.table_index(validated.box_index, &transition.table)?;
                let global_table = self.global_table(validated.box_index, table_index);
                writeln!(out, "  if (!active[{index}]) for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) if (wins[candidate_offsets[{rule_id}] + row]) {{ active[{index}] = 1U; break; }}").unwrap();
            }
        }
        out.push_str("}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_build_aggregate_partials(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned char* aggregate_facts, unsigned int aggregate_index, const unsigned char* aggregate_active, unsigned char require_active, unsigned char* partials, const unsigned long long* agg_offsets, unsigned char* aggregate_errors) {\n  unsigned int worker = blockIdx.x * blockDim.x + threadIdx.x;\n  if (worker != 0U || (require_active && !aggregate_active[aggregate_index])) return;\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");
        for (index, spec) in self.aggs.iter().enumerate() {
            let groups = self.global_table(spec.box_index, spec.group_table_index);
            let target = self.global_table(spec.box_index, spec.target_table_index);
            writeln!(out, "  if (aggregate_index == {index}U) {{ unsigned long long group_count = row_counts[{groups}]; {}* values = ({}*)(partials + agg_offsets[{index}] * 2ULL);", spec.ty.cuda(), spec.ty.cuda()).unwrap();
            let rows = Rows::State {
                box_index: spec.box_index,
                table_index: spec.target_table_index,
            };
            self.emit_expr_validation(
                out,
                &spec.filter,
                rows,
                Some(&Ty::Bool),
                "state",
                &format!("row_counts[{target}]"),
                ValidationTarget::AggregateFact,
            )?;
            if let AggOp::Sum { value } = &spec.op {
                self.emit_expr_validation(
                    out,
                    value,
                    rows,
                    Some(&spec.ty),
                    "state",
                    &format!("row_counts[{target}]"),
                    ValidationTarget::AggregateFact,
                )?;
            }
            writeln!(out, "    for (unsigned long long group = 0; group < group_count; ++group) values[group] = ({})0;", spec.ty.cuda()).unwrap();
            let rows = Rows::State {
                box_index: spec.box_index,
                table_index: spec.target_table_index,
            };
            let filter = self
                .render(&spec.filter, rows, Some(&Ty::Bool), "state", "row")?
                .0;
            let fk_attr = &self.model.model().boxes[spec.box_index].tables[spec.target_table_index]
                .attrs[spec.target_fk_column]
                .name;
            let group = self.render_attr(rows, fk_attr, "state", "row")?.0;
            // Match eval.rs: evaluate the complete filter column, then the
            // complete Sum value column, then fold selected rows in order.
            writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{target}]; ++row) {{ local_error = 0; (void)({filter}); if (local_error) {{ aggregate_errors[0] = 2U; return; }} }}").unwrap();
            match &spec.op {
                AggOp::Count => {
                    writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{target}]; ++row) {{ local_error = 0; int selected = {filter}; if (local_error) {{ aggregate_errors[0] = 2U; return; }}").unwrap();
                    writeln!(out, "      if (selected) {{ unsigned int group = {group}; if ((unsigned long long)group >= group_count) {{ aggregate_errors[0] = 1U; return; }} values[group] = sembla_add_i64(values[group], 1LL, error); if (local_error) {{ aggregate_errors[0] = 2U; return; }} }}").unwrap();
                }
                AggOp::Sum { value } => {
                    let rendered = self.render(value, rows, Some(&spec.ty), "state", "row")?.0;
                    writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{target}]; ++row) {{ local_error = 0; (void)({rendered}); if (local_error) {{ aggregate_errors[0] = 2U; return; }} }}").unwrap();
                    writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{target}]; ++row) {{ local_error = 0; int selected = {filter}; {} value = ({})({rendered}); if (local_error) {{ aggregate_errors[0] = 2U; return; }}", spec.ty.cuda(), spec.ty.cuda()).unwrap();
                    if spec.ty == Ty::Int {
                        writeln!(out, "      if (selected) {{ unsigned int group = {group}; if ((unsigned long long)group >= group_count) {{ aggregate_errors[0] = 1U; return; }} values[group] = sembla_add_i64(values[group], value, error); if (local_error) {{ aggregate_errors[0] = 2U; return; }} }}").unwrap();
                    } else {
                        writeln!(out, "      if (selected) {{ unsigned int group = {group}; if ((unsigned long long)group >= group_count) {{ aggregate_errors[0] = 1U; return; }} values[group] = values[group] + (double)value; }}").unwrap();
                    }
                }
            }
            out.push_str("    }\n  }\n");
        }
        out.push_str("}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_finish_aggregates(const unsigned char* partials, const unsigned long long* row_counts, unsigned int aggregate_index, const unsigned char* aggregate_active, unsigned char require_active, unsigned char* aggs, const unsigned long long* agg_offsets, unsigned char* aggregate_errors) {\n  unsigned long long group = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  if (require_active && !aggregate_active[aggregate_index]) return;\n");
        for (index, spec) in self.aggs.iter().enumerate() {
            let groups = self.global_table(spec.box_index, spec.group_table_index);
            writeln!(out, "  if (aggregate_index == {index}U && group < row_counts[{groups}]) {{ const {}* base = (const {}*)(partials + agg_offsets[{index}] * 2ULL); {}* result = ({}*)(aggs + agg_offsets[{index}]); result[group] = base[group]; }}", spec.ty.cuda(), spec.ty.cuda(), spec.ty.cuda(), spec.ty.cuda()).unwrap();
        }
        out.push_str("}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_record_aggregate_errors(unsigned char* errors, unsigned long long count, unsigned long long aggregate_index, unsigned char* aggregate_facts) {\n  if (blockIdx.x != 0 || threadIdx.x != 0) return;\n  unsigned char code = 0U;\n  for (unsigned long long i = 0; i < count; ++i) { if (code == 0U && errors[i]) code = errors[i]; errors[i] = 0U; }\n  aggregate_facts[aggregate_index] = code;\n}\n");
        Ok(())
    }

    fn emit_transition_kernels(&self, out: &mut String) -> Result<Vec<String>, CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_validate_transition(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned char* aggregate_facts, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, unsigned int rule_id, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  unsigned char local_error = 0U; unsigned char* error = &local_error;\n");
        for validated in self.model.transitions() {
            let transition = &self.model.model().boxes[validated.box_index].transitions
                [validated.transition_index];
            let table_index = self.table_index(validated.box_index, &transition.table)?;
            let global_table = self.global_table(validated.box_index, table_index);
            let rows = Rows::State {
                box_index: validated.box_index,
                table_index,
            };
            writeln!(out, "  if (rule_id == {}U) {{", validated.rule_id).unwrap();
            let identity = format!("candidate_offsets[{}] + row", validated.rule_id);
            self.emit_expr_validation(
                out,
                &transition.guard,
                rows,
                Some(&Ty::Bool),
                "state",
                &format!("row_counts[{global_table}]"),
                ValidationTarget::Status {
                    code: 3,
                    identity: &identity,
                },
            )?;
            self.emit_expr_validation(
                out,
                &transition.hazard,
                rows,
                Some(&Ty::Real),
                "state",
                &format!("row_counts[{global_table}]"),
                ValidationTarget::Status {
                    code: 3,
                    identity: &identity,
                },
            )?;
            for claim in &transition.contests {
                let resource_ty = self.infer(&claim.resource, rows, None)?;
                self.emit_expr_validation(
                    out,
                    &claim.resource,
                    rows,
                    Some(&resource_ty),
                    "state",
                    &format!("row_counts[{global_table}]"),
                    ValidationTarget::Status {
                        code: 10,
                        identity: &identity,
                    },
                )?;
                if let ClaimOrdering::Key { expr } = &claim.ordering {
                    self.emit_expr_validation(
                        out,
                        expr,
                        rows,
                        None,
                        "state",
                        &format!("row_counts[{global_table}]"),
                        ValidationTarget::Status {
                            code: 10,
                            identity: &identity,
                        },
                    )?;
                }
            }
            out.push_str("    return;\n  }\n");
        }
        out.push_str("}\n");

        let mut names = Vec::new();
        for validated in self.model.transitions() {
            let transition = &self.model.model().boxes[validated.box_index].transitions
                [validated.transition_index];
            let table_index = self.table_index(validated.box_index, &transition.table)?;
            let global_table = self.global_table(validated.box_index, table_index);
            let name = format!("sembla_transition_{:08x}", validated.rule_id);
            names.push(name.clone());
            writeln!(out, "\nextern \"C\" __global__ void {name}(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, unsigned long long seed, unsigned int tick, double dt, unsigned char* enabled, double* times, unsigned char* errors, const unsigned long long* status) {{").unwrap();
            out.push_str("  unsigned long long row = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  if (status[0] != 0ULL) return;\n");
            writeln!(out, "  if (row >= row_counts[{global_table}]) return;\n  unsigned long long candidate = candidate_offsets[{}] + row;\n  unsigned char local_error = 0; unsigned char* error = &local_error;", validated.rule_id).unwrap();
            let rows = Rows::State {
                box_index: validated.box_index,
                table_index,
            };
            let guard = self
                .render(&transition.guard, rows, Some(&Ty::Bool), "state", "row")?
                .0;
            let hazard = self
                .render(&transition.hazard, rows, Some(&Ty::Real), "state", "row")?
                .0;
            writeln!(out, "  int guard = {guard};\n  errors[candidate * 2ULL] = local_error;\n  local_error = 0;\n  double lambda = (double)({hazard});\n  errors[candidate * 2ULL + 1ULL] = local_error;\n  double time = sembla_exp(seed, tick, {}U, (unsigned int)row, 0U, lambda);\n  times[candidate] = time;\n  enabled[candidate] = (unsigned char)(errors[candidate * 2ULL] == 0U && errors[candidate * 2ULL + 1ULL] == 0U && guard && lambda > 0.0 && time < dt);\n}}", validated.rule_id).unwrap();
        }
        Ok(names)
    }

    fn emit_error_check_kernel(&self, out: &mut String) {
        out.push_str("\nextern \"C\" __global__ void sembla_check_candidate_errors(const unsigned char* errors, unsigned long long candidate_begin, unsigned long long candidate_count, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  for (unsigned long long row = 0; row < candidate_count; ++row) { unsigned long long candidate = candidate_begin + row; if (errors[candidate * 2ULL]) { status[0] = 3ULL; status[1] = candidate; return; } }\n  for (unsigned long long row = 0; row < candidate_count; ++row) { unsigned long long candidate = candidate_begin + row; if (errors[candidate * 2ULL + 1ULL]) { status[0] = 3ULL; status[1] = candidate; return; } }\n}\n");
    }

    fn emit_resolve_kernel(&self, out: &mut String) -> Result<(), CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_validate_claims(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, unsigned int rule_id, const unsigned char* enabled, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");
        for validated in self.model.transitions() {
            let transition = &self.model.model().boxes[validated.box_index].transitions
                [validated.transition_index];
            if transition.contests.is_empty() {
                continue;
            }
            let table_index = self.table_index(validated.box_index, &transition.table)?;
            let table_global = self.global_table(validated.box_index, table_index);
            let rows = Rows::State {
                box_index: validated.box_index,
                table_index,
            };
            writeln!(out, "  if (rule_id == {}U) {{", validated.rule_id).unwrap();
            for claim in &transition.contests {
                let resource_ty = self.infer(&claim.resource, rows, None)?;
                let resource = self
                    .render(&claim.resource, rows, Some(&resource_ty), "state", "row")?
                    .0;
                writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{table_global}]; ++row) {{ unsigned long long candidate = candidate_offsets[{}] + row; local_error = 0; (void)({resource}); if (local_error) {{ status[0] = 10ULL; status[1] = candidate; return; }} }}", validated.rule_id).unwrap();
                if let ClaimOrdering::Key { expr } = &claim.ordering {
                    let key = self.render(expr, rows, None, "state", "row")?.0;
                    writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{table_global}]; ++row) {{ unsigned long long candidate = candidate_offsets[{}] + row; local_error = 0; (void)({key}); if (local_error) {{ status[0] = 10ULL; status[1] = candidate; return; }} }}", validated.rule_id).unwrap();
                }
            }
            out.push_str("    return;\n  }\n");
        }
        out.push_str("}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_validate_claim_compatibility(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, const unsigned char* enabled, unsigned int box_index, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");

        // Claim expressions are evaluated eagerly above, before compatibility
        // is considered. Emit each statically incompatible claim pair once in
        // canonical transition/claim order, then inspect only enabled runtime
        // candidates in this single-thread kernel. This preserves CPU error
        // precedence without a result-bearing race in the parallel resolver.
        let transitions = self.model.transitions();
        for (left_transition_position, left) in transitions.iter().enumerate() {
            let left_transition =
                &self.model.model().boxes[left.box_index].transitions[left.transition_index];
            let left_table_index = self.table_index(left.box_index, &left_transition.table)?;
            let left_global = self.global_table(left.box_index, left_table_index);
            let left_rows = Rows::State {
                box_index: left.box_index,
                table_index: left_table_index,
            };
            for (left_claim_index, left_claim) in left_transition.contests.iter().enumerate() {
                let left_ty = self.infer(&left_claim.resource, left_rows, None)?;
                let Ty::Ref(left_target) = left_ty else {
                    return Err(codegen("claim resource is not Ref"));
                };
                for (right_transition_position, right) in transitions
                    .iter()
                    .enumerate()
                    .skip(left_transition_position)
                {
                    if right.box_index != left.box_index {
                        continue;
                    }
                    let right_transition = &self.model.model().boxes[right.box_index].transitions
                        [right.transition_index];
                    let right_table_index =
                        self.table_index(right.box_index, &right_transition.table)?;
                    let right_global = self.global_table(right.box_index, right_table_index);
                    let right_rows = Rows::State {
                        box_index: right.box_index,
                        table_index: right_table_index,
                    };
                    let first_right_claim = if right_transition_position == left_transition_position
                    {
                        left_claim_index + 1
                    } else {
                        0
                    };
                    for right_claim in right_transition.contests.iter().skip(first_right_claim) {
                        let right_ty = self.infer(&right_claim.resource, right_rows, None)?;
                        if right_ty != Ty::Ref(left_target.clone())
                            || claim_ordering_type(self, left_claim, left_rows)?
                                == claim_ordering_type(self, right_claim, right_rows)?
                        {
                            continue;
                        }
                        let left_resource = self
                            .render(
                                &left_claim.resource,
                                left_rows,
                                Some(&Ty::Ref(left_target.clone())),
                                "state",
                                "left_row",
                            )?
                            .0;
                        let right_resource = self
                            .render(
                                &right_claim.resource,
                                right_rows,
                                Some(&right_ty),
                                "state",
                                "right_row",
                            )?
                            .0;
                        writeln!(out, "  if (box_index == {}U) {{\n    for (unsigned long long left_row = 0; left_row < row_counts[{left_global}]; ++left_row) {{\n      unsigned long long left_candidate = candidate_offsets[{}] + left_row;\n      if (!enabled[left_candidate]) continue;\n      unsigned int left_resource = (unsigned int)({left_resource});\n      for (unsigned long long right_row = 0; right_row < row_counts[{right_global}]; ++right_row) {{\n        unsigned long long right_candidate = candidate_offsets[{}] + right_row;\n        if (!enabled[right_candidate]) continue;\n        unsigned int right_resource = (unsigned int)({right_resource});\n        if (left_resource != right_resource) continue;\n        status[0] = 4ULL; status[1] = left_candidate; status[2] = right_candidate; return;\n      }}\n    }}\n  }}", left.box_index, left.rule_id, right.rule_id).unwrap();
                    }
                }
            }
        }
        out.push_str("}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_resolve_conflicts(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, unsigned long long candidate_begin, unsigned long long candidate_count, const unsigned char* enabled, const double* times, unsigned char* wins, const unsigned long long* status) {\n  unsigned long long local_candidate = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  if (local_candidate >= candidate_count || status[0] != 0ULL) return;\n  unsigned long long self_candidate = candidate_begin + local_candidate;\n  wins[self_candidate] = enabled[self_candidate];\n  if (!enabled[self_candidate]) return;\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");
        for validated in self.model.transitions() {
            let transition = &self.model.model().boxes[validated.box_index].transitions
                [validated.transition_index];
            if transition.contests.is_empty() {
                continue;
            }
            let table_index = self.table_index(validated.box_index, &transition.table)?;
            let table_global = self.global_table(validated.box_index, table_index);
            let rows = Rows::State {
                box_index: validated.box_index,
                table_index,
            };
            writeln!(out, "  if (self_candidate >= candidate_offsets[{}] && self_candidate < candidate_offsets[{}] + row_counts[{table_global}]) {{ unsigned long long row = self_candidate - candidate_offsets[{}];", validated.rule_id, validated.rule_id, validated.rule_id).unwrap();
            for (claim_index, claim) in transition.contests.iter().enumerate() {
                let resource_ty = self.infer(&claim.resource, rows, None)?;
                let Ty::Ref(target_name) = resource_ty else {
                    return Err(codegen("claim resource is not Ref"));
                };
                let resource = self
                    .render(
                        &claim.resource,
                        rows,
                        Some(&Ty::Ref(target_name.clone())),
                        "state",
                        "row",
                    )?
                    .0;
                writeln!(out, "    unsigned int resource_{claim_index} = {resource};").unwrap();
                let (self_key, self_key_ty) =
                    self.claim_key(claim, rows, validated.rule_id, "row", "self_candidate")?;
                writeln!(out, "    {} best_key_{claim_index} = {self_key}; unsigned int best_rule_{claim_index} = {}U; unsigned int best_entity_{claim_index} = (unsigned int)row;", self_key_ty.cuda(), validated.rule_id).unwrap();
                for other in self.model.transitions() {
                    let other_transition = &self.model.model().boxes[other.box_index].transitions
                        [other.transition_index];
                    let other_table_index =
                        self.table_index(other.box_index, &other_transition.table)?;
                    let other_rows = Rows::State {
                        box_index: other.box_index,
                        table_index: other_table_index,
                    };
                    let other_global = self.global_table(other.box_index, other_table_index);
                    for other_claim in &other_transition.contests {
                        let other_ty = self.infer(&other_claim.resource, other_rows, None)?;
                        if other_ty != Ty::Ref(target_name.clone())
                            || other.box_index != validated.box_index
                        {
                            continue;
                        }
                        let compatible = claim_ordering_type(self, claim, rows)?
                            == claim_ordering_type(self, other_claim, other_rows)?;
                        if !compatible {
                            continue;
                        }
                        let other_resource = self
                            .render(
                                &other_claim.resource,
                                other_rows,
                                Some(&other_ty),
                                "state",
                                "other_row",
                            )?
                            .0;
                        writeln!(out, "    for (unsigned long long other_row = 0; other_row < row_counts[{other_global}]; ++other_row) {{ unsigned long long other_candidate = candidate_offsets[{}] + other_row; if (!enabled[other_candidate] || (unsigned int)({other_resource}) != resource_{claim_index}) continue;", other.rule_id).unwrap();
                        let (other_key, _) = self.claim_key(
                            other_claim,
                            other_rows,
                            other.rule_id,
                            "other_row",
                            "other_candidate",
                        )?;
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

    fn claim_key(
        &self,
        claim: &sembla_ir::ResourceClaim,
        rows: Rows,
        _rule: u32,
        row: &str,
        candidate: &str,
    ) -> Result<(String, Ty), CudaError> {
        match &claim.ordering {
            ClaimOrdering::RaceTime => Ok((format!("times[{candidate}]"), Ty::Real)),
            ClaimOrdering::Key { expr } => self.render(expr, rows, None, "state", row),
        }
    }

    fn emit_apply_kernel(&self, out: &mut String) -> Result<(), CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_validate_effects(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned char* aggregate_facts, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, const unsigned char* wins, unsigned int box_index, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  unsigned char local_error = 0U; unsigned char* error = &local_error;\n");
        for validated in self.model.transitions() {
            let transition = &self.model.model().boxes[validated.box_index].transitions
                [validated.transition_index];
            let table_index = self.table_index(validated.box_index, &transition.table)?;
            let table = &self.model.model().boxes[validated.box_index].tables[table_index];
            let global_table = self.global_table(validated.box_index, table_index);
            let rows = Rows::State {
                box_index: validated.box_index,
                table_index,
            };
            writeln!(out, "  if (box_index == {}U) {{ int any_winner = 0; for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) any_winner |= wins[candidate_offsets[{}] + row] != 0; if (any_winner) {{", validated.box_index, validated.rule_id).unwrap();
            let identity = format!("candidate_offsets[{}] + row", validated.rule_id);
            for effect in &transition.effects {
                let Effect::SetAttr { attr, value } = effect;
                let attr_index = attr_index(table, attr)?;
                let ty = Ty::from(&table.attrs[attr_index].ty);
                self.emit_expr_validation(
                    out,
                    value,
                    rows,
                    Some(&ty),
                    "state",
                    &format!("row_counts[{global_table}]"),
                    ValidationTarget::Status {
                        code: 5,
                        identity: &identity,
                    },
                )?;
                let rendered = self.render(value, rows, Some(&ty), "state", "row")?.0;
                match &ty {
                    Ty::Enum(variants) => writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) {{ local_error = 0U; unsigned long long value = (unsigned long long)({rendered}); if (local_error) {{ status[0] = 5ULL; status[1] = candidate_offsets[{}] + row; return; }} if (value >= {}ULL) {{ status[0] = 6ULL; status[1] = candidate_offsets[{}] + row; return; }} }}", validated.rule_id, variants.len(), validated.rule_id).unwrap(),
                    Ty::Ref(target) => {
                        let target_index = self.table_index(validated.box_index, target)?;
                        let target_global = self.global_table(validated.box_index, target_index);
                        writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) {{ local_error = 0U; unsigned long long value = (unsigned long long)({rendered}); if (local_error) {{ status[0] = 5ULL; status[1] = candidate_offsets[{}] + row; return; }} if (value >= row_counts[{target_global}]) {{ status[0] = 7ULL; status[1] = candidate_offsets[{}] + row; return; }} }}", validated.rule_id, validated.rule_id).unwrap();
                    }
                    _ => {}
                }
            }
            out.push_str("  } }\n");
        }
        out.push_str("}\n");

        out.push_str("\nextern \"C\" __global__ void sembla_prepare_effects(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, const unsigned char* wins, const unsigned long long* write_offsets, int* owners, unsigned long long* owner_values, unsigned long long owner_count, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  for (unsigned long long i = 0; i < owner_count; ++i) owners[i] = -1;\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");
        for validated in self.model.transitions() {
            let transition = &self.model.model().boxes[validated.box_index].transitions
                [validated.transition_index];
            let table_index = self.table_index(validated.box_index, &transition.table)?;
            let table = &self.model.model().boxes[validated.box_index].tables[table_index];
            let global_table = self.global_table(validated.box_index, table_index);
            let rows = Rows::State {
                box_index: validated.box_index,
                table_index,
            };
            writeln!(out, "  {{ int any_winner = 0; for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) any_winner |= wins[candidate_offsets[{}] + row] != 0; if (any_winner) {{", validated.rule_id).unwrap();
            writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) {{ unsigned long long candidate = candidate_offsets[{}] + row; if (!wins[candidate]) continue;", validated.rule_id).unwrap();
            for effect in &transition.effects {
                let Effect::SetAttr { attr, value } = effect;
                let attr_index = attr_index(table, attr)?;
                let ty = Ty::from(&table.attrs[attr_index].ty);
                let column = self.column(validated.box_index, table_index, attr_index);
                let rendered = self.render(value, rows, Some(&ty), "state", "row")?.0;
                out.push_str("      {\n");
                writeln!(out, "      local_error = 0U; {} value = ({})({rendered}); if (local_error) {{ status[0] = 5ULL; status[1] = candidate; return; }}", ty.cuda(), ty.cuda()).unwrap();
                match &ty {
                    Ty::Enum(variants) => writeln!(out, "      if ((unsigned long long)value >= {}ULL) {{ status[0] = 6ULL; status[1] = candidate; return; }}", variants.len()).unwrap(),
                    Ty::Ref(target) => {
                        let target_index = self.table_index(validated.box_index, target)?;
                        let target_global = self.global_table(validated.box_index, target_index);
                        writeln!(out, "      if ((unsigned long long)value >= row_counts[{target_global}]) {{ status[0] = 7ULL; status[1] = candidate; return; }}").unwrap();
                    }
                    _ => {}
                }
                writeln!(out, "      {{ unsigned long long owner = write_offsets[{column}] + row; if (owners[owner] != -1) {{ status[0] = 8ULL; status[1] = owner; status[2] = (unsigned long long)owners[owner]; status[3] = {}ULL; return; }} owners[owner] = (int){}U;", validated.rule_id, validated.rule_id).unwrap();
                match ty {
                    Ty::Real => out.push_str("        owner_values[owner] = (unsigned long long)__double_as_longlong(value);\n"),
                    _ => out.push_str("        owner_values[owner] = (unsigned long long)value;\n"),
                }
                out.push_str("      }\n      }\n");
            }
            out.push_str("    }\n  }\n  }\n");
        }
        out.push_str("}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_apply_effects(unsigned char* next_state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned long long* write_offsets, const int* owners, const unsigned long long* owner_values, unsigned long long owner_count, const unsigned long long* status) {\n  unsigned long long owner = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  if (owner >= owner_count || status[0] != 0ULL || owners[owner] == -1) return;\n");
        for (box_index, model_box) in self.model.model().boxes.iter().enumerate() {
            for (table_index, table) in model_box.tables.iter().enumerate() {
                let global_table = self.global_table(box_index, table_index);
                for (attr_index, attr) in table.attrs.iter().enumerate() {
                    let column = self.column(box_index, table_index, attr_index);
                    let ty = Ty::from(&attr.ty);
                    writeln!(out, "  if (owner >= write_offsets[{column}] && owner < write_offsets[{column}] + row_counts[{global_table}]) {{ unsigned long long row = owner - write_offsets[{column}];").unwrap();
                    match ty {
                        Ty::Real => writeln!(out, "    *((double*)(next_state + column_offsets[{column}]) + row) = __longlong_as_double((long long)owner_values[owner]); return; }}").unwrap(),
                        _ => writeln!(out, "    *(({}*)(next_state + column_offsets[{column}]) + row) = ({})owner_values[owner]; return; }}", ty.cuda(), ty.cuda()).unwrap(),
                    }
                }
            }
        }
        out.push_str("}\n");
        Ok(())
    }

    fn emit_output_kernel(&self, out: &mut String) -> Result<(), CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_validate_outputs(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned char* aggregate_facts, const unsigned long long* agg_offsets, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  unsigned char local_error = 0U; unsigned char* error = &local_error;\n");
        for wire in &self.model.model().wires {
            let from_box = self
                .model
                .model()
                .boxes
                .iter()
                .position(|entry| entry.name == wire.from.r#box)
                .ok_or_else(|| codegen("wire source box disappeared"))?;
            let to_box = self
                .model
                .model()
                .boxes
                .iter()
                .position(|entry| entry.name == wire.to.r#box)
                .ok_or_else(|| codegen("wire target box disappeared"))?;
            let output = self.model.model().boxes[from_box]
                .outputs
                .iter()
                .find(|entry| entry.name == wire.from.port)
                .ok_or_else(|| codegen("wire output disappeared"))?;
            let to_port_index = self.port_index(to_box, &wire.to.port)?;
            let sembla_ir::OutputBuilder::PerTable { table, fields } = &output.builder;
            let table_index = self.table_index(from_box, table)?;
            let global_table = self.global_table(from_box, table_index);
            let rows = Rows::State {
                box_index: from_box,
                table_index,
            };
            for (field_index, field) in fields.iter().enumerate() {
                let target_field = self.input_field(to_box, to_port_index, field_index);
                let ty = Ty::from(&output.schema[field_index].ty);
                let identity = target_field.to_string();
                let target = ValidationTarget::Status {
                    code: 9,
                    identity: &identity,
                };
                if let Some(filter) = &field.filter {
                    self.emit_expr_validation(
                        out,
                        filter,
                        rows,
                        Some(&Ty::Bool),
                        "state",
                        &format!("row_counts[{global_table}]"),
                        target,
                    )?;
                }
                if let AggOp::Sum { value } = &field.op {
                    self.emit_expr_validation(
                        out,
                        value,
                        rows,
                        Some(&ty),
                        "state",
                        &format!("row_counts[{global_table}]"),
                        target,
                    )?;
                }
                if ty == Ty::Int {
                    let selected = if let Some(filter) = &field.filter {
                        self.render(filter, rows, Some(&Ty::Bool), "state", "row")?
                            .0
                    } else {
                        "1".to_owned()
                    };
                    let value = match &field.op {
                        AggOp::Count => "1LL".to_owned(),
                        AggOp::Sum { value } => {
                            self.render(value, rows, Some(&ty), "state", "row")?.0
                        }
                    };
                    writeln!(out, "    {{ long long result = 0LL; for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) {{ local_error = 0U; int selected = {selected}; long long value = (long long)({value}); if (local_error) {{ status[0] = 9ULL; status[1] = {target_field}ULL; return; }} if (selected) {{ result = sembla_add_i64(result, value, error); if (local_error) {{ status[0] = 9ULL; status[1] = {target_field}ULL; return; }} }} }} }}").unwrap();
                }
            }
        }
        out.push_str("}\n");

        out.push_str("\nextern \"C\" __global__ void sembla_prepare_outputs(unsigned long long* next_input_counts, unsigned long long port_count, unsigned char* output_errors, unsigned long long error_count) {\n  if (blockIdx.x != 0 || threadIdx.x != 0) return;\n  for (unsigned long long i = 0; i < port_count; ++i) next_input_counts[i] = 0ULL;\n  for (unsigned long long i = 0; i < error_count; ++i) output_errors[i] = 0U;\n");
        for wire in &self.model.model().wires {
            let to_box = self
                .model
                .model()
                .boxes
                .iter()
                .position(|entry| entry.name == wire.to.r#box)
                .ok_or_else(|| codegen("wire target box disappeared"))?;
            let to_port_index = self.port_index(to_box, &wire.to.port)?;
            let to_port = self.port(to_box, to_port_index);
            writeln!(out, "  next_input_counts[{to_port}] = 1ULL;").unwrap();
        }
        out.push_str("}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_build_output_partials(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, unsigned long long* output_partials, unsigned char* output_errors, const unsigned long long* status) {\n  unsigned long long field = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  if (status[0] != 0ULL) return;\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");
        for wire in &self.model.model().wires {
            let from_box = self
                .model
                .model()
                .boxes
                .iter()
                .position(|entry| entry.name == wire.from.r#box)
                .ok_or_else(|| codegen("wire source box disappeared"))?;
            let to_box = self
                .model
                .model()
                .boxes
                .iter()
                .position(|entry| entry.name == wire.to.r#box)
                .ok_or_else(|| codegen("wire target box disappeared"))?;
            let output = self.model.model().boxes[from_box]
                .outputs
                .iter()
                .find(|entry| entry.name == wire.from.port)
                .ok_or_else(|| codegen("wire output disappeared"))?;
            let to_port_index = self.port_index(to_box, &wire.to.port)?;
            let sembla_ir::OutputBuilder::PerTable { table, fields } = &output.builder;
            let table_index = self.table_index(from_box, table)?;
            let global_table = self.global_table(from_box, table_index);
            let rows = Rows::State {
                box_index: from_box,
                table_index,
            };
            for (field_index, field) in fields.iter().enumerate() {
                let target_field = self.input_field(to_box, to_port_index, field_index);
                let ty = Ty::from(&output.schema[field_index].ty);
                writeln!(
                    out,
                    "  if (field == {target_field}ULL) {{ {} result = ({})0;",
                    ty.cuda(),
                    ty.cuda()
                )
                .unwrap();
                let selected = if let Some(filter) = &field.filter {
                    self.render(filter, rows, Some(&Ty::Bool), "state", "row")?
                        .0
                } else {
                    "1".to_owned()
                };
                writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) {{ local_error = 0; (void)({selected}); if (local_error) {{ output_errors[field] = 9U; return; }} }}").unwrap();
                let rendered_value = match &field.op {
                    AggOp::Count => None,
                    AggOp::Sum { value } => {
                        let rendered = self.render(value, rows, Some(&ty), "state", "row")?.0;
                        writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) {{ local_error = 0; (void)({rendered}); if (local_error) {{ output_errors[field] = 9U; return; }} }}").unwrap();
                        Some(rendered)
                    }
                };
                writeln!(out, "    for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) {{ local_error = 0; int selected = {selected};").unwrap();
                match (&field.op, rendered_value) {
                    (AggOp::Count, None) => out.push_str(
                        "      if (selected) result = sembla_add_i64(result, 1LL, error);\n",
                    ),
                    (AggOp::Sum { .. }, Some(value)) => {
                        writeln!(out, "      {} value = ({})({value});", ty.cuda(), ty.cuda())
                            .unwrap();
                        if ty == Ty::Int {
                            out.push_str("      if (selected) result = sembla_add_i64(result, value, error);\n");
                        } else {
                            out.push_str("      if (selected) result = result + (double)value;\n");
                        }
                    }
                    _ => unreachable!("output aggregate operation and rendered value agree"),
                }
                out.push_str(
                    "      if (local_error) { output_errors[field] = 9U; return; }\n    }\n",
                );
                if ty == Ty::Real {
                    out.push_str("    output_partials[field] = (unsigned long long)__double_as_longlong(result);\n  }\n");
                } else {
                    out.push_str("    output_partials[field] = (unsigned long long)result;\n  }\n");
                }
            }
        }
        out.push_str("}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_finish_outputs(const unsigned long long* output_partials, unsigned long long field_count, unsigned char* next_inputs, const unsigned long long* next_input_offsets, unsigned char* output_errors) {\n  unsigned long long field = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  if (field >= field_count) return;\n");
        for wire in &self.model.model().wires {
            let to_box = self
                .model
                .model()
                .boxes
                .iter()
                .position(|entry| entry.name == wire.to.r#box)
                .ok_or_else(|| codegen("wire target box disappeared"))?;
            let from_box = self
                .model
                .model()
                .boxes
                .iter()
                .position(|entry| entry.name == wire.from.r#box)
                .ok_or_else(|| codegen("wire source box disappeared"))?;
            let output = self.model.model().boxes[from_box]
                .outputs
                .iter()
                .find(|entry| entry.name == wire.from.port)
                .ok_or_else(|| codegen("wire output disappeared"))?;
            let to_port_index = self.port_index(to_box, &wire.to.port)?;
            let to_port = self.port(to_box, to_port_index);
            let sembla_ir::OutputBuilder::PerTable { fields, .. } = &output.builder;
            for (field_index, _) in fields.iter().enumerate() {
                let target_field = self.input_field(to_box, to_port_index, field_index);
                let ty = Ty::from(&output.schema[field_index].ty);
                writeln!(out, "  if (field == {target_field}ULL) {{").unwrap();
                if ty == Ty::Int {
                    out.push_str("    *((long long*)(next_inputs + next_input_offsets[field])) = (long long)output_partials[field];\n");
                } else {
                    out.push_str("    *((double*)(next_inputs + next_input_offsets[field])) = __longlong_as_double((long long)output_partials[field]);\n");
                }
                let _ = to_port;
                out.push_str("    return;\n  }\n");
            }
        }
        out.push_str("}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_check_output_errors(const unsigned char* errors, unsigned long long field_count, unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;\n  for (unsigned long long field = 0; field < field_count; ++field) {\n    if (errors[field]) { status[0] = 9ULL; status[1] = field; return; }\n  }\n}\n");
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
        Rows::State {
            box_index,
            table_index,
        } => Ok((box_index, table_index)),
        Rows::Input { .. } => Err(codegen("state aggregate used in input-row context")),
    }
}

fn attr_index(table: &Table, name: &str) -> Result<usize, CudaError> {
    table
        .attrs
        .iter()
        .position(|attr| attr.name == name)
        .ok_or_else(|| codegen(format!("table '{}' has no attribute '{name}'", table.name)))
}

fn claim_ordering_type(
    generator: &Generator<'_>,
    claim: &sembla_ir::ResourceClaim,
    rows: Rows,
) -> Result<String, CudaError> {
    match &claim.ordering {
        ClaimOrdering::RaceTime => Ok("race-time".to_owned()),
        ClaimOrdering::Key { expr } => Ok(format!("key:{:?}", generator.infer(expr, rows, None)?)),
    }
}

fn i64_literal(value: i64) -> String {
    if value == i64::MIN {
        "(-0x7fffffffffffffffLL - 1LL)".to_owned()
    } else {
        format!("{value}LL")
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
// No result-bearing atomics are used. Effects are staged in generated
// rule/effect/row order, then scattered by ascending destination cell.
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
  const long long min = (-0x7fffffffffffffffLL - 1LL);
  const long long max = 0x7fffffffffffffffLL;
  if (a == 0 || b == 0) return 0;
  if ((a == min && b == -1) || (b == min && a == -1)) { *error = 1; return 0; }
  if (a > 0) {
    if ((b > 0 && a > max / b) || (b < 0 && b < min / a)) { *error = 1; return 0; }
  } else {
    if ((b > 0 && a < min / b) || (b < 0 && a < max / b)) { *error = 1; return 0; }
  }
  return a * b;
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

    fn example_model(name: &str) -> sembla_ir::ValidatedModel {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../examples/{name}"));
        let source = std::fs::read_to_string(path).unwrap();
        sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
    }

    fn sir_model() -> sembla_ir::ValidatedModel {
        example_model("sir.json")
    }

    fn nested_output_model(wired: bool) -> sembla_ir::ValidatedModel {
        use sembla_ir::{
            AggJoin, AggOp, Attr, AttrType, Box as ModelBox, Expr, Model, OutputBuilder,
            OutputDecl, OutputField, PortDecl, Table, Wire, WireEndpoint,
        };
        let group_attr = Attr {
            name: "group".to_owned(),
            ty: AttrType::Ref {
                table: "Group".to_owned(),
            },
        };
        let total_attr = Attr {
            name: "total".to_owned(),
            ty: AttrType::Real,
        };
        sembla_ir::validate(Model {
            name: "nested_output".to_owned(),
            dt: 1.0,
            params: Vec::new(),
            boxes: vec![
                ModelBox {
                    name: "source".to_owned(),
                    tables: vec![
                        Table {
                            name: "Group".to_owned(),
                            size_hint: 1,
                            attrs: Vec::new(),
                        },
                        Table {
                            name: "Person".to_owned(),
                            size_hint: 2,
                            attrs: vec![
                                group_attr.clone(),
                                Attr {
                                    name: "x".to_owned(),
                                    ty: AttrType::Real,
                                },
                            ],
                        },
                    ],
                    transitions: Vec::new(),
                    inputs: Vec::new(),
                    outputs: vec![OutputDecl {
                        name: "totals".to_owned(),
                        schema: vec![total_attr.clone()],
                        builder: OutputBuilder::PerTable {
                            table: "Person".to_owned(),
                            fields: vec![OutputField {
                                name: "total".to_owned(),
                                op: AggOp::Sum {
                                    value: Box::new(Expr::Agg {
                                        op: AggOp::Sum {
                                            value: Box::new(Expr::SelfAttr {
                                                name: "x".to_owned(),
                                            }),
                                        },
                                        table: "Person".to_owned(),
                                        on: AggJoin {
                                            fk_attr: "group".to_owned(),
                                            self_fk_attr: "group".to_owned(),
                                        },
                                        filter: Box::new(Expr::Bool { value: true }),
                                    }),
                                },
                                filter: None,
                            }],
                        },
                    }],
                    views: Vec::new(),
                },
                ModelBox {
                    name: "sink".to_owned(),
                    tables: Vec::new(),
                    transitions: Vec::new(),
                    inputs: vec![PortDecl {
                        name: "totals".to_owned(),
                        schema: vec![total_attr],
                    }],
                    outputs: Vec::new(),
                    views: Vec::new(),
                },
            ],
            wires: if wired {
                vec![Wire {
                    from: WireEndpoint {
                        r#box: "source".to_owned(),
                        port: "totals".to_owned(),
                    },
                    to: WireEndpoint {
                        r#box: "sink".to_owned(),
                        port: "totals".to_owned(),
                    },
                }]
            } else {
                Vec::new()
            },
            summaries: Vec::new(),
        })
        .unwrap()
    }

    fn contested_model() -> sembla_ir::ValidatedModel {
        let source = r#"{"name":"claims","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Worker","size_hint":1,"attrs":[]},{"name":"Applicant","size_hint":2,"attrs":[{"name":"worker","ty":{"kind":"ref","table":"Worker"}},{"name":"priority","ty":{"kind":"int"}},{"name":"state","ty":{"kind":"enum","variants":["Waiting","Done"]}}]}],"transitions":[{"name":"finish","table":"Applicant","guard":{"kind":"enum_is","attr":"state","variant":"Waiting"},"hazard":{"kind":"real","value":1.0},"effects":[{"kind":"set_attr","attr":"state","value":{"kind":"enum","variant":"Done"}}],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"mul","lhs":{"kind":"self_attr","name":"priority"},"rhs":{"kind":"int","value":2}}}}]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
        sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
    }

    fn incompatible_claim_model() -> sembla_ir::ValidatedModel {
        let source = r#"{"name":"incompatible_claims","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Worker","size_hint":1,"attrs":[]},{"name":"Applicant","size_hint":1,"attrs":[{"name":"worker","ty":{"kind":"ref","table":"Worker"}},{"name":"priority","ty":{"kind":"int"}}]}],"transitions":[{"name":"race","table":"Applicant","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":1e300},"effects":[],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"race_time"}}]},{"name":"priority","table":"Applicant","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":1e300},"effects":[],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"self_attr","name":"priority"}}}]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
        sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
    }

    fn minimum_integer_model() -> sembla_ir::ValidatedModel {
        let source = r#"{"name":"minimum_integer","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Person","size_hint":1,"attrs":[{"name":"x","ty":{"kind":"int"}}]}],"transitions":[{"name":"set_minimum","table":"Person","guard":{"kind":"lt","lhs":{"kind":"int","value":-9223372036854775808},"rhs":{"kind":"self_attr","name":"x"}},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"x","value":{"kind":"int","value":-9223372036854775808}}],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
        sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
    }

    fn input_integer_ordering_model() -> sembla_ir::ValidatedModel {
        let source = r#"{"name":"input_integer_ordering","dt":1.0,"params":[],"boxes":[{"name":"source","tables":[{"name":"Event","size_hint":1,"attrs":[{"name":"amount","ty":{"kind":"int"}}]}],"transitions":[],"inputs":[],"outputs":[{"name":"events","schema":[{"name":"amount","ty":{"kind":"int"}}],"builder":{"kind":"per_table","table":"Event","fields":[{"name":"amount","op":{"kind":"sum","value":{"kind":"self_attr","name":"amount"}},"filter":null}]}}],"views":[]},{"name":"sink","tables":[{"name":"Agent","size_hint":1,"attrs":[{"name":"state","ty":{"kind":"enum","variants":["Off","On"]}}]}],"transitions":[{"name":"activate","table":"Agent","guard":{"kind":"gt","lhs":{"kind":"input","port":"events","agg":{"op":{"kind":"count"},"filter":{"kind":"gt","lhs":{"kind":"self_attr","name":"amount"},"rhs":{"kind":"int","value":9007199254740992}}}},"rhs":{"kind":"int","value":0}},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"state","value":{"kind":"enum","variant":"On"}}],"contests":[]}],"inputs":[{"name":"events","schema":[{"name":"amount","ty":{"kind":"int"}}]}],"outputs":[],"views":[]}],"wires":[{"from":{"box":"source","port":"events"},"to":{"box":"sink","port":"events"}}],"summaries":[]}"#;
        sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
    }

    fn shared_schedule_output_aggregate_model() -> sembla_ir::ValidatedModel {
        let source = r#"{"name":"shared_aggregate","dt":1.0,"params":[],"boxes":[{"name":"source","tables":[{"name":"Group","size_hint":1,"attrs":[]},{"name":"Person","size_hint":1,"attrs":[{"name":"group","ty":{"kind":"ref","table":"Group"}}]}],"transitions":[{"name":"observe","table":"Person","guard":{"kind":"gt","lhs":{"kind":"agg","op":{"kind":"count"},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}},"rhs":{"kind":"int","value":0}},"hazard":{"kind":"real","value":1.0},"effects":[],"contests":[]}],"inputs":[],"outputs":[{"name":"totals","schema":[{"name":"total","ty":{"kind":"int"}}],"builder":{"kind":"per_table","table":"Person","fields":[{"name":"total","op":{"kind":"sum","value":{"kind":"agg","op":{"kind":"count"},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}}},"filter":null}]}}],"views":[]},{"name":"sink","tables":[],"transitions":[],"inputs":[{"name":"totals","schema":[{"name":"total","ty":{"kind":"int"}}]}],"outputs":[],"views":[]}],"wires":[{"from":{"box":"source","port":"totals"},"to":{"box":"sink","port":"totals"}}],"summaries":[]}"#;
        sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
    }

    #[test]
    fn hostile_model_name_is_represented_by_ascii_digest_only() {
        let source = r#"{"name":"ok\n#error injected_model_name\r\\☃","dt":1.0,"params":[],"boxes":[],"wires":[],"summaries":[]}"#;
        let model = sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap();
        let first = generate(&model).unwrap();
        let second = generate(&model).unwrap();
        assert_eq!(first, second);
        assert!(!first.source.contains("#error injected_model_name"));
        let label = first.source.lines().nth(1).unwrap();
        assert!(label.starts_with("// model-name-sha256: "));
        assert_eq!(label.len(), "// model-name-sha256: ".len() + 64);
        assert!(label.is_ascii());
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
        assert!(first.source.contains("sembla_build_aggregate_partials"));
        assert!(first.source.contains("sembla_finish_aggregates"));
        assert!(first.source.contains("sembla_validate_claims"));
        assert!(first.source.contains("sembla_resolve_conflicts"));
        assert!(first.source.contains("sembla_prepare_effects"));
        assert!(first.source.contains("sembla_apply_effects"));
        assert!(first.source.contains("sembla_build_output_partials"));
        assert!(first.source.contains("sembla_finish_outputs"));
        assert!(!first.source.contains("atomicAdd"));
        assert!(!first.source.contains("atomicMin"));
    }

    #[test]
    fn nested_output_aggregate_is_collected_before_ordered_output() {
        let generated = generate(&nested_output_model(true)).unwrap();
        assert_eq!(generated.aggregate_group_tables.len(), 1);
        assert!(generated.schedule_aggregate_indices.is_empty());
        assert!(generated.effect_aggregate_indices.is_empty());
        assert_eq!(generated.output_aggregate_indices, [0]);
        assert!(generated.source.contains("const unsigned char* aggs"));
        assert!(generated.source.contains("sembla_build_output_partials"));
    }

    #[test]
    fn unwired_output_aggregates_are_not_collected() {
        let generated = generate(&nested_output_model(false)).unwrap();
        assert!(generated.aggregate_group_tables.is_empty());
        assert!(generated.schedule_aggregate_indices.is_empty());
        assert!(generated.effect_aggregate_indices.is_empty());
        assert!(generated.output_aggregate_indices.is_empty());
    }

    #[test]
    fn contested_source_eagerly_checks_claims_and_uses_candidate_parallel_argmin() {
        let generated = generate(&contested_model()).unwrap();
        assert!(generated.source.contains("sembla_validate_claims"));
        assert!(generated.source.contains("status[0] = 10ULL"));
        assert!(generated
            .source
            .contains("self_candidate = candidate_begin + local_candidate"));
        assert!(generated.source.contains("sembla_prepare_effects"));
        assert!(generated.source.contains("owner_values[owner]"));
    }

    #[test]
    fn incompatible_claims_are_checked_serially_before_parallel_argmin() {
        let generated = generate(&incompatible_claim_model()).unwrap();
        let (before_resolve, resolver_and_after) = generated
            .source
            .split_once("extern \"C\" __global__ void sembla_resolve_conflicts")
            .unwrap();
        let (resolver, _) = resolver_and_after
            .split_once("extern \"C\" __global__ void sembla_prepare_effects")
            .unwrap();

        assert!(before_resolve.contains("const unsigned char* enabled"));
        assert!(before_resolve.contains("status[0] = 4ULL"));
        assert!(before_resolve.contains("if (!enabled[left_candidate]) continue"));
        assert!(before_resolve.contains("if (!enabled[right_candidate]) continue"));
        assert!(resolver.contains("const unsigned long long* status"));
        assert!(!resolver.contains("status[0] ="));
        assert!(!resolver.contains("status[1] ="));
        assert!(!resolver.contains("status[2] ="));
        assert!(!generated.source.contains("atomicAdd"));
        assert!(!generated.source.contains("atomicMin"));
    }

    #[test]
    fn minimum_integer_literal_remains_signed_and_generation_is_deterministic() {
        let first = generate(&minimum_integer_model()).unwrap();
        let second = generate(&minimum_integer_model()).unwrap();

        assert_eq!(first, second);
        assert!(first.source.contains("(-0x7fffffffffffffffLL - 1LL)"));
        let oversized_decimal = ["-9223372036854775808", "LL"].concat();
        assert!(!first.source.contains(&oversized_decimal));
    }

    #[test]
    fn input_integer_ordering_promotes_both_operands_to_f64() {
        let generated = generate(&input_integer_ordering_model()).unwrap();
        assert!(generated.source.contains("(double)(9007199254740992LL)"));
        assert!(generated
            .source
            .contains("(double)((*((const long long*)(inputs"));
    }

    #[test]
    fn shared_aggregate_is_staged_for_schedule_and_output() {
        let generated = generate(&shared_schedule_output_aggregate_model()).unwrap();
        assert_eq!(generated.aggregate_group_tables.len(), 1);
        assert_eq!(generated.state_aggregate_indices, [0]);
        assert_eq!(generated.schedule_aggregate_indices, [0]);
        assert_eq!(generated.schedule_aggregate_indices_by_rule, [vec![0]]);
        assert!(generated.effect_aggregate_indices.is_empty());
        assert_eq!(generated.output_aggregate_indices, [0]);
        assert!(generated
            .source
            .contains("aggregate_facts[aggregate_index] = code"));
        assert!(generated.source.contains("status[1] = 0ULL"));
    }

    #[test]
    fn policy_source_contains_prospective_output_and_parallel_result_stages() {
        let generated = generate(&example_model("sir_policy.json")).unwrap();
        assert!(generated.source.contains("sembla_build_output_partials"));
        assert!(generated.source.contains("sembla_finish_outputs"));
        assert!(generated
            .source
            .contains("self_candidate = candidate_begin + local_candidate"));
        assert!(generated
            .source
            .contains("owner = (unsigned long long)blockIdx.x"));
        assert!(!generated.source.contains("long long result = a * b"));
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
