//! Ordered CUDA tick launch pipeline and device-status commit.

use super::{
    commit_validation_reduction, control_count_launch_config, device_status, driver_error,
    fused_launch_builder, global_table, mem, CudaBackend, CudaError, LaunchConfig,
    VALIDATION_REDUCTION_PASSES,
};

impl CudaBackend {
    fn validation_launch_config(&self, rows: u32, phase: u64, one: LaunchConfig) -> LaunchConfig {
        if rows == 0 {
            return one;
        }
        #[cfg(test)]
        if let Some(geometry) = self.validation_launch_override {
            return geometry.config();
        }
        let mut config = LaunchConfig::for_num_elems(rows);
        // Recovery passes return at entry on success. Their grid-stride loops
        // still visit every row on error, without scheduling a full-size grid
        // for the overwhelmingly common successful path.
        if phase != 0 {
            config.grid_dim.0 = config.grid_dim.0.min(32);
        }
        config
    }

    fn conflict_launch_config(&self, elements: u32, one: LaunchConfig) -> LaunchConfig {
        if elements == 0 {
            return one;
        }
        #[cfg(test)]
        if let Some(geometry) = self.conflict_launch_override {
            return geometry.config();
        }
        LaunchConfig::for_num_elems(elements)
    }

    pub(super) fn execute_tick(&mut self) -> Result<(), CudaError> {
        self.execute_tick_batch_statuses()?
            .into_iter()
            .next()
            .unwrap_or(Ok(()))
    }

    pub(super) fn execute_tick_batch_statuses(
        &mut self,
    ) -> Result<Vec<Result<(), CudaError>>, CudaError> {
        let one = LaunchConfig {
            grid_dim: (1, 1, 1),
            block_dim: (1, 1, 1),
            shared_mem_bytes: 0,
        };
        self.reset_and_build_state_aggregates(one)?;
        self.schedule_resolve_and_validate_boxes(one)?;
        self.prepare_and_apply_effects(one)?;
        self.prepare_outputs(one)?;
        self.reduce_controls_and_commit_tick()
    }

    fn reset_and_build_state_aggregates(&mut self, one: LaunchConfig) -> Result<(), CudaError> {
        let aggregate_error_count = (self.layout.aggregate_max_groups + 2) as u64;
        {
            let mut args =
                fused_launch_builder(&self.stream, &self.reset_status, self.fused_batch.as_ref());
            args.arg(&mut self.status)
                .arg(&mut self.aggregate_errors)
                .arg(&aggregate_error_count);
            args.launch_generated(one).map_err(driver_error)?;
        }
        {
            let rule_count = self.layout.candidate_offsets.len() as u64;
            let mut args = fused_launch_builder(
                &self.stream,
                &self.init_validation_scratch,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.status)
                .arg(&mut self.effect_active)
                .arg(&rule_count);
            args.launch_generated(one).map_err(driver_error)?;
        }
        let rule_count = self.layout.candidate_offsets.len() as u64;
        let table_count = self.layout.row_counts.len() as u64;
        {
            let mut args = fused_launch_builder(
                &self.stream,
                &self.init_control_counts,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.fired_counts)
                .arg(&rule_count)
                .arg(&mut self.deferred_counts)
                .arg(&table_count);
            args.launch_generated(control_count_launch_config(rule_count.max(table_count)))
                .map_err(driver_error)?;
        }
        // Build all tick-start aggregates without committing errors. Each
        // aggregate leaves a device error fact which the ordered validators
        // surface only when the CPU evaluator would first reach that node.
        let require_active = 0_u8;
        for aggregate_slot in self.generated.state_aggregate_indices.clone() {
            let group_table = self.generated.aggregate_group_tables[aggregate_slot];
            let aggregate_index = u32::try_from(aggregate_slot)
                .map_err(|_| CudaError::InvalidInput("aggregate count exceeds u32".to_owned()))?;
            let mut args = fused_launch_builder(
                &self.stream,
                &self.build_aggregate_partials,
                self.fused_batch.as_ref(),
            );
            args.arg(&self.state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.inputs)
                .arg(&self.input_offsets)
                .arg(&self.input_counts)
                .arg(&self.params)
                .arg(&self.aggregates)
                .arg(&self.aggregate_facts)
                .arg(&aggregate_index)
                .arg(&self.aggregate_active)
                .arg(&require_active)
                .arg(&mut self.aggregate_partials)
                .arg(&self.aggregate_offsets)
                .arg(&mut self.aggregate_errors);
            args.launch_generated(one).map_err(driver_error)?;
            let groups = u32::try_from(self.layout.row_counts[group_table]).map_err(|_| {
                CudaError::InvalidInput("aggregate group count exceeds u32".to_owned())
            })?;
            if groups != 0 {
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.finish_aggregates,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.aggregate_partials)
                    .arg(&self.row_counts)
                    .arg(&aggregate_index)
                    .arg(&self.aggregate_active)
                    .arg(&require_active)
                    .arg(&mut self.aggregates)
                    .arg(&self.aggregate_offsets)
                    .arg(&mut self.aggregate_errors);
                args.launch_generated(LaunchConfig::for_num_elems(groups))
                    .map_err(driver_error)?;
            }
            let aggregate_identity = u64::from(aggregate_index);
            let mut args = fused_launch_builder(
                &self.stream,
                &self.record_aggregate_errors,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.aggregate_errors)
                .arg(&aggregate_error_count)
                .arg(&aggregate_identity)
                .arg(&mut self.aggregate_facts);
            args.launch_generated(one).map_err(driver_error)?;
        }

        Ok(())
    }

    fn schedule_resolve_and_validate_boxes(&mut self, one: LaunchConfig) -> Result<(), CudaError> {
        // Mirror stage_box: schedule, resolve, and validate winning effects
        // for one box before any expression in the following box is reached.
        for box_index in 0..self.model.model().boxes.len() {
            let transition_positions = self
                .model
                .transitions()
                .iter()
                .enumerate()
                .filter_map(|(index, transition)| {
                    (transition.box_index == box_index).then_some((index, transition))
                })
                .collect::<Vec<_>>();

            for (index, transition) in &transition_positions {
                let rule_id = transition.rule_id;
                let table_index = transition.table_index;
                let global_table = global_table(&self.model, transition.box_index, table_index);
                let rows = u32::try_from(self.layout.row_counts[global_table]).map_err(|_| {
                    CudaError::InvalidInput(format!(
                        "rule {} row count exceeds u32 entity IDs",
                        transition.rule_id
                    ))
                })?;
                // Scalar input/aggregate checks inside the kernel still need
                // one worker when the table is empty, so zero rows keeps a
                // single-thread launch instead of a zero-block one.
                if self.generated.transition_validation[*index] {
                    for validation_phase in 0..VALIDATION_REDUCTION_PASSES {
                        let validation_config =
                            self.validation_launch_config(rows, validation_phase, one);
                        {
                            let mut args = fused_launch_builder(
                                &self.stream,
                                &self.validate_transition,
                                self.fused_batch.as_ref(),
                            );
                            args.arg(&self.state)
                                .arg(&self.column_offsets)
                                .arg(&self.row_counts)
                                .arg(&self.inputs)
                                .arg(&self.input_offsets)
                                .arg(&self.input_counts)
                                .arg(&self.params)
                                .arg(&self.aggregates)
                                .arg(&self.aggregate_facts)
                                .arg(&self.aggregate_offsets)
                                .arg(&self.candidate_offsets)
                                .arg(&rule_id)
                                .arg(&mut self.status)
                                .arg(&validation_phase);
                            args.launch_generated(validation_config)
                                .map_err(driver_error)?;
                        }
                    }
                    commit_validation_reduction(
                        &self.stream,
                        &self.commit_validation_status,
                        &mut self.status,
                        one,
                        self.fused_batch.as_ref(),
                    )?;
                }

                if rows == 0 {
                    continue;
                }
                let dt = self.model.model().dt;
                let tick = self.next_tick;
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.transition_functions[*index],
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.state)
                    .arg(&self.column_offsets)
                    .arg(&self.row_counts)
                    .arg(&self.inputs)
                    .arg(&self.input_offsets)
                    .arg(&self.input_counts)
                    .arg(&self.params)
                    .arg(&self.aggregates)
                    .arg(&self.aggregate_offsets)
                    .arg(&self.candidate_offsets)
                    .arg(&self.seed)
                    .arg(&tick)
                    .arg(&dt)
                    .arg(&mut self.enabled)
                    .arg(&mut self.times)
                    .arg(&self.status);
                args.launch_generated(LaunchConfig::for_num_elems(rows))
                    .map_err(driver_error)?;
            }

            let box_index_u32 = u32::try_from(box_index)
                .map_err(|_| CudaError::InvalidInput("box count exceeds u32".to_owned()))?;
            {
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.validate_claim_compatibility,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.state)
                    .arg(&self.column_offsets)
                    .arg(&self.row_counts)
                    .arg(&self.inputs)
                    .arg(&self.input_offsets)
                    .arg(&self.input_counts)
                    .arg(&self.params)
                    .arg(&self.aggregates)
                    .arg(&self.aggregate_offsets)
                    .arg(&self.candidate_offsets)
                    .arg(&self.enabled)
                    .arg(&box_index_u32)
                    .arg(&mut self.status);
                args.launch_generated(one).map_err(driver_error)?;
            }

            let mut candidate_begin = 0_u64;
            let mut candidate_count = 0_u64;
            let mut claim_instance_begin = 0_u64;
            let mut claim_instance_count = 0_u64;
            if let Some((_, first)) = transition_positions.first() {
                let rule_index = usize::try_from(first.rule_id).map_err(|_| {
                    CudaError::InvalidInput("rule id exceeds host index width".to_owned())
                })?;
                candidate_begin = self.layout.candidate_offsets[rule_index];
                claim_instance_begin = self.layout.claim_instance_offsets[rule_index];
                for (_, transition) in &transition_positions {
                    let model_transition = &self.model.model().boxes[transition.box_index]
                        .transitions[transition.transition_index];
                    let table_index = transition.table_index;
                    let global_table = global_table(&self.model, transition.box_index, table_index);
                    let rows = self.layout.row_counts[global_table];
                    candidate_count = candidate_count.checked_add(rows).ok_or_else(|| {
                        CudaError::InvalidInput("box candidate count overflow".to_owned())
                    })?;
                    let claims = u64::try_from(model_transition.contests.len()).map_err(|_| {
                        CudaError::InvalidInput("claim count exceeds u64".to_owned())
                    })?;
                    claim_instance_count = claim_instance_count
                        .checked_add(rows.checked_mul(claims).ok_or_else(|| {
                            CudaError::InvalidInput("box claim-instance count overflow".to_owned())
                        })?)
                        .ok_or_else(|| {
                            CudaError::InvalidInput("box claim-instance count overflow".to_owned())
                        })?;
                }
            }
            if candidate_count != 0 {
                let candidate_launch_count = u32::try_from(candidate_count).map_err(|_| {
                    CudaError::InvalidInput(
                        "box candidate count exceeds CUDA launch capacity".to_owned(),
                    )
                })?;
                let candidate_config = self.conflict_launch_config(candidate_launch_count, one);
                let resource_table_count = self.generated.resource_tables.len() as u64;

                if claim_instance_count != 0 {
                    let instance_launch_count =
                        u32::try_from(claim_instance_count).map_err(|_| {
                            CudaError::InvalidInput(
                                "box claim-instance count exceeds CUDA launch capacity".to_owned(),
                            )
                        })?;
                    let resource_count =
                        u64::try_from(self.layout.resource_count).map_err(|_| {
                            CudaError::InvalidInput("resource count exceeds u64".to_owned())
                        })?;
                    let resource_launch_count = u32::try_from(resource_count).map_err(|_| {
                        CudaError::InvalidInput(
                            "resource count exceeds CUDA launch capacity".to_owned(),
                        )
                    })?;
                    let resource_config = self.conflict_launch_config(resource_launch_count, one);
                    let instance_config = self.conflict_launch_config(instance_launch_count, one);

                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.init_conflict_winners,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&resource_count)
                        .arg(&mut self.winner_keys)
                        .arg(&mut self.winner_rules)
                        .arg(&mut self.winner_entities);
                    args.launch_generated(resource_config)
                        .map_err(driver_error)?;

                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.build_claim_instances,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&self.state)
                        .arg(&self.column_offsets)
                        .arg(&self.row_counts)
                        .arg(&self.inputs)
                        .arg(&self.input_offsets)
                        .arg(&self.input_counts)
                        .arg(&self.params)
                        .arg(&self.aggregates)
                        .arg(&self.aggregate_offsets)
                        .arg(&self.candidate_offsets)
                        .arg(&self.claim_instance_offsets)
                        .arg(&self.resource_offsets)
                        .arg(&candidate_begin)
                        .arg(&candidate_count)
                        .arg(&self.enabled)
                        .arg(&self.times)
                        .arg(&mut self.instance_resources)
                        .arg(&mut self.instance_keys)
                        .arg(&mut self.instance_rules)
                        .arg(&mut self.instance_entities)
                        .arg(&self.status);
                    args.launch_generated(candidate_config)
                        .map_err(driver_error)?;

                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.reduce_claim_keys,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&claim_instance_begin)
                        .arg(&claim_instance_count)
                        .arg(&self.instance_resources)
                        .arg(&self.instance_keys)
                        .arg(&mut self.winner_keys);
                    args.launch_generated(instance_config)
                        .map_err(driver_error)?;

                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.reduce_claim_rules,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&claim_instance_begin)
                        .arg(&claim_instance_count)
                        .arg(&self.instance_resources)
                        .arg(&self.instance_keys)
                        .arg(&self.instance_rules)
                        .arg(&self.winner_keys)
                        .arg(&mut self.winner_rules);
                    args.launch_generated(instance_config)
                        .map_err(driver_error)?;

                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.reduce_claim_entities,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&claim_instance_begin)
                        .arg(&claim_instance_count)
                        .arg(&self.instance_resources)
                        .arg(&self.instance_keys)
                        .arg(&self.instance_rules)
                        .arg(&self.instance_entities)
                        .arg(&self.winner_keys)
                        .arg(&self.winner_rules)
                        .arg(&mut self.winner_entities);
                    args.launch_generated(instance_config)
                        .map_err(driver_error)?;
                }

                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.resolve_conflicts,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.row_counts)
                    .arg(&self.candidate_offsets)
                    .arg(&self.claim_instance_offsets)
                    .arg(&candidate_begin)
                    .arg(&candidate_count)
                    .arg(&resource_table_count)
                    .arg(&self.enabled)
                    .arg(&self.instance_resources)
                    .arg(&self.winner_rules)
                    .arg(&self.winner_entities)
                    .arg(&mut self.wins)
                    .arg(&mut self.deferred)
                    .arg(&self.status);
                args.launch_generated(candidate_config)
                    .map_err(driver_error)?;
            }

            // Count each rule once, before validation consumes its activity
            // flag. The same reduction supplies the final host report.
            let mut effects_rows = 0_u32;
            for (_, transition) in &transition_positions {
                let table_index = transition.table_index;
                let global_table = global_table(&self.model, transition.box_index, table_index);
                let rows = u32::try_from(self.layout.row_counts[global_table]).map_err(|_| {
                    CudaError::InvalidInput(format!(
                        "rule {} row count exceeds u32 entity IDs",
                        transition.rule_id
                    ))
                })?;
                effects_rows = effects_rows.max(rows);
                if rows == 0 {
                    continue;
                }
                let rule_index = usize::try_from(transition.rule_id).map_err(|_| {
                    CudaError::InvalidInput("rule id exceeds host index width".to_owned())
                })?;
                let candidate_count = self.layout.candidate_count as u64;
                let rule_count = self.layout.candidate_offsets.len() as u64;
                let rule = rule_index as u64;
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.count_fired,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.wins)
                    .arg(&self.candidate_offsets)
                    .arg(&candidate_count)
                    .arg(&rule_count)
                    .arg(&rule)
                    .arg(&mut self.fired_counts)
                    .arg(&mut self.effect_active);
                args.launch_generated(control_count_launch_config(u64::from(rows)))
                    .map_err(driver_error)?;
            }
            for validation_phase in 0..VALIDATION_REDUCTION_PASSES {
                let effects_config =
                    self.validation_launch_config(effects_rows, validation_phase, one);
                {
                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.validate_effects,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&self.state)
                        .arg(&self.column_offsets)
                        .arg(&self.row_counts)
                        .arg(&self.inputs)
                        .arg(&self.input_offsets)
                        .arg(&self.input_counts)
                        .arg(&self.params)
                        .arg(&self.aggregates)
                        .arg(&self.aggregate_facts)
                        .arg(&self.aggregate_offsets)
                        .arg(&self.candidate_offsets)
                        .arg(&self.wins)
                        .arg(&self.effect_active)
                        .arg(&box_index_u32)
                        .arg(&mut self.status)
                        .arg(&validation_phase);
                    args.launch_generated(effects_config)
                        .map_err(driver_error)?;
                }
            }
            commit_validation_reduction(
                &self.stream,
                &self.commit_validation_status,
                &mut self.status,
                one,
                self.fused_batch.as_ref(),
            )?;
        }
        Ok(())
    }

    fn prepare_and_apply_effects(&mut self, one: LaunchConfig) -> Result<(), CudaError> {
        self.stream
            .memcpy_dtod(&self.state, &mut self.next_state)
            .map_err(driver_error)?;
        let owner_count = self.layout.owner_count as u64;
        let owner_launch_count = u32::try_from(self.layout.owner_count).map_err(|_| {
            CudaError::InvalidInput("write-owner count exceeds CUDA launch capacity".to_owned())
        })?;
        if owner_launch_count != 0 {
            let mut args = fused_launch_builder(
                &self.stream,
                &self.init_effect_owners,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.owners).arg(&owner_count);
            args.launch_generated(LaunchConfig::for_num_elems(owner_launch_count))
                .map_err(driver_error)?;
        }
        for transition in self.model.transitions() {
            let model_transition = &self.model.model().boxes[transition.box_index].transitions
                [transition.transition_index];
            if model_transition.effects.is_empty() {
                continue;
            }
            let table_index = transition.table_index;
            let global_table = global_table(&self.model, transition.box_index, table_index);
            let rows = u32::try_from(self.layout.row_counts[global_table]).map_err(|_| {
                CudaError::InvalidInput(format!(
                    "rule {} row count exceeds u32 entity IDs",
                    transition.rule_id
                ))
            })?;
            if rows == 0 {
                continue;
            }
            let rule_id = transition.rule_id;
            let exclusive = self.generated.exclusive_effect_writes[rule_id as usize];
            let passes = if exclusive {
                1
            } else {
                VALIDATION_REDUCTION_PASSES
            };
            for validation_phase in 0..passes {
                let config = self.validation_launch_config(rows, validation_phase, one);
                {
                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.prepare_effects,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&self.state)
                        .arg(&self.column_offsets)
                        .arg(&self.row_counts)
                        .arg(&self.inputs)
                        .arg(&self.input_offsets)
                        .arg(&self.input_counts)
                        .arg(&self.params)
                        .arg(&self.aggregates)
                        .arg(&self.aggregate_offsets)
                        .arg(&self.candidate_offsets)
                        .arg(&self.wins)
                        .arg(&self.write_offsets)
                        .arg(&mut self.owners)
                        .arg(&mut self.owner_values)
                        .arg(&rule_id)
                        .arg(&mut self.status)
                        .arg(&validation_phase);
                    args.launch_generated(config).map_err(driver_error)?;
                }
            }
            if !exclusive {
                commit_validation_reduction(
                    &self.stream,
                    &self.commit_validation_status,
                    &mut self.status,
                    one,
                    self.fused_batch.as_ref(),
                )?;
            }
        }
        if self.layout.owner_count != 0 {
            let launch_count = owner_launch_count;
            let mut args =
                fused_launch_builder(&self.stream, &self.apply_effects, self.fused_batch.as_ref());
            args.arg(&mut self.next_state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.write_offsets)
                .arg(&self.owners)
                .arg(&self.owner_values)
                .arg(&owner_count)
                .arg(&self.status);
            args.launch_generated(LaunchConfig::for_num_elems(launch_count))
                .map_err(driver_error)?;
        }
        Ok(())
    }

    fn prepare_outputs(&mut self, one: LaunchConfig) -> Result<(), CudaError> {
        if self.layout.ports.is_empty() {
            return Ok(());
        }
        // Moore outputs observe prospective state, so rebuild only aggregates
        // reachable from wired output expressions against next_state.
        let require_active = 0_u8;
        let aggregate_error_count = (self.layout.aggregate_max_groups + 2) as u64;
        for &aggregate_slot in &self.generated.output_aggregate_indices {
            let group_table = self.generated.aggregate_group_tables[aggregate_slot];
            let aggregate_index = u32::try_from(aggregate_slot)
                .map_err(|_| CudaError::InvalidInput("aggregate count exceeds u32".to_owned()))?;
            let mut args = fused_launch_builder(
                &self.stream,
                &self.build_aggregate_partials,
                self.fused_batch.as_ref(),
            );
            args.arg(&self.next_state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.inputs)
                .arg(&self.input_offsets)
                .arg(&self.input_counts)
                .arg(&self.params)
                .arg(&self.aggregates)
                .arg(&self.aggregate_facts)
                .arg(&aggregate_index)
                .arg(&self.aggregate_active)
                .arg(&require_active)
                .arg(&mut self.aggregate_partials)
                .arg(&self.aggregate_offsets)
                .arg(&mut self.aggregate_errors);
            args.launch_generated(LaunchConfig::for_num_elems(1))
                .map_err(driver_error)?;
            let groups = u32::try_from(self.layout.row_counts[group_table]).map_err(|_| {
                CudaError::InvalidInput("aggregate group count exceeds u32".to_owned())
            })?;
            if groups != 0 {
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.finish_aggregates,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.aggregate_partials)
                    .arg(&self.row_counts)
                    .arg(&aggregate_index)
                    .arg(&self.aggregate_active)
                    .arg(&require_active)
                    .arg(&mut self.aggregates)
                    .arg(&self.aggregate_offsets)
                    .arg(&mut self.aggregate_errors);
                args.launch_generated(LaunchConfig::for_num_elems(groups))
                    .map_err(driver_error)?;
            }
            let aggregate_identity = u64::from(aggregate_index);
            let mut args = fused_launch_builder(
                &self.stream,
                &self.record_aggregate_errors,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.aggregate_errors)
                .arg(&aggregate_error_count)
                .arg(&aggregate_identity)
                .arg(&mut self.aggregate_facts);
            args.launch_generated(one).map_err(driver_error)?;
        }
        {
            // The validator grid-strides each wired output's source table, so
            // the launch covers the largest wired source table; scalar
            // input/aggregate checks still get one worker when no wired
            // table has rows.
            let mut output_rows = 0_u32;
            for wire in self.model.wires() {
                let from_box = wire.from_box_index;
                let output = &self.model.model().boxes[from_box].outputs[wire.output_index];
                let sembla_ir::OutputBuilder::PerTable { table, .. } = &output.builder;
                let table_index = self.model.table_index(from_box, table).ok_or_else(|| {
                    CudaError::InvalidInput("wire source table missing".to_owned())
                })?;
                let global = global_table(&self.model, from_box, table_index);
                let rows = u32::try_from(self.layout.row_counts[global]).map_err(|_| {
                    CudaError::InvalidInput(
                        "wired output row count exceeds CUDA launch capacity".to_owned(),
                    )
                })?;
                output_rows = output_rows.max(rows);
            }
            for validation_phase in 0..VALIDATION_REDUCTION_PASSES {
                let output_config =
                    self.validation_launch_config(output_rows, validation_phase, one);
                {
                    let mut args = fused_launch_builder(
                        &self.stream,
                        &self.validate_outputs,
                        self.fused_batch.as_ref(),
                    );
                    args.arg(&self.next_state)
                        .arg(&self.column_offsets)
                        .arg(&self.row_counts)
                        .arg(&self.inputs)
                        .arg(&self.input_offsets)
                        .arg(&self.input_counts)
                        .arg(&self.params)
                        .arg(&self.aggregates)
                        .arg(&self.aggregate_facts)
                        .arg(&self.aggregate_offsets)
                        .arg(&mut self.status)
                        .arg(&validation_phase);
                    args.launch_generated(output_config).map_err(driver_error)?;
                }
            }
            commit_validation_reduction(
                &self.stream,
                &self.commit_validation_status,
                &mut self.status,
                one,
                self.fused_batch.as_ref(),
            )?;
        }
        {
            let port_count = self.layout.ports.len() as u64;
            let field_count = self.layout.input_offsets.len() as u64;
            let error_count = field_count.saturating_mul(3).max(3);
            let mut args = fused_launch_builder(
                &self.stream,
                &self.prepare_outputs,
                self.fused_batch.as_ref(),
            );
            args.arg(&mut self.next_input_counts)
                .arg(&port_count)
                .arg(&mut self.output_errors)
                .arg(&error_count);
            args.launch_generated(one).map_err(driver_error)?;
        }
        if !self.layout.input_offsets.is_empty() {
            let field_count = u32::try_from(self.layout.input_offsets.len()).map_err(|_| {
                CudaError::InvalidInput(
                    "output field count exceeds CUDA launch capacity".to_owned(),
                )
            })?;
            let mut args = fused_launch_builder(
                &self.stream,
                &self.build_output_partials,
                self.fused_batch.as_ref(),
            );
            args.arg(&self.next_state)
                .arg(&self.column_offsets)
                .arg(&self.row_counts)
                .arg(&self.inputs)
                .arg(&self.input_offsets)
                .arg(&self.input_counts)
                .arg(&self.params)
                .arg(&self.aggregates)
                .arg(&self.aggregate_offsets)
                .arg(&mut self.output_partials)
                .arg(&mut self.output_errors)
                .arg(&self.status);
            args.launch_generated(LaunchConfig::for_num_elems(field_count))
                .map_err(driver_error)?;
            let field_count_u64 = u64::from(field_count);
            let mut args = fused_launch_builder(
                &self.stream,
                &self.finish_outputs,
                self.fused_batch.as_ref(),
            );
            args.arg(&self.output_partials)
                .arg(&field_count_u64)
                .arg(&mut self.next_inputs)
                .arg(&self.input_offsets)
                .arg(&mut self.output_errors);
            args.launch_generated(LaunchConfig::for_num_elems(field_count))
                .map_err(driver_error)?;
            let mut args = fused_launch_builder(
                &self.stream,
                &self.check_output_errors,
                self.fused_batch.as_ref(),
            );
            args.arg(&self.output_errors)
                .arg(&field_count_u64)
                .arg(&mut self.status);
            args.launch_generated(one).map_err(driver_error)?;
        }
        Ok(())
    }

    fn reduce_controls_and_commit_tick(&mut self) -> Result<Vec<Result<(), CudaError>>, CudaError> {
        // These diagnostics are consumed only by the host report. Reduce them
        // while the raw control buffers remain device-resident; the terminal
        // status readback below orders all three kernels before compact D2H.
        let table_count = u64::try_from(self.generated.resource_tables.len())
            .map_err(|_| CudaError::InvalidInput("table count exceeds u64".to_owned()))?;
        let candidate_count = u64::try_from(self.layout.candidate_count)
            .map_err(|_| CudaError::InvalidInput("candidate count exceeds u64".to_owned()))?;
        if candidate_count != 0 {
            let config = control_count_launch_config(candidate_count);
            // Other table counts retain the zero written by initialization:
            // no contest can set a deferred flag for those tables.
            for (deferred_table, &table) in self.generated.resource_tables.iter().enumerate() {
                let deferred_table = deferred_table as u64;
                let table = table as u64;
                let mut args = fused_launch_builder(
                    &self.stream,
                    &self.count_deferred,
                    self.fused_batch.as_ref(),
                );
                args.arg(&self.deferred)
                    .arg(&candidate_count)
                    .arg(&table_count)
                    .arg(&deferred_table)
                    .arg(&table)
                    .arg(&mut self.deferred_counts);
                args.launch_generated(config).map_err(driver_error)?;
            }
        }

        let status = self
            .stream
            .memcpy_dtov(&self.status)
            .map_err(driver_error)?;
        let active_width = self
            .fused_batch
            .as_ref()
            .map_or(1, |batch| batch.active_width);
        let mut results = Vec::with_capacity(active_width);
        let mut deactivate = false;
        for slot in 0..active_width {
            let begin = slot * 12;
            let slot_status = &status[begin..begin + 12];
            if slot_status[0] == 0 {
                results.push(Ok(()));
            } else {
                results.push(Err(device_status(slot_status)));
                if let Some(batch) = self.fused_batch.as_mut() {
                    batch.active_host[slot] = 0;
                    deactivate = true;
                }
            }
        }
        if deactivate {
            let batch = self.fused_batch.as_mut().expect("fused batch exists");
            self.stream
                .memcpy_htod(&batch.active_host, &mut batch.active)
                .map_err(driver_error)?;
        }
        if self.fused_batch.is_none() {
            if let Some(error) = results.iter().find_map(|status| status.as_ref().err()) {
                return Err(error.clone());
            }
        }
        mem::swap(&mut self.state, &mut self.next_state);
        mem::swap(&mut self.inputs, &mut self.next_inputs);
        mem::swap(&mut self.input_counts, &mut self.next_input_counts);
        self.next_tick = self
            .next_tick
            .checked_add(1)
            .ok_or_else(|| CudaError::DeviceExecution("tick coordinate overflow".to_owned()))?;
        self.host_state_current = false;
        Ok(results)
    }
}
