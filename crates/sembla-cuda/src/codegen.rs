use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sembla_ir::{
    AggOp, AttrType, ClaimOrdering, Effect, Expr, ParamType, Table, ValidatedModel, ViewReduce,
};
#[cfg(any(feature = "cuda", test))]
use sembla_runtime::executor::GroupedViewValue;
use sembla_runtime::executor::{device_observation_eligibility, DeviceObservationEligibility};
use sha2::{Digest, Sha256};

use crate::CudaError;

pub const DUMP_ENV: &str = "SEMBLA_CUDA_DUMP_DIR";

/// Maximum counters allocated for one dense grouped-observation histogram.
/// Runtime-derived key spaces above this bound fail instead of falling back.
#[cfg(any(feature = "cuda", test))]
pub const GROUPED_OBSERVATION_KEY_SPACE_LIMIT: usize = 1_048_576;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GroupedObservationAxis {
    Enum {
        column: usize,
        cardinality: u64,
    },
    Ref {
        column: usize,
        target_table: usize,
    },
    BandedInt {
        column: usize,
        width: u64,
        extrema_index: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedGroupedObservation {
    pub box_name: String,
    pub name: String,
    pub table: usize,
    pub axes: Vec<GroupedObservationAxis>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedEnumObservation {
    pub table: usize,
}

#[cfg(any(feature = "cuda", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GroupedObservationAxisLayout {
    pub minimum: i64,
    pub cardinality: u64,
}

#[cfg(any(feature = "cuda", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GroupedObservationLayout {
    pub axes: Vec<GroupedObservationAxisLayout>,
    pub key_space_size: usize,
}

/// Executes the host-state fallback only when the run-wide observation gate
/// requires it. Keeping this tiny router toolkit-free makes the transfer gate
/// executable in local tests while the supplied closure remains the real CUDA
/// download/reconstruction path on hardware.
#[cfg_attr(not(feature = "cuda"), allow(dead_code))]
pub(crate) fn host_observation_fallback<T, E>(
    required: bool,
    fallback: impl FnOnce() -> Result<T, E>,
) -> Result<Option<T>, E> {
    if required {
        fallback().map(Some)
    } else {
        Ok(None)
    }
}

#[cfg(any(feature = "cuda", test))]
fn decimal_product(factors: &[u128]) -> String {
    const BASE: u128 = 1_000_000_000;
    let mut digits = vec![1_u32];
    for factor in factors {
        let mut carry = 0_u128;
        for digit in &mut digits {
            let value = u128::from(*digit) * *factor + carry;
            *digit = (value % BASE) as u32;
            carry = value / BASE;
        }
        while carry != 0 {
            digits.push((carry % BASE) as u32);
            carry /= BASE;
        }
    }
    let mut output = digits.pop().unwrap_or(0).to_string();
    while let Some(digit) = digits.pop() {
        write!(&mut output, "{digit:09}").unwrap();
    }
    output
}

#[cfg(any(feature = "cuda", test))]
pub(crate) fn grouped_observation_layout(
    view: &GeneratedGroupedObservation,
    row_counts: &[u64],
    band_extrema: &[i64],
) -> Result<GroupedObservationLayout, CudaError> {
    let rows = *row_counts.get(view.table).ok_or_else(|| {
        CudaError::InvalidInput(format!(
            "grouped observation '{}' in box '{}' has no source row count",
            view.name, view.box_name
        ))
    })?;
    if rows == 0 {
        return Ok(GroupedObservationLayout {
            axes: view
                .axes
                .iter()
                .map(|_| GroupedObservationAxisLayout {
                    minimum: 0,
                    cardinality: 0,
                })
                .collect(),
            key_space_size: 0,
        });
    }

    let mut bounds = Vec::with_capacity(view.axes.len());
    let mut factors = Vec::with_capacity(view.axes.len());
    for axis in &view.axes {
        let (minimum, cardinality) = match *axis {
            GroupedObservationAxis::Enum { cardinality, .. } => (0_i64, u128::from(cardinality)),
            GroupedObservationAxis::Ref { target_table, .. } => {
                let cardinality = *row_counts.get(target_table).ok_or_else(|| {
                    CudaError::InvalidInput(format!(
                        "grouped observation '{}' in box '{}' has no Ref target row count",
                        view.name, view.box_name
                    ))
                })?;
                (0_i64, u128::from(cardinality))
            }
            GroupedObservationAxis::BandedInt {
                width,
                extrema_index,
                ..
            } => {
                let offset = extrema_index.checked_mul(2).ok_or_else(|| {
                    CudaError::InvalidInput("grouped extrema index overflow".to_owned())
                })?;
                let minimum = *band_extrema.get(offset).ok_or_else(|| {
                    CudaError::InvalidInput(format!(
                        "grouped observation '{}' in box '{}' is missing a band minimum",
                        view.name, view.box_name
                    ))
                })?;
                let maximum = *band_extrema.get(offset + 1).ok_or_else(|| {
                    CudaError::InvalidInput(format!(
                        "grouped observation '{}' in box '{}' is missing a band maximum",
                        view.name, view.box_name
                    ))
                })?;
                let width = i128::from(width);
                let minimum = i128::from(minimum).div_euclid(width);
                let maximum = i128::from(maximum).div_euclid(width);
                let cardinality = u128::try_from(maximum - minimum + 1).map_err(|_| {
                    CudaError::InvalidInput(format!(
                        "grouped observation '{}' in box '{}' has invalid band bounds",
                        view.name, view.box_name
                    ))
                })?;
                (
                    i64::try_from(minimum).map_err(|_| {
                        CudaError::InvalidInput("grouped band minimum exceeds i64".to_owned())
                    })?,
                    cardinality,
                )
            }
        };
        bounds.push(minimum);
        factors.push(cardinality);
    }

    let limit = GROUPED_OBSERVATION_KEY_SPACE_LIMIT;
    let mut key_space_size = 1_usize;
    let exceeds_limit = if factors.contains(&0) {
        key_space_size = 0;
        false
    } else {
        factors.iter().any(|factor| {
            usize::try_from(*factor).map_or(true, |factor| {
                key_space_size
                    .checked_mul(factor)
                    .filter(|size| *size <= limit)
                    .map_or(true, |size| {
                        key_space_size = size;
                        false
                    })
            })
        })
    };
    if exceeds_limit {
        return Err(CudaError::InvalidInput(format!(
            "grouped observation key space exceeds limit: box='{}' view='{}' computed_size={} limit={limit}",
            view.box_name,
            view.name,
            decimal_product(&factors)
        )));
    }

    let axes = bounds
        .into_iter()
        .zip(factors)
        .map(|(minimum, cardinality)| {
            Ok(GroupedObservationAxisLayout {
                minimum,
                cardinality: u64::try_from(cardinality).map_err(|_| {
                    CudaError::InvalidInput("grouped axis cardinality exceeds u64".to_owned())
                })?,
            })
        })
        .collect::<Result<Vec<_>, CudaError>>()?;
    Ok(GroupedObservationLayout {
        axes,
        key_space_size,
    })
}

#[cfg(any(feature = "cuda", test))]
pub(crate) fn decode_grouped_histogram(
    view: &GeneratedGroupedObservation,
    layout: &GroupedObservationLayout,
    counters: &[u64],
) -> Result<Vec<GroupedViewValue>, CudaError> {
    if counters.len() != layout.key_space_size {
        return Err(CudaError::InvalidInput(format!(
            "grouped observation '{}' in box '{}' returned {} counters for key space {}",
            view.name,
            view.box_name,
            counters.len(),
            layout.key_space_size
        )));
    }
    let mut output = Vec::new();
    for (flat_index, count) in counters.iter().copied().enumerate() {
        if count == 0 {
            continue;
        }
        let mut remainder = flat_index as u64;
        let mut keys = vec![0_i128; layout.axes.len()];
        for (index, axis) in layout.axes.iter().enumerate().rev() {
            let coordinate = remainder % axis.cardinality;
            remainder /= axis.cardinality;
            keys[index] = i128::from(axis.minimum) + i128::from(coordinate);
        }
        output.push(GroupedViewValue {
            box_name: view.box_name.clone(),
            name: view.name.clone(),
            keys,
            count: usize::try_from(count).map_err(|_| {
                CudaError::InvalidInput(format!(
                    "grouped observation '{}' count exceeds host usize",
                    view.name
                ))
            })?,
        });
    }
    Ok(output)
}

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
    /// Run-wide IR decision used by the backend to gate host state download.
    pub observation_eligibility: DeviceObservationEligibility,
    /// Global table supplying rows for each declaration-ordered scalar view.
    pub observation_view_tables: Vec<usize>,
    /// Declaration-ordered grouped views and their runtime-bound key axes.
    pub(crate) grouped_observation_views: Vec<GeneratedGroupedObservation>,
    /// Number of banded Int axes whose extrema are reduced each tick.
    pub(crate) grouped_observation_band_axes: usize,
    /// Hidden enum counts needed by the legacy CSV when grouped views are the
    /// only declared observations.
    pub(crate) generic_enum_observations: Vec<GeneratedEnumObservation>,
    pub(crate) generic_enum_count: usize,
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

#[derive(Clone)]
struct ObservationSpec {
    box_index: usize,
    table_index: usize,
    reduce: ViewReduce,
    filter: Option<Expr>,
    value: Option<Expr>,
}

#[derive(Clone)]
enum GroupedObservationKeySpec {
    Enum {
        attr_index: usize,
        cardinality: u64,
    },
    Ref {
        attr_index: usize,
        target_table_index: usize,
    },
    BandedInt {
        attr_index: usize,
        width: u64,
        extrema_index: usize,
    },
}

#[derive(Clone)]
struct GroupedObservationSpec {
    box_index: usize,
    box_name: String,
    name: String,
    table_index: usize,
    filter: Option<Expr>,
    keys: Vec<GroupedObservationKeySpec>,
}

#[derive(Clone)]
struct EnumObservationSpec {
    box_index: usize,
    table_index: usize,
    attr_index: usize,
    offset: usize,
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
    observation_eligibility: DeviceObservationEligibility,
    observation_views: Vec<ObservationSpec>,
    grouped_observation_views: Vec<GroupedObservationSpec>,
    grouped_observation_band_axes: usize,
    generic_enum_observations: Vec<EnumObservationSpec>,
    generic_enum_count: usize,
    next_validation_scan: Cell<u64>,
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
        let observation_eligibility = device_observation_eligibility(model);
        let mut observation_views = Vec::new();
        let mut grouped_observation_views = Vec::new();
        let mut grouped_observation_band_axes = 0;
        let mut generic_enum_observations = Vec::new();
        let mut generic_enum_count = 0_usize;
        if observation_eligibility.eligible {
            for (box_index, model_box) in model.model().boxes.iter().enumerate() {
                for view in &model_box.views {
                    let table_index = model_box
                        .tables
                        .iter()
                        .position(|table| table.name == view.table)
                        .expect("validated observation table is indexed");
                    observation_views.push(ObservationSpec {
                        box_index,
                        table_index,
                        reduce: view.reduce,
                        filter: view.filter.clone(),
                        value: view.value.clone(),
                    });
                }
                for view in &model_box.grouped_views {
                    let table_index = model_box
                        .tables
                        .iter()
                        .position(|table| table.name == view.table)
                        .expect("validated grouped observation table is indexed");
                    let table = &model_box.tables[table_index];
                    let keys = view
                        .keys
                        .iter()
                        .map(|key| {
                            let attr_index = table
                                .attrs
                                .iter()
                                .position(|attr| attr.name == key.attr)
                                .expect("validated grouped observation key is indexed");
                            match (&table.attrs[attr_index].ty, key.band_width) {
                                (AttrType::Enum { variants }, None) => {
                                    GroupedObservationKeySpec::Enum {
                                        attr_index,
                                        cardinality: u64::try_from(variants.len())
                                            .expect("enum cardinality exceeds u64"),
                                    }
                                }
                                (AttrType::Ref { table }, None) => {
                                    let target_table_index = model_box
                                        .tables
                                        .iter()
                                        .position(|candidate| candidate.name == *table)
                                        .expect("validated grouped Ref target is indexed");
                                    GroupedObservationKeySpec::Ref {
                                        attr_index,
                                        target_table_index,
                                    }
                                }
                                (AttrType::Int, Some(width)) => {
                                    let extrema_index = grouped_observation_band_axes;
                                    grouped_observation_band_axes += 1;
                                    GroupedObservationKeySpec::BandedInt {
                                        attr_index,
                                        width,
                                        extrema_index,
                                    }
                                }
                                _ => unreachable!("validated grouped key kind disappeared"),
                            }
                        })
                        .collect();
                    grouped_observation_views.push(GroupedObservationSpec {
                        box_index,
                        box_name: model_box.name.clone(),
                        name: view.name.clone(),
                        table_index,
                        filter: view.filter.as_deref().cloned(),
                        keys,
                    });
                }
            }
            if observation_views.is_empty() && !grouped_observation_views.is_empty() {
                for (box_index, model_box) in model.model().boxes.iter().enumerate() {
                    for (table_index, table) in model_box.tables.iter().enumerate() {
                        for (attr_index, attr) in table.attrs.iter().enumerate() {
                            let AttrType::Enum { variants } = &attr.ty else {
                                continue;
                            };
                            let cardinality = variants.len();
                            let offset = generic_enum_count;
                            generic_enum_count = generic_enum_count
                                .checked_add(cardinality)
                                .ok_or_else(|| codegen("generic enum observation size overflow"))?;
                            generic_enum_observations.push(EnumObservationSpec {
                                box_index,
                                table_index,
                                attr_index,
                                offset,
                            });
                        }
                    }
                }
            }
        }
        let mut this = Self {
            model,
            global_tables,
            columns,
            ports,
            input_fields,
            params,
            aggs: Vec::new(),
            inputs: Vec::new(),
            observation_eligibility,
            observation_views,
            grouped_observation_views,
            grouped_observation_band_axes,
            generic_enum_observations,
            generic_enum_count,
            next_validation_scan: Cell::new(0),
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

    /// Allocates the deterministic scan ordinal for one emitted row-major
    /// validation scan. Ordinals increase in emission order, which matches
    /// the CPU evaluator's recursive validation order, so the parallel
    /// reduction's minimum reproduces the serial validator's first failure.
    fn validation_scan(&self) -> u64 {
        let scan = self.next_validation_scan.get();
        self.next_validation_scan.set(scan + 1);
        scan
    }

    fn emit_scalar_validation_failure(
        &self,
        out: &mut String,
        target: ValidationTarget<'_>,
        scan: u64,
        branch: u64,
        control: &str,
    ) {
        match target {
            ValidationTarget::AggregateFact => {
                out.push_str("      aggregate_errors[0] = 2U; return;\n");
            }
            ValidationTarget::Status { code, identity } => {
                writeln!(out, "      sembla_record_validation_failure(status, {code}ULL, (unsigned long long)({identity}), {scan}ULL, {branch}ULL);{control}").unwrap();
            }
        }
    }

    /// Opens a per-row validation loop. Aggregate-fact construction still
    /// runs on one worker and keeps the serial loop; the four parallel
    /// validation kernels grid-stride so every launch geometry covers the
    /// same rows exactly once.
    fn emit_validation_loop_open(
        &self,
        out: &mut String,
        row_count: &str,
        target: ValidationTarget<'_>,
    ) {
        match target {
            ValidationTarget::AggregateFact => {
                writeln!(
                    out,
                    "    for (unsigned long long row = 0; row < {row_count}; ++row) {{"
                )
                .unwrap();
            }
            ValidationTarget::Status { .. } => {
                writeln!(out, "    for (unsigned long long row = validation_worker; row < {row_count}; row += (unsigned long long)gridDim.x * blockDim.x) {{").unwrap();
            }
        }
    }

    fn emit_aggregate_validation_failure(
        &self,
        out: &mut String,
        target: ValidationTarget<'_>,
        aggregate_index: usize,
        scan: u64,
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
                writeln!(out, "      sembla_record_validation_failure(status, (unsigned long long)aggregate_facts[{aggregate_index}], {aggregate_index}ULL, {scan}ULL, 0ULL);").unwrap();
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
                    // One row-major scan covers the left, right, and checked
                    // op branches: the reduction orders by candidate before
                    // branch, matching the serial loop's per-row branch order.
                    let scan = self.validation_scan();
                    let in_row_loop = matches!(target, ValidationTarget::Status { .. });
                    let control = if in_row_loop { " continue;" } else { "" };
                    self.emit_validation_loop_open(out, row_count, target);
                    writeln!(out, "      local_error = 0U; long long validation_left = (long long)({left}); if (local_error) {{").unwrap();
                    self.emit_scalar_validation_failure(out, target, scan, 0, control);
                    out.push_str("      }\n");
                    writeln!(out, "      local_error = 0U; long long validation_right = (long long)({right}); if (local_error) {{").unwrap();
                    self.emit_scalar_validation_failure(out, target, scan, 1, control);
                    out.push_str("      }\n      local_error = 0U; (void)");
                    writeln!(
                        out,
                        "{helper}(validation_left, validation_right, error); if (local_error) {{"
                    )
                    .unwrap();
                    self.emit_scalar_validation_failure(out, target, scan, 2, control);
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
                match target {
                    ValidationTarget::AggregateFact => {
                        writeln!(out, "    {{ unsigned long long row = 0ULL; local_error = 0U; (void)sembla_input_{index}(inputs, input_offsets, input_counts, params, error); if (local_error) {{").unwrap();
                        self.emit_scalar_validation_failure(out, target, 0, 0, "");
                        out.push_str("      }\n    }\n");
                    }
                    ValidationTarget::Status { .. } => {
                        let scan = self.validation_scan();
                        writeln!(out, "    if (validation_worker == 0ULL) {{ unsigned long long row = 0ULL; local_error = 0U; (void)sembla_input_{index}(inputs, input_offsets, input_counts, params, error); if (local_error) {{").unwrap();
                        self.emit_scalar_validation_failure(out, target, scan, 0, "");
                        out.push_str("      }\n    }\n");
                    }
                }
            }
            Expr::Agg { .. } => {
                let (box_index, table_index) = rows_state(rows)?;
                let key = format!("{box_index}:{table_index}:{expr:?}");
                let index = self
                    .aggs
                    .iter()
                    .position(|entry| entry.key == key)
                    .ok_or_else(|| codegen("group aggregate was not collected"))?;
                match target {
                    ValidationTarget::AggregateFact => {
                        writeln!(out, "    if (aggregate_facts[{index}] != 0U) {{").unwrap();
                        self.emit_aggregate_validation_failure(out, target, index, 0)?;
                        out.push_str("    }\n");
                    }
                    ValidationTarget::Status { .. } => {
                        let scan = self.validation_scan();
                        writeln!(out, "    if (validation_worker == 0ULL && aggregate_facts[{index}] != 0U) {{").unwrap();
                        self.emit_aggregate_validation_failure(out, target, index, scan)?;
                        out.push_str("    }\n");
                    }
                }
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
        self.emit_validation_status_kernels(&mut out);
        self.emit_resolve_kernel(&mut out)?;
        self.emit_apply_kernel(&mut out)?;
        self.emit_output_kernel(&mut out)?;
        self.emit_observation_kernel(&mut out)?;
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
        let observation_view_tables = self
            .observation_views
            .iter()
            .map(|view| self.global_table(view.box_index, view.table_index))
            .collect();
        let grouped_observation_views = self
            .grouped_observation_views
            .iter()
            .map(|view| GeneratedGroupedObservation {
                box_name: view.box_name.clone(),
                name: view.name.clone(),
                table: self.global_table(view.box_index, view.table_index),
                axes: view
                    .keys
                    .iter()
                    .map(|key| match *key {
                        GroupedObservationKeySpec::Enum {
                            attr_index,
                            cardinality,
                        } => GroupedObservationAxis::Enum {
                            column: self.column(view.box_index, view.table_index, attr_index),
                            cardinality,
                        },
                        GroupedObservationKeySpec::Ref {
                            attr_index,
                            target_table_index,
                        } => GroupedObservationAxis::Ref {
                            column: self.column(view.box_index, view.table_index, attr_index),
                            target_table: self.global_table(view.box_index, target_table_index),
                        },
                        GroupedObservationKeySpec::BandedInt {
                            attr_index,
                            width,
                            extrema_index,
                        } => GroupedObservationAxis::BandedInt {
                            column: self.column(view.box_index, view.table_index, attr_index),
                            width,
                            extrema_index,
                        },
                    })
                    .collect(),
            })
            .collect();
        let generic_enum_observations = self
            .generic_enum_observations
            .iter()
            .map(|observation| GeneratedEnumObservation {
                table: self.global_table(observation.box_index, observation.table_index),
            })
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
            observation_eligibility: self.observation_eligibility,
            observation_view_tables,
            grouped_observation_views,
            grouped_observation_band_axes: self.grouped_observation_band_axes,
            generic_enum_observations,
            generic_enum_count: self.generic_enum_count,
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

    /// Emits only the commutative-monoid observation fragment: filtered
    /// counts and filtered Int min/max. Threads first reduce into one shared
    /// scalar per block, so each view performs only one global atomic per block.
    fn emit_observation_kernel(&self, out: &mut String) -> Result<(), CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_init_observations(long long* values, unsigned int count) {\n  unsigned int view = blockIdx.x * blockDim.x + threadIdx.x;\n  if (view >= count) return;\n");
        for (index, spec) in self.observation_views.iter().enumerate() {
            let identity = match spec.reduce {
                ViewReduce::Count => "0LL",
                ViewReduce::Min => "0x7fffffffffffffffLL",
                ViewReduce::Max => "(-0x7fffffffffffffffLL - 1LL)",
                ViewReduce::Sum => unreachable!("Sum is not device-observation eligible"),
            };
            writeln!(out, "  if (view == {index}U) values[{index}] = {identity};").unwrap();
        }
        out.push_str("}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_observe_view(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* params, long long* values, unsigned int view_index) {\n  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  extern __shared__ long long partials[];\n  unsigned char local_error = 0U; unsigned char* error = &local_error;\n");
        for (index, spec) in self.observation_views.iter().enumerate() {
            let rows = Rows::State {
                box_index: spec.box_index,
                table_index: spec.table_index,
            };
            let table = self.global_table(spec.box_index, spec.table_index);
            let selected = match &spec.filter {
                Some(filter) => {
                    self.render(filter, rows, Some(&Ty::Bool), "state", "row")?
                        .0
                }
                None => "1".to_owned(),
            };
            let identity = match spec.reduce {
                ViewReduce::Count => "0LL",
                ViewReduce::Min => "0x7fffffffffffffffLL",
                ViewReduce::Max => "(-0x7fffffffffffffffLL - 1LL)",
                ViewReduce::Sum => unreachable!("Sum is not device-observation eligible"),
            };
            writeln!(
                out,
                "  if (view_index == {index}U) {{\n    long long local = {identity};"
            )
            .unwrap();
            writeln!(out, "    for (unsigned long long row = worker; row < row_counts[{table}]; row += (unsigned long long)gridDim.x * blockDim.x) {{\n      int selected = {selected};").unwrap();
            match spec.reduce {
                ViewReduce::Count => out.push_str("      if (selected) local += 1LL;\n"),
                ViewReduce::Min | ViewReduce::Max => {
                    let value = self
                        .render(
                            spec.value
                                .as_ref()
                                .expect("eligible min/max observation has a value"),
                            rows,
                            Some(&Ty::Int),
                            "state",
                            "row",
                        )?
                        .0;
                    let comparison = if spec.reduce == ViewReduce::Min {
                        "<"
                    } else {
                        ">"
                    };
                    writeln!(out, "      if (selected) {{ long long value = (long long)({value}); if (value {comparison} local) local = value; }}").unwrap();
                }
                ViewReduce::Sum => unreachable!("Sum is not device-observation eligible"),
            }
            out.push_str("    }\n    partials[threadIdx.x] = local;\n    __syncthreads();\n    for (unsigned int stride = (blockDim.x + 1U) / 2U; stride != 0U; stride = (stride + 1U) / 2U) {\n      if (threadIdx.x < stride && threadIdx.x + stride < blockDim.x) {\n");
            match spec.reduce {
                ViewReduce::Count => out.push_str("        partials[threadIdx.x] += partials[threadIdx.x + stride];\n"),
                ViewReduce::Min => out.push_str("        if (partials[threadIdx.x + stride] < partials[threadIdx.x]) partials[threadIdx.x] = partials[threadIdx.x + stride];\n"),
                ViewReduce::Max => out.push_str("        if (partials[threadIdx.x + stride] > partials[threadIdx.x]) partials[threadIdx.x] = partials[threadIdx.x + stride];\n"),
                ViewReduce::Sum => unreachable!("Sum is not device-observation eligible"),
            }
            out.push_str("      }\n      __syncthreads();\n      if (stride == 1U) break;\n    }\n    if (threadIdx.x == 0U) {\n");
            match spec.reduce {
                ViewReduce::Count => writeln!(out, "      atomicAdd((unsigned long long*)(values + {index}), (unsigned long long)partials[0]);").unwrap(),
                ViewReduce::Min => writeln!(out, "      sembla_atomic_min_i64(values + {index}, partials[0]);").unwrap(),
                ViewReduce::Max => writeln!(out, "      sembla_atomic_max_i64(values + {index}, partials[0]);").unwrap(),
                ViewReduce::Sum => unreachable!("Sum is not device-observation eligible"),
            }
            out.push_str("    }\n  }\n");
        }
        out.push_str("}\n");

        out.push_str("\nextern \"C\" __global__ void sembla_init_grouped_extrema(long long* extrema, unsigned int count) {\n  unsigned int axis = blockIdx.x * blockDim.x + threadIdx.x;\n  if (axis >= count) return;\n  extrema[(unsigned long long)axis * 2ULL] = 0x7fffffffffffffffLL;\n  extrema[(unsigned long long)axis * 2ULL + 1ULL] = (-0x7fffffffffffffffLL - 1LL);\n}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_bound_grouped_view(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, long long* extrema, unsigned int view_index) {\n  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n");
        for (view_index, view) in self.grouped_observation_views.iter().enumerate() {
            let rows = Rows::State {
                box_index: view.box_index,
                table_index: view.table_index,
            };
            let table = self.global_table(view.box_index, view.table_index);
            writeln!(out, "  if (view_index == {view_index}U) {{").unwrap();
            writeln!(out, "    for (unsigned long long row = worker; row < row_counts[{table}]; row += (unsigned long long)gridDim.x * blockDim.x) {{").unwrap();
            for key in &view.keys {
                let GroupedObservationKeySpec::BandedInt {
                    attr_index,
                    extrema_index,
                    ..
                } = *key
                else {
                    continue;
                };
                let attr = &self.model.model().boxes[view.box_index].tables[view.table_index].attrs
                    [attr_index];
                let value = self.render_attr(rows, &attr.name, "state", "row")?.0;
                writeln!(out, "      {{ long long value = (long long)({value}); sembla_atomic_min_i64(extrema + {extrema_index}ULL * 2ULL, value); sembla_atomic_max_i64(extrema + {extrema_index}ULL * 2ULL + 1ULL, value); }}").unwrap();
            }
            out.push_str("    }\n    return;\n  }\n");
        }
        out.push_str("}\n");

        out.push_str("\nextern \"C\" __global__ void sembla_init_grouped_histogram(unsigned long long* counts, unsigned long long count) {\n  unsigned long long index = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  if (index < count) counts[index] = 0ULL;\n}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_observe_grouped_view(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* params, const long long* axis_mins, const unsigned long long* axis_cardinalities, unsigned long long* counts, unsigned int view_index) {\n  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned char local_error = 0U; unsigned char* error = &local_error;\n");
        let mut axis_offset = 0_usize;
        for (view_index, view) in self.grouped_observation_views.iter().enumerate() {
            let rows = Rows::State {
                box_index: view.box_index,
                table_index: view.table_index,
            };
            let table = self.global_table(view.box_index, view.table_index);
            let selected = match &view.filter {
                Some(filter) => {
                    self.render(filter, rows, Some(&Ty::Bool), "state", "row")?
                        .0
                }
                None => "1".to_owned(),
            };
            writeln!(out, "  if (view_index == {view_index}U) {{").unwrap();
            writeln!(out, "    for (unsigned long long row = worker; row < row_counts[{table}]; row += (unsigned long long)gridDim.x * blockDim.x) {{\n      int selected = {selected};\n      if (selected) {{ unsigned long long group = 0ULL;").unwrap();
            for (key_index, key) in view.keys.iter().enumerate() {
                let global_axis = axis_offset + key_index;
                let (attr_index, value) = match *key {
                    GroupedObservationKeySpec::Enum { attr_index, .. }
                    | GroupedObservationKeySpec::Ref { attr_index, .. } => {
                        let attr = &self.model.model().boxes[view.box_index].tables
                            [view.table_index]
                            .attrs[attr_index];
                        (
                            attr_index,
                            format!(
                                "(long long)({})",
                                self.render_attr(rows, &attr.name, "state", "row")?.0
                            ),
                        )
                    }
                    GroupedObservationKeySpec::BandedInt {
                        attr_index, width, ..
                    } => {
                        let attr = &self.model.model().boxes[view.box_index].tables
                            [view.table_index]
                            .attrs[attr_index];
                        (
                            attr_index,
                            format!(
                                "sembla_div_euclid_i64_u64((long long)({}), {width}ULL)",
                                self.render_attr(rows, &attr.name, "state", "row")?.0
                            ),
                        )
                    }
                };
                let _ = attr_index;
                writeln!(out, "        {{ long long key = {value}; unsigned long long coordinate = (unsigned long long)(key - axis_mins[{global_axis}]); group = group * axis_cardinalities[{global_axis}] + coordinate; }}").unwrap();
            }
            out.push_str(
                "        atomicAdd(counts + group, 1ULL);\n      }\n    }\n    return;\n  }\n",
            );
            axis_offset += view.keys.len();
        }
        out.push_str("}\n");

        out.push_str("\nextern \"C\" __global__ void sembla_init_generic_enum_counts(unsigned long long* counts, unsigned long long count) {\n  unsigned long long index = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  if (index < count) counts[index] = 0ULL;\n}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_observe_generic_enum(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, unsigned long long* counts, unsigned int observation_index) {\n  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n");
        for (observation_index, observation) in self.generic_enum_observations.iter().enumerate() {
            let rows = Rows::State {
                box_index: observation.box_index,
                table_index: observation.table_index,
            };
            let table = self.global_table(observation.box_index, observation.table_index);
            let attr = &self.model.model().boxes[observation.box_index].tables
                [observation.table_index]
                .attrs[observation.attr_index];
            let value = self.render_attr(rows, &attr.name, "state", "row")?.0;
            writeln!(out, "  if (observation_index == {observation_index}U) {{\n    for (unsigned long long row = worker; row < row_counts[{table}]; row += (unsigned long long)gridDim.x * blockDim.x) {{ unsigned long long value = (unsigned long long)({value}); atomicAdd(counts + {}ULL + value, 1ULL); }}\n    return;\n  }}", observation.offset).unwrap();
        }
        out.push_str("}\n");

        // Control diagnostics are reduced after the simulation has finished
        // writing wins/deferred. Each block contributes one integer partial;
        // scheduling cannot affect the exact result.
        out.push_str("\nextern \"C\" __global__ void sembla_init_control_counts(unsigned long long* fired_counts, unsigned long long rule_count, unsigned long long* deferred_counts, unsigned long long table_count) {\n  unsigned long long index = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned long long stride = (unsigned long long)gridDim.x * blockDim.x;\n  for (unsigned long long rule = index; rule < rule_count; rule += stride) fired_counts[rule] = 0ULL;\n  for (unsigned long long table = index; table < table_count; table += stride) deferred_counts[table] = 0ULL;\n}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_count_fired(const unsigned char* wins, const unsigned long long* candidate_offsets, unsigned long long candidate_count, unsigned long long rule_count, unsigned long long rule, unsigned long long* fired_counts) {\n  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned long long begin = candidate_offsets[rule];\n  unsigned long long end = rule + 1ULL < rule_count ? candidate_offsets[rule + 1ULL] : candidate_count;\n  unsigned long long local = 0ULL;\n  for (unsigned long long candidate = begin + worker; candidate < end; candidate += (unsigned long long)gridDim.x * blockDim.x) local += wins[candidate] != 0U;\n  extern __shared__ unsigned long long fired_partials[];\n  fired_partials[threadIdx.x] = local;\n  __syncthreads();\n  for (unsigned int stride = blockDim.x / 2U; stride != 0U; stride /= 2U) {\n    if (threadIdx.x < stride) fired_partials[threadIdx.x] += fired_partials[threadIdx.x + stride];\n    __syncthreads();\n  }\n  if (threadIdx.x == 0U) atomicAdd(fired_counts + rule, fired_partials[0]);\n}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_count_deferred(const unsigned char* deferred, unsigned long long candidate_count, unsigned long long table_count, unsigned long long table, unsigned long long* deferred_counts) {\n  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned long long local = 0ULL;\n  for (unsigned long long candidate = worker; candidate < candidate_count; candidate += (unsigned long long)gridDim.x * blockDim.x) local += deferred[candidate * table_count + table] != 0U;\n  extern __shared__ unsigned long long deferred_partials[];\n  deferred_partials[threadIdx.x] = local;\n  __syncthreads();\n  for (unsigned int stride = blockDim.x / 2U; stride != 0U; stride /= 2U) {\n    if (threadIdx.x < stride) deferred_partials[threadIdx.x] += deferred_partials[threadIdx.x + stride];\n    __syncthreads();\n  }\n  if (threadIdx.x == 0U) atomicAdd(deferred_counts + table, deferred_partials[0]);\n}\n");
        Ok(())
    }

    fn emit_transition_kernels(&self, out: &mut String) -> Result<Vec<String>, CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_validate_transition(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned char* aggregate_facts, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, unsigned int rule_id, unsigned long long* status) {\n  if (status[0] != 0ULL) return;\n  unsigned long long validation_worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned char local_error = 0U; unsigned char* error = &local_error;\n");
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
            writeln!(out, "  int guard = {guard};\n  errors[candidate * 2ULL] = local_error;\n  local_error = 0;\n  double lambda = (double)({hazard});\n  errors[candidate * 2ULL + 1ULL] = local_error;\n  double time = sembla_exp(seed, tick, {}U, (unsigned int)row, 0U, lambda);\n  times[candidate] = time;\n  enabled[candidate] = (unsigned char)(errors[candidate * 2ULL] == 0U && errors[candidate * 2ULL + 1ULL] == 0U && guard && lambda > 0.0 && time < dt);\n}}", validated.rule_word).unwrap();
        }
        Ok(names)
    }

    /// Emits the four status-protocol helper kernels used by the parallel
    /// validation kernels. Stream-ordered kernel boundaries separate the four
    /// lock-free reduction passes, so results are independent of launch
    /// geometry without requiring a device-wide critical section.
    fn emit_validation_status_kernels(&self, out: &mut String) {
        // Runs once per tick after sembla_reset_status: prepares the
        // reduction scratch slots and clears the per-rule effect activity
        // flags that sembla_mark_effect_active repopulates each tick.
        out.push_str("\nextern \"C\" __global__ void sembla_init_validation_scratch(unsigned long long* status, unsigned int* effect_active, unsigned long long rule_count) {\n  if (blockIdx.x != 0 || threadIdx.x != 0) return;\n  status[4] = 0ULL;\n  status[5] = 0xffffffffffffffffULL;\n  status[6] = 0xffffffffffffffffULL;\n  status[7] = 0ULL;\n  status[8] = 0xffffffffffffffffULL;\n  status[9] = 0ULL;\n  status[10] = 0ULL;\n  status[11] = 0ULL;\n  for (unsigned long long i = 0; i < rule_count; ++i) effect_active[i] = 0U;\n}\n");
        // Advances from scan to identity to branch to payload recovery. The
        // stream boundary before this single-thread kernel is the global
        // synchronization point between pure atomicMin passes.
        out.push_str("\nextern \"C\" __global__ void sembla_advance_validation_phase(unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0) return;\n  ++status[4];\n}\n");
        // Runs on the stream after the payload-recovery pass. Publishes the
        // winning payload when this logical launch failed, then resets scratch
        // for the next validator. The payload is written before the code; the
        // preceding kernel boundary guarantees that its matching-key writer
        // finished before this single-thread commit reads it.
        out.push_str("\nextern \"C\" __global__ void sembla_commit_validation_status(unsigned long long* status) {\n  if (blockIdx.x != 0 || threadIdx.x != 0) return;\n  if (status[0] == 0ULL && status[5] != 0xffffffffffffffffULL) {\n    status[1] = status[9];\n    status[2] = status[10];\n    status[3] = status[11];\n    __threadfence();\n    status[0] = status[7];\n  }\n  status[4] = 0ULL;\n  status[5] = 0xffffffffffffffffULL;\n  status[6] = 0xffffffffffffffffULL;\n  status[7] = 0ULL;\n  status[8] = 0xffffffffffffffffULL;\n  status[9] = 0ULL;\n  status[10] = 0ULL;\n  status[11] = 0ULL;\n}\n");
        // Parallel OR-reduction of wins over one rule's candidate range into
        // a stable per-rule activity flag. sembla_validate_effects validates
        // a transition's whole column exactly when this flag is set, which
        // reproduces the serial any_winner scan without per-thread rescans.
        out.push_str("\nextern \"C\" __global__ void sembla_mark_effect_active(const unsigned char* wins, unsigned long long candidate_begin, unsigned long long candidate_count, unsigned int rule_id, unsigned int* effect_active) {\n  unsigned long long row = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  if (row >= candidate_count) return;\n  if (wins[candidate_begin + row] != 0U) atomicOr(effect_active + rule_id, 1U);\n}\n");
    }

    fn emit_error_check_kernel(&self, out: &mut String) {
        let guard_scan = self.validation_scan();
        let hazard_scan = self.validation_scan();
        out.push_str("\nextern \"C\" __global__ void sembla_check_candidate_errors(const unsigned char* errors, unsigned long long candidate_begin, unsigned long long candidate_count, unsigned long long* status) {\n  if (status[0] != 0ULL) return;\n  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned long long stride = (unsigned long long)gridDim.x * blockDim.x;\n");
        writeln!(out, "  for (unsigned long long row = worker; row < candidate_count; row += stride) {{ unsigned long long candidate = candidate_begin + row; if (errors[candidate * 2ULL]) sembla_record_validation_failure(status, 3ULL, candidate, {guard_scan}ULL, 0ULL); }}").unwrap();
        writeln!(out, "  for (unsigned long long row = worker; row < candidate_count; row += stride) {{ unsigned long long candidate = candidate_begin + row; if (errors[candidate * 2ULL + 1ULL]) sembla_record_validation_failure(status, 3ULL, candidate, {hazard_scan}ULL, 0ULL); }}\n}}").unwrap();
    }

    fn emit_resolve_kernel(&self, out: &mut String) -> Result<(), CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_validate_claims(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, unsigned int rule_id, const unsigned char* enabled, unsigned long long* status) {\n  if (status[0] != 0ULL) return;\n  unsigned long long validation_worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");
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
                let scan = self.validation_scan();
                writeln!(out, "    for (unsigned long long row = validation_worker; row < row_counts[{table_global}]; row += (unsigned long long)gridDim.x * blockDim.x) {{ unsigned long long candidate = candidate_offsets[{}] + row; local_error = 0; (void)({resource}); if (local_error) {{ sembla_record_validation_failure(status, 10ULL, candidate, {scan}ULL, 0ULL); }} }}", validated.rule_id).unwrap();
                if let ClaimOrdering::Key { expr } = &claim.ordering {
                    let key = self.render(expr, rows, None, "state", "row")?.0;
                    let scan = self.validation_scan();
                    writeln!(out, "    for (unsigned long long row = validation_worker; row < row_counts[{table_global}]; row += (unsigned long long)gridDim.x * blockDim.x) {{ unsigned long long candidate = candidate_offsets[{}] + row; local_error = 0; (void)({key}); if (local_error) {{ sembla_record_validation_failure(status, 10ULL, candidate, {scan}ULL, 0ULL); }} }}", validated.rule_id).unwrap();
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
        // The CPU oracle flattens (candidate, claim) instances, groups them by
        // resource identity, and takes a lexicographic argmin under
        // compare_instances: ordering key, rule_word, entity_id. Materialize
        // that same list at stable (rule, row, claim) indices. A final instance
        // index component orders otherwise identical duplicate claims without
        // changing the winning candidate.
        out.push_str("\nextern \"C\" __global__ void sembla_init_conflict_winners(unsigned long long resource_count, unsigned long long* winner_keys, unsigned int* winner_rules, unsigned int* winner_entities, unsigned long long* winner_instances) {\n  unsigned long long stride = (unsigned long long)gridDim.x * blockDim.x;\n  for (unsigned long long resource = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x; resource < resource_count; resource += stride) {\n    winner_keys[resource] = 0xffffffffffffffffULL;\n    winner_rules[resource] = 0xffffffffU;\n    winner_entities[resource] = 0xffffffffU;\n    winner_instances[resource] = 0xffffffffffffffffULL;\n  }\n}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_build_claim_instances(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, const unsigned long long* claim_instance_offsets, const unsigned long long* resource_offsets, unsigned long long candidate_begin, unsigned long long candidate_count, const unsigned char* enabled, const double* times, unsigned long long* instance_resources, unsigned long long* instance_keys, unsigned int* instance_rules, unsigned int* instance_entities, const unsigned long long* status) {\n  if (status[0] != 0ULL) return;\n  unsigned long long stride = (unsigned long long)gridDim.x * blockDim.x;\n  for (unsigned long long local_candidate = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x; local_candidate < candidate_count; local_candidate += stride) {\n    unsigned long long self_candidate = candidate_begin + local_candidate;\n    unsigned char local_error = 0; unsigned char* error = &local_error;\n");
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
            writeln!(out, "    if (self_candidate >= candidate_offsets[{}] && self_candidate < candidate_offsets[{}] + row_counts[{table_global}]) {{ unsigned long long row = self_candidate - candidate_offsets[{}];", validated.rule_id, validated.rule_id, validated.rule_id).unwrap();
            for (claim_index, claim) in transition.contests.iter().enumerate() {
                let resource_ty = self.infer(&claim.resource, rows, None)?;
                let Ty::Ref(target_name) = resource_ty else {
                    return Err(codegen("claim resource is not Ref"));
                };
                let target_table = self.table_index(validated.box_index, &target_name)?;
                let target_global = self.global_table(validated.box_index, target_table);
                let resource = self
                    .render(
                        &claim.resource,
                        rows,
                        Some(&Ty::Ref(target_name)),
                        "state",
                        "row",
                    )?
                    .0;
                let (key, key_ty) = self.claim_key(claim, rows, "row", "self_candidate")?;
                let order_key = match key_ty {
                    Ty::Real => format!("sembla_f64_order_key({key})"),
                    Ty::Int => format!("sembla_i64_order_key({key})"),
                    Ty::Enum(_) => format!("(unsigned long long)({key})"),
                    _ => return Err(codegen("contest key must be Real, Int, or Enum")),
                };
                writeln!(out, "      {{ unsigned long long instance = claim_instance_offsets[{}] + row * {}ULL + {claim_index}ULL; if (enabled[self_candidate]) {{ instance_resources[instance] = resource_offsets[{target_global}] + (unsigned long long)({resource}); instance_keys[instance] = {order_key}; instance_rules[instance] = {}U; instance_entities[instance] = (unsigned int)row; }} else {{ instance_resources[instance] = 0xffffffffffffffffULL; }} }}", validated.rule_id, transition.contests.len(), validated.rule_word).unwrap();
            }
            out.push_str("    }\n");
        }
        out.push_str("  }\n}\n");

        // Each pass restricts itself to the exact prefix fixed by prior passes.
        // Therefore these atomic minima compute one lexicographic argmin rather
        // than unrelated component-wise minima, independent of arrival order.
        out.push_str("\nextern \"C\" __global__ void sembla_reduce_claim_keys(unsigned long long instance_begin, unsigned long long instance_count, const unsigned long long* instance_resources, const unsigned long long* instance_keys, unsigned long long* winner_keys) {\n  unsigned long long stride = (unsigned long long)gridDim.x * blockDim.x;\n  for (unsigned long long local = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x; local < instance_count; local += stride) { unsigned long long instance = instance_begin + local; unsigned long long resource = instance_resources[instance]; if (resource != 0xffffffffffffffffULL) atomicMin(winner_keys + resource, instance_keys[instance]); }\n}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_reduce_claim_rules(unsigned long long instance_begin, unsigned long long instance_count, const unsigned long long* instance_resources, const unsigned long long* instance_keys, const unsigned int* instance_rules, const unsigned long long* winner_keys, unsigned int* winner_rules) {\n  unsigned long long stride = (unsigned long long)gridDim.x * blockDim.x;\n  for (unsigned long long local = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x; local < instance_count; local += stride) { unsigned long long instance = instance_begin + local; unsigned long long resource = instance_resources[instance]; if (resource != 0xffffffffffffffffULL && instance_keys[instance] == winner_keys[resource]) atomicMin(winner_rules + resource, instance_rules[instance]); }\n}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_reduce_claim_entities(unsigned long long instance_begin, unsigned long long instance_count, const unsigned long long* instance_resources, const unsigned long long* instance_keys, const unsigned int* instance_rules, const unsigned int* instance_entities, const unsigned long long* winner_keys, const unsigned int* winner_rules, unsigned int* winner_entities) {\n  unsigned long long stride = (unsigned long long)gridDim.x * blockDim.x;\n  for (unsigned long long local = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x; local < instance_count; local += stride) { unsigned long long instance = instance_begin + local; unsigned long long resource = instance_resources[instance]; if (resource != 0xffffffffffffffffULL && instance_keys[instance] == winner_keys[resource] && instance_rules[instance] == winner_rules[resource]) atomicMin(winner_entities + resource, instance_entities[instance]); }\n}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_reduce_claim_instances(unsigned long long instance_begin, unsigned long long instance_count, const unsigned long long* instance_resources, const unsigned long long* instance_keys, const unsigned int* instance_rules, const unsigned int* instance_entities, const unsigned long long* winner_keys, const unsigned int* winner_rules, const unsigned int* winner_entities, unsigned long long* winner_instances) {\n  unsigned long long stride = (unsigned long long)gridDim.x * blockDim.x;\n  for (unsigned long long local = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x; local < instance_count; local += stride) { unsigned long long instance = instance_begin + local; unsigned long long resource = instance_resources[instance]; if (resource != 0xffffffffffffffffULL && instance_keys[instance] == winner_keys[resource] && instance_rules[instance] == winner_rules[resource] && instance_entities[instance] == winner_entities[resource]) atomicMin(winner_instances + resource, instance); }\n}\n");

        out.push_str("\nextern \"C\" __global__ void sembla_resolve_conflicts(const unsigned long long* row_counts, const unsigned long long* candidate_offsets, const unsigned long long* claim_instance_offsets, unsigned long long candidate_begin, unsigned long long candidate_count, unsigned long long resource_table_count, const unsigned char* enabled, const unsigned long long* instance_resources, const unsigned int* winner_rules, const unsigned int* winner_entities, unsigned char* wins, unsigned char* deferred, const unsigned long long* status) {\n  if (status[0] != 0ULL) return;\n  unsigned long long stride = (unsigned long long)gridDim.x * blockDim.x;\n  for (unsigned long long local_candidate = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x; local_candidate < candidate_count; local_candidate += stride) {\n    unsigned long long self_candidate = candidate_begin + local_candidate;\n    for (unsigned long long table = 0; table < resource_table_count; ++table) deferred[self_candidate * resource_table_count + table] = 0U;\n    wins[self_candidate] = enabled[self_candidate];\n    if (!enabled[self_candidate]) continue;\n");
        for validated in self.model.transitions() {
            let transition = &self.model.model().boxes[validated.box_index].transitions
                [validated.transition_index];
            if transition.contests.is_empty() {
                continue;
            }
            let table_index = self.table_index(validated.box_index, &transition.table)?;
            let table_global = self.global_table(validated.box_index, table_index);
            writeln!(out, "    if (self_candidate >= candidate_offsets[{}] && self_candidate < candidate_offsets[{}] + row_counts[{table_global}]) {{ unsigned long long row = self_candidate - candidate_offsets[{}];", validated.rule_id, validated.rule_id, validated.rule_id).unwrap();
            for (claim_index, claim) in transition.contests.iter().enumerate() {
                let resource_ty = self.infer(
                    &claim.resource,
                    Rows::State {
                        box_index: validated.box_index,
                        table_index,
                    },
                    None,
                )?;
                let Ty::Ref(target_name) = resource_ty else {
                    return Err(codegen("claim resource is not Ref"));
                };
                let target_table = self.table_index(validated.box_index, &target_name)?;
                let target_global = self.global_table(validated.box_index, target_table);
                writeln!(out, "      {{ unsigned long long instance = claim_instance_offsets[{}] + row * {}ULL + {claim_index}ULL; unsigned long long resource = instance_resources[instance]; if (winner_rules[resource] != {}U || winner_entities[resource] != (unsigned int)row) {{ wins[self_candidate] = 0U; deferred[self_candidate * resource_table_count + {target_global}ULL] = 1U; }} }}", validated.rule_id, transition.contests.len(), validated.rule_word).unwrap();
            }
            out.push_str("    }\n");
        }
        out.push_str("  }\n}\n");
        Ok(())
    }

    fn claim_key(
        &self,
        claim: &sembla_ir::ResourceClaim,
        rows: Rows,
        row: &str,
        candidate: &str,
    ) -> Result<(String, Ty), CudaError> {
        match &claim.ordering {
            ClaimOrdering::RaceTime => Ok((format!("times[{candidate}]"), Ty::Real)),
            ClaimOrdering::Key { expr } => self.render(expr, rows, None, "state", row),
        }
    }

    fn emit_apply_kernel(&self, out: &mut String) -> Result<(), CudaError> {
        out.push_str("\nextern \"C\" __global__ void sembla_validate_effects(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned char* aggregate_facts, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, const unsigned char* wins, const unsigned int* effect_active, unsigned int box_index, unsigned long long* status) {\n  if (status[0] != 0ULL) return;\n  unsigned long long validation_worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned char local_error = 0U; unsigned char* error = &local_error;\n");
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
            // A transition's effects are validated for its whole column only
            // when the transition has a winner. The per-rule activity flag is
            // computed by sembla_mark_effect_active after conflict resolution,
            // so every worker here observes the same stable decision.
            writeln!(
                out,
                "  if (box_index == {}U) {{ if (effect_active[{}] != 0U) {{",
                validated.box_index, validated.rule_id
            )
            .unwrap();
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
                    Ty::Enum(variants) => {
                        let scan = self.validation_scan();
                        writeln!(out, "    for (unsigned long long row = validation_worker; row < row_counts[{global_table}]; row += (unsigned long long)gridDim.x * blockDim.x) {{ local_error = 0U; unsigned long long value = (unsigned long long)({rendered}); if (local_error) {{ sembla_record_validation_failure(status, 5ULL, candidate_offsets[{}] + row, {scan}ULL, 0ULL); continue; }} if (value >= {}ULL) {{ sembla_record_validation_failure(status, 6ULL, candidate_offsets[{}] + row, {scan}ULL, 1ULL); }} }}", validated.rule_id, variants.len(), validated.rule_id).unwrap()
                    }
                    Ty::Ref(target) => {
                        let target_index = self.table_index(validated.box_index, target)?;
                        let target_global = self.global_table(validated.box_index, target_index);
                        let scan = self.validation_scan();
                        writeln!(out, "    for (unsigned long long row = validation_worker; row < row_counts[{global_table}]; row += (unsigned long long)gridDim.x * blockDim.x) {{ local_error = 0U; unsigned long long value = (unsigned long long)({rendered}); if (local_error) {{ sembla_record_validation_failure(status, 5ULL, candidate_offsets[{}] + row, {scan}ULL, 0ULL); continue; }} if (value >= row_counts[{target_global}]) {{ sembla_record_validation_failure(status, 7ULL, candidate_offsets[{}] + row, {scan}ULL, 1ULL); }} }}", validated.rule_id, validated.rule_id).unwrap();
                    }
                    _ => {}
                }
            }
            out.push_str("  } }\n");
        }
        out.push_str("}\n");

        out.push_str("\nextern \"C\" __global__ void sembla_init_effect_owners(int* owners, unsigned long long owner_count) {\n  unsigned long long stride = (unsigned long long)gridDim.x * blockDim.x;\n  for (unsigned long long owner = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x; owner < owner_count; owner += stride) owners[owner] = -1;\n}\n");
        out.push_str("\nextern \"C\" __global__ void sembla_prepare_effects(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned long long* agg_offsets, const unsigned long long* candidate_offsets, const unsigned char* wins, const unsigned long long* write_offsets, int* owners, unsigned long long* owner_values, unsigned int rule_id, unsigned long long* status) {\n  if (status[0] != 0ULL) return;\n  unsigned long long validation_phase = status[4];\n  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned long long stride = (unsigned long long)gridDim.x * blockDim.x;\n  unsigned char local_error = 0; unsigned char* error = &local_error;\n");
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
            let scan = self.validation_scan();
            writeln!(out, "  if (rule_id == {}U) {{", validated.rule_id).unwrap();
            writeln!(out, "    for (unsigned long long row = worker; row < row_counts[{global_table}]; row += stride) {{ unsigned long long candidate = candidate_offsets[{}] + row; if (!wins[candidate]) continue;", validated.rule_id).unwrap();
            for (effect_index, effect) in transition.effects.iter().enumerate() {
                let Effect::SetAttr { attr, value } = effect;
                let attr_index = attr_index(table, attr)?;
                let ty = Ty::from(&table.attrs[attr_index].ty);
                let column = self.column(validated.box_index, table_index, attr_index);
                let rendered = self.render(value, rows, Some(&ty), "state", "row")?.0;
                let branch = u64::try_from(effect_index)
                    .map_err(|_| codegen("effect index exceeds u64"))?
                    .checked_mul(3)
                    .ok_or_else(|| codegen("effect diagnostic branch overflow"))?;
                out.push_str("      {\n");
                writeln!(
                    out,
                    "      local_error = 0U; {} value = ({})({rendered});",
                    ty.cuda(),
                    ty.cuda()
                )
                .unwrap();
                writeln!(out, "      if (local_error) {{ sembla_record_validation_failure(status, 5ULL, candidate, {scan}ULL, {branch}ULL, candidate, 0ULL, 0ULL); }} else {{").unwrap();
                match &ty {
                    Ty::Enum(variants) => writeln!(out, "        if ((unsigned long long)value >= {}ULL) {{ sembla_record_validation_failure(status, 6ULL, candidate, {scan}ULL, {}ULL, candidate, 0ULL, 0ULL); }} else", variants.len(), branch + 1).unwrap(),
                    Ty::Ref(target) => {
                        let target_index = self.table_index(validated.box_index, target)?;
                        let target_global = self.global_table(validated.box_index, target_index);
                        writeln!(out, "        if ((unsigned long long)value >= row_counts[{target_global}]) {{ sembla_record_validation_failure(status, 7ULL, candidate, {scan}ULL, {}ULL, candidate, 0ULL, 0ULL); }} else", branch + 1).unwrap();
                    }
                    _ => out.push_str("       "),
                }
                let repeats_attr = transition.effects[..effect_index].iter().any(|earlier| {
                    matches!(earlier, Effect::SetAttr { attr: earlier_attr, .. } if earlier_attr == attr)
                });
                writeln!(out, "        {{ unsigned long long owner = write_offsets[{column}] + row; if (validation_phase == 0ULL) {{ if (owners[owner] != -1) {{ sembla_record_validation_failure(status, 8ULL, candidate, {scan}ULL, {}ULL, owner, (unsigned long long)owners[owner], {}ULL); }} else {{ owners[owner] = (int){}U;", branch + 2, validated.rule_id, validated.rule_id).unwrap();
                match ty {
                    Ty::Real => out.push_str("          owner_values[owner] = (unsigned long long)__double_as_longlong(value);\n"),
                    _ => out.push_str("          owner_values[owner] = (unsigned long long)value;\n"),
                }
                writeln!(out, "        }} }} else if (owners[owner] != -1 && (owners[owner] != (int){}U || {})) {{ sembla_record_validation_failure(status, 8ULL, candidate, {scan}ULL, {}ULL, owner, (unsigned long long)owners[owner], {}ULL); }} }}", validated.rule_id, u8::from(repeats_attr), branch + 2, validated.rule_id).unwrap();
                out.push_str("      }\n      }\n");
            }
            out.push_str("    }\n    return;\n  }\n");
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
        out.push_str("\nextern \"C\" __global__ void sembla_validate_outputs(const unsigned char* state, const unsigned long long* column_offsets, const unsigned long long* row_counts, const unsigned char* inputs, const unsigned long long* input_offsets, const unsigned long long* input_counts, const unsigned char* params, const unsigned char* aggs, const unsigned char* aggregate_facts, const unsigned long long* agg_offsets, unsigned long long* status) {\n  if (status[0] != 0ULL) return;\n  unsigned long long validation_worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned char local_error = 0U; unsigned char* error = &local_error;\n");
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
                    // Checked addition is order-sensitive, so the ordered
                    // prefix fold stays on one worker (narrow documented
                    // exception to grid-striding every row loop); the
                    // independent per-row checks above run across the device.
                    let scan = self.validation_scan();
                    writeln!(out, "    {{ long long result = 0LL; if (validation_worker == 0ULL) for (unsigned long long row = 0; row < row_counts[{global_table}]; ++row) {{ local_error = 0U; int selected = {selected}; long long value = (long long)({value}); if (local_error) {{ sembla_record_validation_failure(status, 9ULL, {target_field}ULL, {scan}ULL, 0ULL); break; }} if (selected) {{ result = sembla_add_i64(result, value, error); if (local_error) {{ sembla_record_validation_failure(status, 9ULL, {target_field}ULL, {scan}ULL, 1ULL); break; }} }} }} }}").unwrap();
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

#[cfg(test)]
fn cuda_f64_order_key(value: f64) -> u64 {
    let signed_bits = value.to_bits() as i64;
    let total_key = signed_bits ^ ((((signed_bits >> 63) as u64) >> 1) as i64);
    (total_key as u64) ^ 0x8000_0000_0000_0000
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

/// Draw-major mutable buffer slots used by the hidden fused-sweep spike.
/// Values are ABI-visible to generated CUDA through `sembla_batch_strides`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum FusedBuffer {
    State,
    NextState,
    Inputs,
    NextInputs,
    InputCounts,
    NextInputCounts,
    Params,
    Aggregates,
    AggregatePartials,
    AggregateErrors,
    AggregateFacts,
    AggregateActive,
    Enabled,
    Times,
    CandidateErrors,
    Wins,
    Deferred,
    FiredCounts,
    DeferredCounts,
    InstanceResources,
    InstanceKeys,
    InstanceRules,
    InstanceEntities,
    WinnerKeys,
    WinnerRules,
    WinnerEntities,
    WinnerInstances,
    Owners,
    OwnerValues,
    OutputPartials,
    OutputErrors,
    ObservationValues,
    GroupedExtrema,
    GroupedAxisMins,
    GroupedAxisCardinalities,
    GroupedHistogram,
    GenericEnumCounts,
    EffectActive,
    Status,
}

#[cfg_attr(not(feature = "cuda"), allow(dead_code))]
pub(crate) const FUSED_BUFFER_COUNT: usize = FusedBuffer::Status as usize + 1;

fn fused_pointer_buffer(kernel: &str, name: &str) -> Option<FusedBuffer> {
    use FusedBuffer as B;
    Some(match name {
        "state" => B::State,
        "next_state" => B::NextState,
        "inputs" => B::Inputs,
        "next_inputs" => B::NextInputs,
        "input_counts" => B::InputCounts,
        "next_input_counts" => B::NextInputCounts,
        "params" => B::Params,
        "aggs" => B::Aggregates,
        "partials" => B::AggregatePartials,
        "aggregate_errors" => B::AggregateErrors,
        "aggregate_facts" => B::AggregateFacts,
        "aggregate_active" | "active" => B::AggregateActive,
        "enabled" => B::Enabled,
        "times" => B::Times,
        "wins" => B::Wins,
        "deferred" => B::Deferred,
        "fired_counts" => B::FiredCounts,
        "deferred_counts" => B::DeferredCounts,
        "instance_resources" => B::InstanceResources,
        "instance_keys" => B::InstanceKeys,
        "instance_rules" => B::InstanceRules,
        "instance_entities" => B::InstanceEntities,
        "winner_keys" => B::WinnerKeys,
        "winner_rules" => B::WinnerRules,
        "winner_entities" => B::WinnerEntities,
        "winner_instances" => B::WinnerInstances,
        "owners" => B::Owners,
        "owner_values" => B::OwnerValues,
        "output_partials" => B::OutputPartials,
        "output_errors" => B::OutputErrors,
        "extrema" => B::GroupedExtrema,
        "axis_mins" => B::GroupedAxisMins,
        "axis_cardinalities" => B::GroupedAxisCardinalities,
        "generic_enum_counts" => B::GenericEnumCounts,
        "effect_active" => B::EffectActive,
        "status" => B::Status,
        "values" => B::ObservationValues,
        "errors" if kernel == "sembla_record_aggregate_errors" => B::AggregateErrors,
        "errors" if kernel == "sembla_check_candidate_errors" => B::CandidateErrors,
        "errors" if kernel == "sembla_check_output_errors" => B::OutputErrors,
        "errors" => B::CandidateErrors,
        "counts" if kernel.contains("grouped") => B::GroupedHistogram,
        "counts" => B::GenericEnumCounts,
        // Immutable layout metadata and the Philox test-kernel pointers are
        // deliberately shared/unmodified.
        _ => return None,
    })
}

fn fused_batch_source(source: &str) -> Result<String, CudaError> {
    const MARKER: &str = "extern \"C\" __global__ void ";
    let mut out = String::with_capacity(source.len() + source.len() / 8);
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find(MARKER) {
        let start = cursor + relative;
        out.push_str(&source[cursor..start]);
        let name_start = start + MARKER.len();
        let open = source[name_start..]
            .find('(')
            .map(|offset| name_start + offset)
            .ok_or_else(|| codegen("fused kernel signature is missing '('"))?;
        let kernel_name = source[name_start..open].trim();
        let mut depth = 1_usize;
        let mut close = None;
        for (offset, byte) in source.as_bytes()[open + 1..].iter().copied().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(open + 1 + offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close = close.ok_or_else(|| codegen("fused kernel signature is unterminated"))?;
        let body = source[close + 1..]
            .find('{')
            .map(|offset| close + 1 + offset)
            .ok_or_else(|| codegen("fused kernel body is missing '{'"))?;
        if kernel_name == "sembla_philox_vectors" {
            out.push_str(&source[start..=body]);
            cursor = body + 1;
            continue;
        }

        let arguments = &source[open + 1..close];
        out.push_str(&source[start..=open]);
        out.push_str("const unsigned long long* sembla_batch_strides, const unsigned char* sembla_batch_active, const unsigned long long* sembla_batch_seeds");
        if !arguments.trim().is_empty() {
            out.push_str(", ");
            out.push_str(arguments);
        }
        out.push_str(&source[close..=body]);
        out.push_str("\n  const unsigned int sembla_slot = blockIdx.y;\n  if (sembla_batch_active[sembla_slot] == 0U) return;\n");

        for argument in arguments.split(',') {
            let argument = argument.trim();
            if !argument.contains('*') {
                continue;
            }
            let Some(argument_name) = argument
                .split_whitespace()
                .last()
                .map(|name| name.trim_start_matches('*'))
            else {
                continue;
            };
            if let Some(buffer) = fused_pointer_buffer(kernel_name, argument_name) {
                writeln!(
                    out,
                    "  {argument_name} += (unsigned long long)sembla_slot * sembla_batch_strides[{}ULL];",
                    buffer as usize
                )
                .unwrap();
            }
        }
        if arguments.contains("unsigned long long seed") {
            out.push_str("  seed = sembla_batch_seeds[sembla_slot];\n");
        }
        cursor = body + 1;
    }
    out.push_str(&source[cursor..]);
    Ok(out)
}

/// Generates the separate grid-y translation unit used only by the hidden
/// fused sweep spike. Ordinary `generate()` bytes and hashes are untouched.
pub fn generate_fused_batch(model: &ValidatedModel) -> Result<GeneratedCuda, CudaError> {
    let mut generated = Generator::new(model)?.emit()?;
    generated.source = fused_batch_source(&generated.source)?;
    generated.source_sha256 = hex(Sha256::digest(generated.source.as_bytes()).as_slice());
    Ok(generated)
}

const PRELUDE: &str = r#"
// Conflict winners use ordered atomicMin passes over an explicit total key;
// every pass is order-independent and only sees the prefix selected earlier.
// Other simulation results are staged in generated rule/effect/row order, then
// scattered by ascending destination cell. Validation *diagnostics* are reduced
// with atomics under a short lock so the reported failure is independent of
// launch geometry; the committed status is written only by the single-thread
// commit kernel.
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
// Unsigned encodings whose ordinary integer order exactly matches the CPU
// oracle's signed i64 order and Rust f64::total_cmp order. The sign-bit flip
// converts the signed total-order key above into the unsigned domain required
// by atomicMin without changing its ordering, including signed zero and NaNs.
__device__ __forceinline__ unsigned long long sembla_i64_order_key(long long value) {
  return ((unsigned long long)value) ^ 0x8000000000000000ULL;
}
__device__ __forceinline__ void sembla_atomic_min_i64(long long* address, long long value) {
  unsigned long long* bits = (unsigned long long*)address;
  unsigned long long observed = *bits;
  while (value < (long long)observed) {
    unsigned long long prior = atomicCAS(bits, observed, (unsigned long long)value);
    if (prior == observed) return;
    observed = prior;
  }
}
__device__ __forceinline__ void sembla_atomic_max_i64(long long* address, long long value) {
  unsigned long long* bits = (unsigned long long*)address;
  unsigned long long observed = *bits;
  while (value > (long long)observed) {
    unsigned long long prior = atomicCAS(bits, observed, (unsigned long long)value);
    if (prior == observed) return;
    observed = prior;
  }
}
__device__ __forceinline__ long long sembla_div_euclid_i64_u64(long long value, unsigned long long width) {
  if (value >= 0LL) return (long long)((unsigned long long)value / width);
  unsigned long long magnitude_minus_one = (unsigned long long)(-(value + 1LL));
  return -1LL - (long long)(magnitude_minus_one / width);
}
__device__ __forceinline__ unsigned long long sembla_f64_order_key(double value) {
  return ((unsigned long long)sembla_total_key(value)) ^ 0x8000000000000000ULL;
}
// Records one validation failure into scratch slots status[4..=11] without
// touching the committed diagnostic status[0..=3]. Full-width scan, identity,
// and branch components cannot be packed into one 64-bit key without changing
// their order, so the host replays each validation launch in four stream-ordered
// passes. The first three passes mirror conflict resolution's segmented argmin:
// each pure atomicMin considers only failures matching the winning prefix. The
// fourth pass recovers payload only from the exact winning key. Duplicate
// observations of one exact key are the same logical check and carry identical
// payload, so concurrent payload stores cannot create a mixed diagnostic.
// status[4]: phase, status[5]: scan, status[6]: order identity, status[7]: code,
// status[8]: branch, status[9]: reported identity, status[10..=11]: details.
__device__ __forceinline__ void sembla_record_validation_failure(
    unsigned long long* status, unsigned long long code,
    unsigned long long order_identity, unsigned long long scan,
    unsigned long long branch, unsigned long long reported_identity,
    unsigned long long detail_2, unsigned long long detail_3) {
  unsigned long long phase = status[4];
  if (phase == 0ULL) {
    atomicMin(status + 5, scan);
  } else if (phase == 1ULL) {
    if (scan == status[5]) atomicMin(status + 6, order_identity);
  } else if (phase == 2ULL) {
    if (scan == status[5] && order_identity == status[6])
      atomicMin(status + 8, branch);
  } else if (scan == status[5] && order_identity == status[6] &&
             branch == status[8]) {
    status[7] = code;
    status[9] = reported_identity;
    status[10] = detail_2;
    status[11] = detail_3;
  }
}
__device__ __forceinline__ void sembla_record_validation_failure(
    unsigned long long* status, unsigned long long code,
    unsigned long long candidate, unsigned long long scan,
    unsigned long long branch) {
  sembla_record_validation_failure(
      status, code, candidate, scan, branch, candidate, 0ULL, 0ULL);
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

    use sembla_ir::{Expr, ViewReduce};

    use super::{
        cuda_f64_order_key, decode_grouped_histogram, generate, grouped_observation_layout,
        host_observation_fallback, GeneratedGroupedObservation, GroupedObservationAxis,
        GroupedViewValue, DUMP_ENV, GROUPED_OBSERVATION_KEY_SPACE_LIMIT,
    };

    fn example_model(name: &str) -> sembla_ir::ValidatedModel {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../examples/{name}"));
        let source = std::fs::read_to_string(path).unwrap();
        sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
    }

    fn sir_model() -> sembla_ir::ValidatedModel {
        example_model("sir.json")
    }

    fn grouped_model() -> sembla_ir::ValidatedModel {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../sembla-cli/tests/fixtures/grouped_observation.json");
        let source = std::fs::read_to_string(path).unwrap();
        let features =
            sembla_ir::FeatureSet::from([sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
        sembla_ir::validate_with_features(sembla_ir::parse_json(&source).unwrap(), &features)
            .unwrap()
    }

    fn grouped_only_model() -> sembla_ir::ValidatedModel {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../sembla-cli/tests/fixtures/grouped_observation.json");
        let mut model = sembla_ir::parse_json(&std::fs::read_to_string(path).unwrap()).unwrap();
        model.boxes[0].views.clear();
        let features =
            sembla_ir::FeatureSet::from([sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
        sembla_ir::validate_with_features(model, &features).unwrap()
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
                    grouped_views: Vec::new(),
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
                    grouped_views: Vec::new(),
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

    fn stable_contested_model() -> sembla_ir::ValidatedModel {
        use sembla_ir::{
            occurrence_of_leaf, rule_word, transition_identity, ExecutablePlanV1, IdentityMapV1,
            LeafIdentityV1, PlanOrigin, SchedulerDomainV1, TransitionIdentityV1,
            EXECUTABLE_PLAN_SCHEMA, STABLE_IDENTITY_SCHEME,
        };

        let mut model = contested_model().into_model();
        model.boxes[0]
            .tables
            .sort_by(|left, right| left.name.cmp(&right.name));
        let mut second = model.boxes[0].transitions[0].clone();
        second.name = String::from("finish_other");
        model.boxes[0].transitions.push(second);

        let occurrence = occurrence_of_leaf("world");
        let mut transitions = model.boxes[0]
            .transitions
            .iter()
            .map(|transition| {
                let identity = transition_identity(&occurrence, &transition.name);
                TransitionIdentityV1 {
                    r#box: "world".to_owned(),
                    name: transition.name.clone(),
                    rule_word: rule_word(&identity),
                    identity,
                }
            })
            .collect::<Vec<_>>();
        transitions.sort_by(|left, right| left.identity.cmp(&right.identity));
        let plan = ExecutablePlanV1 {
            schema_version: EXECUTABLE_PLAN_SCHEMA.to_owned(),
            identity_scheme: STABLE_IDENTITY_SCHEME.to_owned(),
            origin: PlanOrigin::DirectStable,
            identity: IdentityMapV1 {
                model_id: "model:claims".to_owned(),
                enabled_features: Vec::new(),
                scheduler_domains: vec![SchedulerDomainV1 {
                    id: "domain:global".to_owned(),
                    algorithm: "tau_leap".to_owned(),
                    leaves: vec!["world".to_owned()],
                }],
                leaves: vec![LeafIdentityV1 {
                    r#box: "world".to_owned(),
                    occurrence,
                }],
                transitions,
                mailboxes: Vec::new(),
            },
            model,
            linked_provenance: None,
        };
        sembla_ir::validate_plan(&plan)
            .unwrap()
            .model_with_rule_words()
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

    /// Extracts one emitted kernel body for scoped source assertions.
    /// Generated kernels close with a brace at column 0; every nested brace
    /// is indented.
    fn kernel_body<'a>(source: &'a str, name: &str) -> &'a str {
        let marker = format!("extern \"C\" __global__ void {name}(");
        let start = source
            .find(&marker)
            .unwrap_or_else(|| panic!("kernel {name} missing from emitted source"));
        let rest = &source[start..];
        let end = rest
            .find("\n}\n")
            .map(|index| index + 2)
            .unwrap_or(rest.len());
        &rest[..end]
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
        let observation = kernel_body(&first.source, "sembla_observe_view");
        assert!(observation.contains("extern __shared__ long long partials[]"));
        assert!(observation.contains("atomicAdd"));
        // The argmin uses only order-independent atomic minima in staged
        // prefix passes. Finalization itself is a bounded own-claim lookup.
        let resolver = kernel_body(&first.source, "sembla_resolve_conflicts");
        assert!(!resolver.contains("atomicMin"));
        assert!(!resolver.contains("atomicAdd"));
        assert!(!resolver.contains("other_row"));
    }

    #[test]
    fn host_ineligible_route_executes_download_and_device_route_does_not() {
        let mut downloads = 0;
        let fallback = host_observation_fallback(true, || {
            downloads += 1;
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(fallback, Some(()));
        assert_eq!(downloads, 1);

        let fast = host_observation_fallback(false, || {
            downloads += 1;
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(fast, None);
        assert_eq!(downloads, 1);
    }

    #[test]
    fn device_observation_codegen_is_all_or_nothing() {
        let eligible = generate(&sir_model()).unwrap();
        assert!(eligible.observation_eligibility.eligible);
        assert_eq!(eligible.observation_view_tables.len(), 3);
        let kernel = kernel_body(&eligible.source, "sembla_observe_view");
        assert!(kernel.contains("extern __shared__ long long partials[]"));
        assert!(kernel.contains("if (view_index == 0U)"));

        let ineligible = generate(&example_model("observations.json")).unwrap();
        assert!(!ineligible.observation_eligibility.eligible);
        assert!(ineligible.observation_view_tables.is_empty());
        let kernel = kernel_body(&ineligible.source, "sembla_observe_view");
        assert!(!kernel.contains("if (view_index == 0U)"));

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../sembla-cli/tests/fixtures/grouped_observation.json");
        let mut raw = sembla_ir::parse_json(&std::fs::read_to_string(path).unwrap()).unwrap();
        raw.boxes[0].views[0].reduce = ViewReduce::Sum;
        raw.boxes[0].views[0].value = Some(Expr::SelfAttr {
            name: "age_months".to_owned(),
        });
        let features =
            sembla_ir::FeatureSet::from([sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
        let model = sembla_ir::validate_with_features(raw, &features).unwrap();
        let ineligible_mixed = generate(&model).unwrap();
        assert!(!ineligible_mixed.observation_eligibility.eligible);
        assert!(ineligible_mixed.grouped_observation_views.is_empty());
        let grouped = kernel_body(&ineligible_mixed.source, "sembla_observe_grouped_view");
        assert!(!grouped.contains("if (view_index == 0U)"));
    }

    #[test]
    fn generated_unsigned_band_formula_matches_host_euclidean_division() {
        let cases = [
            (-1_i64, 60_u64),
            (-60, 60),
            (-61, 60),
            (0, 60),
            (61, 60),
            (i64::MIN, 1),
            (i64::MIN, i64::MAX as u64 + 1),
            (i64::MAX, u64::MAX),
        ];
        for (value, width) in cases {
            let device_formula = if value >= 0 {
                (value as u64 / width) as i64
            } else {
                let magnitude_minus_one = (-(value + 1)) as u64;
                -1 - (magnitude_minus_one / width) as i64
            };
            let host = i128::from(value).div_euclid(i128::from(width));
            assert_eq!(i128::from(device_formula), host, "{value} / {width}");
        }
    }

    #[test]
    fn grouped_layout_uses_exact_banded_extrema_and_fails_past_the_limit() {
        let banded = GeneratedGroupedObservation {
            box_name: "world".to_owned(),
            name: "by_band".to_owned(),
            table: 0,
            axes: vec![GroupedObservationAxis::BandedInt {
                column: 0,
                width: 60,
                extrema_index: 0,
            }],
        };
        let layout = grouped_observation_layout(&banded, &[7], &[-121, 121]).unwrap();
        assert_eq!(layout.axes[0].minimum, -3);
        assert_eq!(layout.axes[0].cardinality, 6);
        assert_eq!(layout.key_space_size, 6);

        let exact = GeneratedGroupedObservation {
            box_name: "world".to_owned(),
            name: "exact_limit".to_owned(),
            table: 0,
            axes: vec![GroupedObservationAxis::Enum {
                column: 0,
                cardinality: GROUPED_OBSERVATION_KEY_SPACE_LIMIT as u64,
            }],
        };
        assert_eq!(
            grouped_observation_layout(&exact, &[1], &[])
                .unwrap()
                .key_space_size,
            GROUPED_OBSERVATION_KEY_SPACE_LIMIT
        );

        let over = GeneratedGroupedObservation {
            name: "over_limit".to_owned(),
            axes: vec![GroupedObservationAxis::Enum {
                column: 0,
                cardinality: GROUPED_OBSERVATION_KEY_SPACE_LIMIT as u64 + 1,
            }],
            ..exact.clone()
        };
        let error = grouped_observation_layout(&over, &[1], &[])
            .unwrap_err()
            .to_string();
        assert!(error.contains("box='world' view='over_limit'"), "{error}");
        assert!(
            error.contains("computed_size=1048577 limit=1048576"),
            "{error}"
        );

        let full_i64_range = GeneratedGroupedObservation {
            box_name: "world".to_owned(),
            name: "full_i64_range".to_owned(),
            table: 0,
            axes: vec![GroupedObservationAxis::BandedInt {
                column: 0,
                width: 1,
                extrema_index: 0,
            }],
        };
        let error = grouped_observation_layout(&full_i64_range, &[1], &[i64::MIN, i64::MAX])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("computed_size=18446744073709551616 limit=1048576"),
            "{error}"
        );
    }

    #[test]
    fn grouped_histogram_omits_empty_groups_and_matches_btree_order_exactly() {
        let view = GeneratedGroupedObservation {
            box_name: "world".to_owned(),
            name: "cells".to_owned(),
            table: 0,
            axes: vec![
                GroupedObservationAxis::Enum {
                    column: 0,
                    cardinality: 2,
                },
                GroupedObservationAxis::BandedInt {
                    column: 1,
                    width: 60,
                    extrema_index: 0,
                },
            ],
        };
        let layout = grouped_observation_layout(&view, &[5], &[-61, 121]).unwrap();
        assert_eq!(layout.key_space_size, 10);
        let mut counters = vec![0_u64; layout.key_space_size];
        counters[0] = 3;
        counters[4] = 1;
        counters[6] = 2;
        assert_eq!(
            decode_grouped_histogram(&view, &layout, &counters).unwrap(),
            vec![
                GroupedViewValue {
                    box_name: "world".to_owned(),
                    name: "cells".to_owned(),
                    keys: vec![0, -2],
                    count: 3,
                },
                GroupedViewValue {
                    box_name: "world".to_owned(),
                    name: "cells".to_owned(),
                    keys: vec![0, 2],
                    count: 1,
                },
                GroupedViewValue {
                    box_name: "world".to_owned(),
                    name: "cells".to_owned(),
                    keys: vec![1, -1],
                    count: 2,
                },
            ]
        );
    }

    #[test]
    fn grouped_codegen_collects_boundable_axes_and_emits_dense_histogram_kernels() {
        let generated = generate(&grouped_model()).unwrap();
        assert!(generated.observation_eligibility.eligible);
        assert_eq!(generated.grouped_observation_band_axes, 1);
        assert_eq!(generated.grouped_observation_views.len(), 1);
        let view = &generated.grouped_observation_views[0];
        assert_eq!(
            (view.box_name.as_str(), view.name.as_str()),
            ("world", "population_cells")
        );
        assert!(matches!(
            view.axes.as_slice(),
            [
                GroupedObservationAxis::Enum { cardinality: 2, .. },
                GroupedObservationAxis::Ref { .. },
                GroupedObservationAxis::BandedInt { width: 60, .. }
            ]
        ));
        for symbol in [
            "sembla_init_grouped_extrema",
            "sembla_bound_grouped_view",
            "sembla_init_grouped_histogram",
            "sembla_observe_grouped_view",
            "sembla_div_euclid_i64_u64",
        ] {
            assert!(generated.source.contains(symbol), "missing {symbol}");
        }
        let histogram = kernel_body(&generated.source, "sembla_observe_grouped_view");
        assert!(histogram.contains("atomicAdd(counts + group, 1ULL)"));
        assert!(histogram.contains("group = group * axis_cardinalities"));

        let layout = grouped_observation_layout(view, &[4, 5], &[0, 119]).unwrap();
        assert_eq!(layout.key_space_size, 16);
        let mut counters = vec![0_u64; layout.key_space_size];
        counters[0] = 2;
        counters[15] = 1;
        assert_eq!(
            decode_grouped_histogram(view, &layout, &counters).unwrap(),
            vec![
                GroupedViewValue {
                    box_name: "world".to_owned(),
                    name: "population_cells".to_owned(),
                    keys: vec![0, 0, 0],
                    count: 2,
                },
                GroupedViewValue {
                    box_name: "world".to_owned(),
                    name: "population_cells".to_owned(),
                    keys: vec![1, 3, 1],
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn grouped_only_models_emit_legacy_enum_counts_without_state_download() {
        let generated = generate(&grouped_only_model()).unwrap();
        assert!(generated.observation_eligibility.eligible);
        assert!(generated.observation_view_tables.is_empty());
        assert_eq!(generated.generic_enum_observations.len(), 2);
        assert_eq!(generated.generic_enum_count, 4);
        let kernel = kernel_body(&generated.source, "sembla_observe_generic_enum");
        assert!(kernel.contains("atomicAdd(counts + 0ULL + value, 1ULL)"));
        assert!(kernel.contains("atomicAdd(counts + 2ULL + value, 1ULL)"));
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
        assert!(generated
            .source
            .contains("sembla_record_validation_failure(status, 10ULL, candidate,"));
        assert!(generated
            .source
            .contains("self_candidate = candidate_begin + local_candidate"));
        assert!(generated.source.contains("sembla_prepare_effects"));
        assert!(generated.source.contains("owner_values[owner]"));
    }

    #[test]
    fn stable_rule_words_key_philox_and_conflict_ordering_while_ordinals_index() {
        let model = stable_contested_model();
        assert!(model
            .transitions()
            .iter()
            .all(|transition| transition.rule_word != transition.rule_id));
        let generated = generate(&model).unwrap();

        for transition in model.transitions() {
            assert!(generated.source.contains(&format!(
                "sembla_exp(seed, tick, {}U,",
                transition.rule_word
            )));
            assert!(generated.source.contains(&format!(
                "instance_rules[instance] = {}U",
                transition.rule_word
            )));
            assert!(generated
                .source
                .contains(&format!("candidate_offsets[{}]", transition.rule_id)));
            assert!(generated
                .source
                .contains(&format!("sembla_transition_{:08x}", transition.rule_id)));
            assert!(!generated
                .source
                .contains(&format!("sembla_exp(seed, tick, {}U,", transition.rule_id)));
        }
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
        assert!(!resolver.contains("atomicAdd"));
        // The validation diagnostic reduction and prefix argmin passes may
        // use atomicMin; candidate finalization itself must not.
        assert!(!resolver.contains("atomicMin"));
    }

    #[test]
    fn segmented_argmin_has_no_cross_table_row_scan_and_prefixes_every_pass() {
        let generated = generate(&stable_contested_model()).unwrap();
        for kernel in [
            "sembla_build_claim_instances",
            "sembla_reduce_claim_keys",
            "sembla_reduce_claim_rules",
            "sembla_reduce_claim_entities",
            "sembla_reduce_claim_instances",
            "sembla_resolve_conflicts",
        ] {
            let body = kernel_body(&generated.source, kernel);
            assert!(
                !body.contains("other_row") && !body.contains("row_counts[other"),
                "{kernel} contains a cross-table all-row scan"
            );
        }

        let rules = kernel_body(&generated.source, "sembla_reduce_claim_rules");
        assert!(rules.contains("instance_keys[instance] == winner_keys[resource]"));
        let entities = kernel_body(&generated.source, "sembla_reduce_claim_entities");
        assert!(entities.contains("instance_keys[instance] == winner_keys[resource]"));
        assert!(entities.contains("instance_rules[instance] == winner_rules[resource]"));
        let instances = kernel_body(&generated.source, "sembla_reduce_claim_instances");
        assert!(instances.contains("instance_keys[instance] == winner_keys[resource]"));
        assert!(instances.contains("instance_rules[instance] == winner_rules[resource]"));
        assert!(instances.contains("instance_entities[instance] == winner_entities[resource]"));
    }

    #[test]
    fn prefix_reduction_is_lexicographic_not_component_wise() {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        struct Instance {
            key: u64,
            rule: u32,
            entity: u32,
            stable: u64,
        }

        // Every component minimum comes from a different instance. An
        // incorrect component-wise reduction would synthesize (0, 0, 0, 0),
        // while compare_instances' lexicographic key selects the first row.
        let instances = [
            Instance {
                key: 0,
                rule: 90,
                entity: 90,
                stable: 90,
            },
            Instance {
                key: 1,
                rule: 0,
                entity: 80,
                stable: 80,
            },
            Instance {
                key: 2,
                rule: 70,
                entity: 0,
                stable: 70,
            },
            Instance {
                key: 3,
                rule: 60,
                entity: 60,
                stable: 0,
            },
        ];
        let cpu = *instances
            .iter()
            .min_by_key(|instance| {
                (
                    instance.key,
                    instance.rule,
                    instance.entity,
                    instance.stable,
                )
            })
            .unwrap();

        for order in [[0, 1, 2, 3], [3, 2, 1, 0], [1, 3, 0, 2]] {
            let key = order.iter().map(|&i| instances[i].key).min().unwrap();
            let rule = order
                .iter()
                .filter(|&&i| instances[i].key == key)
                .map(|&i| instances[i].rule)
                .min()
                .unwrap();
            let entity = order
                .iter()
                .filter(|&&i| instances[i].key == key && instances[i].rule == rule)
                .map(|&i| instances[i].entity)
                .min()
                .unwrap();
            let stable = order
                .iter()
                .filter(|&&i| {
                    instances[i].key == key
                        && instances[i].rule == rule
                        && instances[i].entity == entity
                })
                .map(|&i| instances[i].stable)
                .min()
                .unwrap();
            assert_eq!(
                Instance {
                    key,
                    rule,
                    entity,
                    stable
                },
                cpu
            );
        }
        assert_eq!(cpu, instances[0]);
    }

    #[test]
    fn cuda_f64_order_key_exactly_matches_rust_total_cmp() {
        let values = [
            f64::from_bits(0xfff8_0000_0000_0001), // negative quiet NaN
            f64::from_bits(0xfff0_0000_0000_0001), // negative signaling NaN
            f64::NEG_INFINITY,
            -1.0,
            f64::from_bits(0x8000_0000_0000_0001), // negative subnormal
            -0.0,
            0.0,
            f64::from_bits(0x0000_0000_0000_0001), // positive subnormal
            1.0,
            f64::INFINITY,
            f64::from_bits(0x7ff0_0000_0000_0001), // positive signaling NaN
            f64::from_bits(0x7ff8_0000_0000_0001), // positive quiet NaN
        ];
        for left in values {
            for right in values {
                assert_eq!(
                    left.total_cmp(&right),
                    cuda_f64_order_key(left).cmp(&cuda_f64_order_key(right)),
                    "left={:#018x} right={:#018x}",
                    left.to_bits(),
                    right.to_bits()
                );
            }
        }
        assert!(cuda_f64_order_key(-0.0) < cuda_f64_order_key(0.0));
    }

    #[test]
    fn frozen_demographic_model_has_fifty_million_claim_instances() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/demographic/benchmark/demographic_slots.no-grouped.json");
        let source = std::fs::read_to_string(path).unwrap();
        let model = sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap();
        let claims_per_slot: usize = model
            .model()
            .boxes
            .iter()
            .flat_map(|model_box| &model_box.transitions)
            .map(|transition| transition.contests.len())
            .sum();
        assert_eq!(claims_per_slot, 5);
        let instance_count = 10_000_000_usize.checked_mul(claims_per_slot).unwrap();
        assert_eq!(instance_count, 50_000_000);
        // Four SoA fields: resource/key u64 and rule/entity u32.
        assert_eq!(instance_count * (8 + 8 + 4 + 4), 1_200_000_000);
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
    fn sir_simulation_source_matches_unchanged_checked_in_golden() {
        fn remove_between(source: &mut String, begin: &str, end: &str) {
            let begin = source.find(begin).unwrap();
            let end = source[begin..]
                .find(end)
                .map(|offset| begin + offset)
                .unwrap();
            source.replace_range(begin..end, "");
        }

        let generated = generate(&sir_model()).unwrap();
        let mut simulation = generated.source;
        let mut golden = include_str!("../tests/fixtures/sir.generated.cu").to_owned();

        // PRD 0008 replaces these regions with focused lock-free protocol
        // assertions. Excluding them from both sides keeps the pre-existing
        // broad source golden byte-identical rather than blessing unrelated
        // generated-source churn while updating a correctness protocol.
        for source in [&mut simulation, &mut golden] {
            remove_between(
                source,
                "// Records one validation failure into scratch slots",
                "__device__ __forceinline__ long long sembla_add_i64",
            );
            remove_between(
                source,
                "\nextern \"C\" __global__ void sembla_init_validation_scratch",
                "\nextern \"C\" __global__ void sembla_mark_effect_active",
            );
            remove_between(
                source,
                "\nextern \"C\" __global__ void sembla_prepare_effects",
                "\nextern \"C\" __global__ void sembla_apply_effects",
            );
        }

        let helpers_begin = simulation
            .find("__device__ __forceinline__ void sembla_atomic_min_i64")
            .unwrap();
        let helpers_end = simulation[helpers_begin..]
            .find("__device__ __forceinline__ unsigned long long sembla_f64_order_key")
            .map(|offset| helpers_begin + offset)
            .unwrap();
        simulation.replace_range(helpers_begin..helpers_end, "");
        let observation_begin = simulation
            .find("\nextern \"C\" __global__ void sembla_init_observations")
            .unwrap();
        let observation_end = simulation[observation_begin..]
            .find("\nextern \"C\" __global__ void sembla_philox_vectors")
            .map(|offset| observation_begin + offset)
            .unwrap();
        simulation.replace_range(observation_begin..observation_end, "");
        assert_eq!(simulation, golden);
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
