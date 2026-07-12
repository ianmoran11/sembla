use std::{fmt::Write as _, path::Path, sync::mpsc, time::Instant};

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

pub const TARGET_ROWS: u32 = 26_000_000;
pub const TARGET_GROUPS: u32 = 1_300_000;
const WORKGROUP_SIZE: u32 = 256;
const MAX_DISPATCH_X: u32 = 65_535;
const SEED: u64 = 0x0123_4567_89ab_cdef;
// A finite high hazard keeps almost every susceptible row eligible so the
// i % 10 == 5 selector genuinely sends about 10% of all rows through argmin.
const BETA: f32 = 100.0;
const DT: f32 = 0.25;
#[rustfmt::skip]
const KERNELS: [&str; 6] = [
    "clear", "aggregate", "hazard/map", "argmin race", "argmin tie", "state write",
];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Config {
    rows: u32,
    groups: u32,
    workgroups_x: u32,
    tick: u32,
    seed_lo: u32,
    seed_hi: u32,
    group_size: u32,
    _pad0: u32,
    beta: f32,
    dt: f32,
    _pad1: u32,
    _pad2: u32,
}

#[cfg(test)]
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct KatInput {
    seed_lo: u32,
    seed_hi: u32,
    tick: u32,
    rule_id: u32,
    entity_id: u32,
    draw_idx: u32,
}

#[derive(Debug, Clone)]
pub struct AdapterDescription {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub driver_info: String,
    pub software: bool,
    pub timestamps: bool,
    pub shader_f64: bool,
}

#[derive(Debug)]
pub struct BenchmarkResult {
    pub adapter: AdapterDescription,
    pub rows: u32,
    pub groups: u32,
    pub warmup_ticks: usize,
    pub measured_ticks: usize,
    pub kernel_ms: Option<Vec<f64>>,
    pub total_ms: f64,
    pub total_from_gpu_timestamps: bool,
    pub rows_per_second: f64,
    pub fired_last_tick: u32,
    pub contested_candidates_last_tick: u32,
    pub downscale_reason: Option<String>,
}

struct Pipelines {
    clear: wgpu::ComputePipeline,
    aggregate: wgpu::ComputePipeline,
    map: wgpu::ComputePipeline,
    argmin: wgpu::ComputePipeline,
    tie: wgpu::ComputePipeline,
    write: wgpu::ComputePipeline,
}

struct BindGroups {
    clear: wgpu::BindGroup,
    aggregate: wgpu::BindGroup,
    map: wgpu::BindGroup,
    argmin: wgpu::BindGroup,
    tie: wgpu::BindGroup,
    write: wgpu::BindGroup,
}

pub struct GpuTick {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: wgpu::AdapterInfo,
    software: bool,
    timestamps: bool,
    shader_f64: bool,
    rows: u32,
    groups: u32,
    dispatch: (u32, u32),
    config: Config,
    config_buffer: wgpu::Buffer,
    candidate: wgpu::Buffer,
    winner: wgpu::Buffer,
    fired: wgpu::Buffer,
    pipelines: Pipelines,
    bind_groups: BindGroups,
}

fn storage_buffer(device: &wgpu::Device, label: &str, bytes: &[u8]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    })
}

fn bind_entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn dispatch_for(items: u32) -> (u32, u32) {
    let workgroups = items.div_ceil(WORKGROUP_SIZE);
    let x = workgroups.min(MAX_DISPATCH_X).max(1);
    (x, workgroups.div_ceil(x).max(1))
}

async fn request_adapter() -> Result<(wgpu::Instance, wgpu::Adapter), String> {
    let instance = wgpu::Instance::default();
    let options = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    };
    if let Some(adapter) = instance.request_adapter(&options).await {
        return Ok((instance, adapter));
    }
    let fallback = wgpu::RequestAdapterOptions {
        force_fallback_adapter: true,
        ..options
    };
    let adapter = instance
        .request_adapter(&fallback)
        .await
        .ok_or_else(|| "wgpu found no compute adapter".to_owned())?;
    Ok((instance, adapter))
}

fn is_software(info: &wgpu::AdapterInfo) -> bool {
    let text = format!("{} {} {}", info.name, info.driver, info.driver_info).to_ascii_lowercase();
    matches!(
        info.device_type,
        wgpu::DeviceType::Cpu | wgpu::DeviceType::Other
    ) || [
        "lavapipe",
        "llvmpipe",
        "swiftshader",
        "softpipe",
        "swrast",
        "warp",
        "microsoft basic render",
        "software rasterizer",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

#[rustfmt::skip]
impl GpuTick {
    pub async fn new(requested_rows: u32, requested_groups: u32) -> Result<(Self, Option<String>), String> {
        let (_instance, adapter) = request_adapter().await?;
        let info = adapter.get_info();
        let software = is_software(&info);
        let adapter_limits = adapter.limits();
        let per_buffer_rows = (adapter_limits
            .max_storage_buffer_binding_size
            .min(adapter_limits.max_buffer_size.min(u64::from(u32::MAX)) as u32)
            / 4)
            .max(10_000);
        // wgpu exposes no portable heap-budget query. Use half the adapter's
        // maximum buffer size, capped at 512 MiB, as a conservative aggregate
        // resident-column budget. The workload uses about 16 bytes/person plus
        // 12 bytes/employer; this still permits the proven full Apple run.
        let resident_budget = (adapter_limits.max_buffer_size / 2)
            .clamp(64 * 1024 * 1024, 512 * 1024 * 1024);
        let ratio = requested_rows as f64 / requested_groups.max(1) as f64;
        let bytes_per_row = 16.0 + 12.0 / ratio;
        let resident_rows = ((resident_budget as f64 / bytes_per_row) as u64)
            .min(u64::from(u32::MAX)) as u32;
        let software_cap = if software { 200_000 } else { requested_rows };
        let rows = requested_rows
            .min(per_buffer_rows)
            .min(resident_rows.max(10_000))
            .min(software_cap)
            .max(10_000);
        let groups = requested_groups
            .min(((rows as f64 / ratio).round() as u32).max(1))
            .min(rows);
        let downscale_reason = (rows < requested_rows).then(|| {
            if software {
                format!("software adapter safety cap reduced {} requested rows to {rows}", requested_rows)
            } else if rows == per_buffer_rows {
                format!(
                    "adapter max storage-buffer binding size {} bytes reduced {} requested rows to {rows}",
                    adapter_limits.max_storage_buffer_binding_size, requested_rows
                )
            } else {
                format!(
                    "conservative aggregate resident-memory budget of {resident_budget} bytes reduced {} requested rows to {rows}",
                    requested_rows
                )
            }
        });

        let adapter_features = adapter.features();
        let timestamp_supported = adapter_features.contains(wgpu::Features::TIMESTAMP_QUERY);
        let shader_f64 = adapter_features.contains(wgpu::Features::SHADER_F64);
        let required_features = if timestamp_supported {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        };
        let mut required_limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter_limits);
        required_limits.max_storage_buffers_per_shader_stage = 6;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Sembla GPU throughput spike"),
                    required_features,
                    required_limits,
                },
                None,
            )
            .await
            .map_err(|error| format!("request_device failed: {error}"))?;

        let group_size = rows.div_ceil(groups);
        let health: Vec<u32> = (0..rows)
            .map(|i| if i % group_size < group_size / 5 { 1 } else { 0 })
            .collect();
        let employer: Vec<u32> = (0..rows).map(|i| (i / group_size).min(groups - 1)).collect();
        let zeros_rows = vec![0_u32; rows as usize];
        let zeros_groups = vec![0_u32; groups as usize];
        let max_groups = vec![u32::MAX; groups as usize];

        let health = storage_buffer(&device, "health", bytemuck::cast_slice(&health));
        let employer = storage_buffer(&device, "employer", bytemuck::cast_slice(&employer));
        let counts = storage_buffer(&device, "infectious counts", bytemuck::cast_slice(&zeros_groups));
        let candidate = storage_buffer(&device, "candidate flags", bytemuck::cast_slice(&zeros_rows));
        let race = storage_buffer(&device, "race bits", bytemuck::cast_slice(&zeros_rows));
        let best_race = storage_buffer(&device, "best race", bytemuck::cast_slice(&max_groups));
        let winner = storage_buffer(&device, "winner", bytemuck::cast_slice(&max_groups));
        let fired = storage_buffer(&device, "fired counter", bytemuck::bytes_of(&0_u32));

        let dispatch = dispatch_for(rows);
        let config = Config {
            rows,
            groups,
            workgroups_x: dispatch.0,
            tick: 0,
            seed_lo: SEED as u32,
            seed_hi: (SEED >> 32) as u32,
            group_size,
            _pad0: 0,
            beta: BETA,
            dt: DT,
            _pad1: 0,
            _pad2: 0,
        };
        let config_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tick config"),
            contents: bytemuck::bytes_of(&config),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sembla tick kernels"),
            source: wgpu::ShaderSource::Wgsl(include_str!("kernels.wgsl").into()),
        });
        let pipeline = |label: &'static str, entry_point: &'static str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: None,
                module: &module,
                entry_point,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            })
        };
        let pipelines = Pipelines {
            clear: pipeline("clear", "clear"),
            aggregate: pipeline("aggregate", "aggregate"),
            map: pipeline("hazard/map", "hazard_map"),
            argmin: pipeline("argmin race", "argmin_race"),
            tie: pipeline("argmin tie", "argmin_tie"),
            write: pipeline("state write", "state_write"),
        };
        let bind_groups = BindGroups {
            clear: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("clear bindings"),
                layout: &pipelines.clear.get_bind_group_layout(0),
                entries: &[bind_entry(10, &config_buffer), bind_entry(11, &counts), bind_entry(12, &best_race), bind_entry(13, &winner), bind_entry(14, &fired), bind_entry(15, &health)],
            }),
            aggregate: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("aggregate bindings"),
                layout: &pipelines.aggregate.get_bind_group_layout(0),
                entries: &[bind_entry(20, &config_buffer), bind_entry(21, &health), bind_entry(22, &employer), bind_entry(23, &counts)],
            }),
            map: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("map bindings"),
                layout: &pipelines.map.get_bind_group_layout(0),
                entries: &[bind_entry(30, &config_buffer), bind_entry(31, &health), bind_entry(32, &employer), bind_entry(33, &counts), bind_entry(34, &candidate), bind_entry(35, &race)],
            }),
            argmin: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("argmin bindings"),
                layout: &pipelines.argmin.get_bind_group_layout(0),
                entries: &[bind_entry(40, &config_buffer), bind_entry(41, &employer), bind_entry(42, &candidate), bind_entry(43, &race), bind_entry(44, &best_race)],
            }),
            tie: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tie bindings"),
                layout: &pipelines.tie.get_bind_group_layout(0),
                entries: &[bind_entry(50, &config_buffer), bind_entry(51, &employer), bind_entry(52, &candidate), bind_entry(53, &race), bind_entry(54, &best_race), bind_entry(55, &winner)],
            }),
            write: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("write bindings"),
                layout: &pipelines.write.get_bind_group_layout(0),
                entries: &[bind_entry(60, &config_buffer), bind_entry(61, &health), bind_entry(62, &employer), bind_entry(63, &candidate), bind_entry(64, &winner), bind_entry(65, &fired)],
            }),
        };

        Ok((Self {
            device,
            queue,
            info,
            software,
            timestamps: timestamp_supported,
            shader_f64,
            rows,
            groups,
            dispatch,
            config,
            config_buffer,
            candidate,
            winner,
            fired,
            pipelines,
            bind_groups,
        }, downscale_reason))
    }

    pub fn adapter_description(&self) -> AdapterDescription {
        AdapterDescription {
            name: self.info.name.clone(),
            backend: format!("{:?}", self.info.backend),
            device_type: format!("{:?}", self.info.device_type),
            driver: self.info.driver.clone(),
            driver_info: self.info.driver_info.clone(),
            software: self.software,
            timestamps: self.timestamps,
            shader_f64: self.shader_f64,
        }
    }

    fn encode_tick(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        query_set: Option<&wgpu::QuerySet>,
    ) {
        let stages = [
            (&self.pipelines.clear, &self.bind_groups.clear, self.dispatch),
            (&self.pipelines.aggregate, &self.bind_groups.aggregate, self.dispatch),
            (&self.pipelines.map, &self.bind_groups.map, self.dispatch),
            (&self.pipelines.argmin, &self.bind_groups.argmin, self.dispatch),
            (&self.pipelines.tie, &self.bind_groups.tie, self.dispatch),
            (&self.pipelines.write, &self.bind_groups.write, self.dispatch),
        ];
        for (index, (pipeline, bindings, dispatch)) in stages.iter().enumerate() {
            let timestamps = query_set.map(|set| wgpu::ComputePassTimestampWrites {
                query_set: set,
                beginning_of_pass_write_index: Some((index * 2) as u32),
                end_of_pass_write_index: Some((index * 2 + 1) as u32),
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(KERNELS[index]),
                timestamp_writes: timestamps,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bindings, &[]);
            pass.dispatch_workgroups(dispatch.0, dispatch.1, 1);
        }
    }

    fn set_tick(&mut self, tick: u32) {
        self.config.tick = tick;
        self.queue.write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&self.config));
    }

    pub fn run_one(&mut self, tick: u32) {
        self.set_tick(tick);
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("one tick") });
        self.encode_tick(&mut encoder, None);
        self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);
    }

    pub fn read_candidates(&self) -> Vec<u32> {
        read_buffer_u32(&self.device, &self.queue, &self.candidate, self.rows as usize)
    }

    pub fn read_winners(&self) -> Vec<u32> {
        read_buffer_u32(&self.device, &self.queue, &self.winner, self.groups as usize)
    }

    pub fn read_fired(&self) -> u32 {
        read_buffer_u32(&self.device, &self.queue, &self.fired, 1)[0]
    }

    pub fn benchmark(mut self, warmup_ticks: usize, measured_ticks: usize, downscale_reason: Option<String>) -> BenchmarkResult {
        assert!(measured_ticks >= 100);
        for tick in 0..warmup_ticks {
            self.run_one(tick as u32);
        }

        let query_count = KERNELS.len() * 2;
        let query_set = self.timestamps.then(|| self.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("per-kernel and end-to-end timestamp queries"),
            ty: wgpu::QueryType::Timestamp,
            count: query_count as u32,
        }));
        let query_bytes = (query_count * std::mem::size_of::<u64>()) as u64;
        let query_resolve = query_set.as_ref().map(|_| self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamp resolve"), size: query_bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));
        let query_read = query_set.as_ref().map(|_| self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamp read"), size: query_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }));

        let mut cpu_totals = Vec::with_capacity(measured_ticks);
        let mut gpu_samples = vec![Vec::with_capacity(measured_ticks); KERNELS.len()];
        let mut gpu_totals = Vec::with_capacity(measured_ticks);
        for sample in 0..measured_ticks {
            self.set_tick((warmup_ticks + sample) as u32);
            let start = Instant::now();
            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("measured tick") });
            self.encode_tick(&mut encoder, query_set.as_ref());
            if let (Some(set), Some(resolve), Some(read)) = (&query_set, &query_resolve, &query_read) {
                encoder.resolve_query_set(set, 0..query_count as u32, resolve, 0);
                encoder.copy_buffer_to_buffer(resolve, 0, read, 0, query_bytes);
            }
            self.queue.submit(Some(encoder.finish()));
            self.device.poll(wgpu::Maintain::Wait);
            cpu_totals.push(start.elapsed().as_secs_f64() * 1000.0);
            if let Some(read) = &query_read {
                let values = read_buffer_u64(&self.device, read, query_count);
                let period = f64::from(self.queue.get_timestamp_period()) / 1_000_000.0;
                let mut stage_sum = 0.0;
                for index in 0..KERNELS.len() {
                    let elapsed = values[index * 2 + 1]
                        .saturating_sub(values[index * 2]) as f64 * period;
                    gpu_samples[index].push(elapsed);
                    stage_sum += elapsed;
                }
                // End-to-end GPU tick time is final-pass-end minus
                // first-pass-begin, never a sum of stage intervals. Some
                // backends return incomplete cross-pass pairs; reject those
                // and use the synchronized wall-clock fallback.
                let begin = values[0];
                let end = values[KERNELS.len() * 2 - 1];
                let elapsed = end.saturating_sub(begin) as f64 * period;
                if begin != 0 && end > begin && elapsed >= stage_sum * 0.99 {
                    gpu_totals.push(elapsed);
                }
            }
        }
        let timestamp_total = (gpu_totals.len() == measured_ticks).then(|| median(&mut gpu_totals));
        let kernel_ms = query_set.as_ref().map(|_| gpu_samples.iter_mut().map(|samples| median(samples)).collect::<Vec<_>>());
        let total_from_gpu_timestamps = timestamp_total.is_some();
        let total_ms = timestamp_total.unwrap_or_else(|| median(&mut cpu_totals));
        let fired_last_tick = self.read_fired();
        let contested_candidates_last_tick = self
            .read_candidates()
            .into_iter()
            .enumerate()
            .filter(|(entity, candidate)| entity % 10 == 5 && *candidate != 0)
            .count() as u32;
        BenchmarkResult {
            adapter: self.adapter_description(), rows: self.rows, groups: self.groups,
            warmup_ticks, measured_ticks, kernel_ms, total_ms, total_from_gpu_timestamps,
            rows_per_second: f64::from(self.rows) / (total_ms / 1000.0), fired_last_tick,
            contested_candidates_last_tick, downscale_reason,
        }
    }
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    if values.len() % 2 == 0 {
        (values[values.len() / 2 - 1] + values[values.len() / 2]) / 2.0
    } else {
        values[values.len() / 2]
    }
}

#[rustfmt::skip]
fn read_buffer_u32(device: &wgpu::Device, queue: &wgpu::Queue, source: &wgpu::Buffer, count: usize) -> Vec<u32> {
    let size = (count * 4) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("u32 readback"), size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("u32 readback") });
    encoder.copy_buffer_to_buffer(source, 0, &staging, 0, size);
    queue.submit(Some(encoder.finish()));
    map_read(device, &staging);
    let result = bytemuck::cast_slice(&staging.slice(..).get_mapped_range()).to_vec();
    staging.unmap();
    result
}

fn read_buffer_u64(device: &wgpu::Device, source: &wgpu::Buffer, count: usize) -> Vec<u64> {
    map_read(device, source);
    let result = bytemuck::cast_slice(&source.slice(..).get_mapped_range())[..count].to_vec();
    source.unmap();
    result
}

#[rustfmt::skip]
fn map_read(device: &wgpu::Device, buffer: &wgpu::Buffer) {
    let (sender, receiver) = mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| sender.send(result).unwrap());
    device.poll(wgpu::Maintain::Wait);
    receiver.recv().unwrap().unwrap();
}

#[cfg(test)]
#[rustfmt::skip]
fn cpu_philox(seed: u64, tick: u32, rule: u32, entity: u32, draw: u32) -> [u32; 4] {
    let mut c = [tick, rule, entity, draw];
    let mut key = [seed as u32, (seed >> 32) as u32];
    for round in 0..10 {
        let p0 = u64::from(0xd251_1f53_u32) * u64::from(c[0]);
        let p1 = u64::from(0xcd9e_8d57_u32) * u64::from(c[2]);
        c = [(p1 >> 32) as u32 ^ c[1] ^ key[0], p1 as u32, (p0 >> 32) as u32 ^ c[3] ^ key[1], p0 as u32];
        if round != 9 { key[0] = key[0].wrapping_add(0x9e37_79b9); key[1] = key[1].wrapping_add(0xbb67_ae85); }
    }
    c
}

#[cfg(test)]
#[rustfmt::skip]
async fn gpu_philox(inputs: &[KatInput]) -> Result<Vec<[u32; 4]>, String> {
    let (_instance, adapter) = request_adapter().await?;
    let limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Philox KAT device"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
            },
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Philox KAT"), source: wgpu::ShaderSource::Wgsl(include_str!("kernels.wgsl").into()) });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("Philox KAT"), layout: None, module: &module, entry_point: "philox_kat", compilation_options: Default::default() });
    let input = storage_buffer(&device, "KAT input", bytemuck::cast_slice(inputs));
    let output = storage_buffer(&device, "KAT output", &vec![0; inputs.len() * 16]);
    let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("KAT bindings"), layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: input.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: output.as_entire_binding() }],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    { let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("KAT"), timestamp_writes: None }); pass.set_pipeline(&pipeline); pass.set_bind_group(0, &bindings, &[]); pass.dispatch_workgroups((inputs.len() as u32).div_ceil(64), 1, 1); }
    queue.submit(Some(encoder.finish()));
    let words = read_buffer_u32(&device, &queue, &output, inputs.len() * 4);
    Ok(words.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect())
}

#[rustfmt::skip]
pub fn write_results(path: &Path, result: &BenchmarkResult) -> std::io::Result<()> {
    let extrapolated = result.total_ms * f64::from(TARGET_ROWS) / f64::from(result.rows);
    let mut kernel_table = String::new();
    if let Some(values) = &result.kernel_ms {
        for (name, ms) in KERNELS.iter().zip(values) {
            writeln!(kernel_table, "| {name} | {ms:.4} |").unwrap();
        }
    } else {
        for name in KERNELS {
            writeln!(kernel_table, "| {name} | unsupported (CPU wall-clock total only) |").unwrap();
        }
    }
    let status = if result.adapter.software { "**SOFTWARE ADAPTER — THROUGHPUT QUESTION UNANSWERED.**" } else { "Hardware adapter measurement of the portable WGSL `f32` kernel skeleton." };
    let bottleneck = result.kernel_ms.as_ref().and_then(|values| {
        values.iter().enumerate().max_by(|left, right| left.1.total_cmp(right.1)).map(|(index, _)| KERNELS[index])
    }).unwrap_or("per-kernel timing unavailable");
    let verdict = if result.adapter.software {
        "The v0.1 success-criterion #4 throughput question is explicitly **unanswered on this hardware**. Software rasterizer timings are not evidence for or against credible ticks/sec at 26M rows. The per-row hazard/map and the atomic infectious-count/contested-argmin stages are the likely bandwidth and contention bottlenecks to validate on a discrete or integrated hardware GPU before designing the real v0.2 backend.".to_owned()
    } else {
        format!("Against v0.1 success criterion #4, the measured {:.3} ticks/sec at 26M rows makes the core GPU kernel shape plausible on this hardware. However, this portable WGSL path provides only `f32` hazard/race arithmetic, while Sembla's production convention requires `f64`; compliant production-`f64` throughput therefore remains **unanswered on this hardware**, and this rate is optimistic directional evidence rather than a production-backend result. The measured **{bottleneck}** stage is the bottleneck; v0.2 should first validate a supported `f64` strategy and then profile its bandwidth and contention while retaining deterministic two-pass lexicographic conflict resolution.", 1000.0 / extrapolated)
    };
    let reason = result.downscale_reason.as_deref().unwrap_or("none (full requested scale fit adapter limits and resident-memory safety budget)");
    let contested_percent = f64::from(result.contested_candidates_last_tick) * 100.0 / f64::from(result.rows);
    let total_method = if result.total_from_gpu_timestamps {
        "end-to-end GPU timestamp from before the first pass to after the final pass"
    } else {
        "synchronized CPU wall-clock fallback because complete cross-pass GPU timestamps were unavailable"
    };
    let markdown = format!("# GPU throughput spike results\n\n{status}\n\nGenerated by `cargo run --release` on this machine.\n\n## Hardware\n\n- Adapter: `{}`\n- Backend: `{}`\n- Device type: `{}`\n- Driver: `{}`\n- Driver info: `{}`\n- Timestamp queries: {}\n- Shader `f64` capability: {}\n\n## Workload and sizes\n\n- Requested scale: {TARGET_ROWS} person rows / {TARGET_GROUPS} employer groups\n- Actual scale: {} person rows / {} employer groups\n- Downscale reason: {reason}\n- Warmup ticks: {}\n- Measured ticks: {}\n- Device-resident data: yes\n- Numeric precision: portable WGSL `f32` hazard and race arithmetic; this does not satisfy the production `f64` convention, so the 10k smoke test is exact only against the spike's scalar `f32` implementation\n- Steady-state setup: the clear kernel restores the fixed initial enum state before aggregation so every measured tick has the same contested workload\n- Aggregate choice: per-employer `atomicAdd` infectious counts (**atomics / Level C fork**, not deterministic reduction)\n- Conflict choice: selector `entity_id % 10 == 5` makes exactly 10% of rows eligible (two susceptible contenders per 20-row employer); finite `beta = {BETA}` and `dt = {DT}` make almost all eligible rows candidates\n- Actual final-tick argmin candidates: {} ({contested_percent:.3}% of all rows); two-pass segmented atomic argmin uses lexicographic `(race_bits, rule_id, entity_id)` tie-break (`rule_id = 0`)\n- Final measured-tick fired counter: {}\n\n## Steady-state measurements\n\nMedian of {} ticks after {} warmup ticks. Per-kernel values use pass timestamp pairs where supported. Total timing method: {total_method}.\n\n| Kernel | Median ms/tick |\n|---|---:|\n{}| **Total** | **{:.4}** |\n\n- Throughput: **{:.3} million rows/sec**\n- Linear extrapolation to 26M rows: **{:.4} ms/tick** ({:.3} ticks/sec). This extrapolation is reported even when full scale ran; it is an `f32` kernel-skeleton measurement and is not production-`f64` evidence (nor hardware evidence when the adapter is software).\n\n## Verdict\n\n{verdict}\n", result.adapter.name, result.adapter.backend, result.adapter.device_type, result.adapter.driver, result.adapter.driver_info, if result.adapter.timestamps { "supported" } else { "unsupported" }, if result.adapter.shader_f64 { "supported" } else { "unsupported" }, result.rows, result.groups, result.warmup_ticks, result.measured_ticks, result.contested_candidates_last_tick, result.fired_last_tick, result.measured_ticks, result.warmup_ticks, kernel_table, result.total_ms, result.rows_per_second / 1_000_000.0, extrapolated, 1000.0 / extrapolated);
    std::fs::write(path, markdown)
}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
    use super::*;

    #[test]
    fn gpu_philox_matches_four_copied_cpu_known_answers() {
        let vectors = [
            (KatInput { seed_lo: 0, seed_hi: 0, tick: 0, rule_id: 0, entity_id: 0, draw_idx: 0 }, [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8]),
            (KatInput { seed_lo: u32::MAX, seed_hi: u32::MAX, tick: u32::MAX, rule_id: u32::MAX, entity_id: u32::MAX, draw_idx: u32::MAX }, [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd]),
            (KatInput { seed_lo: 0xa409_3822, seed_hi: 0x299f_31d0, tick: 0x243f_6a88, rule_id: 0x85a3_08d3, entity_id: 0x1319_8a2e, draw_idx: 0x0370_7344 }, [0xd16c_fe09, 0x94fd_cceb, 0x5001_e420, 0x2412_6ea1]),
            (KatInput { seed_lo: 0x89ab_cdef, seed_hi: 0x0123_4567, tick: 17, rule_id: 23, entity_id: 42, draw_idx: 5 }, [0x53b5_bef1, 0x964c_53ca, 0x38fa_3e88, 0x3e9c_0772]),
        ];
        let inputs: Vec<_> = vectors.iter().map(|(input, _)| *input).collect();
        let actual = pollster::block_on(gpu_philox(&inputs)).unwrap();
        let expected: Vec<_> = vectors.iter().map(|(_, expected)| *expected).collect();
        assert_eq!(actual, expected);
        for (input, expected) in vectors { assert_eq!(cpu_philox(u64::from(input.seed_hi) << 32 | u64::from(input.seed_lo), input.tick, input.rule_id, input.entity_id, input.draw_idx), expected); }
    }

    #[test]
    fn ten_thousand_row_gpu_smoke_matches_cpu_candidate_flags_and_count() {
        const ROWS: u32 = 10_000;
        const GROUPS: u32 = 500;
        let (mut gpu, _) = pollster::block_on(GpuTick::new(ROWS, GROUPS)).unwrap();
        gpu.run_one(0);
        let actual = gpu.read_candidates();
        let group_size = ROWS.div_ceil(GROUPS);
        let infectious = group_size / 5;
        let expected: Vec<u32> = (0..ROWS).map(|i| {
            let health = if i % group_size < infectious { 1 } else { 0 };
            let lane = cpu_philox(SEED, 0, 0, i, 0)[0];
            let u = (lane as f32 + 0.5) * (1.0 / 4_294_967_296.0);
            let lambda = BETA * infectious as f32 / group_size as f32;
            let race = -u.ln() / lambda;
            u32::from(health == 0 && race < DT)
        }).collect();
        assert_eq!(actual, expected, "GPU and scalar CPU candidate flags differ");
        assert_eq!(actual.iter().sum::<u32>(), expected.iter().sum::<u32>());
        let mut expected_winners = vec![u32::MAX; GROUPS as usize];
        let mut best_keys = vec![(u32::MAX, u32::MAX); GROUPS as usize];
        let mut contested_candidates = 0_u32;
        let mut expected_fired = 0_u32;
        for (entity, candidate) in expected.iter().copied().enumerate() {
            if candidate == 0 { continue; }
            if entity % 10 == 5 {
                contested_candidates += 1;
                let entity = entity as u32;
                let employer = (entity / group_size).min(GROUPS - 1) as usize;
                let lane = cpu_philox(SEED, 0, 0, entity, 0)[0];
                let u = (lane as f32 + 0.5) * (1.0 / 4_294_967_296.0);
                let lambda = BETA * infectious as f32 / group_size as f32;
                let race_bits = (-u.ln() / lambda).to_bits();
                let key = (race_bits, entity);
                if key < best_keys[employer] {
                    best_keys[employer] = key;
                    expected_winners[employer] = entity;
                }
            } else {
                expected_fired += 1;
            }
        }
        assert!(contested_candidates > ROWS * 9 / 100, "argmin did not exercise about 10% of all rows");
        assert_eq!(gpu.read_winners(), expected_winners, "GPU segmented lexicographic winners differ from scalar CPU");
        expected_fired += expected_winners.iter().filter(|winner| **winner != u32::MAX).count() as u32;
        assert_eq!(gpu.read_fired(), expected_fired, "GPU write did not suppress contested losers exactly");
    }
}
