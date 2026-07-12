//! Deterministic, snapshot-isolated single-box tick execution.

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use sembla_ir::{AttrType, ClaimOrdering, Effect, Expr, ValidatedModel};

use crate::eval::{
    eval_column, eval_typed_ref_column, AggCache, EvalError, EvalTable, ParamEnv, ValueColumn,
};
use crate::rng::exp_f64;
use crate::state::{StateError, StateStore};

/// Observable result of one committed tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickReport {
    pub tick: u32,
    pub fired: Vec<(u32, usize)>,
    pub deferred_per_resource_table: Vec<(String, usize)>,
}

/// A structured saturation warning produced by [`run`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaturationWarning {
    pub tick: u32,
    pub table: String,
    pub deferred_count: usize,
    pub fired_count: usize,
}

/// Observable result of a multi-tick run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunReport {
    pub ticks: Vec<TickReport>,
    pub warnings: Vec<SaturationWarning>,
}

/// A deterministic tick execution failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TickError {
    UnsupportedBoxCount {
        found: usize,
    },
    Evaluation(String),
    State(String),
    InvalidRuntimeType {
        context: String,
        found: String,
    },
    EntityIdOverflow {
        rule_id: u32,
        row: usize,
    },
    IncompatibleClaimOrdering {
        table: String,
        row: u32,
    },
    DoubleWrite {
        box_name: Box<str>,
        table: Box<str>,
        attr: Box<str>,
        row: usize,
        first_rule_id: u32,
        first_transition: Box<str>,
        second_rule_id: u32,
        second_transition: Box<str>,
    },
}

impl fmt::Display for TickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedBoxCount { found } => write!(
                formatter,
                "tick executor requires exactly one box, found {found}"
            ),
            Self::Evaluation(message) => write!(formatter, "expression evaluation failed: {message}"),
            Self::State(message) => write!(formatter, "state operation failed: {message}"),
            Self::InvalidRuntimeType { context, found } => {
                write!(formatter, "{context} evaluated to {found}")
            }
            Self::EntityIdOverflow { rule_id, row } => write!(
                formatter,
                "rule {rule_id} row {row} cannot be represented as a u32 entity ID"
            ),
            Self::IncompatibleClaimOrdering { table, row } => write!(
                formatter,
                "resource '{table}' row {row} has incompatible claim ordering modes or key types"
            ),
            Self::DoubleWrite {
                box_name,
                table,
                attr,
                row,
                first_rule_id,
                first_transition,
                second_rule_id,
                second_transition,
            } => write!(
                formatter,
                "double write to {box_name}.{table}.{attr}[{row}] by transition '{first_transition}' (rule {first_rule_id}) and transition '{second_transition}' (rule {second_rule_id})"
            ),
        }
    }
}

impl Error for TickError {}

impl From<EvalError> for TickError {
    fn from(error: EvalError) -> Self {
        Self::Evaluation(error.to_string())
    }
}

impl From<StateError> for TickError {
    fn from(error: StateError) -> Self {
        Self::State(error.to_string())
    }
}

#[derive(Clone, Debug)]
struct Candidate {
    rule_id: u32,
    table_index: usize,
    entity_id: u32,
    row: usize,
    claims: Vec<CandidateClaim>,
}

#[derive(Clone, Debug)]
struct CandidateClaim {
    table_index: usize,
    resource_row: u32,
    ordering: OrderingValue,
}

#[derive(Clone, Debug)]
enum OrderingValue {
    RaceTime(f64),
    Real(f64),
    Int(i64),
    Enum {
        table_index: usize,
        attr_index: usize,
        value: u16,
    },
}

#[derive(Clone, Copy, Debug)]
struct ClaimInstance {
    candidate_index: usize,
    claim_index: usize,
}

#[derive(Clone, Debug)]
enum PendingValue {
    Real(f64),
    Int(i64),
    Enum(u16),
    Ref(u32),
}

#[derive(Clone, Debug)]
struct PendingWrite {
    table_index: usize,
    attr_index: usize,
    row: usize,
    value: PendingValue,
    rule_id: u32,
    transition_name: String,
}

struct TickOutcome {
    report: TickReport,
    fired_per_resource_table: Vec<usize>,
}

struct Resolution {
    fires: Vec<bool>,
    deferred: Vec<usize>,
    fired_per_resource_table: Vec<usize>,
}

/// Executes and commits one deterministic, snapshot-isolated tick.
pub fn run_tick(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
) -> Result<TickReport, TickError> {
    Ok(execute_tick(model, state, params, seed, tick)?.report)
}

/// Executes ticks `0..n_ticks` and records strict saturation warnings.
pub fn run(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    n_ticks: u32,
) -> Result<RunReport, TickError> {
    ensure_single_box(model)?;
    let mut ticks = Vec::with_capacity(n_ticks as usize);
    let mut warnings = Vec::new();
    for tick in 0..n_ticks {
        let outcome = execute_tick(model, state, params, seed, tick)?;
        for (table_index, (_, deferred_count)) in outcome
            .report
            .deferred_per_resource_table
            .iter()
            .enumerate()
        {
            // The report omits zero entries, so resolve by name rather than by vector position.
            let table_name = &outcome.report.deferred_per_resource_table[table_index].0;
            let declaration_index = model.model().boxes[0]
                .tables
                .iter()
                .position(|table| table.name == *table_name)
                .expect("reported table came from the validated model");
            let fired_count = outcome.fired_per_resource_table[declaration_index];
            if exceeds_saturation_threshold(*deferred_count, fired_count) {
                let warning = SaturationWarning {
                    tick,
                    table: table_name.clone(),
                    deferred_count: *deferred_count,
                    fired_count,
                };
                eprintln!(
                    "warning: tick {} resource table '{}': {} deferred exceeds 10% of {} fired",
                    warning.tick, warning.table, warning.deferred_count, warning.fired_count
                );
                warnings.push(warning);
            }
        }
        ticks.push(outcome.report);
    }
    Ok(RunReport { ticks, warnings })
}

fn exceeds_saturation_threshold(deferred: usize, fired: usize) -> bool {
    (deferred as u128) * 10 > fired as u128
}

fn ensure_single_box(model: &ValidatedModel) -> Result<(), TickError> {
    let found = model.model().boxes.len();
    if found == 1 {
        Ok(())
    } else {
        Err(TickError::UnsupportedBoxCount { found })
    }
}

fn execute_tick(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
) -> Result<TickOutcome, TickError> {
    ensure_single_box(model)?;
    let model_box = &model.model().boxes[0];
    let snapshot = state.snapshot();
    let mut cache = AggCache::new(model, &snapshot, params);
    let mut candidates = Vec::new();

    for validated in model.transitions() {
        let transition = &model_box.transitions[validated.transition_index];
        let table_index = model_box
            .tables
            .iter()
            .position(|table| table.name == transition.table)
            .expect("validated transition table disappeared");
        let table = EvalTable::new(model, &model_box.name, &transition.table)?;
        let guards = match eval_column(&transition.guard, table, &snapshot, params, &mut cache)? {
            ValueColumn::Bool(values) => values,
            other => return Err(runtime_type("transition guard", &other)),
        };
        let hazards = match eval_column(&transition.hazard, table, &snapshot, params, &mut cache)? {
            ValueColumn::Real(values) => values,
            other => return Err(runtime_type("transition hazard", &other)),
        };

        let mut claim_columns = Vec::with_capacity(transition.contests.len());
        for claim in &transition.contests {
            let resources =
                eval_typed_ref_column(&claim.resource, table, &snapshot, params, &mut cache)?;
            let resource_table_index = model_box
                .tables
                .iter()
                .position(|schema| schema.name == resources.target_table)
                .expect("validated Ref target table disappeared");
            let ordering = match &claim.ordering {
                ClaimOrdering::RaceTime => None,
                ClaimOrdering::Key { expr } => {
                    Some(eval_column(expr, table, &snapshot, params, &mut cache)?)
                }
            };
            claim_columns.push((resource_table_index, resources.values, ordering, claim));
        }

        for (row, (guard, lambda)) in guards.into_iter().zip(hazards).enumerate() {
            if !guard || lambda.partial_cmp(&0.0) != Some(Ordering::Greater) {
                continue;
            }
            let entity_id = u32::try_from(row).map_err(|_| TickError::EntityIdOverflow {
                rule_id: validated.rule_id,
                row,
            })?;
            let race_time = exp_f64(seed, tick, validated.rule_id, entity_id, 0, lambda);
            if race_time.partial_cmp(&model.model().dt) != Some(Ordering::Less) {
                continue;
            }
            let mut claims = Vec::with_capacity(claim_columns.len());
            for (resource_table, resources, key_column, claim) in &claim_columns {
                let ordering = match (&claim.ordering, key_column) {
                    (ClaimOrdering::RaceTime, None) => OrderingValue::RaceTime(race_time),
                    (ClaimOrdering::Key { expr }, Some(column)) => {
                        key_at(column, expr, table_index, model_box, row)?
                    }
                    _ => unreachable!("claim ordering column construction is exhaustive"),
                };
                claims.push(CandidateClaim {
                    table_index: *resource_table,
                    resource_row: resources[row],
                    ordering,
                });
            }
            candidates.push(Candidate {
                rule_id: validated.rule_id,
                table_index,
                entity_id,
                row,
                claims,
            });
        }
    }

    let resolution = resolve_claims(&candidates, model_box.tables.len(), model_box)?;
    let fires = resolution.fires;
    let deferred = resolution.deferred;
    let fired_per_resource_table = resolution.fired_per_resource_table;
    let mut pending = Vec::new();

    for validated in model.transitions() {
        let transition = &model_box.transitions[validated.transition_index];
        let winner_indices: Vec<usize> = candidates
            .iter()
            .enumerate()
            .filter(|(index, candidate)| candidate.rule_id == validated.rule_id && fires[*index])
            .map(|(index, _)| index)
            .collect();
        if winner_indices.is_empty() {
            continue;
        }
        let table = EvalTable::new(model, &model_box.name, &transition.table)?;
        let table_index = candidates[winner_indices[0]].table_index;
        let schema = &model_box.tables[table_index];
        let mut effect_columns = Vec::with_capacity(transition.effects.len());
        for effect in &transition.effects {
            let Effect::SetAttr { attr, value } = effect;
            let attr_index = schema
                .attrs
                .iter()
                .position(|declaration| declaration.name == *attr)
                .expect("validated effect attribute disappeared");
            let destination = &schema.attrs[attr_index];
            let value = match &destination.ty {
                AttrType::Ref { .. } => PendingColumn::Ref(
                    eval_typed_ref_column(value, table, &snapshot, params, &mut cache)?.values,
                ),
                _ => PendingColumn::Value(eval_column(
                    value,
                    table.with_expected_attr(attr)?,
                    &snapshot,
                    params,
                    &mut cache,
                )?),
            };
            effect_columns.push((attr_index, value));
        }
        for candidate_index in winner_indices {
            let candidate = &candidates[candidate_index];
            for (attr_index, values) in &effect_columns {
                pending.push(PendingWrite {
                    table_index,
                    attr_index: *attr_index,
                    row: candidate.row,
                    value: values.at(candidate.row)?,
                    rule_id: candidate.rule_id,
                    transition_name: transition.name.clone(),
                });
            }
        }
    }

    detect_double_writes(&pending, model_box)?;
    drop(cache);

    let apply_result = {
        let mut writes = state.write_buffer()?;
        pending.iter().try_for_each(|write| {
            let table = &model_box.tables[write.table_index];
            let attr = &table.attrs[write.attr_index];
            match &write.value {
                PendingValue::Real(value) => {
                    writes.set_real(&model_box.name, &table.name, &attr.name, write.row, *value)
                }
                PendingValue::Int(value) => {
                    writes.set_int(&model_box.name, &table.name, &attr.name, write.row, *value)
                }
                PendingValue::Enum(value) => {
                    writes.set_enum(&model_box.name, &table.name, &attr.name, write.row, *value)
                }
                PendingValue::Ref(value) => {
                    writes.set_ref(&model_box.name, &table.name, &attr.name, write.row, *value)
                }
            }
        })
    };
    if let Err(error) = apply_result {
        state.discard_writes();
        return Err(error.into());
    }
    if let Err(error) = state.commit() {
        state.discard_writes();
        return Err(error.into());
    }

    let mut fired = vec![(0, 0); model.transitions().len()];
    for validated in model.transitions() {
        fired[validated.rule_id as usize].0 = validated.rule_id;
    }
    for (candidate, fire) in candidates.iter().zip(&fires) {
        if *fire {
            fired[candidate.rule_id as usize].1 += 1;
        }
    }
    let deferred_per_resource_table = deferred
        .into_iter()
        .enumerate()
        .filter(|(_, count)| *count != 0)
        .map(|(table_index, count)| (model_box.tables[table_index].name.clone(), count))
        .collect();

    Ok(TickOutcome {
        report: TickReport {
            tick,
            fired,
            deferred_per_resource_table,
        },
        fired_per_resource_table,
    })
}

fn runtime_type(context: &str, column: &ValueColumn) -> TickError {
    let found = match column {
        ValueColumn::Real(_) => "Real",
        ValueColumn::Int(_) => "Int",
        ValueColumn::Bool(_) => "Bool",
        ValueColumn::Enum(_) => "Enum",
    };
    TickError::InvalidRuntimeType {
        context: context.to_owned(),
        found: found.to_owned(),
    }
}

fn key_at(
    column: &ValueColumn,
    expr: &Expr,
    table_index: usize,
    model_box: &sembla_ir::Box,
    row: usize,
) -> Result<OrderingValue, TickError> {
    match column {
        ValueColumn::Real(values) => Ok(OrderingValue::Real(values[row])),
        ValueColumn::Int(values) => Ok(OrderingValue::Int(values[row])),
        ValueColumn::Enum(values) => {
            let Expr::SelfAttr { name } = expr else {
                return Err(TickError::Evaluation(
                    "Enum contest key has no source attribute identity".to_owned(),
                ));
            };
            let attr_index = model_box.tables[table_index]
                .attrs
                .iter()
                .position(|attr| attr.name == *name)
                .expect("validated key attribute disappeared");
            Ok(OrderingValue::Enum {
                table_index,
                attr_index,
                value: values[row],
            })
        }
        ValueColumn::Bool(_) => Err(runtime_type("contest key", column)),
    }
}

fn resolve_claims(
    candidates: &[Candidate],
    table_count: usize,
    model_box: &sembla_ir::Box,
) -> Result<Resolution, TickError> {
    let mut instances = Vec::new();
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        for claim_index in 0..candidate.claims.len() {
            instances.push(ClaimInstance {
                candidate_index,
                claim_index,
            });
        }
    }
    instances.sort_by(|lhs, rhs| {
        let lhs_candidate = &candidates[lhs.candidate_index];
        let rhs_candidate = &candidates[rhs.candidate_index];
        let lhs_claim = &lhs_candidate.claims[lhs.claim_index];
        let rhs_claim = &rhs_candidate.claims[rhs.claim_index];
        (
            lhs_claim.table_index,
            lhs_claim.resource_row,
            lhs_candidate.rule_id,
            lhs_candidate.entity_id,
        )
            .cmp(&(
                rhs_claim.table_index,
                rhs_claim.resource_row,
                rhs_candidate.rule_id,
                rhs_candidate.entity_id,
            ))
            .then(lhs.claim_index.cmp(&rhs.claim_index))
    });

    let mut won_all = vec![true; candidates.len()];
    let mut deferred_table = vec![vec![false; table_count]; candidates.len()];
    let mut start = 0;
    while start < instances.len() {
        let first = instances[start];
        let first_claim = &candidates[first.candidate_index].claims[first.claim_index];
        let mut end = start + 1;
        while end < instances.len() {
            let claim =
                &candidates[instances[end].candidate_index].claims[instances[end].claim_index];
            if (claim.table_index, claim.resource_row)
                != (first_claim.table_index, first_claim.resource_row)
            {
                break;
            }
            end += 1;
        }
        let mut winner = first;
        for instance in &instances[start + 1..end] {
            if compare_instances(*instance, winner, candidates, model_box)? == Ordering::Less {
                winner = *instance;
            }
        }
        let winner_candidate = winner.candidate_index;
        for instance in &instances[start..end] {
            if instance.candidate_index != winner_candidate {
                won_all[instance.candidate_index] = false;
                deferred_table[instance.candidate_index][first_claim.table_index] = true;
            }
        }
        start = end;
    }

    let mut deferred = vec![0; table_count];
    let mut fired_per_resource_table = vec![0; table_count];
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        for (table_index, lost) in deferred_table[candidate_index].iter().enumerate() {
            if *lost {
                deferred[table_index] += 1;
            }
        }
        if won_all[candidate_index] {
            let mut counted = vec![false; table_count];
            for claim in &candidate.claims {
                if !counted[claim.table_index] {
                    counted[claim.table_index] = true;
                    fired_per_resource_table[claim.table_index] += 1;
                }
            }
        }
    }
    Ok(Resolution {
        fires: won_all,
        deferred,
        fired_per_resource_table,
    })
}

fn compare_instances(
    lhs: ClaimInstance,
    rhs: ClaimInstance,
    candidates: &[Candidate],
    model_box: &sembla_ir::Box,
) -> Result<Ordering, TickError> {
    let lhs_candidate = &candidates[lhs.candidate_index];
    let rhs_candidate = &candidates[rhs.candidate_index];
    let lhs_claim = &lhs_candidate.claims[lhs.claim_index];
    let rhs_claim = &rhs_candidate.claims[rhs.claim_index];
    let key_order = match (&lhs_claim.ordering, &rhs_claim.ordering) {
        (OrderingValue::RaceTime(lhs), OrderingValue::RaceTime(rhs))
        | (OrderingValue::Real(lhs), OrderingValue::Real(rhs)) => lhs.total_cmp(rhs),
        (OrderingValue::Int(lhs), OrderingValue::Int(rhs)) => lhs.cmp(rhs),
        (
            OrderingValue::Enum {
                table_index: lhs_table,
                attr_index: lhs_attr,
                value: lhs,
            },
            OrderingValue::Enum {
                table_index: rhs_table,
                attr_index: rhs_attr,
                value: rhs,
            },
        ) if enum_domains_match(model_box, *lhs_table, *lhs_attr, *rhs_table, *rhs_attr) => {
            lhs.cmp(rhs)
        }
        _ => {
            return Err(TickError::IncompatibleClaimOrdering {
                table: model_box.tables[lhs_claim.table_index].name.clone(),
                row: lhs_claim.resource_row,
            })
        }
    };
    Ok(key_order.then_with(|| {
        (lhs_candidate.rule_id, lhs_candidate.entity_id)
            .cmp(&(rhs_candidate.rule_id, rhs_candidate.entity_id))
    }))
}

fn enum_domains_match(
    model_box: &sembla_ir::Box,
    lhs_table: usize,
    lhs_attr: usize,
    rhs_table: usize,
    rhs_attr: usize,
) -> bool {
    match (
        &model_box.tables[lhs_table].attrs[lhs_attr].ty,
        &model_box.tables[rhs_table].attrs[rhs_attr].ty,
    ) {
        (
            AttrType::Enum {
                variants: lhs_variants,
            },
            AttrType::Enum {
                variants: rhs_variants,
            },
        ) => lhs_variants == rhs_variants,
        _ => false,
    }
}

enum PendingColumn {
    Value(ValueColumn),
    Ref(Vec<u32>),
}

impl PendingColumn {
    fn at(&self, row: usize) -> Result<PendingValue, TickError> {
        match self {
            Self::Value(ValueColumn::Real(values)) => Ok(PendingValue::Real(values[row])),
            Self::Value(ValueColumn::Int(values)) => Ok(PendingValue::Int(values[row])),
            Self::Value(ValueColumn::Enum(values)) => Ok(PendingValue::Enum(values[row])),
            Self::Ref(values) => Ok(PendingValue::Ref(values[row])),
            Self::Value(ValueColumn::Bool(_)) => Err(TickError::InvalidRuntimeType {
                context: "effect value".to_owned(),
                found: "Bool".to_owned(),
            }),
        }
    }
}

fn detect_double_writes(
    pending: &[PendingWrite],
    model_box: &sembla_ir::Box,
) -> Result<(), TickError> {
    let mut order: Vec<usize> = (0..pending.len()).collect();
    order.sort_by_key(|index| {
        let write = &pending[*index];
        (write.table_index, write.attr_index, write.row)
    });
    for pair in order.windows(2) {
        let first = &pending[pair[0]];
        let second = &pending[pair[1]];
        if (first.table_index, first.attr_index, first.row)
            == (second.table_index, second.attr_index, second.row)
        {
            return Err(TickError::DoubleWrite {
                box_name: model_box.name.clone().into_boxed_str(),
                table: model_box.tables[first.table_index]
                    .name
                    .clone()
                    .into_boxed_str(),
                attr: model_box.tables[first.table_index].attrs[first.attr_index]
                    .name
                    .clone()
                    .into_boxed_str(),
                row: first.row,
                first_rule_id: first.rule_id,
                first_transition: first.transition_name.clone().into_boxed_str(),
                second_rule_id: second.rule_id,
                second_transition: second.transition_name.clone().into_boxed_str(),
            });
        }
    }
    Ok(())
}
