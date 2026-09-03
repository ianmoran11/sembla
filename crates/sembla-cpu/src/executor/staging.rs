//! Per-box CPU candidate, conflict, and effect staging.

use super::*;

pub(super) fn stage_box(
    model: &ValidatedModel,
    box_index: usize,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
    tiled_candidates: &mut [Option<Result<Vec<Candidate>, TickError>>],
) -> Result<BoxOutcome, TickError> {
    let model_box = &model.model().boxes[box_index];
    let transitions: Vec<_> = model
        .transitions()
        .iter()
        .filter(|transition| transition.box_index == box_index)
        .collect();
    let mut cache = AggCache::new(model, snapshot, params);
    let mut candidates = Vec::new();
    for validated in &transitions {
        if let Some(result) = tiled_candidates[validated.transition_index].take() {
            candidates.extend(result?);
            continue;
        }
        let transition = &model_box.transitions[validated.transition_index];
        let table_index = model_box
            .tables
            .iter()
            .position(|table| table.name == transition.table)
            .expect("validated transition table disappeared");
        let table = EvalTable::new(model, &model_box.name, &transition.table)?;
        let guards = match eval_column(&transition.guard, table, snapshot, params, &mut cache)? {
            ValueColumn::Bool(values) => values,
            other => return Err(runtime_type("transition guard", &other)),
        };
        let hazards = match eval_column(&transition.hazard, table, snapshot, params, &mut cache)? {
            ValueColumn::Real(values) => values,
            other => return Err(runtime_type("transition hazard", &other)),
        };
        let mut claim_columns = Vec::with_capacity(transition.contests.len());
        for claim in &transition.contests {
            let resources =
                eval_typed_ref_column(&claim.resource, table, snapshot, params, &mut cache)?;
            let resource_table_index = model_box
                .tables
                .iter()
                .position(|schema| schema.name == resources.target_table)
                .expect("validated Ref target table disappeared");
            let ordering = match &claim.ordering {
                ClaimOrdering::RaceTime => None,
                ClaimOrdering::Key { expr } => {
                    Some(eval_column(expr, table, snapshot, params, &mut cache)?)
                }
            };
            claim_columns.push((resource_table_index, resources.values, ordering, claim));
        }
        let race_strategy = if has_constant_hazard(&transition.hazard) {
            RacingClockStrategy::for_transition(
                hazards
                    .first()
                    .and_then(|lambda| RacingClockFilter::for_hazard(*lambda, model.model().dt)),
                transition,
            )
        } else {
            RacingClockStrategy::Canonical
        };
        let mut push_candidate = |row: usize, firing: CandidateFiring| -> Result<(), TickError> {
            let mut claims = Vec::with_capacity(claim_columns.len());
            for (resource_table, resources, key_column, claim) in &claim_columns {
                let ordering = match (&claim.ordering, key_column) {
                    (ClaimOrdering::RaceTime, None) => OrderingValue::RaceTime(
                        firing
                            .race_time
                            .expect("a contested transition must sample its race time"),
                    ),
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
                rule_word: validated.rule_word,
                table_index,
                entity_id: firing.entity_id,
                row,
                claims,
            });
            Ok(())
        };
        for (row, (guard, lambda)) in guards.into_iter().zip(hazards).enumerate() {
            if !guard || lambda.partial_cmp(&0.0) != Some(Ordering::Greater) {
                continue;
            }
            let Some(firing) = candidate_race_time(
                RacingClockCoordinates {
                    seed,
                    tick,
                    rule_id: validated.rule_id,
                    rule_word: validated.rule_word,
                    row,
                },
                lambda,
                model.model().dt,
                race_strategy,
            )?
            else {
                continue;
            };
            push_candidate(row, firing)?;
        }
    }
    let resolution = resolve_claims(&candidates, model_box.tables.len(), model_box)?;
    let mut destinations = Vec::new();
    let mut pending = Vec::new();
    for validated in &transitions {
        let transition = &model_box.transitions[validated.transition_index];
        let winner_indices: Vec<usize> = candidates
            .iter()
            .enumerate()
            .filter(|(index, candidate)| {
                candidate.rule_id == validated.rule_id && resolution.fires[*index]
            })
            .map(|(index, _)| index)
            .collect();
        if winner_indices.is_empty() {
            continue;
        }
        let table = EvalTable::new(model, &model_box.name, &transition.table)?;
        let table_index = candidates[winner_indices[0]].table_index;
        let schema = &model_box.tables[table_index];
        let mut winner_rows = None;
        let mut effect_columns = Vec::with_capacity(transition.effects.len());
        for effect in &transition.effects {
            let Effect::SetAttr { attr, value } = effect;
            let attr_index = schema
                .attrs
                .iter()
                .position(|declaration| declaration.name == *attr)
                .expect("validated effect attribute disappeared");
            let destination = &schema.attrs[attr_index];
            let effect_table = match &destination.ty {
                AttrType::Ref { .. } => table,
                _ => table.with_expected_attr(attr)?,
            };
            let gather = expr_is_gather_eligible(value, effect_table)?;
            let rows = gather.then(|| {
                winner_rows.get_or_insert_with(|| {
                    let rows = winner_indices
                        .iter()
                        .map(|index| candidates[*index].row)
                        .collect::<Vec<_>>();
                    debug_assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));
                    rows
                })
            });
            let (values, gathered) = match (&destination.ty, rows) {
                (AttrType::Ref { .. }, Some(rows)) => (
                    PendingColumn::Ref(
                        eval_typed_ref_gather(
                            value,
                            effect_table,
                            rows,
                            snapshot,
                            params,
                            &mut cache,
                        )?
                        .expect("gather eligibility and preparation must agree")
                        .values,
                    ),
                    true,
                ),
                (AttrType::Ref { .. }, None) => (
                    PendingColumn::Ref(
                        eval_typed_ref_column(value, effect_table, snapshot, params, &mut cache)?
                            .values,
                    ),
                    false,
                ),
                (_, Some(rows)) => (
                    PendingColumn::Value(
                        eval_gather(value, effect_table, rows, snapshot, params, &mut cache)?
                            .expect("gather eligibility and preparation must agree"),
                    ),
                    true,
                ),
                (_, None) => (
                    PendingColumn::Value(eval_column(
                        value,
                        effect_table,
                        snapshot,
                        params,
                        &mut cache,
                    )?),
                    false,
                ),
            };
            // Resolve once now, but publish a failure only at this effect's
            // first pending write. That retains later effect-evaluation,
            // DoubleWrite, and write-buffer error precedence from the original
            // per-write lookup path.
            let destination_index = destinations.len();
            destinations.push(PendingDestination {
                box_index,
                table_index,
                attr_index,
                resolution: snapshot.resolve_write_column(
                    &model_box.name,
                    &schema.name,
                    &destination.name,
                ),
            });
            effect_columns.push(EffectColumn {
                destination_index,
                values,
                gathered,
            });
        }
        for (winner_offset, candidate_index) in winner_indices.into_iter().enumerate() {
            let candidate = &candidates[candidate_index];
            for effect in &effect_columns {
                let value_index = if effect.gathered {
                    winner_offset
                } else {
                    candidate.row
                };
                pending.push(PendingWrite {
                    destination_index: effect.destination_index,
                    row: candidate.row,
                    value: effect.values.at(value_index)?,
                    rule_id: candidate.rule_id,
                });
            }
        }
    }
    let mut fired = transitions
        .iter()
        .map(|transition| (transition.rule_id, 0))
        .collect::<Vec<_>>();
    for (candidate, fire) in candidates.iter().zip(&resolution.fires) {
        if *fire {
            let entry = fired
                .iter_mut()
                .find(|(rule_id, _)| *rule_id == candidate.rule_id)
                .expect("candidate has validated transition");
            entry.1 += 1;
        }
    }
    Ok(BoxOutcome {
        destinations,
        pending,
        fired,
        deferred: resolution.deferred,
        fired_per_resource_table: resolution.fired_per_resource_table,
        aggregate_builds: cache.build_count(),
    })
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
            lhs_candidate.rule_word,
            lhs_candidate.entity_id,
        )
            .cmp(&(
                rhs_claim.table_index,
                rhs_claim.resource_row,
                rhs_candidate.rule_word,
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
        (lhs_candidate.rule_word, lhs_candidate.entity_id)
            .cmp(&(rhs_candidate.rule_word, rhs_candidate.entity_id))
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

struct EffectColumn {
    destination_index: usize,
    values: PendingColumn,
    gathered: bool,
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

pub(super) fn detect_double_writes(
    pending: &[PendingWrite],
    destinations: &[PendingDestination],
    model: &ValidatedModel,
) -> Result<(), TickError> {
    DOUBLE_WRITE_SCRATCH.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        scratch.prepare(pending, destinations);

        // The replaced stable sort and map both reported the lexicographically
        // first duplicated cell. The bitmap finds that cell without retaining
        // writer identity; the terminating error path recovers the first two
        // push-order writers with one linear scan.
        let mut collision = None;
        for write in pending {
            if scratch.mark(write) {
                let cell = write_cell(write, destinations);
                if collision.map_or(true, |reported| cell < reported) {
                    collision = Some(cell);
                }
            }
        }

        let Some(collision) = collision else {
            return Ok(());
        };
        let mut first_index = None;
        for (index, write) in pending.iter().enumerate() {
            if write_cell(write, destinations) != collision {
                continue;
            }
            let Some(first_index) = first_index else {
                first_index = Some(index);
                continue;
            };
            return double_write_error(&pending[first_index], write, destinations, model);
        }
        unreachable!("a bitmap collision must have at least two writers")
    })
}

fn write_cell(write: &PendingWrite, destinations: &[PendingDestination]) -> WriteCell {
    let destination = &destinations[write.destination_index];
    (
        destination.box_index,
        destination.table_index,
        destination.attr_index,
        write.row,
    )
}

fn double_write_error(
    first: &PendingWrite,
    second: &PendingWrite,
    destinations: &[PendingDestination],
    model: &ValidatedModel,
) -> Result<(), TickError> {
    let destination = &destinations[first.destination_index];
    let model_box = &model.model().boxes[destination.box_index];
    Err(TickError::DoubleWrite {
        box_name: model_box.name.clone().into_boxed_str(),
        table: model_box.tables[destination.table_index]
            .name
            .clone()
            .into_boxed_str(),
        attr: model_box.tables[destination.table_index].attrs[destination.attr_index]
            .name
            .clone()
            .into_boxed_str(),
        row: first.row,
        first_rule_id: first.rule_id,
        first_transition: transition_name(model, first.rule_id).into(),
        second_rule_id: second.rule_id,
        second_transition: transition_name(model, second.rule_id).into(),
    })
}

fn transition_name(model: &ValidatedModel, rule_id: u32) -> &str {
    let validated = model
        .transitions()
        .iter()
        .find(|transition| transition.rule_id == rule_id)
        .expect("pending write has a validated transition");
    &model.model().boxes[validated.box_index].transitions[validated.transition_index].name
}
