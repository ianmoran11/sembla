//! Native binary64 WGSL/Vulkan strategy with graceful capability gating.
//!
//! The f64 shader module is never created on Metal or on an adapter lacking
//! `SHADER_F64`. A capable Vulkan device gets a separate device requested with
//! exactly that feature, so PRD-0002's portable module remains unaffected.

use std::{borrow::Cow, error::Error, fmt, sync::mpsc};

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::{
    f64_mirror::{run_f64_mirror, F64MirrorResult},
    fp64::Fp64Throughput,
    gpu::{accuracy_workload_config, ACCURACY_TICK},
    oracle::{run_oracle, OracleResult},
    workload::Workload,
};

const PARTIALS_PER_GROUP: u32 = 2;
const REDUCE_WORKGROUP_SIZE: u32 = 64;
const MAP_WORKGROUP_SIZE: u32 = 256;
const SHADER_SOURCE: &str = include_str!("wgsl/f64_native.wgsl");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeF64Device {
    pub adapter_name: String,
    pub backend: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub throughput: Fp64Throughput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeF64Status {
    Unsupported {
        adapter_name: String,
        backend: String,
        reason: String,
        throughput: Fp64Throughput,
    },
    Supported(NativeF64Device),
}

impl NativeF64Status {
    #[must_use]
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Supported(_))
    }
}

impl fmt::Display for NativeF64Status {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported {
                adapter_name,
                backend,
                reason,
                throughput,
            } => write!(
                formatter,
                "native_f64: unsupported; adapter={adapter_name}; backend={backend}; reason={reason}; {throughput}"
            ),
            Self::Supported(device) => write!(
                formatter,
                "native_f64: supported; adapter={}; backend={}; vendor={:#06x}; device={:#06x}; {}",
                device.adapter_name,
                device.backend,
                device.vendor_id,
                device.device_id,
                device.throughput
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeF64Error(String);

impl NativeF64Error {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for NativeF64Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for NativeF64Error {}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeF64TickResult {
    pub segmented_sums: Vec<f64>,
    pub winner_entity_ids: Vec<u32>,
    pub fired_flags: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeReductionError {
    pub max_relative: f64,
    pub mean_relative: f64,
    pub bitwise_mismatch_groups: usize,
    /// GPU and fixed-tree mirror agree, while the ascending oracle differs.
    pub reduction_order_artifact_groups: usize,
    /// GPU differs from the fixed-tree mirror and cannot be attributed to order.
    pub unexplained_groups: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeF64AccuracyReport {
    pub device: NativeF64Device,
    pub rows: u32,
    pub groups: u32,
    pub tick: u32,
    pub reduction: NativeReductionError,
    pub winner_mismatch_count: usize,
    pub winner_mismatch_rate: f64,
    pub fired_mismatch_count: usize,
}

impl NativeF64AccuracyReport {
    pub fn assert_expected(&self) -> Result<(), String> {
        if self.reduction.unexplained_groups != 0 {
            return Err(format!(
                "native f64 has {} segmented sums differing from its fixed-tree Rust mirror",
                self.reduction.unexplained_groups
            ));
        }
        if self.winner_mismatch_count != 0 {
            return Err(format!(
                "native f64 has {} winner mismatches; reduction order is the only permitted explanation, and the fixed-tree sum attribution is reported separately",
                self.winner_mismatch_count
            ));
        }
        if self.fired_mismatch_count != 0 {
            return Err(format!(
                "native f64 has {} fired-flag mismatches",
                self.fired_mismatch_count
            ));
        }
        Ok(())
    }
}

impl fmt::Display for NativeF64AccuracyReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native f64 accuracy: N={} G={} tick={}; {}; reduction rel-error max={:.6e} mean={:.6e}; bitwise sum mismatches={} (order-attributed={}, unexplained={}); winner mismatches={}/{} ({:.6e}); fired mismatches={}",
            self.rows,
            self.groups,
            self.tick,
            self.device.throughput,
            self.reduction.max_relative,
            self.reduction.mean_relative,
            self.reduction.bitwise_mismatch_groups,
            self.reduction.reduction_order_artifact_groups,
            self.reduction.unexplained_groups,
            self.winner_mismatch_count,
            self.groups,
            self.winner_mismatch_rate,
            self.fired_mismatch_count,
        )
    }
}

pub enum NativeF64Outcome {
    Unsupported(NativeF64Status),
    Executed(NativeF64AccuracyReport),
}

impl fmt::Display for NativeF64Outcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(status) => status.fmt(formatter),
            Self::Executed(report) => report.fmt(formatter),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct NativeConfig {
    rows: u32,
    groups: u32,
    tick: u32,
    map_workgroups_x: u32,
    seed_lo: u32,
    seed_hi: u32,
    partials_per_group: u32,
    pad0: u32,
    beta: f64,
    dt: f64,
}

struct NativePipelines {
    partial: wgpu::ComputePipeline,
    finish: wgpu::ComputePipeline,
    map: wgpu::ComputePipeline,
    argmin: wgpu::ComputePipeline,
}

struct NativeBindings {
    partial: wgpu::BindGroup,
    finish: wgpu::BindGroup,
    map: wgpu::BindGroup,
    argmin: wgpu::BindGroup,
}

pub enum NativeF64RunnerInit {
    Unsupported(NativeF64Status),
    Ready(NativeF64Runner),
}

/// Retained native-f64 buffers and pipelines, reusable by PRD 0005.
pub struct NativeF64Runner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    profile: NativeF64Device,
    rows: u32,
    groups: u32,
    map_dispatch: (u32, u32),
    config_template: NativeConfig,
    config: wgpu::Buffer,
    sums: wgpu::Buffer,
    winners: wgpu::Buffer,
    fired: wgpu::Buffer,
    pipelines: NativePipelines,
    bindings: NativeBindings,
}

/// Capability-only probe. It does not request a device or create f64 WGSL.
pub async fn probe_native_f64() -> NativeF64Status {
    let Some(adapter) = request_adapter().await else {
        return NativeF64Status::Unsupported {
            adapter_name: "none".to_owned(),
            backend: "none".to_owned(),
            reason: "wgpu found no compute adapter".to_owned(),
            throughput: Fp64Throughput::from_model_name("unknown adapter"),
        };
    };
    status_for_adapter(&adapter)
}

impl NativeF64Runner {
    pub async fn new(workload: &Workload) -> Result<NativeF64RunnerInit, NativeF64Error> {
        let Some(adapter) = request_adapter().await else {
            return Ok(NativeF64RunnerInit::Unsupported(
                NativeF64Status::Unsupported {
                    adapter_name: "none".to_owned(),
                    backend: "none".to_owned(),
                    reason: "wgpu found no compute adapter".to_owned(),
                    throughput: Fp64Throughput::from_model_name("unknown adapter"),
                },
            ));
        };
        let status = status_for_adapter(&adapter);
        let NativeF64Status::Supported(profile) = status else {
            return Ok(NativeF64RunnerInit::Unsupported(status));
        };

        let limits = adapter.limits();
        if limits.max_storage_buffers_per_shader_stage < 6 {
            return Err(NativeF64Error::new(format!(
                "adapter exposes only {} storage buffers per stage; 6 required",
                limits.max_storage_buffers_per_shader_stage
            )));
        }
        let largest_storage = u64::from(workload.config.rows) * 8;
        if largest_storage > limits.max_buffer_size
            || largest_storage > u64::from(limits.max_storage_buffer_binding_size)
        {
            return Err(NativeF64Error::new(format!(
                "native f64 row buffer requires {largest_storage} bytes, exceeding adapter limits"
            )));
        }
        let partial_count = workload
            .config
            .groups
            .checked_mul(PARTIALS_PER_GROUP)
            .ok_or_else(|| NativeF64Error::new("native f64 partial count overflow"))?;
        let max_dispatch = limits.max_compute_workgroups_per_dimension;
        let map_workgroups = workload.config.rows.div_ceil(MAP_WORKGROUP_SIZE);
        let map_x = map_workgroups.min(max_dispatch);
        let map_y = map_workgroups.div_ceil(map_x);
        let partial_workgroups = partial_count.div_ceil(REDUCE_WORKGROUP_SIZE);
        let group_workgroups = workload.config.groups.div_ceil(REDUCE_WORKGROUP_SIZE);
        if map_y > max_dispatch
            || partial_workgroups > max_dispatch
            || group_workgroups > max_dispatch
        {
            return Err(NativeF64Error::new(
                "native f64 dispatch exceeds adapter grid limits",
            ));
        }

        let mut required_limits =
            wgpu::Limits::downlevel_defaults().using_resolution(limits.clone());
        required_limits.max_storage_buffers_per_shader_stage = 6;
        required_limits.max_storage_buffer_binding_size = limits.max_storage_buffer_binding_size;
        required_limits.max_buffer_size = limits.max_buffer_size;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Sembla native f64 kernels"),
                    required_features: wgpu::Features::SHADER_F64,
                    required_limits,
                },
                None,
            )
            .await
            .map_err(|error| {
                NativeF64Error::new(format!("native f64 request_device failed: {error}"))
            })?;

        let config_template = NativeConfig {
            rows: workload.config.rows,
            groups: workload.config.groups,
            tick: 0,
            map_workgroups_x: map_x,
            seed_lo: workload.config.seed as u32,
            seed_hi: (workload.config.seed >> 32) as u32,
            partials_per_group: PARTIALS_PER_GROUP,
            pad0: 0,
            beta: workload.config.beta,
            dt: workload.config.dt,
        };
        debug_assert_eq!(std::mem::size_of::<NativeConfig>(), 48);
        let config = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("native f64 config"),
            contents: bytemuck::bytes_of(&config_template),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let offsets = storage_buffer(
            &device,
            "native group offsets",
            bytemuck::cast_slice(&workload.group_offsets),
            false,
        );
        let employers = storage_buffer(
            &device,
            "native employers",
            bytemuck::cast_slice(&workload.employer),
            false,
        );
        let health = storage_buffer(
            &device,
            "native health",
            bytemuck::cast_slice(&workload.health),
            false,
        );
        let weights = storage_buffer(
            &device,
            "native f64 weights",
            bytemuck::cast_slice(&workload.weight),
            false,
        );
        let partials = storage_buffer(
            &device,
            "native f64 partials",
            bytemuck::cast_slice(&vec![0.0_f64; partial_count as usize]),
            false,
        );
        let sums = storage_buffer(
            &device,
            "native f64 sums",
            bytemuck::cast_slice(&vec![0.0_f64; workload.config.groups as usize]),
            true,
        );
        let races = storage_buffer(
            &device,
            "native f64 races",
            bytemuck::cast_slice(&vec![0.0_f64; workload.config.rows as usize]),
            false,
        );
        let winners = storage_buffer(
            &device,
            "native f64 winners",
            bytemuck::cast_slice(&vec![u32::MAX; workload.config.groups as usize]),
            true,
        );
        let fired = storage_buffer(
            &device,
            "native f64 fired",
            bytemuck::cast_slice(&vec![0_u32; workload.config.rows as usize]),
            true,
        );

        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("native f64 WGSL"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER_SOURCE)),
        });
        let pipelines = NativePipelines {
            partial: create_pipeline(&device, &module, "native f64 partial", "reduce_partial_f64"),
            finish: create_pipeline(&device, &module, "native f64 finish", "reduce_finish_f64"),
            map: create_pipeline(&device, &module, "native f64 map", "map_f64"),
            argmin: create_pipeline(&device, &module, "native f64 argmin", "argmin_f64"),
        };
        let bindings = create_bindings(
            &device, &pipelines, &config, &offsets, &employers, &health, &weights, &partials,
            &sums, &races, &winners, &fired,
        );
        if let Some(error) = device.pop_error_scope().await {
            return Err(NativeF64Error::new(format!(
                "native f64 shader/pipeline validation failed: {error}"
            )));
        }

        Ok(NativeF64RunnerInit::Ready(Self {
            device,
            queue,
            profile,
            rows: workload.config.rows,
            groups: workload.config.groups,
            map_dispatch: (map_x, map_y),
            config_template,
            config,
            sums,
            winners,
            fired,
            pipelines,
            bindings,
        }))
    }

    #[must_use]
    pub fn profile(&self) -> &NativeF64Device {
        &self.profile
    }

    pub fn dispatch_reduction_only(&self, tick: u32) {
        self.write_tick(tick);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("native f64 reduction"),
            });
        self.encode_reduction(&mut encoder);
        self.queue.submit(Some(encoder.finish()));
    }

    pub fn dispatch_map_argmin_only(&self, tick: u32) {
        self.write_tick(tick);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("native f64 map argmin"),
            });
        self.encode_map_argmin(&mut encoder);
        self.queue.submit(Some(encoder.finish()));
    }

    pub fn dispatch_tick_only(&self, tick: u32) {
        self.write_tick(tick);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("native f64 tick"),
            });
        self.encode_reduction(&mut encoder);
        self.encode_map_argmin(&mut encoder);
        self.queue.submit(Some(encoder.finish()));
    }

    pub fn dispatch_tick(&self, tick: u32) -> Result<NativeF64TickResult, NativeF64Error> {
        self.dispatch_tick_only(tick);
        self.read_tick_result()
    }

    pub fn wait(&self) {
        self.device.poll(wgpu::Maintain::Wait);
    }

    fn write_tick(&self, tick: u32) {
        let mut config = self.config_template;
        config.tick = tick;
        self.queue
            .write_buffer(&self.config, 0, bytemuck::bytes_of(&config));
    }

    fn encode_reduction(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("native f64 reduction passes"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipelines.partial);
        pass.set_bind_group(0, &self.bindings.partial, &[]);
        pass.dispatch_workgroups(
            (self.groups * PARTIALS_PER_GROUP).div_ceil(REDUCE_WORKGROUP_SIZE),
            1,
            1,
        );
        pass.set_pipeline(&self.pipelines.finish);
        pass.set_bind_group(0, &self.bindings.finish, &[]);
        pass.dispatch_workgroups(self.groups.div_ceil(REDUCE_WORKGROUP_SIZE), 1, 1);
    }

    fn encode_map_argmin(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("native f64 map and argmin"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipelines.map);
        pass.set_bind_group(0, &self.bindings.map, &[]);
        pass.dispatch_workgroups(self.map_dispatch.0, self.map_dispatch.1, 1);
        pass.set_pipeline(&self.pipelines.argmin);
        pass.set_bind_group(0, &self.bindings.argmin, &[]);
        pass.dispatch_workgroups(self.groups.div_ceil(REDUCE_WORKGROUP_SIZE), 1, 1);
    }

    fn read_tick_result(&self) -> Result<NativeF64TickResult, NativeF64Error> {
        let sum_bytes = u64::from(self.groups) * 8;
        let winner_bytes = u64::from(self.groups) * 4;
        let fired_bytes = u64::from(self.rows) * 4;
        let winner_offset = sum_bytes;
        let fired_offset = winner_offset + winner_bytes;
        let total = fired_offset + fired_bytes;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("native f64 tick readback"),
            size: total,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("native f64 readback copy"),
            });
        encoder.copy_buffer_to_buffer(&self.sums, 0, &staging, 0, sum_bytes);
        encoder.copy_buffer_to_buffer(&self.winners, 0, &staging, winner_offset, winner_bytes);
        encoder.copy_buffer_to_buffer(&self.fired, 0, &staging, fired_offset, fired_bytes);
        self.queue.submit(Some(encoder.finish()));
        map_buffer(&self.device, &staging)?;
        let mapped = staging.slice(..).get_mapped_range();
        let segmented_sums =
            bytemuck::cast_slice::<u8, f64>(&mapped[..sum_bytes as usize]).to_vec();
        let winner_entity_ids =
            bytemuck::cast_slice::<u8, u32>(&mapped[winner_offset as usize..fired_offset as usize])
                .to_vec();
        let fired_flags =
            bytemuck::cast_slice::<u8, u32>(&mapped[fired_offset as usize..]).to_vec();
        drop(mapped);
        staging.unmap();
        Ok(NativeF64TickResult {
            segmented_sums,
            winner_entity_ids,
            fired_flags,
        })
    }
}

/// Probes first, so unsupported development machines do not generate the 1M-row
/// correctness workload merely to report that native f64 is unanswered.
pub async fn run_native_f64_accuracy_smoke() -> Result<NativeF64Outcome, NativeF64Error> {
    let status = probe_native_f64().await;
    if !status.is_supported() {
        return Ok(NativeF64Outcome::Unsupported(status));
    }
    let workload = Workload::generate(accuracy_workload_config()).map_err(|error| {
        NativeF64Error::new(format!("native accuracy workload failed: {error}"))
    })?;
    let oracle = run_oracle(&workload, ACCURACY_TICK);
    let mirror = run_f64_mirror(&workload, ACCURACY_TICK);
    let NativeF64RunnerInit::Ready(runner) = NativeF64Runner::new(&workload).await? else {
        return Err(NativeF64Error::new(
            "native f64 support changed between probe and device creation",
        ));
    };
    let output = runner.dispatch_tick(ACCURACY_TICK)?;
    Ok(NativeF64Outcome::Executed(score_native(
        runner.profile().clone(),
        workload.config.rows,
        workload.config.groups,
        ACCURACY_TICK,
        &output,
        &oracle,
        &mirror,
    )))
}

pub(crate) fn score_native(
    device: NativeF64Device,
    rows: u32,
    groups: u32,
    tick: u32,
    output: &NativeF64TickResult,
    oracle: &OracleResult,
    mirror: &F64MirrorResult,
) -> NativeF64AccuracyReport {
    assert_eq!(output.segmented_sums.len(), oracle.segmented_sums.len());
    let mut max_relative = 0.0_f64;
    let mut relative_sum = 0.0_f64;
    let mut bitwise_mismatch_groups = 0;
    let mut reduction_order_artifact_groups = 0;
    let mut unexplained_groups = 0;
    for ((actual, expected), fixed_tree) in output
        .segmented_sums
        .iter()
        .zip(&oracle.segmented_sums)
        .zip(&mirror.segmented_sums)
    {
        let relative = if *expected == 0.0 {
            (actual - expected).abs()
        } else {
            (actual - expected).abs() / expected.abs()
        };
        max_relative = max_relative.max(relative);
        relative_sum += relative;
        if actual.to_bits() != expected.to_bits() {
            bitwise_mismatch_groups += 1;
            if actual.to_bits() == fixed_tree.to_bits() {
                reduction_order_artifact_groups += 1;
            }
        }
        if actual.to_bits() != fixed_tree.to_bits() {
            unexplained_groups += 1;
        }
    }
    let winner_mismatch_count = output
        .winner_entity_ids
        .iter()
        .zip(&oracle.winner_entity_ids)
        .filter(|(actual, expected)| actual != expected)
        .count();
    let fired_mismatch_count = output
        .fired_flags
        .iter()
        .zip(&oracle.fired_flags)
        .filter(|(actual, expected)| actual != expected)
        .count();
    NativeF64AccuracyReport {
        device,
        rows,
        groups,
        tick,
        reduction: NativeReductionError {
            max_relative,
            mean_relative: relative_sum / oracle.segmented_sums.len() as f64,
            bitwise_mismatch_groups,
            reduction_order_artifact_groups,
            unexplained_groups,
        },
        winner_mismatch_count,
        winner_mismatch_rate: winner_mismatch_count as f64 / groups as f64,
        fired_mismatch_count,
    }
}

async fn request_adapter() -> Option<wgpu::Adapter> {
    wgpu::Instance::default()
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
}

fn status_for_adapter(adapter: &wgpu::Adapter) -> NativeF64Status {
    let info = adapter.get_info();
    let backend = format!("{:?}", info.backend);
    let throughput = Fp64Throughput::from_model_name(info.name.clone());
    let reason = if info.backend != wgpu::Backend::Vulkan {
        Some("native WGSL f64 is supported by wgpu only on Vulkan")
    } else if !adapter.features().contains(wgpu::Features::SHADER_F64) {
        Some("adapter does not expose Features::SHADER_F64")
    } else {
        None
    };
    if let Some(reason) = reason {
        NativeF64Status::Unsupported {
            adapter_name: info.name,
            backend,
            reason: reason.to_owned(),
            throughput,
        }
    } else {
        NativeF64Status::Supported(NativeF64Device {
            adapter_name: info.name,
            backend,
            vendor_id: info.vendor,
            device_id: info.device,
            throughput,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn create_bindings(
    device: &wgpu::Device,
    pipelines: &NativePipelines,
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
) -> NativeBindings {
    NativeBindings {
        partial: bind_group(
            device,
            "native partial bindings",
            &pipelines.partial,
            &[
                (0, config),
                (1, offsets),
                (3, health),
                (4, weights),
                (5, partials),
            ],
        ),
        finish: bind_group(
            device,
            "native finish bindings",
            &pipelines.finish,
            &[(0, config), (5, partials), (6, sums)],
        ),
        map: bind_group(
            device,
            "native map bindings",
            &pipelines.map,
            &[
                (0, config),
                (1, offsets),
                (2, employers),
                (3, health),
                (6, sums),
                (7, races),
                (9, fired),
            ],
        ),
        argmin: bind_group(
            device,
            "native argmin bindings",
            &pipelines.argmin,
            &[
                (0, config),
                (1, offsets),
                (7, races),
                (8, winners),
                (9, fired),
            ],
        ),
    }
}

fn bind_group(
    device: &wgpu::Device,
    label: &str,
    pipeline: &wgpu::ComputePipeline,
    buffers: &[(u32, &wgpu::Buffer)],
) -> wgpu::BindGroup {
    let entries: Vec<wgpu::BindGroupEntry<'_>> = buffers
        .iter()
        .map(|(binding, buffer)| wgpu::BindGroupEntry {
            binding: *binding,
            resource: buffer.as_entire_binding(),
        })
        .collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    })
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

fn map_buffer(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Result<(), NativeF64Error> {
    let (sender, receiver) = mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device.poll(wgpu::Maintain::Wait);
    receiver
        .recv()
        .map_err(|error| NativeF64Error::new(format!("native readback channel failed: {error}")))?
        .map_err(|error| NativeF64Error::new(format!("native buffer mapping failed: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_f64_wgsl_validates_locally_with_float64_capability() {
        let module = naga::front::wgsl::parse_str(SHADER_SOURCE).unwrap();
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::FLOAT64,
        );
        validator.validate(&module).unwrap();

        let mut without_f64 = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        assert!(without_f64.validate(&module).is_err());
    }

    #[test]
    fn native_f64_is_capability_gated_and_smokes_when_available() {
        pollster::block_on(async {
            let outcome = run_native_f64_accuracy_smoke().await.unwrap();
            println!("{outcome}");
            match outcome {
                NativeF64Outcome::Unsupported(status) => assert!(!status.is_supported()),
                NativeF64Outcome::Executed(report) => report.assert_expected().unwrap(),
            }
        });
    }

    #[test]
    fn native_config_layout_matches_wgsl() {
        assert_eq!(std::mem::size_of::<NativeConfig>(), 48);
        assert_eq!(std::mem::align_of::<NativeConfig>(), 8);
    }
}
