//! Portable WGSL `f32` and double-single precision strategies.
//!
//! Each strategy exposes separate reduction and map/argmin dispatch hooks plus a
//! one-tick readback path. Reduction is two fixed passes: two ascending-row
//! partials per employer followed by an ordered merge. No floating-point
//! atomics are used, so both variants have a stable reduction tree.

use std::{borrow::Cow, error::Error, fmt, sync::mpsc, time::Instant};

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::{
    oracle::{run_oracle, OracleResult},
    timing::{self, StageTiming, TimingMethod, BENCHMARK_TICK, MEASURED_TICKS, WARMUP_TICKS},
    workload::{Workload, WorkloadConfig},
};

const PARTIALS_PER_GROUP: u32 = 2;
const REDUCE_WORKGROUP_SIZE: u32 = 64;
const MAP_WORKGROUP_SIZE: u32 = 256;
const SHADER_SOURCE: &str = concat!(
    include_str!("wgsl/df64.wgsl"),
    "\n",
    include_str!("wgsl/portable.wgsl")
);

/// Accuracy smoke scale requested by PRD 0002.
pub const ACCURACY_ROWS: u32 = 1_000_000;
pub const ACCURACY_GROUPS: u32 = 50_000;
pub const ACCURACY_TICK: u32 = 7;
/// This fixed seed includes a copied Philox near-tie: entities 756845 and
/// 756855 share the same high 24 random bits, while the latter has the smaller
/// 53-bit uniform. It makes the f32 winner loss deterministic instead of hoping
/// a random 1M-row sample happens to contain a precision tie.
pub const ACCURACY_SEED: u64 = 0x0123_4567_89ab_cdfc;
/// A wide tick window admits both members of the documented near-tie. This does
/// not change the race ordering being scored.
pub const ACCURACY_DT: f64 = 100.0;

/// Absolute double-single guard. The relative comparison below is stricter on
/// the checked workload; this ceiling prevents a broken f32-pair path from
/// passing merely because the baseline is also inaccurate.
pub const DF64_MAX_REDUCTION_RELATIVE_ERROR: f64 = 1.0e-10;
/// Double-single max and mean reduction errors must each be at most 1% of f32.
pub const DF64_REDUCTION_ERROR_FACTOR: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortableStrategy {
    F32,
    Df64,
}

impl fmt::Display for PortableStrategy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::F32 => formatter.write_str("f32"),
            Self::Df64 => formatter.write_str("double-single"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GpuTickResult {
    pub strategy: PortableStrategy,
    /// Reconstructed as `hi + lo` on the host (`lo` is zero for f32).
    pub segmented_sums: Vec<f64>,
    pub winner_entity_ids: Vec<u32>,
    pub fired_flags: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RelativeError {
    pub max: f64,
    pub mean: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrategyAccuracy {
    pub strategy: PortableStrategy,
    pub reduction_relative_error: RelativeError,
    pub winner_mismatch_count: usize,
    pub contested_key_count: usize,
    pub winner_mismatch_rate: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FastMathStatus {
    pub adapter_name: String,
    pub backend: String,
    /// The pinned wgpu-hal fork requests strict compilation on its Metal path.
    /// Other backends are reported as unsupported rather than assumed strict.
    pub strict_math_requested: bool,
    pub strict_math_backend_supported: bool,
    pub fma_contraction_observed: bool,
    pub reassociation_observed: bool,
    pub df64_residuals_preserved: bool,
    pub trustworthy_on_adapter: bool,
}

impl fmt::Display for FastMathStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "adapter={} backend={}; strict-math-requested={}; strict-math-backend-supported={}; FMA-contraction-observed={}; reassociation-observed={}; df64-residuals-preserved={}; trustworthy={}",
            self.adapter_name,
            self.backend,
            self.strict_math_requested,
            self.strict_math_backend_supported,
            self.fma_contraction_observed,
            self.reassociation_observed,
            self.df64_residuals_preserved,
            self.trustworthy_on_adapter,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccuracyReport {
    pub rows: u32,
    pub groups: u32,
    pub tick: u32,
    pub f32: StrategyAccuracy,
    pub df64: StrategyAccuracy,
    pub fast_math: FastMathStatus,
}

impl AccuracyReport {
    /// Enforces the numerical PRD-0002 thresholds on every backend.
    ///
    /// This deliberately does not treat the absence of a backend strict-math
    /// switch as a numerical waiver: a silently broken double-single kernel
    /// must fail the PRD-0005 regression guard on Vulkan as well as Metal.
    pub fn assert_numerical_thresholds(&self) -> Result<(), String> {
        if self.df64.reduction_relative_error.max > DF64_MAX_REDUCTION_RELATIVE_ERROR {
            return Err(format!(
                "df64 max reduction error {} exceeds {}",
                self.df64.reduction_relative_error.max, DF64_MAX_REDUCTION_RELATIVE_ERROR
            ));
        }
        let max_limit = self.f32.reduction_relative_error.max * DF64_REDUCTION_ERROR_FACTOR;
        let mean_limit = self.f32.reduction_relative_error.mean * DF64_REDUCTION_ERROR_FACTOR;
        if self.df64.reduction_relative_error.max > max_limit
            || self.df64.reduction_relative_error.mean > mean_limit
        {
            return Err(format!(
                "df64 reduction errors ({}, {}) are not at most {}x f32 ({}, {})",
                self.df64.reduction_relative_error.max,
                self.df64.reduction_relative_error.mean,
                DF64_REDUCTION_ERROR_FACTOR,
                self.f32.reduction_relative_error.max,
                self.f32.reduction_relative_error.mean,
            ));
        }
        if self.df64.winner_mismatch_rate >= self.f32.winner_mismatch_rate {
            return Err(format!(
                "df64 winner mismatch rate {} is not strictly below f32 {}",
                self.df64.winner_mismatch_rate, self.f32.winner_mismatch_rate
            ));
        }
        Ok(())
    }

    /// Enforces numerical thresholds plus the Metal strict-compilation trust
    /// contract used by PRD 0002's development-machine evidence.
    pub fn assert_thresholds(&self) -> Result<(), String> {
        self.assert_numerical_thresholds()?;
        if !self.fast_math.trustworthy_on_adapter {
            return Err(format!(
                "double-single arithmetic behavior is not trustworthy: {}",
                self.fast_math
            ));
        }
        Ok(())
    }
}

impl fmt::Display for AccuracyReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "portable accuracy: N={} G={} tick={}",
            self.rows, self.groups, self.tick
        )?;
        for result in [&self.f32, &self.df64] {
            writeln!(
                formatter,
                "  {}: reduction rel-error max={:.6e} mean={:.6e}; winner mismatches={}/{} ({:.6e})",
                result.strategy,
                result.reduction_relative_error.max,
                result.reduction_relative_error.mean,
                result.winner_mismatch_count,
                result.contested_key_count,
                result.winner_mismatch_rate,
            )?;
        }
        write!(formatter, "  fast-math/FMA: {}", self.fast_math)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuError(String);

impl GpuError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for GpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for GpuError {}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShaderConfig {
    rows: u32,
    groups: u32,
    tick: u32,
    map_workgroups_x: u32,
    seed_lo: u32,
    seed_hi: u32,
    partials_per_group: u32,
    pad0: u32,
    beta: [f32; 2],
    dt: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct PhiloxKnownAnswer {
    pub seed_lo: u32,
    pub seed_hi: u32,
    pub tick: u32,
    pub rule_id: u32,
    pub entity_id: u32,
    pub draw_idx: u32,
    pub expected: [u32; 4],
}

/// Four frozen CPU-derived vectors, including the two Random123 boundary
/// vectors and two workload-coordinate vectors.
pub const PHILOX_KNOWN_ANSWERS: [PhiloxKnownAnswer; 4] = [
    PhiloxKnownAnswer {
        seed_lo: 0,
        seed_hi: 0,
        tick: 0,
        rule_id: 0,
        entity_id: 0,
        draw_idx: 0,
        expected: [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8],
    },
    PhiloxKnownAnswer {
        seed_lo: u32::MAX,
        seed_hi: u32::MAX,
        tick: u32::MAX,
        rule_id: u32::MAX,
        entity_id: u32::MAX,
        draw_idx: u32::MAX,
        expected: [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd],
    },
    PhiloxKnownAnswer {
        seed_lo: 0x89ab_cdef,
        seed_hi: 0x0123_4567,
        tick: 0,
        rule_id: 0xffff_fe00,
        entity_id: 123,
        draw_idx: 0,
        expected: [0x00f9_797c, 0x4c2d_676a, 0xddf3_2bdf, 0xb9d3_f58a],
    },
    PhiloxKnownAnswer {
        seed_lo: 0x89ab_cdfc,
        seed_hi: 0x0123_4567,
        tick: 7,
        rule_id: 0,
        entity_id: 756_845,
        draw_idx: 0,
        expected: [0x99de_7a8d, 0xff77_122c, 0x2b06_9fe6, 0x7504_6e31],
    },
];

struct StrategyPipelines {
    partial: wgpu::ComputePipeline,
    finish: wgpu::ComputePipeline,
    map: wgpu::ComputePipeline,
    argmin: wgpu::ComputePipeline,
}

struct StrategyBindGroups {
    partial: wgpu::BindGroup,
    finish: wgpu::BindGroup,
    map: wgpu::BindGroup,
    argmin: wgpu::BindGroup,
}

struct StrategyGpu {
    pipelines: StrategyPipelines,
    bindings: StrategyBindGroups,
}

/// Reusable portable GPU context. Output and intermediate buffers are retained
/// across calls so `dispatch_*_only` methods are steady-state throughput hooks.
pub struct PortableRunner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,
    backend: String,
    strict_math_backend_supported: bool,
    timestamp_supported: bool,
    rows: u32,
    groups: u32,
    map_dispatch: (u32, u32),
    config_template: ShaderConfig,
    config: wgpu::Buffer,
    sums: wgpu::Buffer,
    winners: wgpu::Buffer,
    fired: wgpu::Buffer,
    f32: StrategyGpu,
    df64: StrategyGpu,
    philox_pipeline: wgpu::ComputePipeline,
    arithmetic_probe_pipeline: wgpu::ComputePipeline,
}

impl PortableRunner {
    pub async fn new(workload: &Workload) -> Result<Self, GpuError> {
        if SHADER_SOURCE.contains("fma(") {
            return Err(GpuError::new(
                "portable df64 shader must not request explicit FMA",
            ));
        }

        let instance = wgpu::Instance::default();
        let options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        };
        let adapter = match instance.request_adapter(&options).await {
            Some(adapter) => adapter,
            None => instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    force_fallback_adapter: true,
                    ..options
                })
                .await
                .ok_or_else(|| GpuError::new("wgpu found no compute adapter"))?,
        };
        let adapter_info = adapter.get_info();
        let strict_math_backend_supported = adapter_info.backend == wgpu::Backend::Metal;
        let adapter_features = adapter.features();
        let timestamp_supported = adapter_features.contains(wgpu::Features::TIMESTAMP_QUERY);
        let adapter_limits = adapter.limits();
        if adapter_limits.max_storage_buffers_per_shader_stage < 6 {
            return Err(GpuError::new(format!(
                "adapter exposes only {} storage buffers per shader stage; 6 required",
                adapter_limits.max_storage_buffers_per_shader_stage
            )));
        }

        let largest_storage = u64::from(workload.config.rows) * 8;
        if largest_storage > u64::from(adapter_limits.max_storage_buffer_binding_size)
            || largest_storage > adapter_limits.max_buffer_size
        {
            return Err(GpuError::new(format!(
                "workload's {largest_storage}-byte row buffer exceeds adapter limits"
            )));
        }

        let mut required_limits =
            wgpu::Limits::downlevel_defaults().using_resolution(adapter_limits.clone());
        required_limits.max_storage_buffers_per_shader_stage = 6;
        required_limits.max_storage_buffer_binding_size =
            adapter_limits.max_storage_buffer_binding_size;
        required_limits.max_buffer_size = adapter_limits.max_buffer_size;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Sembla portable precision kernels"),
                    required_features: if timestamp_supported {
                        wgpu::Features::TIMESTAMP_QUERY
                    } else {
                        wgpu::Features::empty()
                    },
                    required_limits,
                },
                None,
            )
            .await
            .map_err(|error| GpuError::new(format!("request_device failed: {error}")))?;

        let max_dispatch = adapter_limits.max_compute_workgroups_per_dimension;
        let map_workgroups = workload.config.rows.div_ceil(MAP_WORKGROUP_SIZE);
        let map_x = map_workgroups.min(max_dispatch);
        let map_y = map_workgroups.div_ceil(map_x);
        if map_y > max_dispatch {
            return Err(GpuError::new("map dispatch exceeds the adapter's 2D grid"));
        }
        let partial_workgroups =
            (workload.config.groups * PARTIALS_PER_GROUP).div_ceil(REDUCE_WORKGROUP_SIZE);
        let group_workgroups = workload.config.groups.div_ceil(REDUCE_WORKGROUP_SIZE);
        if partial_workgroups > max_dispatch || group_workgroups > max_dispatch {
            return Err(GpuError::new(
                "segmented dispatch exceeds the adapter's 1D grid",
            ));
        }

        let weight_pairs: Vec<[f32; 2]> = workload.weight.iter().copied().map(split_f64).collect();
        let partial_zeroes =
            vec![[0.0_f32; 2]; (workload.config.groups * PARTIALS_PER_GROUP) as usize];
        let sum_zeroes = vec![[0.0_f32; 2]; workload.config.groups as usize];
        let race_zeroes = vec![[0.0_f32; 2]; workload.config.rows as usize];
        let winner_zeroes = vec![u32::MAX; workload.config.groups as usize];
        let fired_zeroes = vec![0_u32; workload.config.rows as usize];

        let config_template = ShaderConfig {
            rows: workload.config.rows,
            groups: workload.config.groups,
            tick: 0,
            map_workgroups_x: map_x,
            seed_lo: workload.config.seed as u32,
            seed_hi: (workload.config.seed >> 32) as u32,
            partials_per_group: PARTIALS_PER_GROUP,
            pad0: 0,
            beta: split_f64(workload.config.beta),
            dt: split_f64(workload.config.dt),
        };
        let config = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("portable precision config"),
            contents: bytemuck::bytes_of(&config_template),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let offsets = storage_buffer(
            &device,
            "group offsets",
            bytemuck::cast_slice(&workload.group_offsets),
            false,
        );
        let employers = storage_buffer(
            &device,
            "employers",
            bytemuck::cast_slice(&workload.employer),
            false,
        );
        let health = storage_buffer(
            &device,
            "health",
            bytemuck::cast_slice(&workload.health),
            false,
        );
        let weights = storage_buffer(
            &device,
            "df64 weights",
            bytemuck::cast_slice(&weight_pairs),
            false,
        );
        let partials = storage_buffer(
            &device,
            "partial sums",
            bytemuck::cast_slice(&partial_zeroes),
            false,
        );
        let sums = storage_buffer(
            &device,
            "segmented sums",
            bytemuck::cast_slice(&sum_zeroes),
            true,
        );
        let races = storage_buffer(
            &device,
            "race times",
            bytemuck::cast_slice(&race_zeroes),
            false,
        );
        let winners = storage_buffer(
            &device,
            "winner entities",
            bytemuck::cast_slice(&winner_zeroes),
            true,
        );
        let fired = storage_buffer(
            &device,
            "fired flags",
            bytemuck::cast_slice(&fired_zeroes),
            true,
        );

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("portable f32 + df64 WGSL"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER_SOURCE)),
        });
        let f32_pipelines = create_strategy_pipelines(&device, &module, PortableStrategy::F32);
        let df64_pipelines = create_strategy_pipelines(&device, &module, PortableStrategy::Df64);
        let f32_bindings = create_strategy_bindings(
            &device,
            &f32_pipelines,
            &config,
            &offsets,
            &employers,
            &health,
            &weights,
            &partials,
            &sums,
            &races,
            &winners,
            &fired,
        );
        let df64_bindings = create_strategy_bindings(
            &device,
            &df64_pipelines,
            &config,
            &offsets,
            &employers,
            &health,
            &weights,
            &partials,
            &sums,
            &races,
            &winners,
            &fired,
        );
        let philox_pipeline = create_pipeline(
            &device,
            &module,
            "WGSL Philox known answers",
            "philox_known_answers",
        );
        let arithmetic_probe_pipeline = create_pipeline(
            &device,
            &module,
            "df64 arithmetic behavior probe",
            "arithmetic_behavior_probe",
        );

        Ok(Self {
            device,
            queue,
            adapter_name: adapter_info.name,
            backend: format!("{:?}", adapter_info.backend),
            strict_math_backend_supported,
            timestamp_supported,
            rows: workload.config.rows,
            groups: workload.config.groups,
            map_dispatch: (map_x, map_y),
            config_template,
            config,
            sums,
            winners,
            fired,
            f32: StrategyGpu {
                pipelines: f32_pipelines,
                bindings: f32_bindings,
            },
            df64: StrategyGpu {
                pipelines: df64_pipelines,
                bindings: df64_bindings,
            },
            philox_pipeline,
            arithmetic_probe_pipeline,
        })
    }

    /// Dispatches only the two reduction passes and retains results on-device.
    pub fn dispatch_reduction_only(&self, strategy: PortableStrategy, tick: u32) {
        self.write_tick(tick);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("portable reduction dispatch"),
            });
        self.encode_reduction(&mut encoder, strategy, None);
        self.queue.submit(Some(encoder.finish()));
    }

    /// Dispatches only the row map using the currently resident segmented sums.
    pub fn dispatch_map_only(&self, strategy: PortableStrategy, tick: u32) {
        self.write_tick(tick);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("portable map dispatch"),
            });
        self.encode_map(&mut encoder, strategy, None);
        self.queue.submit(Some(encoder.finish()));
    }

    /// Dispatches only segmented argmin using the currently resident race times.
    pub fn dispatch_argmin_only(&self, strategy: PortableStrategy, tick: u32) {
        self.write_tick(tick);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("portable argmin dispatch"),
            });
        self.encode_argmin(&mut encoder, strategy, None);
        self.queue.submit(Some(encoder.finish()));
    }

    /// Dispatches map and argmin using the currently resident segmented sums.
    pub fn dispatch_map_argmin_only(&self, strategy: PortableStrategy, tick: u32) {
        self.write_tick(tick);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("portable map + argmin dispatch"),
            });
        self.encode_map(&mut encoder, strategy, None);
        self.encode_argmin(&mut encoder, strategy, None);
        self.queue.submit(Some(encoder.finish()));
    }

    /// Steady-state full-tick hook: dispatches without allocating or reading back.
    pub fn dispatch_tick_only(&self, strategy: PortableStrategy, tick: u32) {
        self.write_tick(tick);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("portable full tick dispatch"),
            });
        self.encode_reduction(&mut encoder, strategy, None);
        self.encode_map(&mut encoder, strategy, None);
        self.encode_argmin(&mut encoder, strategy, None);
        self.queue.submit(Some(encoder.finish()));
    }

    pub fn dispatch_f32(&self, tick: u32) -> Result<GpuTickResult, GpuError> {
        self.dispatch_tick(PortableStrategy::F32, tick)
    }

    pub fn dispatch_df64(&self, tick: u32) -> Result<GpuTickResult, GpuError> {
        self.dispatch_tick(PortableStrategy::Df64, tick)
    }

    pub fn dispatch_tick(
        &self,
        strategy: PortableStrategy,
        tick: u32,
    ) -> Result<GpuTickResult, GpuError> {
        self.dispatch_tick_only(strategy, tick);
        self.read_tick_result(strategy)
    }

    pub fn wait(&self) {
        self.device.poll(wgpu::Maintain::Wait);
    }

    /// Measures steady-state full ticks and the reduction/argmin hot stages.
    pub fn benchmark(&self, strategy: PortableStrategy) -> Result<StageTiming, GpuError> {
        if self.timestamp_supported {
            // The timestamp path performs one preflight before warmups. A
            // later failure is an error rather than a reason to rerun samples.
            self.benchmark_timestamps(strategy)
        } else {
            self.benchmark_wall_clock(strategy)
        }
    }

    fn benchmark_timestamps(&self, strategy: PortableStrategy) -> Result<StageTiming, GpuError> {
        const QUERY_COUNT: u32 = 6;
        const QUERY_BYTES: u64 = QUERY_COUNT as u64 * std::mem::size_of::<u64>() as u64;
        let query_set = self.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("portable precision stage timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: QUERY_COUNT,
        });
        let resolve = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("portable timestamp resolve"),
            size: QUERY_BYTES,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("portable timestamp readback"),
            size: QUERY_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let period_ms = f64::from(self.queue.get_timestamp_period()) / 1_000_000.0;
        // Some wgpu 0.20 Metal devices advertise TIMESTAMP_QUERY but return
        // zeroes for a later compute pass. Probe once before the benchmark's
        // exactly 10 warmups; an unusable query implementation selects the
        // synchronized fallback without discarding measured samples.
        if self
            .timestamp_sample(strategy, &query_set, &resolve, &readback, period_ms)?
            .is_none()
        {
            return self.benchmark_wall_clock(strategy);
        }

        for _ in 0..WARMUP_TICKS {
            self.dispatch_tick_only(strategy, BENCHMARK_TICK);
            self.wait();
        }

        let mut totals = Vec::with_capacity(MEASURED_TICKS);
        let mut reductions = Vec::with_capacity(MEASURED_TICKS);
        let mut argmins = Vec::with_capacity(MEASURED_TICKS);
        for _ in 0..MEASURED_TICKS {
            let Some((total, reduce, argmin)) =
                self.timestamp_sample(strategy, &query_set, &resolve, &readback, period_ms)?
            else {
                return Err(GpuError::new(
                    "GPU timestamp queries became incomplete after a successful preflight",
                ));
            };
            totals.push(total);
            reductions.push(reduce);
            argmins.push(argmin);
        }

        timing::summarize(
            self.rows,
            WARMUP_TICKS,
            MEASURED_TICKS,
            TimingMethod::GpuTimestampQueries,
            &mut totals,
            &mut reductions,
            &mut argmins,
        )
        .map_err(GpuError::new)
    }

    fn timestamp_sample(
        &self,
        strategy: PortableStrategy,
        query_set: &wgpu::QuerySet,
        resolve: &wgpu::Buffer,
        readback: &wgpu::Buffer,
        period_ms: f64,
    ) -> Result<Option<(f64, f64, f64)>, GpuError> {
        const QUERY_COUNT: u32 = 6;
        const QUERY_BYTES: u64 = QUERY_COUNT as u64 * std::mem::size_of::<u64>() as u64;
        self.write_tick(BENCHMARK_TICK);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("portable timestamped tick"),
            });
        self.encode_reduction(&mut encoder, strategy, Some((query_set, 0, 1)));
        self.encode_map(&mut encoder, strategy, Some((query_set, 2, 3)));
        self.encode_argmin(&mut encoder, strategy, Some((query_set, 4, 5)));
        encoder.resolve_query_set(query_set, 0..QUERY_COUNT, resolve, 0);
        encoder.copy_buffer_to_buffer(resolve, 0, readback, 0, QUERY_BYTES);
        self.queue.submit(Some(encoder.finish()));
        self.wait();

        let timestamps = read_timestamp_buffer(&self.device, readback)?;
        let reduce = timestamp_delta(timestamps[0], timestamps[1], period_ms);
        let map = timestamp_delta(timestamps[2], timestamps[3], period_ms);
        let argmin = timestamp_delta(timestamps[4], timestamps[5], period_ms);
        let total = timestamp_delta(timestamps[0], timestamps[5], period_ms);
        let (Some(total), Some(reduce), Some(map), Some(argmin)) = (total, reduce, map, argmin)
        else {
            return Ok(None);
        };
        if total < (reduce + map + argmin) * 0.99 {
            return Ok(None);
        }
        Ok(Some((total, reduce, argmin)))
    }

    fn benchmark_wall_clock(&self, strategy: PortableStrategy) -> Result<StageTiming, GpuError> {
        for _ in 0..WARMUP_TICKS {
            self.dispatch_tick_only(strategy, BENCHMARK_TICK);
            self.wait();
        }
        let mut totals = Vec::with_capacity(MEASURED_TICKS);
        for _ in 0..MEASURED_TICKS {
            let start = Instant::now();
            self.dispatch_tick_only(strategy, BENCHMARK_TICK);
            self.wait();
            totals.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        for _ in 0..WARMUP_TICKS {
            self.dispatch_reduction_only(strategy, BENCHMARK_TICK);
            self.wait();
        }
        let mut reductions = Vec::with_capacity(MEASURED_TICKS);
        for _ in 0..MEASURED_TICKS {
            let start = Instant::now();
            self.dispatch_reduction_only(strategy, BENCHMARK_TICK);
            self.wait();
            reductions.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        for _ in 0..WARMUP_TICKS {
            self.prepare_argmin(strategy);
            let start = Instant::now();
            self.dispatch_argmin_only(strategy, BENCHMARK_TICK);
            self.wait();
            let _ = start.elapsed();
        }
        let mut argmins = Vec::with_capacity(MEASURED_TICKS);
        for _ in 0..MEASURED_TICKS {
            self.prepare_argmin(strategy);
            let start = Instant::now();
            self.dispatch_argmin_only(strategy, BENCHMARK_TICK);
            self.wait();
            argmins.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        timing::summarize(
            self.rows,
            WARMUP_TICKS,
            MEASURED_TICKS,
            TimingMethod::SynchronizedWallClockFallback,
            &mut totals,
            &mut reductions,
            &mut argmins,
        )
        .map_err(GpuError::new)
    }

    fn prepare_argmin(&self, strategy: PortableStrategy) {
        self.dispatch_reduction_only(strategy, BENCHMARK_TICK);
        self.wait();
        self.dispatch_map_only(strategy, BENCHMARK_TICK);
        self.wait();
    }

    /// Runs the WGSL Philox implementation over all four frozen vectors.
    pub fn run_philox_known_answers(&self) -> Result<Vec<[u32; 4]>, GpuError> {
        let inputs: Vec<[u32; 6]> = PHILOX_KNOWN_ANSWERS
            .iter()
            .map(|answer| {
                [
                    answer.seed_lo,
                    answer.seed_hi,
                    answer.tick,
                    answer.rule_id,
                    answer.entity_id,
                    answer.draw_idx,
                ]
            })
            .collect();
        let output_zeroes = vec![[0_u32; 4]; inputs.len()];
        let input = storage_buffer(
            &self.device,
            "Philox KAT inputs",
            bytemuck::cast_slice(&inputs),
            false,
        );
        let output = storage_buffer(
            &self.device,
            "Philox KAT outputs",
            bytemuck::cast_slice(&output_zeroes),
            true,
        );
        let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Philox KAT bindings"),
            layout: &self.philox_pipeline.get_bind_group_layout(0),
            entries: &[bind_entry(10, &input), bind_entry(11, &output)],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Philox KAT dispatch"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Philox KAT"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.philox_pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups(inputs.len() as u32, 1, 1);
        }
        self.queue.submit(Some(encoder.finish()));
        let bytes = read_buffer(
            &self.device,
            &self.queue,
            &output,
            (inputs.len() * 16) as u64,
        )?;
        Ok(bytemuck::cast_slice::<u8, [u32; 4]>(&bytes).to_vec())
    }

    /// Probes the compiled shader rather than assuming the backend honored the
    /// no-contraction/no-reassociation requirement.
    pub fn fast_math_status(&self) -> Result<FastMathStatus, GpuError> {
        let input_values = [
            4097.0_f32,
            4097.0,
            -16_785_408.0,
            16_777_216.0,
            1.0,
            -16_777_216.0,
            1.0,
            f32::from_bits(0x3380_0000), // 2^-24
        ];
        let output_zeroes = [[0.0_f32; 2]; 3];
        let input = storage_buffer(
            &self.device,
            "arithmetic probe input",
            bytemuck::cast_slice(&input_values),
            false,
        );
        let output = storage_buffer(
            &self.device,
            "arithmetic probe output",
            bytemuck::cast_slice(&output_zeroes),
            true,
        );
        let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arithmetic probe bindings"),
            layout: &self.arithmetic_probe_pipeline.get_bind_group_layout(0),
            entries: &[bind_entry(12, &input), bind_entry(13, &output)],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arithmetic behavior probe dispatch"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("arithmetic behavior probe"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.arithmetic_probe_pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        self.queue.submit(Some(encoder.finish()));
        let bytes = read_buffer(&self.device, &self.queue, &output, 24)?;
        let values: &[[f32; 2]] = bytemuck::cast_slice(&bytes);
        let fma_contraction_observed = values[0][0].to_bits() != 0;
        let reassociation_observed = values[0][1].to_bits() != 0;
        let df64_residuals_preserved = values[1][0] == 1.0
            && values[1][1].to_bits() == 0x3380_0000
            && values[2][0] == 16_785_408.0
            && values[2][1] == 1.0;
        let strict_math_backend_supported = self.strict_math_backend_supported;
        let strict_math_requested = strict_math_backend_supported;
        let trustworthy_on_adapter = strict_math_requested
            && strict_math_backend_supported
            && !fma_contraction_observed
            && !reassociation_observed
            && df64_residuals_preserved;
        Ok(FastMathStatus {
            adapter_name: self.adapter_name.clone(),
            backend: self.backend.clone(),
            strict_math_requested,
            strict_math_backend_supported,
            fma_contraction_observed,
            reassociation_observed,
            df64_residuals_preserved,
            trustworthy_on_adapter,
        })
    }

    fn strategy(&self, strategy: PortableStrategy) -> &StrategyGpu {
        match strategy {
            PortableStrategy::F32 => &self.f32,
            PortableStrategy::Df64 => &self.df64,
        }
    }

    fn write_tick(&self, tick: u32) {
        let mut config = self.config_template;
        config.tick = tick;
        self.queue
            .write_buffer(&self.config, 0, bytemuck::bytes_of(&config));
    }

    fn encode_reduction(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        strategy: PortableStrategy,
        timestamps: Option<(&wgpu::QuerySet, u32, u32)>,
    ) {
        let gpu = self.strategy(strategy);
        let timestamp_writes =
            timestamps.map(|(query_set, begin, end)| wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(begin),
                end_of_pass_write_index: Some(end),
            });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("two-pass segmented reduction"),
            timestamp_writes,
        });
        pass.set_pipeline(&gpu.pipelines.partial);
        pass.set_bind_group(0, &gpu.bindings.partial, &[]);
        pass.dispatch_workgroups(
            (self.groups * PARTIALS_PER_GROUP).div_ceil(REDUCE_WORKGROUP_SIZE),
            1,
            1,
        );
        pass.set_pipeline(&gpu.pipelines.finish);
        pass.set_bind_group(0, &gpu.bindings.finish, &[]);
        pass.dispatch_workgroups(self.groups.div_ceil(REDUCE_WORKGROUP_SIZE), 1, 1);
    }

    fn encode_map(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        strategy: PortableStrategy,
        timestamps: Option<(&wgpu::QuerySet, u32, u32)>,
    ) {
        let gpu = self.strategy(strategy);
        let timestamp_writes =
            timestamps.map(|(query_set, begin, end)| wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(begin),
                end_of_pass_write_index: Some(end),
            });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("hazard/race map"),
            timestamp_writes,
        });
        pass.set_pipeline(&gpu.pipelines.map);
        pass.set_bind_group(0, &gpu.bindings.map, &[]);
        pass.dispatch_workgroups(self.map_dispatch.0, self.map_dispatch.1, 1);
    }

    fn encode_argmin(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        strategy: PortableStrategy,
        timestamps: Option<(&wgpu::QuerySet, u32, u32)>,
    ) {
        let gpu = self.strategy(strategy);
        let timestamp_writes =
            timestamps.map(|(query_set, begin, end)| wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(begin),
                end_of_pass_write_index: Some(end),
            });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("segmented argmin"),
            timestamp_writes,
        });
        pass.set_pipeline(&gpu.pipelines.argmin);
        pass.set_bind_group(0, &gpu.bindings.argmin, &[]);
        pass.dispatch_workgroups(self.groups.div_ceil(REDUCE_WORKGROUP_SIZE), 1, 1);
    }

    fn read_tick_result(&self, strategy: PortableStrategy) -> Result<GpuTickResult, GpuError> {
        let sum_bytes = u64::from(self.groups) * 8;
        let winner_bytes = u64::from(self.groups) * 4;
        let fired_bytes = u64::from(self.rows) * 4;
        let winner_offset = sum_bytes;
        let fired_offset = winner_offset + winner_bytes;
        let total = fired_offset + fired_bytes;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("portable tick readback"),
            size: total,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("portable tick readback copies"),
            });
        encoder.copy_buffer_to_buffer(&self.sums, 0, &staging, 0, sum_bytes);
        encoder.copy_buffer_to_buffer(&self.winners, 0, &staging, winner_offset, winner_bytes);
        encoder.copy_buffer_to_buffer(&self.fired, 0, &staging, fired_offset, fired_bytes);
        self.queue.submit(Some(encoder.finish()));
        map_buffer(&self.device, &staging)?;
        let mapped = staging.slice(..).get_mapped_range();
        let pairs: &[[f32; 2]] = bytemuck::cast_slice(&mapped[..sum_bytes as usize]);
        let segmented_sums = pairs
            .iter()
            .map(|pair| f64::from(pair[0]) + f64::from(pair[1]))
            .collect();
        let winner_entity_ids =
            bytemuck::cast_slice::<u8, u32>(&mapped[winner_offset as usize..fired_offset as usize])
                .to_vec();
        let fired_flags =
            bytemuck::cast_slice::<u8, u32>(&mapped[fired_offset as usize..]).to_vec();
        drop(mapped);
        staging.unmap();
        Ok(GpuTickResult {
            strategy,
            segmented_sums,
            winner_entity_ids,
            fired_flags,
        })
    }
}

/// PRD-0002 fixed correctness workload. The non-default seed/window are solely
/// to make a real f32 Philox near-tie observable at a practical test scale.
#[must_use]
pub fn accuracy_workload_config() -> WorkloadConfig {
    WorkloadConfig {
        rows: ACCURACY_ROWS,
        groups: ACCURACY_GROUPS,
        seed: ACCURACY_SEED,
        dt: ACCURACY_DT,
        ..WorkloadConfig::default()
    }
}

/// One-tick correctness smoke path used by this PRD and PRD 0005's guard.
pub async fn run_accuracy_smoke() -> Result<AccuracyReport, GpuError> {
    let workload = Workload::generate(accuracy_workload_config())
        .map_err(|error| GpuError::new(format!("workload generation failed: {error}")))?;
    let oracle = run_oracle(&workload, ACCURACY_TICK);
    let runner = PortableRunner::new(&workload).await?;

    let philox_outputs = runner.run_philox_known_answers()?;
    for (index, (actual, expected)) in philox_outputs
        .iter()
        .zip(PHILOX_KNOWN_ANSWERS.iter())
        .enumerate()
    {
        if actual != &expected.expected {
            return Err(GpuError::new(format!(
                "WGSL Philox vector {index} mismatch: {actual:08x?} != {:08x?}",
                expected.expected
            )));
        }
    }

    let fast_math = runner.fast_math_status()?;
    let f32_output = runner.dispatch_f32(ACCURACY_TICK)?;
    let df64_output = runner.dispatch_df64(ACCURACY_TICK)?;
    let report = AccuracyReport {
        rows: workload.config.rows,
        groups: workload.config.groups,
        tick: ACCURACY_TICK,
        f32: score_strategy(&f32_output, &oracle),
        df64: score_strategy(&df64_output, &oracle),
        fast_math,
    };
    Ok(report)
}

#[must_use]
pub fn score_strategy(output: &GpuTickResult, oracle: &OracleResult) -> StrategyAccuracy {
    assert_eq!(output.segmented_sums.len(), oracle.segmented_sums.len());
    assert_eq!(
        output.winner_entity_ids.len(),
        oracle.winner_entity_ids.len()
    );

    let mut max_error = 0.0_f64;
    let mut error_sum = 0.0_f64;
    for (actual, expected) in output.segmented_sums.iter().zip(&oracle.segmented_sums) {
        let error = if *expected == 0.0 {
            (actual - expected).abs()
        } else {
            (actual - expected).abs() / expected.abs()
        };
        max_error = max_error.max(error);
        error_sum += error;
    }
    let winner_mismatch_count = output
        .winner_entity_ids
        .iter()
        .zip(&oracle.winner_entity_ids)
        .filter(|(actual, expected)| actual != expected)
        .count();
    let contested_key_count = oracle.winner_entity_ids.len();
    StrategyAccuracy {
        strategy: output.strategy,
        reduction_relative_error: RelativeError {
            max: max_error,
            mean: error_sum / oracle.segmented_sums.len() as f64,
        },
        winner_mismatch_count,
        contested_key_count,
        winner_mismatch_rate: winner_mismatch_count as f64 / contested_key_count as f64,
    }
}

fn split_f64(value: f64) -> [f32; 2] {
    let hi = value as f32;
    let lo = (value - f64::from(hi)) as f32;
    [hi, lo]
}

fn create_strategy_pipelines(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    strategy: PortableStrategy,
) -> StrategyPipelines {
    let suffix = match strategy {
        PortableStrategy::F32 => "f32",
        PortableStrategy::Df64 => "df64",
    };
    StrategyPipelines {
        partial: create_pipeline(
            device,
            module,
            "segmented partial",
            &format!("reduce_partial_{suffix}"),
        ),
        finish: create_pipeline(
            device,
            module,
            "segmented finish",
            &format!("reduce_finish_{suffix}"),
        ),
        map: create_pipeline(device, module, "hazard/race map", &format!("map_{suffix}")),
        argmin: create_pipeline(
            device,
            module,
            "segmented argmin",
            &format!("argmin_{suffix}"),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn create_strategy_bindings(
    device: &wgpu::Device,
    pipelines: &StrategyPipelines,
    config: &wgpu::Buffer,
    offsets: &wgpu::Buffer,
    employers: &wgpu::Buffer,
    health: &wgpu::Buffer,
    weights: &wgpu::Buffer,
    partials: &wgpu::Buffer,
    sums: &wgpu::Buffer,
    races: &wgpu::Buffer,
    winners: &wgpu::Buffer,
    fired: &wgpu::Buffer,
) -> StrategyBindGroups {
    StrategyBindGroups {
        partial: device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("segmented partial bindings"),
            layout: &pipelines.partial.get_bind_group_layout(0),
            entries: &[
                bind_entry(0, config),
                bind_entry(1, offsets),
                bind_entry(3, health),
                bind_entry(4, weights),
                bind_entry(5, partials),
            ],
        }),
        finish: device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("segmented finish bindings"),
            layout: &pipelines.finish.get_bind_group_layout(0),
            entries: &[
                bind_entry(0, config),
                bind_entry(5, partials),
                bind_entry(6, sums),
            ],
        }),
        map: device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hazard/race map bindings"),
            layout: &pipelines.map.get_bind_group_layout(0),
            entries: &[
                bind_entry(0, config),
                bind_entry(1, offsets),
                bind_entry(2, employers),
                bind_entry(3, health),
                bind_entry(6, sums),
                bind_entry(7, races),
                bind_entry(9, fired),
            ],
        }),
        argmin: device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("segmented argmin bindings"),
            layout: &pipelines.argmin.get_bind_group_layout(0),
            entries: &[
                bind_entry(0, config),
                bind_entry(1, offsets),
                bind_entry(7, races),
                bind_entry(8, winners),
                bind_entry(9, fired),
            ],
        }),
    }
}

fn create_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    label: &str,
    entry_point: &str,
) -> wgpu::ComputePipeline {
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: None,
        module,
        entry_point,
        // PipelineCompilationOptions has no strict-math field in wgpu 0.20.
        // The pinned wgpu-hal Metal fork disables fast math while compiling this
        // module; arithmetic_behavior_probe verifies that it was honored.
        compilation_options: wgpu::PipelineCompilationOptions::default(),
    })
}

fn storage_buffer(
    device: &wgpu::Device,
    label: &str,
    contents: &[u8],
    copy_src: bool,
) -> wgpu::Buffer {
    let mut usage = wgpu::BufferUsages::STORAGE;
    if copy_src {
        usage |= wgpu::BufferUsages::COPY_SRC;
    }
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage,
    })
}

fn bind_entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn read_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source: &wgpu::Buffer,
    size: u64,
) -> Result<Vec<u8>, GpuError> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("portable auxiliary readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("portable auxiliary readback copy"),
    });
    encoder.copy_buffer_to_buffer(source, 0, &staging, 0, size);
    queue.submit(Some(encoder.finish()));
    map_buffer(device, &staging)?;
    let bytes = staging.slice(..).get_mapped_range().to_vec();
    staging.unmap();
    Ok(bytes)
}

fn map_buffer(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Result<(), GpuError> {
    let (sender, receiver) = mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device.poll(wgpu::Maintain::Wait);
    receiver
        .recv()
        .map_err(|error| GpuError::new(format!("readback channel failed: {error}")))?
        .map_err(|error| GpuError::new(format!("buffer mapping failed: {error}")))
}

fn read_timestamp_buffer(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
) -> Result<[u64; 6], GpuError> {
    map_buffer(device, buffer)?;
    let mapped = buffer.slice(..).get_mapped_range();
    let values: &[u64] = bytemuck::cast_slice(&mapped);
    let timestamps: [u64; 6] = values
        .try_into()
        .map_err(|_| GpuError::new("timestamp readback did not contain six values"))?;
    drop(mapped);
    buffer.unmap();
    Ok(timestamps)
}

fn timestamp_delta(begin: u64, end: u64, period_ms: f64) -> Option<f64> {
    // Timestamp values are relative to an implementation-defined epoch, so a
    // valid first sample may begin at zero. Only the positive delta matters.
    let elapsed = end.checked_sub(begin)? as f64 * period_ms;
    (elapsed.is_finite() && elapsed > 0.0).then_some(elapsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgsl_philox_matches_four_cpu_derived_vectors() {
        pollster::block_on(async {
            let workload = Workload::generate(WorkloadConfig::with_size(1_000, 50)).unwrap();
            let runner = PortableRunner::new(&workload).await.unwrap();
            let actual = runner.run_philox_known_answers().unwrap();
            let expected: Vec<[u32; 4]> = PHILOX_KNOWN_ANSWERS
                .iter()
                .map(|answer| answer.expected)
                .collect();
            assert_eq!(actual, expected);
        });
    }

    #[test]
    fn one_million_row_accuracy_smoke_beats_f32_on_both_metrics() {
        pollster::block_on(async {
            let report = run_accuracy_smoke().await.unwrap();
            println!("{report}");
            report.assert_numerical_thresholds().unwrap();
            if report.fast_math.strict_math_backend_supported {
                report.assert_thresholds().unwrap();
                assert!(report.fast_math.strict_math_requested);
                assert!(!report.fast_math.fma_contraction_observed);
                assert!(!report.fast_math.reassociation_observed);
                assert!(report.fast_math.df64_residuals_preserved);
                assert!(report.fast_math.trustworthy_on_adapter);
            } else {
                assert!(!report.fast_math.strict_math_requested);
                assert!(!report.fast_math.trustworthy_on_adapter);
            }
            assert!(
                report.df64.reduction_relative_error.max < report.f32.reduction_relative_error.max
            );
            assert!(report.df64.winner_mismatch_rate < report.f32.winner_mismatch_rate);
        });
    }
}
