//! Deterministic, snapshot-only evaluation of validated IR expressions.
//!
//! Expressions are evaluated in syntax-tree order without reassociation. Real
//! arithmetic therefore uses ordinary IEEE-754 `f64` semantics: in particular,
//! division by zero produces infinity or NaN rather than a runtime error.
//! Aggregate sums make one sequential target-table pass in ascending row order;
//! that order is the canonical Level A CPU reduction order (`DESIGN.md` §5.2).

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::sync::OnceLock;

/// Default row tile chosen by the PRD 0001 sweep recorded under `docs/evidence`.
/// At 1,024 rows the benchmark's deepest guard peaks at about 20 KiB: an
/// 8 KiB borrowed Int attribute, an 8 KiB literal, Bool results, and enclosing
/// Bool operands. Final guard, hazard, and Ref claim roots occupy at most 13 KiB.
/// Both bounds fit the M2 Pro's 32 KiB L1 data cache.
pub(crate) const TICK_TILE_ROWS: usize = 1_024;
/// Smaller measured cases consumed 3–6% more single-worker user time. The 1M
/// binding case is the first measured size where that primary metric is flat.
pub(crate) const TICK_TILE_THRESHOLD: usize = 1_000_000;
const EVALUATOR_THREADS_ENV: &str = "SEMBLA_EVAL_THREADS";
const EVALUATOR_TILE_ROWS_ENV: &str = "SEMBLA_EVAL_TILE_ROWS";
const EVALUATOR_TILE_THRESHOLD_ENV: &str = "SEMBLA_EVAL_TILE_THRESHOLD";

static TICK_WORKERS: OnceLock<usize> = OnceLock::new();
static CONFIGURED_TILE_ROWS: OnceLock<usize> = OnceLock::new();
static CONFIGURED_TILE_THRESHOLD: OnceLock<usize> = OnceLock::new();

#[inline]
pub(crate) fn tick_worker_count() -> usize {
    #[cfg(test)]
    if let Some(workers) = TEST_TICK_WORKERS.with(std::cell::Cell::get) {
        return workers;
    }

    *TICK_WORKERS.get_or_init(|| {
        std::env::var(EVALUATOR_THREADS_ENV)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|workers| *workers > 0)
            .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, usize::from))
    })
}

#[inline]
pub(crate) fn tick_tile_rows() -> usize {
    #[cfg(test)]
    if let Some(rows) = TEST_TICK_TILE_ROWS.with(std::cell::Cell::get) {
        return rows;
    }

    *CONFIGURED_TILE_ROWS.get_or_init(|| {
        std::env::var(EVALUATOR_TILE_ROWS_ENV)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|rows| *rows > 0)
            .unwrap_or(TICK_TILE_ROWS)
    })
}

#[inline]
pub(crate) fn tick_tile_threshold() -> usize {
    #[cfg(test)]
    if let Some(rows) = TEST_TICK_TILE_THRESHOLD.with(std::cell::Cell::get) {
        return rows;
    }

    *CONFIGURED_TILE_THRESHOLD.get_or_init(|| {
        std::env::var(EVALUATOR_TILE_THRESHOLD_ENV)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(TICK_TILE_THRESHOLD)
    })
}

#[inline]
pub(crate) fn tick_tiling_enabled(row_count: usize) -> bool {
    row_count >= tick_tile_threshold() && row_count > tick_tile_rows()
}

/// Legacy whole-column maps are deliberately serial. PRD 0001 moved the only
/// execution parallel region above expression evaluation, where a complete row
/// tile carries every eligible transition expression and racing clock.
#[inline]
pub(crate) fn element_wise_parallel_enabled(_row_count: usize) -> bool {
    false
}

#[inline]
pub(crate) fn element_wise_map<T, F>(row_count: usize, map: F) -> Vec<T>
where
    F: Fn(usize) -> T,
{
    (0..row_count).map(map).collect()
}

#[inline]
pub(crate) fn element_wise_map_with_initializer<T, I, F>(
    row_count: usize,
    _initialize: I,
    map: F,
) -> Vec<T>
where
    I: Fn() -> T,
    F: Fn(usize) -> T,
{
    (0..row_count).map(map).collect()
}

#[cfg(test)]
thread_local! {
    static TEST_TICK_WORKERS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static TEST_TICK_TILE_ROWS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static TEST_TICK_TILE_THRESHOLD: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(crate) fn with_test_tick_tiles<R>(
    workers: usize,
    tile_rows: usize,
    threshold: usize,
    run: impl FnOnce() -> R,
) -> R {
    TEST_TICK_WORKERS.with(|worker_slot| {
        TEST_TICK_TILE_ROWS.with(|tile_slot| {
            TEST_TICK_TILE_THRESHOLD.with(|threshold_slot| {
                let previous_workers = worker_slot.replace(Some(workers.max(1)));
                let previous_tile = tile_slot.replace(Some(tile_rows.max(1)));
                let previous_threshold = threshold_slot.replace(Some(threshold));
                let result = run();
                threshold_slot.set(previous_threshold);
                tile_slot.set(previous_tile);
                worker_slot.set(previous_workers);
                result
            })
        })
    })
}

use sembla_ir::{
    AggJoin, AggOp, Aggregate, Attr, AttrType, Expr, ParamType, ParamValue, Table, ValidatedModel,
};

use crate::state::{ColumnData, InputTable, Snapshot, StateError};

/// A typed expression result in query-row order.
#[derive(Clone, Debug, PartialEq)]
pub enum ValueColumn {
    Real(Vec<f64>),
    Int(Vec<i64>),
    Bool(Vec<bool>),
    Enum(Vec<u16>),
}

/// A Ref expression result together with its validator-established target table.
///
/// Ref metadata is kept separate so [`ValueColumn`] retains its frozen four-variant
/// public contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefColumn {
    pub target_table: String,
    pub values: Vec<u32>,
}

impl ValueColumn {
    /// Number of query rows represented by this column.
    pub fn len(&self) -> usize {
        match self {
            Self::Real(values) => values.len(),
            Self::Int(values) => values.len(),
            Self::Bool(values) => values.len(),
            Self::Enum(values) => values.len(),
        }
    }

    /// Whether the result has no query rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Debug, PartialEq)]
enum InternalColumn {
    Real(Vec<f64>),
    Int(Vec<i64>),
    Bool(Vec<bool>),
    Enum(Vec<u16>),
    Ref(Vec<u32>),
}

impl TryFrom<InternalColumn> for ValueColumn {
    type Error = EvalError;

    fn try_from(column: InternalColumn) -> Result<Self, Self::Error> {
        match column {
            InternalColumn::Real(values) => Ok(Self::Real(values)),
            InternalColumn::Int(values) => Ok(Self::Int(values)),
            InternalColumn::Bool(values) => Ok(Self::Bool(values)),
            InternalColumn::Enum(values) => Ok(Self::Enum(values)),
            InternalColumn::Ref(_) => Err(EvalError::new(
                "top-level Ref expressions are internal-only in PRD 0005",
            )),
        }
    }
}

/// One named per-run parameter override.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamOverride {
    pub name: String,
    pub value: ParamValue,
}

impl ParamOverride {
    pub fn new(name: impl Into<String>, value: ParamValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

/// Parameters resolved once from IR defaults and per-run overrides.
///
/// Entries remain in declaration order. Parameter values are never written
/// back into the IR (`DESIGN.md` §4.1).
#[derive(Clone, Debug, PartialEq)]
pub struct ParamEnv {
    values: Vec<(String, ParamValue)>,
}

impl ParamEnv {
    /// Resolves all defaults with no per-run overrides.
    pub fn defaults(model: &ValidatedModel) -> Self {
        Self {
            values: model
                .model()
                .params
                .iter()
                .map(|param| (param.name.clone(), param.default.clone()))
                .collect(),
        }
    }

    /// Resolves defaults overlaid by validated, uniquely named overrides.
    pub fn resolve(model: &ValidatedModel, overrides: &[ParamOverride]) -> Result<Self, EvalError> {
        let mut env = Self::defaults(model);
        for (override_index, parameter_override) in overrides.iter().enumerate() {
            if overrides[..override_index]
                .iter()
                .any(|previous| previous.name == parameter_override.name)
            {
                return Err(EvalError::new(format!(
                    "duplicate override for parameter '{}'",
                    parameter_override.name
                )));
            }
            let declaration = model
                .model()
                .params
                .iter()
                .find(|param| param.name == parameter_override.name)
                .ok_or_else(|| {
                    EvalError::new(format!(
                        "override refers to unknown parameter '{}'",
                        parameter_override.name
                    ))
                })?;
            if !parameter_value_matches(declaration.ty, &parameter_override.value) {
                return Err(EvalError::new(format!(
                    "override for parameter '{}' does not match {:?}",
                    parameter_override.name, declaration.ty
                )));
            }
            if matches!(
                parameter_override.value,
                ParamValue::Real { value } if !value.is_finite()
            ) {
                return Err(EvalError::new(format!(
                    "override for parameter '{}' must be finite",
                    parameter_override.name
                )));
            }
            let entry = env
                .values
                .iter_mut()
                .find(|(name, _)| *name == parameter_override.name)
                .ok_or_else(|| EvalError::new("validated parameter declaration disappeared"))?;
            entry.1 = parameter_override.value.clone();
        }
        Ok(env)
    }

    /// Resolved values in parameter declaration order.
    pub fn values(&self) -> impl Iterator<Item = (&str, &ParamValue)> {
        self.values
            .iter()
            .map(|(name, value)| (name.as_str(), value))
    }

    fn get(&self, name: &str) -> Result<&ParamValue, EvalError> {
        self.values
            .iter()
            .find(|(entry_name, _)| entry_name == name)
            .map(|(_, value)| value)
            .ok_or_else(|| {
                EvalError::new(format!("parameter environment has no value for '{name}'"))
            })
    }
}

fn checked_parameter_value<'params>(
    model: &ValidatedModel,
    params: &'params ParamEnv,
    name: &str,
) -> Result<&'params ParamValue, EvalError> {
    let declaration = model
        .model()
        .params
        .iter()
        .find(|param| param.name == name)
        .ok_or_else(|| EvalError::new(format!("unresolved parameter '{name}'")))?;
    let value = params.get(name)?;
    if parameter_value_matches(declaration.ty, value) {
        Ok(value)
    } else {
        Err(EvalError::new(format!(
            "parameter environment value for '{name}' has the wrong type"
        )))
    }
}

/// A table resolved through a [`ValidatedModel`].
///
/// The private indices preserve box qualification when different boxes use the
/// same local table name.
#[derive(Clone, Copy, Debug)]
pub struct EvalTable<'model> {
    model: &'model ValidatedModel,
    box_index: usize,
    table_index: usize,
    expected_attr_index: Option<usize>,
}

impl<'model> EvalTable<'model> {
    pub fn new(
        model: &'model ValidatedModel,
        box_name: &str,
        table_name: &str,
    ) -> Result<Self, EvalError> {
        let box_index = model
            .model()
            .boxes
            .iter()
            .position(|model_box| model_box.name == box_name)
            .ok_or_else(|| EvalError::new(format!("unknown box '{box_name}'")))?;
        let table_index = model.model().boxes[box_index]
            .tables
            .iter()
            .position(|table| table.name == table_name)
            .ok_or_else(|| {
                EvalError::new(format!(
                    "box '{box_name}' has no table named '{table_name}'"
                ))
            })?;
        Ok(Self {
            model,
            box_index,
            table_index,
            expected_attr_index: None,
        })
    }

    pub fn box_name(&self) -> &str {
        &self.model_box().name
    }

    pub fn table_name(&self) -> &str {
        &self.schema().name
    }

    /// Supplies the validator-established destination attribute context.
    pub fn with_expected_attr(mut self, attr_name: &str) -> Result<Self, EvalError> {
        self.expected_attr_index = Some(
            self.schema()
                .attrs
                .iter()
                .position(|attr| attr.name == attr_name)
                .ok_or_else(|| {
                    EvalError::new(format!(
                        "table '{}' has no expected attribute '{attr_name}'",
                        self.table_name()
                    ))
                })?,
        );
        Ok(self)
    }

    fn expected_type(&self) -> Option<RuntimeType> {
        self.expected_attr_index
            .map(|index| RuntimeType::from(&self.schema().attrs[index].ty))
    }

    fn model_box(&self) -> &sembla_ir::Box {
        &self.model.model().boxes[self.box_index]
    }

    fn schema(&self) -> &Table {
        &self.model_box().tables[self.table_index]
    }

    fn target(&self, table_name: &str) -> Result<Self, EvalError> {
        Self::new(self.model, self.box_name(), table_name)
    }
}

#[derive(Clone, Debug)]
struct AggregateKey {
    box_name: String,
    table: String,
    op: AggOp,
    on: AggJoin,
    filter: Expr,
}

impl PartialEq for AggregateKey {
    fn eq(&self, other: &Self) -> bool {
        self.box_name == other.box_name
            && self.table == other.table
            && self.on == other.on
            && agg_op_structural_eq(&self.op, &other.op)
            && expr_structural_eq(&self.filter, &other.filter)
    }
}

fn agg_op_structural_eq(lhs: &AggOp, rhs: &AggOp) -> bool {
    match (lhs, rhs) {
        (AggOp::Count, AggOp::Count) => true,
        (AggOp::Sum { value: lhs }, AggOp::Sum { value: rhs }) => expr_structural_eq(lhs, rhs),
        _ => false,
    }
}

fn aggregate_structural_eq(lhs: &sembla_ir::Aggregate, rhs: &sembla_ir::Aggregate) -> bool {
    agg_op_structural_eq(&lhs.op, &rhs.op)
        && match (&lhs.filter, &rhs.filter) {
            (Some(lhs), Some(rhs)) => expr_structural_eq(lhs, rhs),
            (None, None) => true,
            _ => false,
        }
}

fn binary_expr_eq(lhs_left: &Expr, lhs_right: &Expr, rhs_left: &Expr, rhs_right: &Expr) -> bool {
    expr_structural_eq(lhs_left, rhs_left) && expr_structural_eq(lhs_right, rhs_right)
}

fn expr_structural_eq(lhs: &Expr, rhs: &Expr) -> bool {
    match (lhs, rhs) {
        (Expr::Real { value: lhs }, Expr::Real { value: rhs }) => lhs.to_bits() == rhs.to_bits(),
        (Expr::Int { value: lhs }, Expr::Int { value: rhs }) => lhs == rhs,
        (Expr::Bool { value: lhs }, Expr::Bool { value: rhs }) => lhs == rhs,
        (Expr::Enum { variant: lhs }, Expr::Enum { variant: rhs }) => lhs == rhs,
        (Expr::Param { name: lhs }, Expr::Param { name: rhs }) => lhs == rhs,
        (Expr::SelfAttr { name: lhs }, Expr::SelfAttr { name: rhs }) => lhs == rhs,
        (
            Expr::Add {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::Add {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        )
        | (
            Expr::Sub {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::Sub {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        )
        | (
            Expr::Mul {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::Mul {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        )
        | (
            Expr::Div {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::Div {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        )
        | (
            Expr::Eq {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::Eq {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        )
        | (
            Expr::Ne {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::Ne {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        )
        | (
            Expr::Lt {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::Lt {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        )
        | (
            Expr::Le {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::Le {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        )
        | (
            Expr::Gt {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::Gt {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        )
        | (
            Expr::Ge {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::Ge {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        )
        | (
            Expr::And {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::And {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        )
        | (
            Expr::Or {
                lhs: lhs_left,
                rhs: lhs_right,
            },
            Expr::Or {
                lhs: rhs_left,
                rhs: rhs_right,
            },
        ) => binary_expr_eq(lhs_left, lhs_right, rhs_left, rhs_right),
        (Expr::Not { expr: lhs }, Expr::Not { expr: rhs }) => expr_structural_eq(lhs, rhs),
        (
            Expr::EnumIs {
                attr: lhs_attr,
                variant: lhs_variant,
            },
            Expr::EnumIs {
                attr: rhs_attr,
                variant: rhs_variant,
            },
        ) => lhs_attr == rhs_attr && lhs_variant == rhs_variant,
        (
            Expr::Input {
                port: lhs_port,
                agg: lhs_agg,
            },
            Expr::Input {
                port: rhs_port,
                agg: rhs_agg,
            },
        ) => lhs_port == rhs_port && aggregate_structural_eq(lhs_agg, rhs_agg),
        (
            Expr::Agg {
                op: lhs_op,
                table: lhs_table,
                on: lhs_on,
                filter: lhs_filter,
            },
            Expr::Agg {
                op: rhs_op,
                table: rhs_table,
                on: rhs_on,
                filter: rhs_filter,
            },
        ) => {
            lhs_table == rhs_table
                && lhs_on == rhs_on
                && agg_op_structural_eq(lhs_op, rhs_op)
                && expr_structural_eq(lhs_filter, rhs_filter)
        }
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Accumulator {
    Int(Vec<i64>),
    Real(Vec<f64>),
}

#[derive(Clone, Debug)]
struct CacheEntry {
    key: AggregateKey,
    values: Accumulator,
}

/// Aggregate accumulators bound to one validated model and tick input scope.
///
/// Holding these references prevents allocator address reuse while entries are
/// live. A fresh cache is required for each snapshot/parameter scope.
#[derive(Clone, Debug)]
pub struct AggCache<'tick, 'state> {
    model: &'tick ValidatedModel,
    snapshot: &'tick Snapshot<'state>,
    params: &'tick ParamEnv,
    entries: Vec<CacheEntry>,
    build_count: usize,
}

impl<'tick, 'state> AggCache<'tick, 'state> {
    pub fn new(
        model: &'tick ValidatedModel,
        snapshot: &'tick Snapshot<'state>,
        params: &'tick ParamEnv,
    ) -> Self {
        Self {
            model,
            snapshot,
            params,
            entries: Vec::new(),
            build_count: 0,
        }
    }

    /// Returns the exact tick-start snapshot bound to this cache.
    pub fn snapshot(&self) -> &'tick Snapshot<'state> {
        self.snapshot
    }

    /// Number of structurally distinct aggregate accumulators currently held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Successful accumulator builds in this tick scope.
    pub fn build_count(&self) -> usize {
        self.build_count
    }

    /// Drops every accumulator while retaining the same tick scope.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.build_count = 0;
    }

    fn validate_scope(
        &self,
        table: EvalTable<'_>,
        snapshot: &Snapshot<'_>,
        params: &ParamEnv,
    ) -> Result<(), EvalError> {
        if !std::ptr::eq(self.model, table.model) {
            return Err(EvalError::new(
                "aggregate cache belongs to a different model",
            ));
        }
        if !std::ptr::eq(self.snapshot, snapshot) {
            return Err(EvalError::new(
                "aggregate cache belongs to a different Snapshot object",
            ));
        }
        if !std::ptr::eq(self.params, params) {
            return Err(EvalError::new(
                "aggregate cache belongs to a different parameter environment",
            ));
        }
        Ok(())
    }
}

/// A deterministic evaluation failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalError {
    message: String,
}

impl EvalError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for EvalError {}

impl From<StateError> for EvalError {
    fn from(error: StateError) -> Self {
        Self::new(error.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RuntimeType {
    Real,
    Int,
    Bool,
    Enum(Vec<String>),
    Ref(String),
}

impl RuntimeType {
    fn name(&self) -> &'static str {
        match self {
            Self::Real => "Real",
            Self::Int => "Int",
            Self::Bool => "Bool",
            Self::Enum(_) => "Enum",
            Self::Ref(_) => "Ref",
        }
    }

    fn is_numeric(&self) -> bool {
        matches!(self, Self::Real | Self::Int)
    }
}

impl From<&AttrType> for RuntimeType {
    fn from(value: &AttrType) -> Self {
        match value {
            AttrType::Real => Self::Real,
            AttrType::Int => Self::Int,
            AttrType::Enum { variants } => Self::Enum(variants.clone()),
            AttrType::Ref { table } => Self::Ref(table.clone()),
        }
    }
}

/// One row-local value produced by a prepared, aggregate-free expression.
#[derive(Clone, Copy, Debug)]
pub(crate) enum PreparedValue {
    Real(f64),
    Int(i64),
    Bool,
    Enum(u16),
    Ref,
}

#[derive(Debug)]
pub(crate) enum PreparedColumn<'state> {
    Real(Cow<'state, [f64]>),
    Int(Cow<'state, [i64]>),
    Bool(Cow<'state, [bool]>),
    Enum(Cow<'state, [u16]>),
    Ref(Cow<'state, [u32]>),
}

/// An expression whose parameters and state columns were resolved once before
/// entering the tick's tile loop. The tree contains no aggregate, input-table
/// reduction, or fallible integer arithmetic, so row evaluation is side-effect
/// free and cannot change the executor's declaration-ordered error behaviour.
#[derive(Debug)]
pub(crate) struct PreparedExpr<'state> {
    node: PreparedNode<'state>,
    ty: RuntimeType,
}

#[derive(Debug)]
enum PreparedNode<'state> {
    Real(f64),
    Int(i64),
    Bool(bool),
    Enum(u16),
    RealAttr(&'state [f64]),
    IntAttr(&'state [i64]),
    EnumAttr(&'state [u16]),
    RefAttr(&'state [u32]),
    Add(Box<Self>, Box<Self>),
    Sub(Box<Self>, Box<Self>),
    Mul(Box<Self>, Box<Self>),
    Div(Box<Self>, Box<Self>),
    Eq(Box<Self>, Box<Self>),
    Ne(Box<Self>, Box<Self>),
    Lt(Box<Self>, Box<Self>),
    Le(Box<Self>, Box<Self>),
    Gt(Box<Self>, Box<Self>),
    Ge(Box<Self>, Box<Self>),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Not(Box<Self>),
    EnumIs(&'state [u16], u16),
}

impl<'state> PreparedExpr<'state> {
    /// Evaluates one fixed absolute row range with expression dispatch outside
    /// the row loops. Intermediate vectors are at most one tile long.
    pub(crate) fn tile(
        &self,
        start: usize,
        end: usize,
    ) -> Result<PreparedColumn<'state>, EvalError> {
        eval_prepared_rows(&self.node, PreparedRows::Range { start, end })
    }

    fn gather(&self, rows: &[usize]) -> Result<PreparedColumn<'state>, EvalError> {
        eval_prepared_rows(&self.node, PreparedRows::Gather(rows))
    }

    pub(crate) fn ref_target(&self) -> Option<&str> {
        match &self.ty {
            RuntimeType::Ref(table) => Some(table),
            _ => None,
        }
    }
}

fn prepared_column_as_real(column: PreparedColumn<'_>) -> Result<Cow<'_, [f64]>, EvalError> {
    match column {
        PreparedColumn::Real(values) => Ok(values),
        PreparedColumn::Int(values) => Ok(Cow::Owned(
            values.iter().map(|value| *value as f64).collect(),
        )),
        _ => Err(EvalError::new(
            "prepared numeric expression did not evaluate to Real or Int",
        )),
    }
}

#[derive(Clone, Copy)]
enum PreparedRows<'rows> {
    Range { start: usize, end: usize },
    Gather(&'rows [usize]),
}

impl PreparedRows<'_> {
    fn len(self) -> usize {
        match self {
            Self::Range { start, end } => end - start,
            Self::Gather(rows) => rows.len(),
        }
    }

    fn absolute_row(self, offset: usize) -> usize {
        match self {
            Self::Range { start, .. } => start + offset,
            Self::Gather(rows) => rows[offset],
        }
    }

    fn select<T: Copy>(self, values: &[T]) -> Cow<'_, [T]> {
        match self {
            Self::Range { start, end } => Cow::Borrowed(&values[start..end]),
            Self::Gather(rows) => Cow::Owned(rows.iter().map(|row| values[*row]).collect()),
        }
    }
}

fn eval_prepared_rows<'state>(
    node: &PreparedNode<'state>,
    rows: PreparedRows<'_>,
) -> Result<PreparedColumn<'state>, EvalError> {
    let row_count = rows.len();
    match node {
        PreparedNode::Real(value) => Ok(PreparedColumn::Real(Cow::Owned(vec![*value; row_count]))),
        PreparedNode::Int(value) => Ok(PreparedColumn::Int(Cow::Owned(vec![*value; row_count]))),
        PreparedNode::Bool(value) => Ok(PreparedColumn::Bool(Cow::Owned(vec![*value; row_count]))),
        PreparedNode::Enum(value) => Ok(PreparedColumn::Enum(Cow::Owned(vec![*value; row_count]))),
        PreparedNode::RealAttr(values) => Ok(PreparedColumn::Real(rows.select(values))),
        PreparedNode::IntAttr(values) => Ok(PreparedColumn::Int(rows.select(values))),
        PreparedNode::EnumAttr(values) => Ok(PreparedColumn::Enum(rows.select(values))),
        PreparedNode::RefAttr(values) => Ok(PreparedColumn::Ref(rows.select(values))),
        PreparedNode::Add(lhs, rhs) | PreparedNode::Sub(lhs, rhs) | PreparedNode::Mul(lhs, rhs) => {
            let lhs = eval_prepared_rows(lhs, rows)?;
            let rhs = eval_prepared_rows(rhs, rows)?;
            if matches!(lhs, PreparedColumn::Real(_)) || matches!(rhs, PreparedColumn::Real(_)) {
                let lhs = prepared_column_as_real(lhs)?;
                let rhs = prepared_column_as_real(rhs)?;
                return Ok(PreparedColumn::Real(Cow::Owned(
                    lhs.iter()
                        .copied()
                        .zip(rhs.iter().copied())
                        .map(|(lhs, rhs)| match node {
                            PreparedNode::Add(_, _) => lhs + rhs,
                            PreparedNode::Sub(_, _) => lhs - rhs,
                            PreparedNode::Mul(_, _) => lhs * rhs,
                            _ => unreachable!(),
                        })
                        .collect(),
                )));
            }
            let (PreparedColumn::Int(lhs), PreparedColumn::Int(rhs)) = (lhs, rhs) else {
                return Err(EvalError::new(
                    "prepared arithmetic operands are not numeric",
                ));
            };
            let values = lhs
                .iter()
                .copied()
                .zip(rhs.iter().copied())
                .enumerate()
                .map(|(offset, (lhs, rhs))| {
                    let value = match node {
                        PreparedNode::Add(_, _) => lhs.checked_add(rhs),
                        PreparedNode::Sub(_, _) => lhs.checked_sub(rhs),
                        PreparedNode::Mul(_, _) => lhs.checked_mul(rhs),
                        _ => unreachable!(),
                    };
                    value.ok_or_else(|| {
                        EvalError::new(format!(
                            "integer arithmetic overflow at row {}",
                            rows.absolute_row(offset)
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PreparedColumn::Int(Cow::Owned(values)))
        }
        PreparedNode::Div(lhs, rhs) => {
            let lhs = prepared_column_as_real(eval_prepared_rows(lhs, rows)?)?;
            let rhs = prepared_column_as_real(eval_prepared_rows(rhs, rows)?)?;
            Ok(PreparedColumn::Real(Cow::Owned(
                lhs.iter()
                    .copied()
                    .zip(rhs.iter().copied())
                    .map(|(lhs, rhs)| lhs / rhs)
                    .collect(),
            )))
        }
        PreparedNode::Eq(lhs, rhs) | PreparedNode::Ne(lhs, rhs) => {
            let lhs = eval_prepared_rows(lhs, rows)?;
            let rhs = eval_prepared_rows(rhs, rows)?;
            let equal: Vec<bool> = match (lhs, rhs) {
                (PreparedColumn::Real(lhs), PreparedColumn::Real(rhs)) => lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(lhs, rhs)| lhs == rhs)
                    .collect(),
                (PreparedColumn::Real(lhs), PreparedColumn::Int(rhs)) => lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(lhs, rhs)| *lhs == *rhs as f64)
                    .collect(),
                (PreparedColumn::Int(lhs), PreparedColumn::Real(rhs)) => lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(lhs, rhs)| *lhs as f64 == *rhs)
                    .collect(),
                (PreparedColumn::Int(lhs), PreparedColumn::Int(rhs)) => lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(lhs, rhs)| lhs == rhs)
                    .collect(),
                (PreparedColumn::Bool(lhs), PreparedColumn::Bool(rhs)) => lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(lhs, rhs)| lhs == rhs)
                    .collect(),
                (PreparedColumn::Enum(lhs), PreparedColumn::Enum(rhs)) => lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(lhs, rhs)| lhs == rhs)
                    .collect(),
                (PreparedColumn::Ref(lhs), PreparedColumn::Ref(rhs)) => lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(lhs, rhs)| lhs == rhs)
                    .collect(),
                _ => {
                    return Err(EvalError::new(
                        "prepared equality operands have incompatible types",
                    ))
                }
            };
            Ok(PreparedColumn::Bool(Cow::Owned(
                if matches!(node, PreparedNode::Ne(_, _)) {
                    equal.into_iter().map(|value| !value).collect()
                } else {
                    equal
                },
            )))
        }
        PreparedNode::Lt(lhs, rhs)
        | PreparedNode::Le(lhs, rhs)
        | PreparedNode::Gt(lhs, rhs)
        | PreparedNode::Ge(lhs, rhs) => {
            let lhs = eval_prepared_rows(lhs, rows)?;
            let rhs = eval_prepared_rows(rhs, rows)?;
            let values = if let (PreparedColumn::Int(lhs), PreparedColumn::Int(rhs)) = (&lhs, &rhs)
            {
                lhs.iter()
                    .zip(rhs.iter())
                    .map(|(lhs, rhs)| match node {
                        PreparedNode::Lt(_, _) => lhs < rhs,
                        PreparedNode::Le(_, _) => lhs <= rhs,
                        PreparedNode::Gt(_, _) => lhs > rhs,
                        PreparedNode::Ge(_, _) => lhs >= rhs,
                        _ => unreachable!(),
                    })
                    .collect()
            } else {
                let lhs = prepared_column_as_real(lhs)?;
                let rhs = prepared_column_as_real(rhs)?;
                lhs.iter()
                    .copied()
                    .zip(rhs.iter().copied())
                    .map(|(lhs, rhs)| match node {
                        PreparedNode::Lt(_, _) => lhs < rhs,
                        PreparedNode::Le(_, _) => lhs <= rhs,
                        PreparedNode::Gt(_, _) => lhs > rhs,
                        PreparedNode::Ge(_, _) => lhs >= rhs,
                        _ => unreachable!(),
                    })
                    .collect()
            };
            Ok(PreparedColumn::Bool(Cow::Owned(values)))
        }
        PreparedNode::And(lhs, rhs) | PreparedNode::Or(lhs, rhs) => {
            let lhs = eval_prepared_rows(lhs, rows)?;
            let rhs = eval_prepared_rows(rhs, rows)?;
            let (PreparedColumn::Bool(lhs), PreparedColumn::Bool(rhs)) = (lhs, rhs) else {
                return Err(EvalError::new("prepared boolean operands are not Bool"));
            };
            Ok(PreparedColumn::Bool(Cow::Owned(
                lhs.iter()
                    .copied()
                    .zip(rhs.iter().copied())
                    .map(|(lhs, rhs)| {
                        if matches!(node, PreparedNode::And(_, _)) {
                            lhs && rhs
                        } else {
                            lhs || rhs
                        }
                    })
                    .collect(),
            )))
        }
        PreparedNode::Not(expr) => match eval_prepared_rows(expr, rows)? {
            PreparedColumn::Bool(values) => Ok(PreparedColumn::Bool(Cow::Owned(
                values.iter().map(|value| !*value).collect(),
            ))),
            _ => Err(EvalError::new("prepared Not operand is not Bool")),
        },
        PreparedNode::EnumIs(values, expected) => Ok(PreparedColumn::Bool(Cow::Owned(
            rows.select(values)
                .iter()
                .map(|value| *value == *expected)
                .collect(),
        ))),
    }
}

/// Prepares a row expression only when tiling cannot alter error precedence.
/// Aggregate and input reductions stay on the canonical whole-column path, as
/// does checked integer arithmetic whose first overflowing row is observable.
pub(crate) fn prepare_row_expr<'state>(
    expr: &Expr,
    table: EvalTable<'_>,
    snapshot: &'state Snapshot<'_>,
    params: &ParamEnv,
) -> Result<Option<PreparedExpr<'state>>, EvalError> {
    let inferred = infer_root_type(expr, table)?;
    prepare_row_expr_with_type(expr, table, snapshot, params, inferred)
}

fn prepare_row_expr_with_type<'state>(
    expr: &Expr,
    table: EvalTable<'_>,
    snapshot: &'state Snapshot<'_>,
    params: &ParamEnv,
    inferred: RuntimeType,
) -> Result<Option<PreparedExpr<'state>>, EvalError> {
    if !expr_is_row_infallible(expr, table, &table.schema().attrs)? {
        return Ok(None);
    }
    let node = prepare_node(
        expr,
        table,
        &table.schema().attrs,
        snapshot,
        params,
        Some(&inferred),
    )?;
    Ok(Some(PreparedExpr { node, ty: inferred }))
}

pub(crate) fn expr_is_gather_eligible(
    expr: &Expr,
    table: EvalTable<'_>,
) -> Result<bool, EvalError> {
    infer_root_type(expr, table)?;
    expr_is_row_infallible(expr, table, &table.schema().attrs)
}

/// Reuses the gather predicate for device observations that additionally need
/// a positively identified `Int` root. Keeping the row-local decision here
/// prevents CUDA observation from growing a second expression whitelist.
pub(crate) fn expr_is_gather_eligible_int(
    expr: &Expr,
    table: EvalTable<'_>,
) -> Result<bool, EvalError> {
    Ok(infer_root_type(expr, table)? == RuntimeType::Int
        && expr_is_row_infallible(expr, table, &table.schema().attrs)?)
}

fn expr_is_row_infallible(
    expr: &Expr,
    table: EvalTable<'_>,
    row_attrs: &[Attr],
) -> Result<bool, EvalError> {
    Ok(match expr {
        Expr::Real { .. }
        | Expr::Int { .. }
        | Expr::Bool { .. }
        | Expr::Enum { .. }
        | Expr::Param { .. }
        | Expr::SelfAttr { .. }
        | Expr::EnumIs { .. } => true,
        Expr::Add { lhs, rhs } | Expr::Sub { lhs, rhs } | Expr::Mul { lhs, rhs } => {
            infer_expr_type(expr, table, row_attrs, None)? == RuntimeType::Real
                && expr_is_row_infallible(lhs, table, row_attrs)?
                && expr_is_row_infallible(rhs, table, row_attrs)?
        }
        Expr::Div { lhs, rhs }
        | Expr::Eq { lhs, rhs }
        | Expr::Ne { lhs, rhs }
        | Expr::Lt { lhs, rhs }
        | Expr::Le { lhs, rhs }
        | Expr::Gt { lhs, rhs }
        | Expr::Ge { lhs, rhs }
        | Expr::And { lhs, rhs }
        | Expr::Or { lhs, rhs } => {
            expr_is_row_infallible(lhs, table, row_attrs)?
                && expr_is_row_infallible(rhs, table, row_attrs)?
        }
        Expr::Not { expr } => expr_is_row_infallible(expr, table, row_attrs)?,
        Expr::Input { .. } | Expr::Agg { .. } => false,
    })
}

fn prepare_node<'state>(
    expr: &Expr,
    table: EvalTable<'_>,
    row_attrs: &[Attr],
    snapshot: &'state Snapshot<'_>,
    params: &ParamEnv,
    expected: Option<&RuntimeType>,
) -> Result<PreparedNode<'state>, EvalError> {
    match expr {
        Expr::Real { value } => Ok(PreparedNode::Real(*value)),
        Expr::Int { value } => Ok(PreparedNode::Int(*value)),
        Expr::Bool { value } => Ok(PreparedNode::Bool(*value)),
        Expr::Enum { variant } => {
            let RuntimeType::Enum(variants) = expected.ok_or_else(|| {
                EvalError::new(format!("enum literal '{variant}' has no type context"))
            })?
            else {
                return Err(EvalError::new(format!(
                    "enum literal '{variant}' requires an Enum context"
                )));
            };
            let index = variants
                .iter()
                .position(|candidate| candidate == variant)
                .ok_or_else(|| EvalError::new(format!("unknown enum variant '{variant}'")))?;
            Ok(PreparedNode::Enum(u16::try_from(index).map_err(|_| {
                EvalError::new(format!("enum variant '{variant}' exceeds u16"))
            })?))
        }
        Expr::Param { name } => match checked_parameter_value(table.model, params, name)? {
            ParamValue::Real { value } => Ok(PreparedNode::Real(*value)),
            ParamValue::Int { value } => Ok(PreparedNode::Int(*value)),
        },
        Expr::SelfAttr { name } => {
            let attr = find_attr(row_attrs, name)?;
            let column = snapshot.resolve_column(table.box_name(), table.table_name(), name)?;
            Ok(match &attr.ty {
                AttrType::Real => PreparedNode::RealAttr(column.real_values()?),
                AttrType::Int => PreparedNode::IntAttr(column.int_values()?),
                AttrType::Enum { .. } => PreparedNode::EnumAttr(column.enum_values()?),
                AttrType::Ref { .. } => PreparedNode::RefAttr(column.ref_values()?),
            })
        }
        Expr::Add { lhs, rhs }
        | Expr::Sub { lhs, rhs }
        | Expr::Mul { lhs, rhs }
        | Expr::Div { lhs, rhs }
        | Expr::Lt { lhs, rhs }
        | Expr::Le { lhs, rhs }
        | Expr::Gt { lhs, rhs }
        | Expr::Ge { lhs, rhs }
        | Expr::And { lhs, rhs }
        | Expr::Or { lhs, rhs } => {
            let lhs = Box::new(prepare_node(lhs, table, row_attrs, snapshot, params, None)?);
            let rhs = Box::new(prepare_node(rhs, table, row_attrs, snapshot, params, None)?);
            Ok(match expr {
                Expr::Add { .. } => PreparedNode::Add(lhs, rhs),
                Expr::Sub { .. } => PreparedNode::Sub(lhs, rhs),
                Expr::Mul { .. } => PreparedNode::Mul(lhs, rhs),
                Expr::Div { .. } => PreparedNode::Div(lhs, rhs),
                Expr::Lt { .. } => PreparedNode::Lt(lhs, rhs),
                Expr::Le { .. } => PreparedNode::Le(lhs, rhs),
                Expr::Gt { .. } => PreparedNode::Gt(lhs, rhs),
                Expr::Ge { .. } => PreparedNode::Ge(lhs, rhs),
                Expr::And { .. } => PreparedNode::And(lhs, rhs),
                Expr::Or { .. } => PreparedNode::Or(lhs, rhs),
                _ => unreachable!(),
            })
        }
        Expr::Eq { lhs, rhs } | Expr::Ne { lhs, rhs } => {
            let (lhs, rhs) = if matches!(lhs.as_ref(), Expr::Enum { .. }) {
                let rhs_type = infer_expr_type(rhs, table, row_attrs, None)?;
                (
                    prepare_node(lhs, table, row_attrs, snapshot, params, Some(&rhs_type))?,
                    prepare_node(rhs, table, row_attrs, snapshot, params, None)?,
                )
            } else {
                let lhs_type = infer_expr_type(lhs, table, row_attrs, None)?;
                (
                    prepare_node(lhs, table, row_attrs, snapshot, params, None)?,
                    prepare_node(rhs, table, row_attrs, snapshot, params, Some(&lhs_type))?,
                )
            };
            Ok(if matches!(expr, Expr::Eq { .. }) {
                PreparedNode::Eq(Box::new(lhs), Box::new(rhs))
            } else {
                PreparedNode::Ne(Box::new(lhs), Box::new(rhs))
            })
        }
        Expr::Not { expr } => Ok(PreparedNode::Not(Box::new(prepare_node(
            expr, table, row_attrs, snapshot, params, None,
        )?))),
        Expr::EnumIs { attr, variant } => {
            let declaration = find_attr(row_attrs, attr)?;
            let AttrType::Enum { variants } = &declaration.ty else {
                return Err(EvalError::new(format!(
                    "EnumIs attribute '{attr}' is not Enum-typed"
                )));
            };
            let expected = variants
                .iter()
                .position(|candidate| candidate == variant)
                .ok_or_else(|| EvalError::new(format!("unknown enum variant for '{attr}'")))?;
            let expected = u16::try_from(expected)
                .map_err(|_| EvalError::new(format!("enum attribute '{attr}' exceeds u16")))?;
            let column = snapshot.resolve_column(table.box_name(), table.table_name(), attr)?;
            Ok(PreparedNode::EnumIs(column.enum_values()?, expected))
        }
        Expr::Input { .. } | Expr::Agg { .. } => Err(EvalError::new(
            "aggregate expressions are not eligible for row tiling",
        )),
    }
}

fn infer_root_type(expr: &Expr, table: EvalTable<'_>) -> Result<RuntimeType, EvalError> {
    let expected = table.expected_type();
    let actual = infer_expr_type(expr, table, &table.schema().attrs, expected.as_ref())?;
    if let Some(expected) = expected {
        require_type(&actual, &expected)?;
    }
    Ok(actual)
}

/// Evaluates one validated expression for every row in `table`.
///
/// Only an immutable [`Snapshot`] is accepted, so same-tick writes are
/// inaccessible. Evaluation follows the expression tree exactly; `f64`
/// division by zero deliberately retains IEEE infinity/NaN semantics.
pub fn eval_column(
    expr: &Expr,
    table: EvalTable<'_>,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    agg_cache: &mut AggCache<'_, '_>,
) -> Result<ValueColumn, EvalError> {
    agg_cache.validate_scope(table, snapshot, params)?;
    let inferred = infer_root_type(expr, table)?;
    ValueColumn::try_from(eval_expr(
        expr,
        table,
        &table.schema().attrs,
        snapshot,
        params,
        agg_cache,
        Some(&inferred),
    )?)
}

/// Evaluates an eligible expression at explicit, strictly ascending rows.
///
/// `None` selects the canonical full-column fallback. Aggregate/input-dependent
/// expressions and row-fallible integer arithmetic deliberately return `None`
/// so skipped rows cannot hide their observable errors.
pub fn eval_gather(
    expr: &Expr,
    table: EvalTable<'_>,
    rows: &[usize],
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    agg_cache: &mut AggCache<'_, '_>,
) -> Result<Option<ValueColumn>, EvalError> {
    let Some(prepared) = prepare_gather(expr, table, rows, snapshot, params, agg_cache)? else {
        return Ok(None);
    };
    let values = match prepared.gather(rows)? {
        PreparedColumn::Real(values) => ValueColumn::Real(values.into_owned()),
        PreparedColumn::Int(values) => ValueColumn::Int(values.into_owned()),
        PreparedColumn::Bool(values) => ValueColumn::Bool(values.into_owned()),
        PreparedColumn::Enum(values) => ValueColumn::Enum(values.into_owned()),
        PreparedColumn::Ref(_) => {
            return Err(EvalError::new(
                "Ref-typed gathers require eval_typed_ref_gather",
            ))
        }
    };
    Ok(Some(values))
}

/// Evaluates an eligible Ref expression at explicit, strictly ascending rows.
pub fn eval_typed_ref_gather(
    expr: &Expr,
    table: EvalTable<'_>,
    rows: &[usize],
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    agg_cache: &mut AggCache<'_, '_>,
) -> Result<Option<RefColumn>, EvalError> {
    let Some(prepared) = prepare_gather(expr, table, rows, snapshot, params, agg_cache)? else {
        return Ok(None);
    };
    let target_table = prepared
        .ref_target()
        .ok_or_else(|| EvalError::new("expected Ref expression for eval_typed_ref_gather"))?;
    match prepared.gather(rows)? {
        PreparedColumn::Ref(values) => Ok(Some(RefColumn {
            target_table: target_table.to_owned(),
            values: values.into_owned(),
        })),
        _ => Err(EvalError::new(
            "Ref-typed expression did not evaluate to Ref values",
        )),
    }
}

fn prepare_gather<'state>(
    expr: &Expr,
    table: EvalTable<'_>,
    rows: &[usize],
    snapshot: &'state Snapshot<'_>,
    params: &ParamEnv,
    agg_cache: &mut AggCache<'_, '_>,
) -> Result<Option<PreparedExpr<'state>>, EvalError> {
    agg_cache.validate_scope(table, snapshot, params)?;
    let inferred = infer_root_type(expr, table)?;
    let row_count = snapshot.row_count(table.box_name(), table.table_name())?;
    validate_gather_rows(rows, row_count)?;
    prepare_row_expr_with_type(expr, table, snapshot, params, inferred)
}

fn validate_gather_rows(rows: &[usize], row_count: usize) -> Result<(), EvalError> {
    if let Some(row) = rows.iter().copied().find(|row| *row >= row_count) {
        return Err(EvalError::new(format!(
            "gather row {row} is out of bounds for {row_count} rows"
        )));
    }
    if rows.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(EvalError::new(
            "gather rows must be strictly ascending without duplicates",
        ));
    }
    Ok(())
}

/// Evaluates a Ref-typed root expression without extending [`ValueColumn`].
///
/// Ref values remain available to later runtime stages such as resource-claim
/// evaluation, while the PRD 0005 public value-column contract stays limited to
/// Real, Int, Bool, and Enum columns.
pub fn eval_ref_column(
    expr: &Expr,
    table: EvalTable<'_>,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    agg_cache: &mut AggCache<'_, '_>,
) -> Result<Vec<u32>, EvalError> {
    Ok(eval_typed_ref_column(expr, table, snapshot, params, agg_cache)?.values)
}

/// Evaluates a Ref-typed root expression and preserves its target-table type.
pub fn eval_typed_ref_column(
    expr: &Expr,
    table: EvalTable<'_>,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    agg_cache: &mut AggCache<'_, '_>,
) -> Result<RefColumn, EvalError> {
    agg_cache.validate_scope(table, snapshot, params)?;
    let inferred = infer_root_type(expr, table)?;
    let RuntimeType::Ref(target_table) = &inferred else {
        return Err(EvalError::new(format!(
            "expected Ref expression, found {}",
            inferred.name()
        )));
    };
    match eval_expr(
        expr,
        table,
        &table.schema().attrs,
        snapshot,
        params,
        agg_cache,
        Some(&inferred),
    )? {
        InternalColumn::Ref(values) => Ok(RefColumn {
            target_table: target_table.clone(),
            values,
        }),
        _ => Err(EvalError::new(
            "Ref-typed expression did not evaluate to Ref values",
        )),
    }
}

fn eval_expr(
    expr: &Expr,
    table: EvalTable<'_>,
    row_attrs: &[Attr],
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    cache: &mut AggCache<'_, '_>,
    expected: Option<&RuntimeType>,
) -> Result<InternalColumn, EvalError> {
    let row_count = snapshot.row_count(table.box_name(), table.table_name())?;
    match expr {
        Expr::Real { value } => Ok(InternalColumn::Real(vec![*value; row_count])),
        Expr::Int { value } => Ok(InternalColumn::Int(vec![*value; row_count])),
        Expr::Bool { value } => Ok(InternalColumn::Bool(vec![*value; row_count])),
        Expr::Enum { variant } => {
            let RuntimeType::Enum(variants) = expected.ok_or_else(|| {
                EvalError::new(format!("enum literal '{variant}' has no type context"))
            })?
            else {
                return Err(EvalError::new(format!(
                    "enum literal '{variant}' requires an Enum context"
                )));
            };
            let index = variants
                .iter()
                .position(|candidate| candidate == variant)
                .ok_or_else(|| EvalError::new(format!("unknown enum variant '{variant}'")))?;
            let index = u16::try_from(index)
                .map_err(|_| EvalError::new(format!("enum variant '{variant}' exceeds u16")))?;
            Ok(InternalColumn::Enum(vec![index; row_count]))
        }
        Expr::Param { name } => match checked_parameter_value(table.model, params, name)? {
            ParamValue::Real { value } => Ok(InternalColumn::Real(vec![*value; row_count])),
            ParamValue::Int { value } => Ok(InternalColumn::Int(vec![*value; row_count])),
        },
        Expr::SelfAttr { name } => eval_self_attr(table, row_attrs, snapshot, name, row_count),
        Expr::Add { lhs, rhs } => eval_arithmetic(
            Arithmetic::Add,
            lhs,
            rhs,
            table,
            row_attrs,
            snapshot,
            params,
            cache,
        ),
        Expr::Sub { lhs, rhs } => eval_arithmetic(
            Arithmetic::Sub,
            lhs,
            rhs,
            table,
            row_attrs,
            snapshot,
            params,
            cache,
        ),
        Expr::Mul { lhs, rhs } => eval_arithmetic(
            Arithmetic::Mul,
            lhs,
            rhs,
            table,
            row_attrs,
            snapshot,
            params,
            cache,
        ),
        Expr::Div { lhs, rhs } => eval_arithmetic(
            Arithmetic::Div,
            lhs,
            rhs,
            table,
            row_attrs,
            snapshot,
            params,
            cache,
        ),
        Expr::Eq { lhs, rhs } => {
            eval_equality(false, lhs, rhs, table, row_attrs, snapshot, params, cache)
        }
        Expr::Ne { lhs, rhs } => {
            eval_equality(true, lhs, rhs, table, row_attrs, snapshot, params, cache)
        }
        Expr::Lt { lhs, rhs } => eval_ordering(
            Ordering::Lt,
            lhs,
            rhs,
            table,
            row_attrs,
            snapshot,
            params,
            cache,
        ),
        Expr::Le { lhs, rhs } => eval_ordering(
            Ordering::Le,
            lhs,
            rhs,
            table,
            row_attrs,
            snapshot,
            params,
            cache,
        ),
        Expr::Gt { lhs, rhs } => eval_ordering(
            Ordering::Gt,
            lhs,
            rhs,
            table,
            row_attrs,
            snapshot,
            params,
            cache,
        ),
        Expr::Ge { lhs, rhs } => eval_ordering(
            Ordering::Ge,
            lhs,
            rhs,
            table,
            row_attrs,
            snapshot,
            params,
            cache,
        ),
        Expr::And { lhs, rhs } | Expr::Or { lhs, rhs } => {
            let lhs = eval_expr(lhs, table, row_attrs, snapshot, params, cache, None)?;
            let rhs = eval_expr(rhs, table, row_attrs, snapshot, params, cache, None)?;
            let (InternalColumn::Bool(lhs), InternalColumn::Bool(rhs)) = (lhs, rhs) else {
                return Err(EvalError::new("boolean operands did not evaluate to Bool"));
            };
            let row_count = lhs.len().min(rhs.len());
            let values = if element_wise_parallel_enabled(row_count) {
                element_wise_map(row_count, |row| {
                    if matches!(expr, Expr::And { .. }) {
                        lhs[row] && rhs[row]
                    } else {
                        lhs[row] || rhs[row]
                    }
                })
            } else {
                lhs.into_iter()
                    .zip(rhs)
                    .map(|(lhs, rhs)| {
                        if matches!(expr, Expr::And { .. }) {
                            lhs && rhs
                        } else {
                            lhs || rhs
                        }
                    })
                    .collect()
            };
            Ok(InternalColumn::Bool(values))
        }
        Expr::Not { expr } => {
            let values = eval_expr(expr, table, row_attrs, snapshot, params, cache, None)?;
            let InternalColumn::Bool(values) = values else {
                return Err(EvalError::new("Not operand did not evaluate to Bool"));
            };
            let values = if element_wise_parallel_enabled(values.len()) {
                element_wise_map(values.len(), |row| !values[row])
            } else {
                values.into_iter().map(|value| !value).collect()
            };
            Ok(InternalColumn::Bool(values))
        }
        Expr::EnumIs { attr, variant } => {
            let declaration = find_attr(row_attrs, attr)?;
            let AttrType::Enum { variants } = &declaration.ty else {
                return Err(EvalError::new(format!(
                    "EnumIs attribute '{attr}' is not Enum-typed"
                )));
            };
            let variant = variants
                .iter()
                .position(|candidate| candidate == variant)
                .ok_or_else(|| EvalError::new(format!("unknown enum variant for '{attr}'")))?;
            let variant = u16::try_from(variant)
                .map_err(|_| EvalError::new(format!("enum attribute '{attr}' exceeds u16")))?;
            if row_count == 0 {
                return Ok(InternalColumn::Bool(Vec::new()));
            }
            let column = snapshot.resolve_column(table.box_name(), table.table_name(), attr)?;
            let enum_values = column.enum_values()?;
            let values = element_wise_map(row_count, |row| enum_values[row] == variant);
            Ok(InternalColumn::Bool(values))
        }
        Expr::Input { port, agg } => {
            let declaration = table
                .model_box()
                .inputs
                .iter()
                .find(|input| input.name == *port)
                .ok_or_else(|| EvalError::new("validated input port disappeared"))?;
            let result_type = infer_agg_type(&agg.op, table, &declaration.schema)?;
            let input = snapshot.input_table(table.box_name(), port)?;
            eval_input_aggregate(input, agg, params, row_count, &result_type)
        }
        Expr::Agg {
            op,
            table: target,
            on,
            filter,
        } => eval_aggregate(op, target, on, filter, table, snapshot, params, cache),
    }
}

#[derive(Clone, Copy, Debug)]
enum InputScalar {
    Real(f64),
    Int(i64),
    Bool(bool),
    Enum(u16),
    Ref(u32),
}

fn eval_input_aggregate(
    input: &InputTable,
    agg: &Aggregate,
    params: &ParamEnv,
    result_rows: usize,
    result_type: &RuntimeType,
) -> Result<InternalColumn, EvalError> {
    let mut selected = Vec::with_capacity(input.row_count);
    for row in 0..input.row_count {
        let keep = match &agg.filter {
            Some(filter) => match eval_input_scalar(filter, input, row, params)? {
                InputScalar::Bool(value) => value,
                _ => return Err(EvalError::new("input aggregate filter is not Bool")),
            },
            None => true,
        };
        selected.push(keep);
    }
    match &agg.op {
        AggOp::Count => {
            let count = selected.iter().filter(|value| **value).count();
            let count = i64::try_from(count)
                .map_err(|_| EvalError::new("input aggregate count exceeds i64"))?;
            Ok(InternalColumn::Int(vec![count; result_rows]))
        }
        AggOp::Sum { value } => {
            let mut int_sum = 0_i64;
            let mut real_sum = 0.0;
            let mut real = matches!(result_type, RuntimeType::Real);
            for (row, keep) in selected.into_iter().enumerate() {
                if !keep {
                    continue;
                }
                match eval_input_scalar(value, input, row, params)? {
                    InputScalar::Int(value) if !real => {
                        int_sum = int_sum.checked_add(value).ok_or_else(|| {
                            EvalError::new(format!("input aggregate integer overflow at row {row}"))
                        })?;
                    }
                    InputScalar::Int(value) => real_sum += value as f64,
                    InputScalar::Real(value) => {
                        if !real {
                            real_sum = int_sum as f64;
                            real = true;
                        }
                        real_sum += value;
                    }
                    _ => return Err(EvalError::new("input aggregate Sum value is not numeric")),
                }
            }
            match result_type {
                RuntimeType::Real => Ok(InternalColumn::Real(vec![real_sum; result_rows])),
                RuntimeType::Int => Ok(InternalColumn::Int(vec![int_sum; result_rows])),
                _ => Err(EvalError::new("input aggregate Sum has non-numeric type")),
            }
        }
    }
}

fn eval_input_scalar(
    expr: &Expr,
    input: &InputTable,
    row: usize,
    params: &ParamEnv,
) -> Result<InputScalar, EvalError> {
    match expr {
        Expr::Real { value } => Ok(InputScalar::Real(*value)),
        Expr::Int { value } => Ok(InputScalar::Int(*value)),
        Expr::Bool { value } => Ok(InputScalar::Bool(*value)),
        Expr::Param { name } => match params.get(name)? {
            ParamValue::Real { value } => Ok(InputScalar::Real(*value)),
            ParamValue::Int { value } => Ok(InputScalar::Int(*value)),
        },
        Expr::SelfAttr { name } => input_scalar_attr(input, name, row),
        Expr::EnumIs { attr, variant } => {
            let declaration = find_attr(&input.schema, attr)?;
            let AttrType::Enum { variants } = &declaration.ty else {
                return Err(EvalError::new(format!(
                    "EnumIs attribute '{attr}' is not Enum-typed"
                )));
            };
            let expected = variants
                .iter()
                .position(|candidate| candidate == variant)
                .ok_or_else(|| EvalError::new(format!("unknown enum variant '{variant}'")))?;
            match input_scalar_attr(input, attr, row)? {
                InputScalar::Enum(value) => Ok(InputScalar::Bool(usize::from(value) == expected)),
                _ => unreachable!("schema and input column type disagree"),
            }
        }
        Expr::Add { lhs, rhs }
        | Expr::Sub { lhs, rhs }
        | Expr::Mul { lhs, rhs }
        | Expr::Div { lhs, rhs } => {
            let lhs = eval_input_scalar(lhs, input, row, params)?;
            let rhs = eval_input_scalar(rhs, input, row, params)?;
            input_arithmetic(expr, lhs, rhs, row)
        }
        Expr::Eq { lhs, rhs } | Expr::Ne { lhs, rhs } => {
            let (lhs_value, rhs_value) = match (lhs.as_ref(), rhs.as_ref()) {
                (Expr::Enum { variant }, _) => (
                    input_enum_literal(input, rhs, variant)?,
                    eval_input_scalar(rhs, input, row, params)?,
                ),
                (_, Expr::Enum { variant }) => (
                    eval_input_scalar(lhs, input, row, params)?,
                    input_enum_literal(input, lhs, variant)?,
                ),
                _ => (
                    eval_input_scalar(lhs, input, row, params)?,
                    eval_input_scalar(rhs, input, row, params)?,
                ),
            };
            let equal = match (lhs_value, rhs_value) {
                (InputScalar::Real(lhs), InputScalar::Real(rhs)) => lhs == rhs,
                (InputScalar::Real(lhs), InputScalar::Int(rhs)) => lhs == rhs as f64,
                (InputScalar::Int(lhs), InputScalar::Real(rhs)) => lhs as f64 == rhs,
                (InputScalar::Int(lhs), InputScalar::Int(rhs)) => lhs == rhs,
                (InputScalar::Bool(lhs), InputScalar::Bool(rhs)) => lhs == rhs,
                (InputScalar::Enum(lhs), InputScalar::Enum(rhs)) => lhs == rhs,
                (InputScalar::Ref(lhs), InputScalar::Ref(rhs)) => lhs == rhs,
                _ => {
                    return Err(EvalError::new(
                        "input equality operands have incompatible types",
                    ))
                }
            };
            Ok(InputScalar::Bool(if matches!(expr, Expr::Ne { .. }) {
                !equal
            } else {
                equal
            }))
        }
        Expr::Lt { lhs, rhs }
        | Expr::Le { lhs, rhs }
        | Expr::Gt { lhs, rhs }
        | Expr::Ge { lhs, rhs } => {
            let lhs = input_number(eval_input_scalar(lhs, input, row, params)?)?;
            let rhs = input_number(eval_input_scalar(rhs, input, row, params)?)?;
            let value = match expr {
                Expr::Lt { .. } => lhs < rhs,
                Expr::Le { .. } => lhs <= rhs,
                Expr::Gt { .. } => lhs > rhs,
                Expr::Ge { .. } => lhs >= rhs,
                _ => unreachable!(),
            };
            Ok(InputScalar::Bool(value))
        }
        Expr::And { lhs, rhs } | Expr::Or { lhs, rhs } => {
            let InputScalar::Bool(lhs) = eval_input_scalar(lhs, input, row, params)? else {
                return Err(EvalError::new("input boolean lhs is not Bool"));
            };
            let InputScalar::Bool(rhs) = eval_input_scalar(rhs, input, row, params)? else {
                return Err(EvalError::new("input boolean rhs is not Bool"));
            };
            Ok(InputScalar::Bool(if matches!(expr, Expr::And { .. }) {
                lhs && rhs
            } else {
                lhs || rhs
            }))
        }
        Expr::Not { expr } => match eval_input_scalar(expr, input, row, params)? {
            InputScalar::Bool(value) => Ok(InputScalar::Bool(!value)),
            _ => Err(EvalError::new("input Not operand is not Bool")),
        },
        Expr::Enum { .. } => Err(EvalError::new(
            "bare enum literals are not supported in input aggregates; use EnumIs",
        )),
        Expr::Input { .. } | Expr::Agg { .. } => Err(EvalError::new(
            "nested aggregates are not supported inside input table aggregates",
        )),
    }
}

fn input_enum_literal(
    input: &InputTable,
    context: &Expr,
    variant: &str,
) -> Result<InputScalar, EvalError> {
    let Expr::SelfAttr { name } = context else {
        return Err(EvalError::new(
            "input enum literal requires a direct Enum attribute context",
        ));
    };
    let declaration = find_attr(&input.schema, name)?;
    let AttrType::Enum { variants } = &declaration.ty else {
        return Err(EvalError::new("input enum literal context is not Enum"));
    };
    let index = variants
        .iter()
        .position(|candidate| candidate == variant)
        .ok_or_else(|| EvalError::new(format!("unknown enum variant '{variant}'")))?;
    Ok(InputScalar::Enum(u16::try_from(index).map_err(|_| {
        EvalError::new("input enum variant index exceeds u16")
    })?))
}

fn input_scalar_attr(input: &InputTable, name: &str, row: usize) -> Result<InputScalar, EvalError> {
    match input
        .column(name)
        .ok_or_else(|| EvalError::new(format!("input table has no column '{name}'")))?
    {
        ColumnData::Real(values) => values.get(row).copied().map(InputScalar::Real),
        ColumnData::Int(values) => values.get(row).copied().map(InputScalar::Int),
        ColumnData::Enum(values) => values.get(row).copied().map(InputScalar::Enum),
        ColumnData::Ref(values) => values.get(row).copied().map(InputScalar::Ref),
    }
    .ok_or_else(|| EvalError::new(format!("input column '{name}' has no row {row}")))
}

fn input_number(value: InputScalar) -> Result<f64, EvalError> {
    match value {
        InputScalar::Real(value) => Ok(value),
        InputScalar::Int(value) => Ok(value as f64),
        _ => Err(EvalError::new("input ordered operand is not numeric")),
    }
}

fn input_arithmetic(
    expr: &Expr,
    lhs: InputScalar,
    rhs: InputScalar,
    row: usize,
) -> Result<InputScalar, EvalError> {
    if matches!(expr, Expr::Div { .. })
        || matches!(lhs, InputScalar::Real(_))
        || matches!(rhs, InputScalar::Real(_))
    {
        let lhs = input_number(lhs)?;
        let rhs = input_number(rhs)?;
        return Ok(InputScalar::Real(match expr {
            Expr::Add { .. } => lhs + rhs,
            Expr::Sub { .. } => lhs - rhs,
            Expr::Mul { .. } => lhs * rhs,
            Expr::Div { .. } => lhs / rhs,
            _ => unreachable!(),
        }));
    }
    let (InputScalar::Int(lhs), InputScalar::Int(rhs)) = (lhs, rhs) else {
        return Err(EvalError::new("input arithmetic operands are not numeric"));
    };
    let value = match expr {
        Expr::Add { .. } => lhs.checked_add(rhs),
        Expr::Sub { .. } => lhs.checked_sub(rhs),
        Expr::Mul { .. } => lhs.checked_mul(rhs),
        _ => unreachable!(),
    }
    .ok_or_else(|| EvalError::new(format!("input integer arithmetic overflow at row {row}")))?;
    Ok(InputScalar::Int(value))
}

fn eval_self_attr(
    table: EvalTable<'_>,
    row_attrs: &[Attr],
    snapshot: &Snapshot<'_>,
    name: &str,
    row_count: usize,
) -> Result<InternalColumn, EvalError> {
    let attr = find_attr(row_attrs, name)?;
    match &attr.ty {
        AttrType::Real if row_count == 0 => Ok(InternalColumn::Real(Vec::new())),
        AttrType::Real => {
            let column = snapshot.resolve_column(table.box_name(), table.table_name(), name)?;
            let values = column.real_values()?;
            Ok(InternalColumn::Real(element_wise_map(row_count, |row| {
                values[row]
            })))
        }
        AttrType::Int if row_count == 0 => Ok(InternalColumn::Int(Vec::new())),
        AttrType::Int => {
            let column = snapshot.resolve_column(table.box_name(), table.table_name(), name)?;
            let values = column.int_values()?;
            Ok(InternalColumn::Int(element_wise_map(row_count, |row| {
                values[row]
            })))
        }
        AttrType::Enum { .. } if row_count == 0 => Ok(InternalColumn::Enum(Vec::new())),
        AttrType::Enum { .. } => {
            let column = snapshot.resolve_column(table.box_name(), table.table_name(), name)?;
            let values = column.enum_values()?;
            Ok(InternalColumn::Enum(element_wise_map(row_count, |row| {
                values[row]
            })))
        }
        AttrType::Ref { .. } if row_count == 0 => Ok(InternalColumn::Ref(Vec::new())),
        AttrType::Ref { .. } => {
            let column = snapshot.resolve_column(table.box_name(), table.table_name(), name)?;
            let values = column.ref_values()?;
            Ok(InternalColumn::Ref(element_wise_map(row_count, |row| {
                values[row]
            })))
        }
    }
}

#[derive(Clone, Copy)]
enum Arithmetic {
    Add,
    Sub,
    Mul,
    Div,
}

#[allow(clippy::too_many_arguments)]
fn eval_arithmetic(
    operation: Arithmetic,
    lhs: &Expr,
    rhs: &Expr,
    table: EvalTable<'_>,
    row_attrs: &[Attr],
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    cache: &mut AggCache<'_, '_>,
) -> Result<InternalColumn, EvalError> {
    let lhs = eval_expr(lhs, table, row_attrs, snapshot, params, cache, None)?;
    let rhs = eval_expr(rhs, table, row_attrs, snapshot, params, cache, None)?;
    if matches!(operation, Arithmetic::Div)
        || matches!(lhs, InternalColumn::Real(_))
        || matches!(rhs, InternalColumn::Real(_))
    {
        let lhs = numeric_as_real(lhs)?;
        let rhs = numeric_as_real(rhs)?;
        let row_count = lhs.len().min(rhs.len());
        let values = if element_wise_parallel_enabled(row_count) {
            element_wise_map(row_count, |row| match operation {
                Arithmetic::Add => lhs[row] + rhs[row],
                Arithmetic::Sub => lhs[row] - rhs[row],
                Arithmetic::Mul => lhs[row] * rhs[row],
                Arithmetic::Div => lhs[row] / rhs[row],
            })
        } else {
            lhs.into_iter()
                .zip(rhs)
                .map(|(lhs, rhs)| match operation {
                    Arithmetic::Add => lhs + rhs,
                    Arithmetic::Sub => lhs - rhs,
                    Arithmetic::Mul => lhs * rhs,
                    Arithmetic::Div => lhs / rhs,
                })
                .collect()
        };
        return Ok(InternalColumn::Real(values));
    }
    let (InternalColumn::Int(lhs), InternalColumn::Int(rhs)) = (lhs, rhs) else {
        return Err(EvalError::new("arithmetic operands are not numeric"));
    };
    let row_count = lhs.len().min(rhs.len());
    let values = if element_wise_parallel_enabled(row_count) {
        element_wise_map_with_initializer(
            row_count,
            || Ok(0_i64),
            |row| {
                let value = match operation {
                    Arithmetic::Add => lhs[row].checked_add(rhs[row]),
                    Arithmetic::Sub => lhs[row].checked_sub(rhs[row]),
                    Arithmetic::Mul => lhs[row].checked_mul(rhs[row]),
                    Arithmetic::Div => unreachable!("division promotes to Real"),
                };
                value.ok_or_else(|| {
                    EvalError::new(format!("integer arithmetic overflow at row {row}"))
                })
            },
        )
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
    } else {
        lhs.into_iter()
            .zip(rhs)
            .enumerate()
            .map(|(row, (lhs, rhs))| {
                let value = match operation {
                    Arithmetic::Add => lhs.checked_add(rhs),
                    Arithmetic::Sub => lhs.checked_sub(rhs),
                    Arithmetic::Mul => lhs.checked_mul(rhs),
                    Arithmetic::Div => unreachable!("division promotes to Real"),
                };
                value.ok_or_else(|| {
                    EvalError::new(format!("integer arithmetic overflow at row {row}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(InternalColumn::Int(values))
}

#[allow(clippy::too_many_arguments)]
fn eval_equality(
    negate: bool,
    lhs_expr: &Expr,
    rhs_expr: &Expr,
    table: EvalTable<'_>,
    row_attrs: &[Attr],
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    cache: &mut AggCache<'_, '_>,
) -> Result<InternalColumn, EvalError> {
    let (lhs, rhs) = if matches!(lhs_expr, Expr::Enum { .. }) {
        let rhs_type = infer_expr_type(rhs_expr, table, row_attrs, None)?;
        let rhs = eval_expr(rhs_expr, table, row_attrs, snapshot, params, cache, None)?;
        let lhs = eval_expr(
            lhs_expr,
            table,
            row_attrs,
            snapshot,
            params,
            cache,
            Some(&rhs_type),
        )?;
        (lhs, rhs)
    } else {
        let lhs_type = infer_expr_type(lhs_expr, table, row_attrs, None)?;
        let lhs = eval_expr(lhs_expr, table, row_attrs, snapshot, params, cache, None)?;
        let rhs = eval_expr(
            rhs_expr,
            table,
            row_attrs,
            snapshot,
            params,
            cache,
            Some(&lhs_type),
        )?;
        (lhs, rhs)
    };
    let equal = equal_columns(lhs, rhs)?;
    let values = if element_wise_parallel_enabled(equal.len()) {
        element_wise_map(
            equal.len(),
            |row| {
                if negate {
                    !equal[row]
                } else {
                    equal[row]
                }
            },
        )
    } else {
        equal
            .into_iter()
            .map(|value| if negate { !value } else { value })
            .collect()
    };
    Ok(InternalColumn::Bool(values))
}

fn equal_columns(lhs: InternalColumn, rhs: InternalColumn) -> Result<Vec<bool>, EvalError> {
    if let (InternalColumn::Int(lhs), InternalColumn::Int(rhs)) = (&lhs, &rhs) {
        let row_count = lhs.len().min(rhs.len());
        return Ok(if element_wise_parallel_enabled(row_count) {
            element_wise_map(row_count, |row| lhs[row] == rhs[row])
        } else {
            lhs.iter().zip(rhs).map(|(lhs, rhs)| lhs == rhs).collect()
        });
    }
    if matches!(lhs, InternalColumn::Real(_) | InternalColumn::Int(_))
        && matches!(rhs, InternalColumn::Real(_) | InternalColumn::Int(_))
    {
        let lhs = numeric_as_real(lhs)?;
        let rhs = numeric_as_real(rhs)?;
        let row_count = lhs.len().min(rhs.len());
        return Ok(if element_wise_parallel_enabled(row_count) {
            element_wise_map(row_count, |row| lhs[row] == rhs[row])
        } else {
            lhs.into_iter()
                .zip(rhs)
                .map(|(lhs, rhs)| lhs == rhs)
                .collect()
        });
    }
    let values = match (lhs, rhs) {
        (InternalColumn::Bool(lhs), InternalColumn::Bool(rhs)) => {
            let row_count = lhs.len().min(rhs.len());
            if element_wise_parallel_enabled(row_count) {
                element_wise_map(row_count, |row| lhs[row] == rhs[row])
            } else {
                lhs.iter().zip(&rhs).map(|(lhs, rhs)| lhs == rhs).collect()
            }
        }
        (InternalColumn::Enum(lhs), InternalColumn::Enum(rhs)) => {
            let row_count = lhs.len().min(rhs.len());
            if element_wise_parallel_enabled(row_count) {
                element_wise_map(row_count, |row| lhs[row] == rhs[row])
            } else {
                lhs.iter().zip(&rhs).map(|(lhs, rhs)| lhs == rhs).collect()
            }
        }
        (InternalColumn::Ref(lhs), InternalColumn::Ref(rhs)) => {
            let row_count = lhs.len().min(rhs.len());
            if element_wise_parallel_enabled(row_count) {
                element_wise_map(row_count, |row| lhs[row] == rhs[row])
            } else {
                lhs.iter().zip(&rhs).map(|(lhs, rhs)| lhs == rhs).collect()
            }
        }
        _ => return Err(EvalError::new("equality operands have incompatible types")),
    };
    Ok(values)
}

#[derive(Clone, Copy)]
enum Ordering {
    Lt,
    Le,
    Gt,
    Ge,
}

#[allow(clippy::too_many_arguments)]
fn eval_ordering(
    operation: Ordering,
    lhs: &Expr,
    rhs: &Expr,
    table: EvalTable<'_>,
    row_attrs: &[Attr],
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    cache: &mut AggCache<'_, '_>,
) -> Result<InternalColumn, EvalError> {
    let lhs = eval_expr(lhs, table, row_attrs, snapshot, params, cache, None)?;
    let rhs = eval_expr(rhs, table, row_attrs, snapshot, params, cache, None)?;
    if let (InternalColumn::Int(lhs), InternalColumn::Int(rhs)) = (&lhs, &rhs) {
        let row_count = lhs.len().min(rhs.len());
        let values = if element_wise_parallel_enabled(row_count) {
            element_wise_map(row_count, |row| match operation {
                Ordering::Lt => lhs[row] < rhs[row],
                Ordering::Le => lhs[row] <= rhs[row],
                Ordering::Gt => lhs[row] > rhs[row],
                Ordering::Ge => lhs[row] >= rhs[row],
            })
        } else {
            lhs.iter()
                .zip(rhs)
                .map(|(lhs, rhs)| match operation {
                    Ordering::Lt => lhs < rhs,
                    Ordering::Le => lhs <= rhs,
                    Ordering::Gt => lhs > rhs,
                    Ordering::Ge => lhs >= rhs,
                })
                .collect()
        };
        return Ok(InternalColumn::Bool(values));
    }
    let lhs = numeric_as_real(lhs)?;
    let rhs = numeric_as_real(rhs)?;
    let row_count = lhs.len().min(rhs.len());
    let values = if element_wise_parallel_enabled(row_count) {
        element_wise_map(row_count, |row| match operation {
            Ordering::Lt => lhs[row] < rhs[row],
            Ordering::Le => lhs[row] <= rhs[row],
            Ordering::Gt => lhs[row] > rhs[row],
            Ordering::Ge => lhs[row] >= rhs[row],
        })
    } else {
        lhs.into_iter()
            .zip(rhs)
            .map(|(lhs, rhs)| match operation {
                Ordering::Lt => lhs < rhs,
                Ordering::Le => lhs <= rhs,
                Ordering::Gt => lhs > rhs,
                Ordering::Ge => lhs >= rhs,
            })
            .collect()
    };
    Ok(InternalColumn::Bool(values))
}

fn numeric_as_real(column: InternalColumn) -> Result<Vec<f64>, EvalError> {
    match column {
        InternalColumn::Real(values) => Ok(values),
        InternalColumn::Int(values) if element_wise_parallel_enabled(values.len()) => {
            Ok(element_wise_map(values.len(), |row| values[row] as f64))
        }
        InternalColumn::Int(values) => Ok(values.into_iter().map(|value| value as f64).collect()),
        _ => Err(EvalError::new(
            "numeric expression did not evaluate to Real or Int",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_aggregate(
    op: &AggOp,
    target_name: &str,
    on: &AggJoin,
    filter: &Expr,
    query: EvalTable<'_>,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    cache: &mut AggCache<'_, '_>,
) -> Result<InternalColumn, EvalError> {
    let key = AggregateKey {
        box_name: query.box_name().to_owned(),
        table: target_name.to_owned(),
        op: op.clone(),
        on: on.clone(),
        filter: filter.clone(),
    };
    let accumulator = if let Some(entry) = cache.entries.iter().find(|entry| entry.key == key) {
        entry.values.clone()
    } else {
        let values = build_aggregate(op, target_name, on, filter, query, snapshot, params, cache)?;
        cache.entries.push(CacheEntry {
            key,
            values: values.clone(),
        });
        cache.build_count += 1;
        values
    };

    let query_rows = snapshot.row_count(query.box_name(), query.table_name())?;
    match accumulator {
        Accumulator::Int(groups) => {
            let references: &[u32] = if query_rows == 0 {
                &[]
            } else {
                let column = snapshot.resolve_column(
                    query.box_name(),
                    query.table_name(),
                    &on.self_fk_attr,
                )?;
                column.ref_values()?
            };
            let mut values = Vec::with_capacity(query_rows);
            for reference in references {
                let group = *reference as usize;
                values.push(*groups.get(group).ok_or_else(|| {
                    EvalError::new(format!(
                        "aggregate broadcast group {group} is out of bounds"
                    ))
                })?);
            }
            Ok(InternalColumn::Int(values))
        }
        Accumulator::Real(groups) => {
            let references: &[u32] = if query_rows == 0 {
                &[]
            } else {
                let column = snapshot.resolve_column(
                    query.box_name(),
                    query.table_name(),
                    &on.self_fk_attr,
                )?;
                column.ref_values()?
            };
            let mut values = Vec::with_capacity(query_rows);
            for reference in references {
                let group = *reference as usize;
                values.push(*groups.get(group).ok_or_else(|| {
                    EvalError::new(format!(
                        "aggregate broadcast group {group} is out of bounds"
                    ))
                })?);
            }
            Ok(InternalColumn::Real(values))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_aggregate(
    op: &AggOp,
    target_name: &str,
    on: &AggJoin,
    filter: &Expr,
    query: EvalTable<'_>,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    cache: &mut AggCache<'_, '_>,
) -> Result<Accumulator, EvalError> {
    let target = query.target(target_name)?;
    let target_fk = find_attr(&target.schema().attrs, &on.fk_attr)?;
    let AttrType::Ref { table: group_table } = &target_fk.ty else {
        return Err(EvalError::new("aggregate target join is not Ref-typed"));
    };
    let group_count = snapshot.row_count(query.box_name(), group_table)?;
    let filter = eval_expr(
        filter,
        target,
        &target.schema().attrs,
        snapshot,
        params,
        cache,
        Some(&RuntimeType::Bool),
    )?;
    let InternalColumn::Bool(filter) = filter else {
        return Err(EvalError::new("aggregate filter did not evaluate to Bool"));
    };

    match op {
        AggOp::Count => {
            let mut groups = vec![0_i64; group_count];
            let mut references = None;
            for (row, include) in filter.iter().copied().enumerate() {
                if include {
                    let reference_values = if let Some(values) = references {
                        values
                    } else {
                        let column = snapshot.resolve_column(
                            target.box_name(),
                            target.table_name(),
                            &on.fk_attr,
                        )?;
                        let values = column.ref_values()?;
                        references = Some(values);
                        values
                    };
                    let group = reference_values[row] as usize;
                    groups[group] = groups[group].checked_add(1).ok_or_else(|| {
                        EvalError::new(format!("aggregate Count overflow in group {group}"))
                    })?;
                }
            }
            Ok(Accumulator::Int(groups))
        }
        AggOp::Sum { value } => {
            let values = eval_expr(
                value,
                target,
                &target.schema().attrs,
                snapshot,
                params,
                cache,
                None,
            )?;
            match values {
                InternalColumn::Int(values) => {
                    let mut groups = vec![0_i64; group_count];
                    let mut references = None;
                    for (row, (include, value)) in filter.iter().copied().zip(values).enumerate() {
                        if include {
                            let reference_values = if let Some(values) = references {
                                values
                            } else {
                                let column = snapshot.resolve_column(
                                    target.box_name(),
                                    target.table_name(),
                                    &on.fk_attr,
                                )?;
                                let values = column.ref_values()?;
                                references = Some(values);
                                values
                            };
                            let group = reference_values[row] as usize;
                            groups[group] = groups[group].checked_add(value).ok_or_else(|| {
                                EvalError::new(format!("integer Sum overflow in group {group}"))
                            })?;
                        }
                    }
                    Ok(Accumulator::Int(groups))
                }
                InternalColumn::Real(values) => {
                    let mut groups = vec![0.0_f64; group_count];
                    let mut references = None;
                    // This ascending target-row pass is the canonical CPU reduction order.
                    for (row, (include, value)) in filter.iter().copied().zip(values).enumerate() {
                        if include {
                            let reference_values = if let Some(values) = references {
                                values
                            } else {
                                let column = snapshot.resolve_column(
                                    target.box_name(),
                                    target.table_name(),
                                    &on.fk_attr,
                                )?;
                                let values = column.ref_values()?;
                                references = Some(values);
                                values
                            };
                            let group = reference_values[row] as usize;
                            groups[group] += value;
                        }
                    }
                    Ok(Accumulator::Real(groups))
                }
                _ => Err(EvalError::new("Sum value did not evaluate to numeric")),
            }
        }
    }
}

fn infer_expr_type(
    expr: &Expr,
    table: EvalTable<'_>,
    row_attrs: &[Attr],
    expected: Option<&RuntimeType>,
) -> Result<RuntimeType, EvalError> {
    match expr {
        Expr::Real { .. } => Ok(RuntimeType::Real),
        Expr::Int { .. } => Ok(RuntimeType::Int),
        Expr::Bool { .. } => Ok(RuntimeType::Bool),
        Expr::Enum { variant } => match expected {
            Some(RuntimeType::Enum(variants)) if variants.contains(variant) => {
                Ok(RuntimeType::Enum(variants.clone()))
            }
            Some(RuntimeType::Enum(_)) => {
                Err(EvalError::new(format!("unknown enum variant '{variant}'")))
            }
            _ => Err(EvalError::new(format!(
                "enum literal '{variant}' requires an Enum context"
            ))),
        },
        Expr::Param { name } => {
            let declaration = table
                .model
                .model()
                .params
                .iter()
                .find(|param| param.name == *name)
                .ok_or_else(|| EvalError::new(format!("unresolved parameter '{name}'")))?;
            Ok(match declaration.ty {
                ParamType::Real => RuntimeType::Real,
                ParamType::Int => RuntimeType::Int,
            })
        }
        Expr::SelfAttr { name } => Ok(RuntimeType::from(&find_attr(row_attrs, name)?.ty)),
        Expr::Add { lhs, rhs } | Expr::Sub { lhs, rhs } | Expr::Mul { lhs, rhs } => {
            infer_numeric_binary(lhs, rhs, table, row_attrs, false)
        }
        Expr::Div { lhs, rhs } => infer_numeric_binary(lhs, rhs, table, row_attrs, true),
        Expr::Eq { lhs, rhs } | Expr::Ne { lhs, rhs } => {
            let (lhs_type, rhs_type) = if matches!(lhs.as_ref(), Expr::Enum { .. }) {
                let rhs_type = infer_expr_type(rhs, table, row_attrs, None)?;
                let lhs_type = infer_expr_type(lhs, table, row_attrs, Some(&rhs_type))?;
                (lhs_type, rhs_type)
            } else {
                let lhs_type = infer_expr_type(lhs, table, row_attrs, None)?;
                let rhs_type = infer_expr_type(rhs, table, row_attrs, Some(&lhs_type))?;
                (lhs_type, rhs_type)
            };
            if lhs_type != rhs_type && !(lhs_type.is_numeric() && rhs_type.is_numeric()) {
                return Err(EvalError::new("equality operands have incompatible types"));
            }
            Ok(RuntimeType::Bool)
        }
        Expr::Lt { lhs, rhs }
        | Expr::Le { lhs, rhs }
        | Expr::Gt { lhs, rhs }
        | Expr::Ge { lhs, rhs } => {
            let lhs = infer_expr_type(lhs, table, row_attrs, None)?;
            let rhs = infer_expr_type(rhs, table, row_attrs, None)?;
            if !(lhs.is_numeric() && rhs.is_numeric()) {
                return Err(EvalError::new(
                    "ordered comparison operands must be numeric",
                ));
            }
            Ok(RuntimeType::Bool)
        }
        Expr::And { lhs, rhs } | Expr::Or { lhs, rhs } => {
            require_type(
                &infer_expr_type(lhs, table, row_attrs, Some(&RuntimeType::Bool))?,
                &RuntimeType::Bool,
            )?;
            require_type(
                &infer_expr_type(rhs, table, row_attrs, Some(&RuntimeType::Bool))?,
                &RuntimeType::Bool,
            )?;
            Ok(RuntimeType::Bool)
        }
        Expr::Not { expr } => {
            require_type(
                &infer_expr_type(expr, table, row_attrs, Some(&RuntimeType::Bool))?,
                &RuntimeType::Bool,
            )?;
            Ok(RuntimeType::Bool)
        }
        Expr::EnumIs { attr, variant } => {
            let declaration = find_attr(row_attrs, attr)?;
            match &declaration.ty {
                AttrType::Enum { variants } if variants.contains(variant) => Ok(RuntimeType::Bool),
                AttrType::Enum { .. } => {
                    Err(EvalError::new(format!("unknown enum variant '{variant}'")))
                }
                _ => Err(EvalError::new(format!(
                    "EnumIs attribute '{attr}' is not Enum-typed"
                ))),
            }
        }
        Expr::Input { port, agg } => {
            let input = table
                .model_box()
                .inputs
                .iter()
                .find(|input| input.name == *port)
                .ok_or_else(|| EvalError::new(format!("unresolved input port '{port}'")))?;
            if let Some(filter) = &agg.filter {
                require_type(
                    &infer_expr_type(filter, table, &input.schema, Some(&RuntimeType::Bool))?,
                    &RuntimeType::Bool,
                )?;
            }
            infer_agg_type(&agg.op, table, &input.schema)
        }
        Expr::Agg {
            op,
            table: target,
            on,
            filter,
        } => {
            let target = table.target(target)?;
            let target_fk = find_attr(&target.schema().attrs, &on.fk_attr)?;
            let self_fk = find_attr(row_attrs, &on.self_fk_attr)?;
            match (&target_fk.ty, &self_fk.ty) {
                (AttrType::Ref { table: lhs }, AttrType::Ref { table: rhs }) if lhs == rhs => {}
                _ => {
                    return Err(EvalError::new(
                        "aggregate joins must be matching Ref attributes",
                    ))
                }
            }
            require_type(
                &infer_expr_type(
                    filter,
                    target,
                    &target.schema().attrs,
                    Some(&RuntimeType::Bool),
                )?,
                &RuntimeType::Bool,
            )?;
            infer_agg_type(op, target, &target.schema().attrs)
        }
    }
}

fn infer_numeric_binary(
    lhs: &Expr,
    rhs: &Expr,
    table: EvalTable<'_>,
    row_attrs: &[Attr],
    division: bool,
) -> Result<RuntimeType, EvalError> {
    let lhs = infer_expr_type(lhs, table, row_attrs, None)?;
    let rhs = infer_expr_type(rhs, table, row_attrs, None)?;
    if !(lhs.is_numeric() && rhs.is_numeric()) {
        return Err(EvalError::new("arithmetic operands must be numeric"));
    }
    if division || lhs == RuntimeType::Real || rhs == RuntimeType::Real {
        Ok(RuntimeType::Real)
    } else {
        Ok(RuntimeType::Int)
    }
}

fn infer_agg_type(
    op: &AggOp,
    table: EvalTable<'_>,
    row_attrs: &[Attr],
) -> Result<RuntimeType, EvalError> {
    match op {
        AggOp::Count => Ok(RuntimeType::Int),
        AggOp::Sum { value } => {
            let value_type = infer_expr_type(value, table, row_attrs, None)?;
            if value_type.is_numeric() {
                Ok(value_type)
            } else {
                Err(EvalError::new("Sum value must be numeric"))
            }
        }
    }
}

fn require_type(actual: &RuntimeType, expected: &RuntimeType) -> Result<(), EvalError> {
    if actual == expected {
        Ok(())
    } else {
        Err(EvalError::new(format!(
            "expected {}, found {}",
            expected.name(),
            actual.name()
        )))
    }
}

fn find_attr<'a>(attrs: &'a [Attr], name: &str) -> Result<&'a Attr, EvalError> {
    attrs
        .iter()
        .find(|attr| attr.name == name)
        .ok_or_else(|| EvalError::new(format!("unknown attribute '{name}'")))
}

fn parameter_value_matches(parameter_type: ParamType, value: &ParamValue) -> bool {
    matches!(
        (parameter_type, value),
        (ParamType::Real, ParamValue::Real { .. }) | (ParamType::Int, ParamValue::Int { .. })
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ColumnInit, StateStore, TableInit};
    use sembla_ir::{validate, Box as ModelBox, Model};

    #[test]
    fn input_enum_equality_accepts_literal_on_the_left() {
        let model = validate(Model {
            name: "input-enum".to_owned(),
            dt: 1.0,
            params: Vec::new(),
            boxes: vec![ModelBox {
                name: "box".to_owned(),
                tables: Vec::new(),
                transitions: Vec::new(),
                inputs: Vec::new(),
                outputs: Vec::new(),
                views: Vec::new(),
                grouped_views: Vec::new(),
            }],
            wires: Vec::new(),
            summaries: Vec::new(),
        })
        .unwrap();
        let params = ParamEnv::defaults(&model);
        let input = InputTable {
            box_name: "box".to_owned(),
            port_name: "events".to_owned(),
            schema: vec![Attr {
                name: "status".to_owned(),
                ty: AttrType::Enum {
                    variants: vec!["Off".to_owned(), "On".to_owned()],
                },
            }],
            row_count: 1,
            columns: vec![ColumnData::Enum(vec![1])],
        };
        let expression = Expr::Eq {
            lhs: Box::new(Expr::Enum {
                variant: "On".to_owned(),
            }),
            rhs: Box::new(Expr::SelfAttr {
                name: "status".to_owned(),
            }),
        };

        assert!(matches!(
            eval_input_scalar(&expression, &input, 0, &params),
            Ok(InputScalar::Bool(true))
        ));
    }

    fn parallel_fixture(row_count: usize) -> (ValidatedModel, StateStore) {
        let model = validate(Model {
            name: "parallel-eval".to_owned(),
            dt: 1.0,
            params: Vec::new(),
            boxes: vec![ModelBox {
                name: "world".to_owned(),
                tables: vec![
                    Table {
                        name: "Group".to_owned(),
                        size_hint: 4,
                        attrs: Vec::new(),
                    },
                    Table {
                        name: "Person".to_owned(),
                        size_hint: row_count as u64,
                        attrs: vec![
                            Attr {
                                name: "x".to_owned(),
                                ty: AttrType::Real,
                            },
                            Attr {
                                name: "group".to_owned(),
                                ty: AttrType::Ref {
                                    table: "Group".to_owned(),
                                },
                            },
                        ],
                    },
                ],
                transitions: Vec::new(),
                inputs: Vec::new(),
                outputs: Vec::new(),
                views: Vec::new(),
                grouped_views: Vec::new(),
            }],
            wires: Vec::new(),
            summaries: Vec::new(),
        })
        .unwrap();
        let values = (0..row_count)
            .map(|row| (row as f64 - 17_000.0) / 7.0)
            .collect();
        let groups = (0..row_count).map(|row| (row % 4) as u32).collect();
        let state = StateStore::new(
            &model,
            vec![
                TableInit::new("world", "Group", 4, Vec::new()),
                TableInit::new(
                    "world",
                    "Person",
                    row_count,
                    vec![
                        ColumnInit::new("x", ColumnData::Real(values)),
                        ColumnInit::new("group", ColumnData::Ref(groups)),
                    ],
                ),
            ],
        )
        .unwrap();
        (model, state)
    }

    fn evaluate_real_bits(
        expr: &Expr,
        model: &ValidatedModel,
        state: &StateStore,
        workers: usize,
        tile_rows: usize,
    ) -> Vec<u64> {
        with_test_tick_tiles(workers, tile_rows, 0, || {
            let params = ParamEnv::defaults(model);
            let snapshot = state.snapshot();
            let table = EvalTable::new(model, "world", "Person").unwrap();
            if let Some(prepared) = prepare_row_expr(expr, table, &snapshot, &params).unwrap() {
                let row_count = snapshot.row_count("world", "Person").unwrap();
                let mut bits = Vec::with_capacity(row_count);
                for start in (0..row_count).step_by(tick_tile_rows()) {
                    let end = (start + tick_tile_rows()).min(row_count);
                    let PreparedColumn::Real(values) = prepared.tile(start, end).unwrap() else {
                        panic!("prepared test expression must be Real");
                    };
                    bits.extend(values.iter().copied().map(f64::to_bits));
                }
                bits
            } else {
                let mut cache = AggCache::new(model, &snapshot, &params);
                let ValueColumn::Real(values) =
                    eval_column(expr, table, &snapshot, &params, &mut cache).unwrap()
                else {
                    panic!("fallback test expression must be Real");
                };
                values.into_iter().map(f64::to_bits).collect()
            }
        })
    }

    #[test]
    fn fixed_row_tiles_depend_only_on_row_index_and_tile_size() {
        let row_count = 10_003;
        for tile_rows in [257, 1_024, 4_093] {
            let expected = (0..row_count)
                .step_by(tile_rows)
                .map(|start| (start, (start + tile_rows).min(row_count)))
                .collect::<Vec<_>>();
            for workers in [1, 2, 4] {
                let actual = with_test_tick_tiles(workers, tile_rows, 0, || {
                    (0..row_count)
                        .step_by(tick_tile_rows())
                        .map(|start| (start, (start + tick_tile_rows()).min(row_count)))
                        .collect::<Vec<_>>()
                });
                assert_eq!(actual, expected, "boundaries changed at {workers} workers");
            }
        }
    }

    #[test]
    fn real_arithmetic_chain_is_bit_identical_across_workers_and_tile_sizes() {
        let (model, state) = parallel_fixture(262_144);
        let expr = Expr::Div {
            lhs: Box::new(Expr::Mul {
                lhs: Box::new(Expr::Add {
                    lhs: Box::new(Expr::SelfAttr {
                        name: "x".to_owned(),
                    }),
                    rhs: Box::new(Expr::Real { value: 0.25 }),
                }),
                rhs: Box::new(Expr::Sub {
                    lhs: Box::new(Expr::SelfAttr {
                        name: "x".to_owned(),
                    }),
                    rhs: Box::new(Expr::Real { value: -0.5 }),
                }),
            }),
            rhs: Box::new(Expr::Real { value: 3.0 }),
        };
        let serial = evaluate_real_bits(&expr, &model, &state, 1, 257);
        for workers in [1, 2, 4] {
            for tile_rows in [257, 1_024, 4_093] {
                assert_eq!(
                    evaluate_real_bits(&expr, &model, &state, workers, tile_rows),
                    serial,
                    "Real arithmetic changed at {workers} workers and {tile_rows} rows/tile"
                );
            }
        }
    }

    #[test]
    fn real_aggregate_reduction_stays_bit_identical_and_sequential() {
        let (model, state) = parallel_fixture(262_144);
        let expr = Expr::Agg {
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
        };
        let params = ParamEnv::defaults(&model);
        let snapshot = state.snapshot();
        assert!(
            prepare_row_expr(
                &expr,
                EvalTable::new(&model, "world", "Person").unwrap(),
                &snapshot,
                &params,
            )
            .unwrap()
            .is_none(),
            "expressions containing f64 reductions must remain off the tiled path"
        );
        let serial = evaluate_real_bits(&expr, &model, &state, 1, 257);
        for workers in [1, 2, 4] {
            for tile_rows in [257, 1_024, 4_093] {
                assert_eq!(
                    evaluate_real_bits(&expr, &model, &state, workers, tile_rows),
                    serial,
                    "Real aggregate changed at {workers} workers and {tile_rows} rows/tile"
                );
            }
        }

        let reduction = include_str!("eval.rs")
            .split_once("// This ascending target-row pass is the canonical CPU reduction order.")
            .unwrap()
            .1
            .split_once("Ok(Accumulator::Real(groups))")
            .unwrap()
            .0;
        assert!(reduction.contains("for (row, (include, value))"));
        assert!(
            !reduction.contains("element_wise_map"),
            "the canonical f64 reduction must remain sequential"
        );
    }
}
