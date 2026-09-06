//! Tick-level timing records and reconciliation.

use super::*;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PhaseDurations {
    pub(crate) execute_tick: Option<Duration>,
    pub(crate) kernels: Option<Duration>,
    pub(crate) readback_control: Option<Duration>,
    pub(crate) state_transfer: Option<Duration>,
    pub(crate) state_reconstruct: Option<Duration>,
    pub(crate) state_hash: Option<Duration>,
    pub(crate) observe_views: Option<Duration>,
    pub(crate) report: Option<Duration>,
    pub(crate) other: Option<Duration>,
}

impl PhaseDurations {
    fn zero_for_backend(backend: BackendSelection) -> Self {
        match backend {
            BackendSelection::Cpu => Self {
                execute_tick: Some(Duration::ZERO),
                state_hash: Some(Duration::ZERO),
                observe_views: Some(Duration::ZERO),
                report: Some(Duration::ZERO),
                other: Some(Duration::ZERO),
                ..Self::default()
            },
            BackendSelection::Cuda => Self {
                kernels: Some(Duration::ZERO),
                readback_control: Some(Duration::ZERO),
                state_transfer: Some(Duration::ZERO),
                state_reconstruct: Some(Duration::ZERO),
                state_hash: Some(Duration::ZERO),
                observe_views: Some(Duration::ZERO),
                report: Some(Duration::ZERO),
                other: Some(Duration::ZERO),
                ..Self::default()
            },
        }
    }

    fn attributed(&self) -> Duration {
        [
            self.execute_tick,
            self.kernels,
            self.readback_control,
            self.state_transfer,
            self.state_reconstruct,
            self.state_hash,
            self.observe_views,
            self.report,
        ]
        .into_iter()
        .flatten()
        .sum()
    }

    fn total(&self) -> Duration {
        self.attributed() + self.other.unwrap_or_default()
    }

    fn add_assign(&mut self, other: Self) {
        fn add(slot: &mut Option<Duration>, value: Option<Duration>) {
            if let Some(value) = value {
                *slot = Some(slot.unwrap_or_default() + value);
            }
        }
        add(&mut self.execute_tick, other.execute_tick);
        add(&mut self.kernels, other.kernels);
        add(&mut self.readback_control, other.readback_control);
        add(&mut self.state_transfer, other.state_transfer);
        add(&mut self.state_reconstruct, other.state_reconstruct);
        add(&mut self.state_hash, other.state_hash);
        add(&mut self.observe_views, other.observe_views);
        add(&mut self.report, other.report);
        add(&mut self.other, other.other);
    }

    fn milliseconds(self) -> TimingPhases {
        TimingPhases {
            execute_tick: self.execute_tick.map(duration_ms),
            kernels: self.kernels.map(duration_ms),
            readback_control: self.readback_control.map(duration_ms),
            state_transfer: self.state_transfer.map(duration_ms),
            state_reconstruct: self.state_reconstruct.map(duration_ms),
            state_hash: self.state_hash.map(duration_ms),
            observe_views: self.observe_views.map(duration_ms),
            report: self.report.map(duration_ms),
            other: self.other.map(duration_ms),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TickTiming {
    tick: u32,
    wall_time: Duration,
    phases: PhaseDurations,
}

pub(crate) fn finish_tick_timing(
    tick: u32,
    wall_time: Duration,
    mut phases: PhaseDurations,
) -> Result<TickTiming, String> {
    phases.other = Some(wall_time.checked_sub(phases.attributed()).ok_or_else(|| {
        format!("tick {tick}: attributed timing exceeds measured tick wall time")
    })?);
    Ok(TickTiming {
        tick,
        wall_time,
        phases,
    })
}

#[derive(Serialize)]
pub(crate) struct TimingSession {
    backend: &'static str,
    scale: usize,
    ticks: u32,
    seed: u64,
    repository_commit: String,
    binary_sha256: String,
}

#[derive(Serialize)]
pub(crate) struct TimerMetadata {
    clock: &'static str,
    resolution: &'static str,
    reported_unit: &'static str,
}

#[derive(Clone, Copy, Serialize)]
pub(crate) struct TimingPhases {
    #[serde(skip_serializing_if = "Option::is_none")]
    execute_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kernels: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readback_control: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_transfer: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_reconstruct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_hash: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observe_views: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    other: Option<f64>,
}

#[derive(Serialize)]
pub(crate) struct TimingTick {
    tick: u32,
    wall_time_ms: f64,
    phases_ms: TimingPhases,
    phase_sum_ms: f64,
    within_tolerance: bool,
}

#[derive(Serialize)]
pub(crate) struct TimingTotals {
    wall_time_ms: f64,
    phases_ms: TimingPhases,
    phase_sum_ms: f64,
}

#[derive(Serialize)]
pub(crate) struct TimingSelfCheck {
    tolerance_ms: f64,
    all_ticks_reconciled: bool,
    other_non_negative: bool,
}

#[derive(Serialize)]
pub(crate) struct TimingDocument {
    schema: &'static str,
    session: TimingSession,
    kernel_sync_inserted: bool,
    timer: TimerMetadata,
    ticks: Vec<TimingTick>,
    totals: TimingTotals,
    self_check: TimingSelfCheck,
}

impl TimingDocument {
    pub(crate) fn new(
        backend: BackendSelection,
        scale: usize,
        ticks: u32,
        seed: u64,
        kernel_sync_inserted: bool,
        tick_timings: Vec<TickTiming>,
    ) -> Result<Self, String> {
        const TOLERANCE_MS: f64 = 0.001;
        let mut total_wall = Duration::ZERO;
        let mut total_phases = PhaseDurations::zero_for_backend(backend);
        let mut rows = Vec::with_capacity(tick_timings.len());
        for timing in tick_timings {
            let phase_sum = timing.phases.total();
            let within_tolerance =
                (duration_ms(phase_sum) - duration_ms(timing.wall_time)).abs() <= TOLERANCE_MS;
            total_wall += timing.wall_time;
            total_phases.add_assign(timing.phases);
            rows.push(TimingTick {
                tick: timing.tick,
                wall_time_ms: duration_ms(timing.wall_time),
                phases_ms: timing.phases.milliseconds(),
                phase_sum_ms: duration_ms(phase_sum),
                within_tolerance,
            });
        }
        let total_phase_time = total_phases.total();
        let all_ticks_reconciled = rows.iter().all(|row| row.within_tolerance)
            && (duration_ms(total_phase_time) - duration_ms(total_wall)).abs() <= TOLERANCE_MS;
        if !all_ticks_reconciled {
            return Err("timing phases did not reconcile with measured tick wall time".to_owned());
        }
        Ok(Self {
            schema: "sembla-execution-timing-v1",
            session: TimingSession {
                backend: match backend {
                    BackendSelection::Cpu => "cpu",
                    BackendSelection::Cuda => "cuda",
                },
                scale,
                ticks,
                seed,
                repository_commit: repository_commit()?,
                binary_sha256: current_binary_sha256()?,
            },
            kernel_sync_inserted,
            timer: TimerMetadata {
                clock: "std::time::Instant",
                resolution: "nanoseconds",
                reported_unit: "milliseconds",
            },
            ticks: rows,
            totals: TimingTotals {
                wall_time_ms: duration_ms(total_wall),
                phases_ms: total_phases.milliseconds(),
                phase_sum_ms: duration_ms(total_phase_time),
            },
            self_check: TimingSelfCheck {
                tolerance_ms: TOLERANCE_MS,
                all_ticks_reconciled,
                other_non_negative: true,
            },
        })
    }
}

pub(crate) fn write_timing_document(path: &str, timing: &TimingDocument) -> Result<(), String> {
    let mut json = serde_json::to_string_pretty(timing)
        .map_err(|error| format!("could not serialize timing JSON: {error}"))?;
    json.push('\n');
    write_atomic(path, json.as_bytes())
}
