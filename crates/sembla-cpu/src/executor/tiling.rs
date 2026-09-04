//! Fixed-task CPU tiling for transition and observation evaluation.

use std::borrow::Cow;

use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TiledPlanProfile {
    retained_root_bytes_per_row: usize,
    peak_bytes_per_row: usize,
    pub(super) node_count: usize,
}

impl TiledPlanProfile {
    pub(super) fn include(&mut self, footprint: TiledExprFootprint) {
        self.retained_root_bytes_per_row = self
            .retained_root_bytes_per_row
            .saturating_add(footprint.root_bytes_per_row);
        self.peak_bytes_per_row = self.peak_bytes_per_row.max(footprint.peak_bytes_per_row);
        self.node_count = self.node_count.saturating_add(footprint.node_count);
    }

    pub(super) fn live_set_bytes_per_row(self) -> usize {
        self.retained_root_bytes_per_row
            .saturating_add(self.peak_bytes_per_row)
            .max(1)
    }
}

struct TransitionTilingCandidate {
    box_index: usize,
    transition_index: usize,
    table_index: usize,
    row_count: usize,
    profile: TiledPlanProfile,
}

struct PreparedTransition<'state> {
    box_index: usize,
    transition_index: usize,
    rule_id: u32,
    rule_word: u32,
    table_index: usize,
    row_count: usize,
    tile_rows: usize,
    guard: PreparedExpr<'state>,
    hazard: PreparedExpr<'state>,
    race_strategy: RacingClockStrategy,
    claims: Vec<PreparedClaim<'state>>,
}

struct PreparedClaim<'state> {
    resource_table_index: usize,
    resource: PreparedExpr<'state>,
    ordering: PreparedClaimOrdering<'state>,
}

enum PreparedClaimOrdering<'state> {
    RaceTime,
    Key {
        expr: PreparedExpr<'state>,
        enum_identity: Option<(usize, usize)>,
    },
}

struct TileTask {
    plan_indices: Vec<usize>,
    start: usize,
    end: usize,
}

struct TilePlanOutput {
    plan_index: usize,
    candidates: Vec<Candidate>,
    error: Option<TickError>,
}

struct CandidateGroup {
    box_index: usize,
    table_index: usize,
    row_count: usize,
    indices: Vec<usize>,
}

fn group_candidates<T>(
    candidates: &[T],
    key: impl Fn(&T) -> (usize, usize, usize),
) -> Vec<CandidateGroup> {
    let mut groups: Vec<CandidateGroup> = Vec::new();
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        let (box_index, table_index, row_count) = key(candidate);
        if let Some(group) = groups
            .iter_mut()
            .find(|group| (group.box_index, group.table_index) == (box_index, table_index))
        {
            group.indices.push(candidate_index);
        } else {
            groups.push(CandidateGroup {
                box_index,
                table_index,
                row_count,
                indices: vec![candidate_index],
            });
        }
    }
    groups
}

fn build_tile_tasks<T>(
    plans: &[T],
    layout: impl Fn(&T) -> (usize, usize, usize, usize),
) -> Vec<TileTask> {
    let mut groups: Vec<(usize, usize, usize, usize, Vec<usize>)> = Vec::new();
    for (plan_index, plan) in plans.iter().enumerate() {
        let (box_index, table_index, tile_rows, row_count) = layout(plan);
        if let Some((_, _, _, _, indices)) =
            groups
                .iter_mut()
                .find(|(group_box, group_table, group_rows, _, _)| {
                    (*group_box, *group_table, *group_rows) == (box_index, table_index, tile_rows)
                })
        {
            indices.push(plan_index);
        } else {
            groups.push((
                box_index,
                table_index,
                tile_rows,
                row_count,
                vec![plan_index],
            ));
        }
    }
    groups
        .into_iter()
        .flat_map(|(_, _, tile_rows, row_count, plan_indices)| {
            (0..row_count)
                .step_by(tile_rows)
                .map(move |start| TileTask {
                    plan_indices: plan_indices.clone(),
                    start,
                    end: (start + tile_rows).min(row_count),
                })
        })
        .collect()
}

fn evaluate_tile_tasks<O: Send>(
    tasks: &[TileTask],
    worker_count: usize,
    evaluate: impl Fn(&TileTask) -> O + Sync,
) -> Vec<O> {
    let worker_count = worker_count.max(1).min(tasks.len().max(1));
    if worker_count == 1 {
        return tasks.iter().map(evaluate).collect();
    }

    let mut worker_tasks = std::iter::repeat_with(Vec::new)
        .take(worker_count)
        .collect::<Vec<Vec<usize>>>();
    for task_index in 0..tasks.len() {
        worker_tasks[task_index % worker_count].push(task_index);
    }
    let mut outputs = std::thread::scope(|scope| {
        let evaluate = &evaluate;
        worker_tasks
            .into_iter()
            .map(|task_indices| {
                scope.spawn(move || {
                    task_indices
                        .into_iter()
                        .map(|task_index| (task_index, evaluate(&tasks[task_index])))
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|handle| handle.join().expect("tile worker panicked"))
            .collect::<Vec<_>>()
    });
    outputs.sort_by_key(|(task_index, _)| *task_index);
    outputs.into_iter().map(|(_, output)| output).collect()
}

pub(super) fn transition_tiling_profile(
    model: &ValidatedModel,
    box_index: usize,
    transition_index: usize,
) -> Result<Option<TiledPlanProfile>, TickError> {
    let model_box = &model.model().boxes[box_index];
    let transition = &model_box.transitions[transition_index];
    let table = EvalTable::new(model, &model_box.name, &transition.table)?;
    let mut profile = TiledPlanProfile::default();
    for expression in [&transition.guard, &transition.hazard] {
        let Some(footprint) = tiled_expr_footprint(expression, table)? else {
            return Ok(None);
        };
        profile.include(footprint);
    }
    for claim in &transition.contests {
        let Some(footprint) = tiled_expr_footprint(&claim.resource, table)? else {
            return Ok(None);
        };
        profile.include(footprint);
        if let ClaimOrdering::Key { expr } = &claim.ordering {
            let Some(footprint) = tiled_expr_footprint(expr, table)? else {
                return Ok(None);
            };
            profile.include(footprint);
        }
    }
    Ok(Some(profile))
}

fn prepare_tiled_transition<'state>(
    model: &ValidatedModel,
    box_index: usize,
    transition_index: usize,
    tile_rows: usize,
    snapshot: &'state Snapshot<'_>,
    params: &ParamEnv,
) -> Result<Option<PreparedTransition<'state>>, TickError> {
    let model_box = &model.model().boxes[box_index];
    let transition = &model_box.transitions[transition_index];
    let validated = model
        .transition_at(box_index, transition_index)
        .expect("validated transition disappeared");
    let table_index = validated.table_index;
    let row_count = snapshot.row_count(&model_box.name, &transition.table)?;
    let table = EvalTable::new(model, &model_box.name, &transition.table)?;
    let Some(guard) = prepare_row_expr(&transition.guard, table, snapshot, params)? else {
        return Ok(None);
    };
    let Some(hazard) = prepare_row_expr(&transition.hazard, table, snapshot, params)? else {
        return Ok(None);
    };
    let mut claims = Vec::with_capacity(transition.contests.len());
    for claim in &transition.contests {
        let Some(resource) = prepare_row_expr(&claim.resource, table, snapshot, params)? else {
            return Ok(None);
        };
        let target_table = resource.ref_target().ok_or_else(|| {
            TickError::Evaluation("contest resource did not prepare as Ref".to_owned())
        })?;
        let resource_table_index = model_box
            .tables
            .iter()
            .position(|schema| schema.name == target_table)
            .expect("validated Ref target table disappeared");
        let ordering = match &claim.ordering {
            ClaimOrdering::RaceTime => PreparedClaimOrdering::RaceTime,
            ClaimOrdering::Key { expr } => {
                let Some(prepared) = prepare_row_expr(expr, table, snapshot, params)? else {
                    return Ok(None);
                };
                let enum_identity = if let Expr::SelfAttr { name } = expr {
                    model_box.tables[table_index]
                        .attrs
                        .iter()
                        .position(|attr| attr.name == *name)
                        .filter(|attr_index| {
                            matches!(
                                model_box.tables[table_index].attrs[*attr_index].ty,
                                AttrType::Enum { .. }
                            )
                        })
                        .map(|attr_index| (table_index, attr_index))
                } else {
                    None
                };
                PreparedClaimOrdering::Key {
                    expr: prepared,
                    enum_identity,
                }
            }
        };
        claims.push(PreparedClaim {
            resource_table_index,
            resource,
            ordering,
        });
    }
    // Direct literals and parameters are the deliberately narrow row-invariant
    // fragment. Compute their filter only after all eager preparation succeeds,
    // preserving declaration-ordered expression errors.
    let race_strategy = if row_count != 0 && has_constant_hazard(&transition.hazard) {
        let PreparedColumn::Real(values) = hazard.tile(0, 1)? else {
            return Err(TickError::Evaluation(
                "transition hazard did not prepare as Real".to_owned(),
            ));
        };
        RacingClockStrategy::for_transition(
            RacingClockFilter::for_hazard(values[0], model.model().dt),
            transition,
        )
    } else {
        RacingClockStrategy::Canonical
    };
    Ok(Some(PreparedTransition {
        box_index,
        transition_index,
        rule_id: validated.rule_id,
        rule_word: validated.rule_word,
        table_index,
        row_count,
        tile_rows,
        guard,
        hazard,
        race_strategy,
        claims,
    }))
}

fn prepared_ordering_value(
    value: PreparedValue,
    enum_identity: Option<(usize, usize)>,
) -> Result<OrderingValue, TickError> {
    match value {
        PreparedValue::Real(value) => Ok(OrderingValue::Real(value)),
        PreparedValue::Int(value) => Ok(OrderingValue::Int(value)),
        PreparedValue::Enum(value) => {
            let (table_index, attr_index) = enum_identity.ok_or_else(|| {
                TickError::Evaluation(
                    "Enum contest key has no source attribute identity".to_owned(),
                )
            })?;
            Ok(OrderingValue::Enum {
                table_index,
                attr_index,
                value,
            })
        }
        PreparedValue::Bool | PreparedValue::Ref => Err(TickError::InvalidRuntimeType {
            context: "contest key".to_owned(),
            found: match value {
                PreparedValue::Bool => "Bool",
                PreparedValue::Ref => "Ref",
                _ => unreachable!(),
            }
            .to_owned(),
        }),
    }
}

fn evaluate_tile_task(
    task: &TileTask,
    plans: &[PreparedTransition<'_>],
    seed: u64,
    tick: u32,
    dt: f64,
) -> Vec<TilePlanOutput> {
    task.plan_indices
        .iter()
        .map(|plan_index| {
            let plan = &plans[*plan_index];
            let evaluated = (|| -> Result<Vec<Candidate>, TickError> {
                // Dispatch each expression node once per tile, not once per
                // row. Every root is still evaluated eagerly before racing.
                let guard = plan.guard.tile(task.start, task.end)?;
                let hazard = plan.hazard.tile(task.start, task.end)?;
                let mut claim_columns = Vec::with_capacity(plan.claims.len());
                for claim in &plan.claims {
                    let resource = claim.resource.tile(task.start, task.end)?;
                    let ordering = match &claim.ordering {
                        PreparedClaimOrdering::RaceTime => None,
                        PreparedClaimOrdering::Key { expr, .. } => {
                            Some(expr.tile(task.start, task.end)?)
                        }
                    };
                    claim_columns.push((resource, ordering));
                }
                let PreparedColumn::Bool(guards) = guard else {
                    return Err(TickError::Evaluation(
                        "transition guard did not prepare as Bool".to_owned(),
                    ));
                };
                let PreparedColumn::Real(hazards) = hazard else {
                    return Err(TickError::Evaluation(
                        "transition hazard did not prepare as Real".to_owned(),
                    ));
                };
                let mut candidates = Vec::new();
                for (offset, (guard, lambda)) in guards
                    .iter()
                    .copied()
                    .zip(hazards.iter().copied())
                    .enumerate()
                {
                    if !guard || lambda.partial_cmp(&0.0) != Some(Ordering::Greater) {
                        continue;
                    }
                    let row = task.start + offset;
                    let Some(firing) = candidate_race_time(
                        RacingClockCoordinates {
                            seed,
                            tick,
                            rule_id: plan.rule_id,
                            rule_word: plan.rule_word,
                            row,
                        },
                        lambda,
                        dt,
                        plan.race_strategy,
                    )?
                    else {
                        continue;
                    };
                    let mut claims = Vec::with_capacity(plan.claims.len());
                    for (claim, (resource, ordering)) in plan.claims.iter().zip(&claim_columns) {
                        let PreparedColumn::Ref(resources) = resource else {
                            return Err(TickError::Evaluation(
                                "contest resource did not prepare as Ref".to_owned(),
                            ));
                        };
                        let ordering = match (&claim.ordering, ordering) {
                            (PreparedClaimOrdering::RaceTime, None) => OrderingValue::RaceTime(
                                firing
                                    .race_time
                                    .expect("a contested transition must sample its race time"),
                            ),
                            (PreparedClaimOrdering::Key { enum_identity, .. }, Some(values)) => {
                                let value = match values {
                                    PreparedColumn::Real(values) => {
                                        PreparedValue::Real(values[offset])
                                    }
                                    PreparedColumn::Int(values) => {
                                        PreparedValue::Int(values[offset])
                                    }
                                    PreparedColumn::Enum(values) => {
                                        PreparedValue::Enum(values[offset])
                                    }
                                    PreparedColumn::Bool(_) => PreparedValue::Bool,
                                    PreparedColumn::Ref(_) => PreparedValue::Ref,
                                };
                                prepared_ordering_value(value, *enum_identity)?
                            }
                            _ => unreachable!("prepared claim ordering is exhaustive"),
                        };
                        claims.push(CandidateClaim {
                            table_index: claim.resource_table_index,
                            resource_row: resources[offset],
                            ordering,
                        });
                    }
                    candidates.push(Candidate {
                        rule_id: plan.rule_id,
                        rule_word: plan.rule_word,
                        table_index: plan.table_index,
                        entity_id: firing.entity_id,
                        row,
                        claims,
                    });
                }
                Ok(candidates)
            })();
            match evaluated {
                Ok(candidates) => TilePlanOutput {
                    plan_index: *plan_index,
                    candidates,
                    error: None,
                },
                Err(error) => TilePlanOutput {
                    plan_index: *plan_index,
                    candidates: Vec::new(),
                    error: Some(error),
                },
            }
        })
        .collect()
}

pub(super) type TiledCandidateResults = Vec<Vec<Option<Result<Vec<Candidate>, TickError>>>>;

/// Prepares every eligible transition, then opens at most one parallel region
/// for the tick. Fixed task boundaries are `(table, tile_start, tile_end)` and
/// depend only on stable model indices, row count, and tile size. Worker count
/// changes only which worker receives a complete fixed task.
pub(super) fn prepare_tiled_candidates(
    model: &ValidatedModel,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
    config: &CpuExecutionConfig,
) -> TiledCandidateResults {
    let mut results = model
        .model()
        .boxes
        .iter()
        .map(|model_box| {
            std::iter::repeat_with(|| None)
                .take(model_box.transitions.len())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    for (box_index, model_box) in model.model().boxes.iter().enumerate() {
        for (transition_index, transition) in model_box.transitions.iter().enumerate() {
            let table_index = model
                .transition_at(box_index, transition_index)
                .map(|validated| validated.table_index)
                .expect("validated transition table disappeared");
            let row_count = match snapshot.row_count(&model_box.name, &transition.table) {
                Ok(row_count) => row_count,
                Err(error) => {
                    results[box_index][transition_index] = Some(Err(error.into()));
                    continue;
                }
            };
            match transition_tiling_profile(model, box_index, transition_index) {
                Ok(Some(profile)) => candidates.push(TransitionTilingCandidate {
                    box_index,
                    transition_index,
                    table_index,
                    row_count,
                    profile,
                }),
                Ok(None) => {}
                Err(error) => results[box_index][transition_index] = Some(Err(error)),
            }
        }
    }

    let mut plans = Vec::new();
    for group in group_candidates(&candidates, |candidate| {
        (
            candidate.box_index,
            candidate.table_index,
            candidate.row_count,
        )
    }) {
        let node_count = group.indices.iter().fold(0_usize, |total, index| {
            total.saturating_add(candidates[*index].profile.node_count)
        });
        let live_set_bytes_per_row = group
            .indices
            .iter()
            .map(|index| candidates[*index].profile.live_set_bytes_per_row())
            .max()
            .unwrap_or(1);
        let tile_rows = tick_tile_rows_for_live_set(config, live_set_bytes_per_row);
        if !tick_tiling_enabled(config, group.row_count, node_count) || group.row_count <= tile_rows
        {
            continue;
        }
        for candidate_index in group.indices {
            let candidate = &candidates[candidate_index];
            match prepare_tiled_transition(
                model,
                candidate.box_index,
                candidate.transition_index,
                tile_rows,
                snapshot,
                params,
            ) {
                Ok(Some(plan)) => plans.push(plan),
                Ok(None) => {}
                Err(error) => {
                    results[candidate.box_index][candidate.transition_index] = Some(Err(error));
                }
            }
        }
    }
    if plans.is_empty() {
        return results;
    }

    let tasks = build_tile_tasks(&plans, |plan| {
        (
            plan.box_index,
            plan.table_index,
            plan.tile_rows,
            plan.row_count,
        )
    });
    let task_outputs = evaluate_tile_tasks(&tasks, tick_worker_count(config), |task| {
        evaluate_tile_task(task, &plans, seed, tick, model.model().dt)
    });
    for outputs in task_outputs {
        for output in outputs {
            let plan = &plans[output.plan_index];
            let slot = &mut results[plan.box_index][plan.transition_index];
            if let Some(error) = output.error {
                if slot.is_none() {
                    *slot = Some(Err(error));
                }
            } else if !matches!(slot, Some(Err(_))) {
                match slot {
                    Some(Ok(candidates)) => candidates.extend(output.candidates),
                    None => *slot = Some(Ok(output.candidates)),
                    Some(Err(_)) => unreachable!(),
                }
            }
        }
    }
    results
}

struct ViewTilingCandidate {
    ordinal: usize,
    box_index: usize,
    view_index: usize,
    table_index: usize,
    row_count: usize,
    profile: TiledPlanProfile,
}

struct PreparedView<'state> {
    ordinal: usize,
    box_index: usize,
    view_index: usize,
    table_index: usize,
    row_count: usize,
    tile_rows: usize,
    reduce: ViewReduce,
    filter: Option<PreparedExpr<'state>>,
    value: Option<PreparedExpr<'state>>,
}

struct ViewTileOutput<'state> {
    plan_index: usize,
    start: usize,
    count: Option<Result<usize, TickError>>,
    filter: Option<Result<PreparedColumn<'state>, TickError>>,
    value: Option<Result<PreparedColumn<'state>, TickError>>,
}

fn evaluate_view_tile_task<'state>(
    task: &TileTask,
    plans: &[PreparedView<'state>],
) -> Vec<ViewTileOutput<'state>> {
    task.plan_indices
        .iter()
        .map(|plan_index| {
            let plan = &plans[*plan_index];
            let filter = plan
                .filter
                .as_ref()
                .map(|filter| filter.tile(task.start, task.end).map_err(Into::into));
            if plan.reduce == ViewReduce::Count {
                let count = match filter {
                    Some(Ok(PreparedColumn::Bool(selected))) => {
                        Ok(selected.iter().filter(|value| **value).count())
                    }
                    Some(Ok(other)) => Err(prepared_runtime_type("view filter", &other)),
                    Some(Err(error)) => Err(error),
                    None => Ok(task.end - task.start),
                };
                ViewTileOutput {
                    plan_index: *plan_index,
                    start: task.start,
                    count: Some(count),
                    filter: None,
                    value: None,
                }
            } else {
                ViewTileOutput {
                    plan_index: *plan_index,
                    start: task.start,
                    count: None,
                    filter,
                    value: plan
                        .value
                        .as_ref()
                        .map(|value| value.tile(task.start, task.end).map_err(Into::into)),
                }
            }
        })
        .collect()
}

fn reduce_prepared_view_tiles(
    box_name: &str,
    view_name: &str,
    reduce: ViewReduce,
    columns: Vec<PreparedColumn<'_>>,
    filters: Vec<Option<Cow<'_, [bool]>>>,
) -> Result<ObservationValue, TickError> {
    match columns.first() {
        Some(PreparedColumn::Int(_)) => {
            let mut result = match reduce {
                ViewReduce::Sum => 0_i64,
                ViewReduce::Min => i64::MAX,
                ViewReduce::Max => i64::MIN,
                ViewReduce::Count => unreachable!("count does not evaluate a value"),
            };
            for (column, filter) in columns.into_iter().zip(filters) {
                let PreparedColumn::Int(values) = column else {
                    return Err(TickError::Evaluation(
                        "view value changed type between tiles".to_owned(),
                    ));
                };
                for (offset, value) in values.iter().copied().enumerate() {
                    if filter.as_ref().is_some_and(|selected| !selected[offset]) {
                        continue;
                    }
                    result = match reduce {
                        ViewReduce::Sum => result.checked_add(value).ok_or_else(|| {
                            TickError::Evaluation(format!(
                                "view '{box_name}.{view_name}' integer sum overflowed"
                            ))
                        })?,
                        ViewReduce::Min => result.min(value),
                        ViewReduce::Max => result.max(value),
                        ViewReduce::Count => unreachable!(),
                    };
                }
            }
            Ok(ObservationValue::Int(result))
        }
        Some(PreparedColumn::Real(_)) => {
            let mut result = match reduce {
                ViewReduce::Sum => 0.0,
                ViewReduce::Min => f64::INFINITY,
                ViewReduce::Max => f64::NEG_INFINITY,
                ViewReduce::Count => unreachable!("count does not evaluate a value"),
            };
            // This is the canonical Level A reduction order: fixed tiles are
            // consumed by ascending start row, and rows stay ascending inside
            // each tile. Workers evaluate row-local values only; they never
            // form floating-point partial sums.
            for (column, filter) in columns.into_iter().zip(filters) {
                let PreparedColumn::Real(values) = column else {
                    return Err(TickError::Evaluation(
                        "view value changed type between tiles".to_owned(),
                    ));
                };
                for (offset, value) in values.iter().copied().enumerate() {
                    if filter.as_ref().is_some_and(|selected| !selected[offset]) {
                        continue;
                    }
                    result = match reduce {
                        ViewReduce::Sum => result + value,
                        ViewReduce::Min if value.total_cmp(&result) == Ordering::Less => value,
                        ViewReduce::Max if value.total_cmp(&result) == Ordering::Greater => value,
                        ViewReduce::Min | ViewReduce::Max => result,
                        ViewReduce::Count => unreachable!(),
                    };
                }
            }
            Ok(ObservationValue::Real(result))
        }
        Some(_) => Err(TickError::InvalidRuntimeType {
            context: "view value".to_owned(),
            found: "non-numeric".to_owned(),
        }),
        None => unreachable!("tiled views always contain at least one row"),
    }
}

type TiledViewResults = Vec<Option<Result<ObservationValue, TickError>>>;

fn collect_view_tiling_candidates(
    model: &ValidatedModel,
    snapshot: &Snapshot<'_>,
    results: &mut TiledViewResults,
) -> Vec<ViewTilingCandidate> {
    let mut candidates = Vec::new();
    let mut ordinal = 0;
    for (box_index, model_box) in model.model().boxes.iter().enumerate() {
        for (view_index, view) in model_box.views.iter().enumerate() {
            let table_index = model
                .table_index(box_index, &view.table)
                .expect("validated view table disappeared");
            let row_count = match snapshot.row_count(&model_box.name, &view.table) {
                Ok(row_count) => row_count,
                Err(error) => {
                    results[ordinal] = Some(Err(error.into()));
                    ordinal += 1;
                    continue;
                }
            };
            let table = match EvalTable::new(model, &model_box.name, &view.table) {
                Ok(table) => table,
                Err(error) => {
                    results[ordinal] = Some(Err(error.into()));
                    ordinal += 1;
                    continue;
                }
            };
            let mut profile = TiledPlanProfile::default();
            let filter_eligible = match &view.filter {
                Some(filter) => match tiled_expr_footprint(filter, table) {
                    Ok(Some(footprint)) => {
                        profile.include(footprint);
                        true
                    }
                    Ok(None) => false,
                    Err(error) => {
                        results[ordinal] = Some(Err(error.into()));
                        false
                    }
                },
                None => true,
            };
            let value_eligible = match view.reduce {
                ViewReduce::Count => true,
                ViewReduce::Sum | ViewReduce::Min | ViewReduce::Max => {
                    let expression = view
                        .value
                        .as_ref()
                        .expect("validated numeric view has a value");
                    match tiled_expr_footprint(expression, table) {
                        Ok(Some(footprint)) => {
                            profile.include(footprint);
                            true
                        }
                        Ok(None) => false,
                        Err(error) => {
                            results[ordinal] = Some(Err(error.into()));
                            false
                        }
                    }
                }
            };
            if filter_eligible && value_eligible {
                candidates.push(ViewTilingCandidate {
                    ordinal,
                    box_index,
                    view_index,
                    table_index,
                    row_count,
                    profile,
                });
            }
            ordinal += 1;
        }
    }
    candidates
}

fn prepare_view_tiling_plans<'state>(
    model: &ValidatedModel,
    snapshot: &'state Snapshot<'_>,
    params: &ParamEnv,
    candidates: &[ViewTilingCandidate],
    results: &mut TiledViewResults,
    config: &CpuExecutionConfig,
) -> Vec<PreparedView<'state>> {
    let mut plans = Vec::new();
    for group in group_candidates(candidates, |candidate| {
        (
            candidate.box_index,
            candidate.table_index,
            candidate.row_count,
        )
    }) {
        // A Count plan drops its filter immediately. Numeric roots remain live
        // until canonical ordered reduction, while work accumulates across plans.
        let mut node_count = 0_usize;
        let mut retained_numeric_roots = 0_usize;
        let mut live_set_bytes_per_row = 1_usize;
        for candidate_index in &group.indices {
            let candidate = &candidates[*candidate_index];
            node_count = node_count.saturating_add(candidate.profile.node_count);
            live_set_bytes_per_row = live_set_bytes_per_row.max(
                retained_numeric_roots.saturating_add(candidate.profile.live_set_bytes_per_row()),
            );
            if model.model().boxes[candidate.box_index].views[candidate.view_index].reduce
                != ViewReduce::Count
            {
                retained_numeric_roots = retained_numeric_roots
                    .saturating_add(candidate.profile.retained_root_bytes_per_row);
            }
        }
        let tile_rows = tick_tile_rows_for_live_set(config, live_set_bytes_per_row);
        if !tick_tiling_enabled(config, group.row_count, node_count) || group.row_count <= tile_rows
        {
            continue;
        }
        for candidate_index in group.indices {
            let candidate = &candidates[candidate_index];
            match prepare_view_tiling_plan(model, snapshot, params, candidate, tile_rows) {
                Ok(Some(plan)) => plans.push(plan),
                Ok(None) => {}
                Err(error) => results[candidate.ordinal] = Some(Err(error)),
            }
        }
    }
    plans
}

fn prepare_view_tiling_plan<'state>(
    model: &ValidatedModel,
    snapshot: &'state Snapshot<'_>,
    params: &ParamEnv,
    candidate: &ViewTilingCandidate,
    tile_rows: usize,
) -> Result<Option<PreparedView<'state>>, TickError> {
    let model_box = &model.model().boxes[candidate.box_index];
    let view = &model_box.views[candidate.view_index];
    let table = EvalTable::new(model, &model_box.name, &view.table)?;
    let filter = match &view.filter {
        Some(filter) => {
            let Some(filter) = prepare_row_expr(filter, table, snapshot, params)? else {
                return Ok(None);
            };
            Some(filter)
        }
        None => None,
    };
    let value = match view.reduce {
        ViewReduce::Count => None,
        ViewReduce::Sum | ViewReduce::Min | ViewReduce::Max => {
            let expression = view
                .value
                .as_ref()
                .expect("validated numeric view has a value");
            let Some(value) = prepare_row_expr(expression, table, snapshot, params)? else {
                return Ok(None);
            };
            Some(value)
        }
    };
    Ok(Some(PreparedView {
        ordinal: candidate.ordinal,
        box_index: candidate.box_index,
        view_index: candidate.view_index,
        table_index: candidate.table_index,
        row_count: candidate.row_count,
        tile_rows,
        reduce: view.reduce,
        filter,
        value,
    }))
}

fn reduce_view_task_outputs(
    model: &ValidatedModel,
    plans: &[PreparedView<'_>],
    task_outputs: Vec<Vec<ViewTileOutput<'_>>>,
    results: &mut TiledViewResults,
) {
    let mut plan_outputs = std::iter::repeat_with(Vec::new)
        .take(plans.len())
        .collect::<Vec<Vec<ViewTileOutput<'_>>>>();
    for outputs in task_outputs {
        for output in outputs {
            plan_outputs[output.plan_index].push(output);
        }
    }
    for (plan_index, plan) in plans.iter().enumerate() {
        let mut outputs = std::mem::take(&mut plan_outputs[plan_index]);
        outputs.sort_by_key(|output| output.start);
        let model_box = &model.model().boxes[plan.box_index];
        let view = &model_box.views[plan.view_index];
        let value = if plan.reduce == ViewReduce::Count {
            reduce_count_view_tiles(&model_box.name, &view.name, outputs)
        } else {
            reduce_numeric_view_tiles(&model_box.name, &view.name, plan.reduce, outputs)
        };
        results[plan.ordinal] = Some(value);
    }
}

fn reduce_count_view_tiles(
    box_name: &str,
    view_name: &str,
    outputs: Vec<ViewTileOutput<'_>>,
) -> Result<ObservationValue, TickError> {
    outputs
        .into_iter()
        .map(|output| output.count.expect("count tile must contain a partial"))
        .try_fold(0_usize, |total, count| count.map(|count| total + count))
        .and_then(|count| {
            i64::try_from(count).map_err(|_| {
                TickError::Evaluation(format!("view '{box_name}.{view_name}' count exceeds i64"))
            })
        })
        .map(ObservationValue::Int)
}

fn reduce_numeric_view_tiles(
    box_name: &str,
    view_name: &str,
    reduce: ViewReduce,
    outputs: Vec<ViewTileOutput<'_>>,
) -> Result<ObservationValue, TickError> {
    let mut filters = Vec::with_capacity(outputs.len());
    let mut columns = Vec::with_capacity(outputs.len());
    for output in outputs {
        let filter = match output.filter {
            Some(Ok(PreparedColumn::Bool(selected))) => Some(selected),
            Some(Ok(other)) => return Err(prepared_runtime_type("view filter", &other)),
            Some(Err(error)) => return Err(error),
            None => None,
        };
        let column = output
            .value
            .expect("numeric view tile must evaluate a value")?;
        filters.push(filter);
        columns.push(column);
    }
    reduce_prepared_view_tiles(box_name, view_name, reduce, columns, filters)
}

/// Prepares eligible committed-state views and opens one fixed-task parallel
/// region for observation. Count filters and row-local numeric expressions are
/// evaluated per tile. Numeric reductions happen only after every tile has
/// joined: integer operations combine in row order for identical overflow
/// behaviour, while `f64` values are accumulated one row at a time in canonical
/// ascending order. Aggregate/input-dependent or row-fallible expressions keep
/// the original whole-column path.
pub(super) fn prepare_tiled_views(
    model: &ValidatedModel,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    config: &CpuExecutionConfig,
) -> Vec<Option<Result<ObservationValue, TickError>>> {
    let view_count = model
        .model()
        .boxes
        .iter()
        .map(|model_box| model_box.views.len())
        .sum();
    let mut results = std::iter::repeat_with(|| None)
        .take(view_count)
        .collect::<TiledViewResults>();
    let candidates = collect_view_tiling_candidates(model, snapshot, &mut results);
    let plans =
        prepare_view_tiling_plans(model, snapshot, params, &candidates, &mut results, config);
    if plans.is_empty() {
        return results;
    }
    let tasks = build_tile_tasks(&plans, |plan| {
        (
            plan.box_index,
            plan.table_index,
            plan.tile_rows,
            plan.row_count,
        )
    });
    let task_outputs = evaluate_tile_tasks(&tasks, tick_worker_count(config), |task| {
        evaluate_view_tile_task(task, &plans)
    });
    reduce_view_task_outputs(model, &plans, task_outputs, &mut results);
    results
}
