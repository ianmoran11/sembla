//! Per-executor CPU scheduling configuration.

const TILE_CACHE_BUDGET_BYTES: usize = 32_768;
const TILE_MIN_ROWS: usize = 64;
const TILE_MAX_ROWS: usize = 4_096;
const TILE_WORK_THRESHOLD: usize = 1_500_000;
const EVALUATOR_THREADS_ENV: &str = "SEMBLA_EVAL_THREADS";
const EVALUATOR_TILE_ROWS_ENV: &str = "SEMBLA_EVAL_TILE_ROWS";
const EVALUATOR_TILE_THRESHOLD_ENV: &str = "SEMBLA_EVAL_TILE_THRESHOLD";

/// Stable scheduling choices reused by a [`crate::CpuExecutor`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpuExecutionConfig {
    worker_count: usize,
    tile_rows: Option<usize>,
    tile_work_threshold: usize,
}

impl CpuExecutionConfig {
    pub fn new(worker_count: usize, tile_rows: Option<usize>, tile_work_threshold: usize) -> Self {
        Self {
            worker_count: worker_count.max(1),
            tile_rows: tile_rows.filter(|rows| *rows > 0),
            tile_work_threshold,
        }
    }

    /// Reads the optional `SEMBLA_EVAL_*` tuning variables once.
    pub fn from_env() -> Self {
        let worker_count = env_usize(EVALUATOR_THREADS_ENV)
            .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, usize::from));
        Self::new(
            worker_count,
            env_usize(EVALUATOR_TILE_ROWS_ENV),
            std::env::var(EVALUATOR_TILE_THRESHOLD_ENV)
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(TILE_WORK_THRESHOLD),
        )
    }

    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    pub fn tile_rows(&self) -> Option<usize> {
        self.tile_rows
    }

    pub fn tile_work_threshold(&self) -> usize {
        self.tile_work_threshold
    }
}

impl Default for CpuExecutionConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

fn env_usize(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
}

#[inline]
pub(crate) fn worker_count(config: &CpuExecutionConfig) -> usize {
    #[cfg(test)]
    if let Some(workers) = TEST_WORKERS.with(std::cell::Cell::get) {
        return workers;
    }
    config.worker_count
}

#[inline]
pub(crate) fn tile_rows(config: &CpuExecutionConfig, live_set_bytes_per_row: usize) -> usize {
    #[cfg(test)]
    if let Some(rows) = TEST_TILE_ROWS.with(std::cell::Cell::get) {
        return rows;
    }
    if let Some(rows) = config.tile_rows {
        return rows;
    }
    let raw = TILE_CACHE_BUDGET_BYTES / live_set_bytes_per_row.max(1);
    let clamped = raw.clamp(TILE_MIN_ROWS, TILE_MAX_ROWS);
    (clamped / 64 * 64).max(TILE_MIN_ROWS)
}

#[inline]
pub(crate) fn tiling_enabled(
    config: &CpuExecutionConfig,
    row_count: usize,
    tiled_node_count: usize,
) -> bool {
    #[cfg(test)]
    if let Some(rows) = TEST_TILE_THRESHOLD.with(std::cell::Cell::get) {
        return row_count >= rows;
    }
    row_count.saturating_mul(tiled_node_count) >= config.tile_work_threshold
}

#[cfg(test)]
thread_local! {
    static TEST_WORKERS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static TEST_TILE_ROWS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static TEST_TILE_THRESHOLD: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(crate) fn with_test_tick_tiles<R>(
    workers: usize,
    tile_rows: usize,
    threshold: usize,
    run: impl FnOnce() -> R,
) -> R {
    TEST_WORKERS.with(|worker_slot| {
        TEST_TILE_ROWS.with(|tile_slot| {
            TEST_TILE_THRESHOLD.with(|threshold_slot| {
                let previous_workers = worker_slot.replace(Some(workers.max(1)));
                let previous_tile = tile_slot.replace(Some(tile_rows.max(1)));
                let previous_threshold = threshold_slot.replace(Some(threshold));
                let result = run();
                threshold_slot.set(previous_threshold);
                tile_slot.set(previous_tile);
                worker_slot.set(previous_workers);
                result
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_configuration_is_normalized_and_reusable() {
        let config = CpuExecutionConfig::new(0, Some(0), 17);
        assert_eq!(config.worker_count(), 1);
        assert_eq!(config.tile_rows(), None);
        assert_eq!(config.tile_work_threshold(), 17);
        assert!(!tiling_enabled(&config, 4, 4));
        assert!(tiling_enabled(&config, 5, 4));
    }
}
