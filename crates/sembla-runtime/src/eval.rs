//! Deterministic, snapshot-only evaluation of validated IR expressions.
//!
//! Expressions are evaluated in syntax-tree order without reassociation. Real
//! arithmetic therefore uses ordinary IEEE-754 `f64` semantics: in particular,
//! division by zero produces infinity or NaN rather than a runtime error.
//! Aggregate sums make one sequential target-table pass in ascending row order;
//! that order is the canonical Level A CPU reduction order (`DESIGN.md` §5.2).

use std::error::Error;
use std::fmt;

use sembla_ir::{
    AggJoin, AggOp, Attr, AttrType, Expr, ParamType, ParamValue, Table, ValidatedModel,
};

use crate::state::{Snapshot, StateError};

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
        Expr::Param { name } => {
            let declaration = table
                .model
                .model()
                .params
                .iter()
                .find(|param| param.name == *name)
                .ok_or_else(|| EvalError::new(format!("unresolved parameter '{name}'")))?;
            match (declaration.ty, params.get(name)?) {
                (ParamType::Real, ParamValue::Real { value }) => {
                    Ok(InternalColumn::Real(vec![*value; row_count]))
                }
                (ParamType::Int, ParamValue::Int { value }) => {
                    Ok(InternalColumn::Int(vec![*value; row_count]))
                }
                _ => Err(EvalError::new(format!(
                    "parameter environment value for '{name}' has the wrong type"
                ))),
            }
        }
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
            let values = lhs
                .into_iter()
                .zip(rhs)
                .map(|(lhs, rhs)| {
                    if matches!(expr, Expr::And { .. }) {
                        lhs && rhs
                    } else {
                        lhs || rhs
                    }
                })
                .collect();
            Ok(InternalColumn::Bool(values))
        }
        Expr::Not { expr } => {
            let values = eval_expr(expr, table, row_attrs, snapshot, params, cache, None)?;
            let InternalColumn::Bool(values) = values else {
                return Err(EvalError::new("Not operand did not evaluate to Bool"));
            };
            Ok(InternalColumn::Bool(
                values.into_iter().map(|value| !value).collect(),
            ))
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
            let mut values = Vec::with_capacity(row_count);
            for row in 0..row_count {
                values.push(
                    snapshot.enum_index(table.box_name(), table.table_name(), attr, row)?
                        == variant,
                );
            }
            Ok(InternalColumn::Bool(values))
        }
        Expr::Input { port, agg } => {
            let input = table
                .model_box()
                .inputs
                .iter()
                .find(|input| input.name == *port)
                .ok_or_else(|| EvalError::new("validated input port disappeared"))?;
            let result_type = infer_agg_type(&agg.op, table, &input.schema)?;
            zero_column(&result_type, row_count)
        }
        Expr::Agg {
            op,
            table: target,
            on,
            filter,
        } => eval_aggregate(op, target, on, filter, table, snapshot, params, cache),
    }
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
        AttrType::Real => (0..row_count)
            .map(|row| snapshot.real(table.box_name(), table.table_name(), name, row))
            .collect::<Result<Vec<_>, _>>()
            .map(InternalColumn::Real)
            .map_err(Into::into),
        AttrType::Int => (0..row_count)
            .map(|row| snapshot.int(table.box_name(), table.table_name(), name, row))
            .collect::<Result<Vec<_>, _>>()
            .map(InternalColumn::Int)
            .map_err(Into::into),
        AttrType::Enum { .. } => (0..row_count)
            .map(|row| snapshot.enum_index(table.box_name(), table.table_name(), name, row))
            .collect::<Result<Vec<_>, _>>()
            .map(InternalColumn::Enum)
            .map_err(Into::into),
        AttrType::Ref { .. } => (0..row_count)
            .map(|row| snapshot.reference(table.box_name(), table.table_name(), name, row))
            .collect::<Result<Vec<_>, _>>()
            .map(InternalColumn::Ref)
            .map_err(Into::into),
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
        let lhs = numeric_as_real(&lhs)?;
        let rhs = numeric_as_real(&rhs)?;
        return Ok(InternalColumn::Real(
            lhs.into_iter()
                .zip(rhs)
                .map(|(lhs, rhs)| match operation {
                    Arithmetic::Add => lhs + rhs,
                    Arithmetic::Sub => lhs - rhs,
                    Arithmetic::Mul => lhs * rhs,
                    Arithmetic::Div => lhs / rhs,
                })
                .collect(),
        ));
    }
    let (InternalColumn::Int(lhs), InternalColumn::Int(rhs)) = (lhs, rhs) else {
        return Err(EvalError::new("arithmetic operands are not numeric"));
    };
    let values = lhs
        .into_iter()
        .zip(rhs)
        .enumerate()
        .map(|(row, (lhs, rhs))| {
            let value = match operation {
                Arithmetic::Add => lhs.checked_add(rhs),
                Arithmetic::Sub => lhs.checked_sub(rhs),
                Arithmetic::Mul => lhs.checked_mul(rhs),
                Arithmetic::Div => unreachable!("division promotes to Real"),
            };
            value.ok_or_else(|| EvalError::new(format!("integer arithmetic overflow at row {row}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
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
    let equal = equal_columns(&lhs, &rhs)?;
    Ok(InternalColumn::Bool(
        equal
            .into_iter()
            .map(|value| if negate { !value } else { value })
            .collect(),
    ))
}

fn equal_columns(lhs: &InternalColumn, rhs: &InternalColumn) -> Result<Vec<bool>, EvalError> {
    if let (InternalColumn::Int(lhs), InternalColumn::Int(rhs)) = (lhs, rhs) {
        return Ok(lhs.iter().zip(rhs).map(|(lhs, rhs)| lhs == rhs).collect());
    }
    if matches!(lhs, InternalColumn::Real(_) | InternalColumn::Int(_))
        && matches!(rhs, InternalColumn::Real(_) | InternalColumn::Int(_))
    {
        return Ok(numeric_as_real(lhs)?
            .into_iter()
            .zip(numeric_as_real(rhs)?)
            .map(|(lhs, rhs)| lhs == rhs)
            .collect());
    }
    let values = match (lhs, rhs) {
        (InternalColumn::Bool(lhs), InternalColumn::Bool(rhs)) => {
            lhs.iter().zip(rhs).map(|(lhs, rhs)| lhs == rhs).collect()
        }
        (InternalColumn::Enum(lhs), InternalColumn::Enum(rhs)) => {
            lhs.iter().zip(rhs).map(|(lhs, rhs)| lhs == rhs).collect()
        }
        (InternalColumn::Ref(lhs), InternalColumn::Ref(rhs)) => {
            lhs.iter().zip(rhs).map(|(lhs, rhs)| lhs == rhs).collect()
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
        return Ok(InternalColumn::Bool(
            lhs.iter()
                .zip(rhs)
                .map(|(lhs, rhs)| match operation {
                    Ordering::Lt => lhs < rhs,
                    Ordering::Le => lhs <= rhs,
                    Ordering::Gt => lhs > rhs,
                    Ordering::Ge => lhs >= rhs,
                })
                .collect(),
        ));
    }
    let lhs = numeric_as_real(&lhs)?;
    let rhs = numeric_as_real(&rhs)?;
    Ok(InternalColumn::Bool(
        lhs.into_iter()
            .zip(rhs)
            .map(|(lhs, rhs)| match operation {
                Ordering::Lt => lhs < rhs,
                Ordering::Le => lhs <= rhs,
                Ordering::Gt => lhs > rhs,
                Ordering::Ge => lhs >= rhs,
            })
            .collect(),
    ))
}

fn numeric_as_real(column: &InternalColumn) -> Result<Vec<f64>, EvalError> {
    match column {
        InternalColumn::Real(values) => Ok(values.clone()),
        InternalColumn::Int(values) => Ok(values.iter().map(|value| *value as f64).collect()),
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
            let mut values = Vec::with_capacity(query_rows);
            for row in 0..query_rows {
                let group = snapshot.reference(
                    query.box_name(),
                    query.table_name(),
                    &on.self_fk_attr,
                    row,
                )? as usize;
                values.push(*groups.get(group).ok_or_else(|| {
                    EvalError::new(format!(
                        "aggregate broadcast group {group} is out of bounds"
                    ))
                })?);
            }
            Ok(InternalColumn::Int(values))
        }
        Accumulator::Real(groups) => {
            let mut values = Vec::with_capacity(query_rows);
            for row in 0..query_rows {
                let group = snapshot.reference(
                    query.box_name(),
                    query.table_name(),
                    &on.self_fk_attr,
                    row,
                )? as usize;
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
            for (row, include) in filter.iter().copied().enumerate() {
                if include {
                    let group = snapshot.reference(
                        target.box_name(),
                        target.table_name(),
                        &on.fk_attr,
                        row,
                    )? as usize;
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
                    for (row, (include, value)) in filter.iter().copied().zip(values).enumerate() {
                        if include {
                            let group = snapshot.reference(
                                target.box_name(),
                                target.table_name(),
                                &on.fk_attr,
                                row,
                            )? as usize;
                            groups[group] = groups[group].checked_add(value).ok_or_else(|| {
                                EvalError::new(format!("integer Sum overflow in group {group}"))
                            })?;
                        }
                    }
                    Ok(Accumulator::Int(groups))
                }
                InternalColumn::Real(values) => {
                    let mut groups = vec![0.0_f64; group_count];
                    // This ascending target-row pass is the canonical CPU reduction order.
                    for (row, (include, value)) in filter.iter().copied().zip(values).enumerate() {
                        if include {
                            let group = snapshot.reference(
                                target.box_name(),
                                target.table_name(),
                                &on.fk_attr,
                                row,
                            )? as usize;
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

fn zero_column(result_type: &RuntimeType, row_count: usize) -> Result<InternalColumn, EvalError> {
    match result_type {
        RuntimeType::Real => Ok(InternalColumn::Real(vec![0.0; row_count])),
        RuntimeType::Int => Ok(InternalColumn::Int(vec![0; row_count])),
        _ => Err(EvalError::new("empty input aggregate is not numeric")),
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
