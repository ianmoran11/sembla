//! Observation and control reductions for generated CUDA.

use std::fmt::Write;

use super::{
    CudaError, Generator, GroupedObservationKeySpec, GroupedObservationSpec, Rows, Ty, ViewReduce,
};

impl Generator<'_> {
    pub(super) fn emit_observation_kernel(&self, out: &mut String) -> Result<(), CudaError> {
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
            if self.execution {
                self.emit_grouped_bounds_reduction(out, view_index, view)?;
                continue;
            }
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
            if self.execution {
                out.push_str("    unsigned long long bins = 1ULL;\n");
                for axis in axis_offset..axis_offset + view.keys.len() {
                    writeln!(out, "    bins *= axis_cardinalities[{axis}];").unwrap();
                }
                out.push_str("    extern __shared__ unsigned long long shared_counts[];\n    bool local_histogram = bins <= 2048ULL;\n    if (local_histogram) {\n      for (unsigned int bin = threadIdx.x; bin < bins; bin += blockDim.x) shared_counts[bin] = 0ULL;\n      __syncthreads();\n    }\n");
            }
            writeln!(out, "    for (unsigned long long row = worker; row < row_counts[{table}]; row += (unsigned long long)gridDim.x * blockDim.x) {{\n      int selected = {selected};\n      if (selected) {{ unsigned long long group = 0ULL;").unwrap();
            for (key_index, key) in view.keys.iter().enumerate() {
                let global_axis = axis_offset + key_index;
                let value = match *key {
                    GroupedObservationKeySpec::Enum { attr_index, .. }
                    | GroupedObservationKeySpec::Ref { attr_index, .. } => {
                        let attr = &self.model.model().boxes[view.box_index].tables
                            [view.table_index]
                            .attrs[attr_index];
                        format!(
                            "(long long)({})",
                            self.render_attr(rows, &attr.name, "state", "row")?.0
                        )
                    }
                    GroupedObservationKeySpec::BandedInt {
                        attr_index, width, ..
                    } => {
                        let attr = &self.model.model().boxes[view.box_index].tables
                            [view.table_index]
                            .attrs[attr_index];
                        format!(
                            "sembla_div_euclid_i64_u64((long long)({}), {width}ULL)",
                            self.render_attr(rows, &attr.name, "state", "row")?.0
                        )
                    }
                };
                writeln!(out, "        {{ long long key = {value}; unsigned long long coordinate = (unsigned long long)(key - axis_mins[{global_axis}]); group = group * axis_cardinalities[{global_axis}] + coordinate; }}").unwrap();
            }
            if self.execution {
                out.push_str("        atomicAdd((local_histogram ? shared_counts : counts) + group, 1ULL);\n      }\n    }\n    if (local_histogram) {\n      __syncthreads();\n      for (unsigned int bin = threadIdx.x; bin < bins; bin += blockDim.x) if (shared_counts[bin] != 0ULL) atomicAdd(counts + bin, shared_counts[bin]);\n    }\n    return;\n  }\n");
            } else {
                out.push_str(
                    "        atomicAdd(counts + group, 1ULL);\n      }\n    }\n    return;\n  }\n",
                );
            }
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
        if self.execution {
            out.push('\n');
            out.push_str(include_str!("count_winners.cuh"));
        } else {
            out.push_str("\nextern \"C\" __global__ void sembla_count_fired(const unsigned char* wins, const unsigned long long* candidate_offsets, unsigned long long candidate_count, unsigned long long rule_count, unsigned long long rule, unsigned long long* fired_counts) {\n  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned long long begin = candidate_offsets[rule];\n  unsigned long long end = rule + 1ULL < rule_count ? candidate_offsets[rule + 1ULL] : candidate_count;\n  unsigned long long local = 0ULL;\n  for (unsigned long long candidate = begin + worker; candidate < end; candidate += (unsigned long long)gridDim.x * blockDim.x) local += wins[candidate] != 0U;\n  extern __shared__ unsigned long long fired_partials[];\n  fired_partials[threadIdx.x] = local;\n  __syncthreads();\n  for (unsigned int stride = blockDim.x / 2U; stride != 0U; stride /= 2U) {\n    if (threadIdx.x < stride) fired_partials[threadIdx.x] += fired_partials[threadIdx.x + stride];\n    __syncthreads();\n  }\n  if (threadIdx.x == 0U) atomicAdd(fired_counts + rule, fired_partials[0]);\n}\n");
        }
        out.push_str("\nextern \"C\" __global__ void sembla_count_deferred(const unsigned char* deferred, unsigned long long candidate_count, unsigned long long table_count, unsigned long long table, unsigned long long* deferred_counts) {\n  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;\n  unsigned long long local = 0ULL;\n  for (unsigned long long candidate = worker; candidate < candidate_count; candidate += (unsigned long long)gridDim.x * blockDim.x) local += deferred[candidate * table_count + table] != 0U;\n  extern __shared__ unsigned long long deferred_partials[];\n  deferred_partials[threadIdx.x] = local;\n  __syncthreads();\n  for (unsigned int stride = blockDim.x / 2U; stride != 0U; stride /= 2U) {\n    if (threadIdx.x < stride) deferred_partials[threadIdx.x] += deferred_partials[threadIdx.x + stride];\n    __syncthreads();\n  }\n  if (threadIdx.x == 0U) atomicAdd(deferred_counts + table, deferred_partials[0]);\n}\n");
        Ok(())
    }

    fn emit_grouped_bounds_reduction(
        &self,
        out: &mut String,
        view_index: usize,
        view: &GroupedObservationSpec,
    ) -> Result<(), CudaError> {
        let rows = Rows::State {
            box_index: view.box_index,
            table_index: view.table_index,
        };
        let table = self.global_table(view.box_index, view.table_index);
        writeln!(
            out,
            "  if (view_index == {view_index}U) {{\n    extern __shared__ long long bounds[];"
        )
        .unwrap();
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
            writeln!(out, "    {{ long long low = 0x7fffffffffffffffLL; long long high = (-0x7fffffffffffffffLL - 1LL);\n      for (unsigned long long row = worker; row < row_counts[{table}]; row += (unsigned long long)gridDim.x * blockDim.x) {{\n        long long value = (long long)({value}); low = value < low ? value : low; high = value > high ? value : high;\n      }}").unwrap();
            out.push_str("      bounds[threadIdx.x] = low; bounds[blockDim.x + threadIdx.x] = high;\n      __syncthreads();\n      for (unsigned int active = blockDim.x; active > 1U; active = (active + 1U) / 2U) {\n        unsigned int half = (active + 1U) / 2U;\n        if (threadIdx.x < active / 2U) {\n          long long other_low = bounds[threadIdx.x + half];\n          long long other_high = bounds[blockDim.x + threadIdx.x + half];\n          if (other_low < bounds[threadIdx.x]) bounds[threadIdx.x] = other_low;\n          if (other_high > bounds[blockDim.x + threadIdx.x]) bounds[blockDim.x + threadIdx.x] = other_high;\n        }\n        __syncthreads();\n      }\n");
            writeln!(out, "      if (threadIdx.x == 0U) {{ sembla_atomic_min_i64(extrema + {extrema_index}ULL * 2ULL, bounds[0]); sembla_atomic_max_i64(extrema + {extrema_index}ULL * 2ULL + 1ULL, bounds[blockDim.x]); }}\n      __syncthreads();\n    }}").unwrap();
        }
        out.push_str("    return;\n  }\n");
        Ok(())
    }
}
