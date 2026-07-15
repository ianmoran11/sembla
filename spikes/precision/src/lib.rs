//! Precision spike: stable workload/oracle plus portable precision kernels.
//!
//! PRD 0002 adds reusable `f32` and double-single WGSL dispatch paths while the
//! sizing/probe API remains shared by later precision strategies.

pub mod benchmark;
pub mod cuda;
pub mod f64_mirror;
pub mod fp64;
pub mod gpu;
pub mod native_f64;
pub mod oracle;
pub mod results;
pub mod timing;
pub mod workload;

use std::{error::Error, fmt};

pub use workload::{DEFAULT_GROUPS, DEFAULT_ROWS};

/// Largest resident footprint the portable sizing heuristic will assume from a
/// `wgpu` adapter. `wgpu` exposes buffer limits, not a heap-budget query, so the
/// estimate is intentionally capped at 1 GiB and its use is recorded whenever
/// it causes a downscale.
pub const MAX_RESIDENT_BUDGET: u64 = 1024 * 1024 * 1024;
/// A software adapter should remain a functional probe, not allocate the 26M-row
/// benchmark workload.
pub const SOFTWARE_ROW_CAP: u32 = 200_000;

/// Stable worst-case footprint reserved for later precision strategies.
///
/// Per row: employer (4), health (4), weight (8), race (8), candidate (4), and
/// fired (4). Per group: `f64` sum (8) and winner entity (4).
pub const RESIDENT_BYTES_PER_ROW: u64 = 32;
pub const RESIDENT_BYTES_PER_GROUP: u64 = 12;
/// The largest single row/group column uses one `f64` value.
pub const LARGEST_ELEMENT_BYTES: u64 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SizingLimits {
    pub max_buffer_size: u64,
    pub max_storage_buffer_binding_size: u64,
    pub resident_budget: u64,
    pub software_adapter: bool,
}

impl SizingLimits {
    #[must_use]
    pub fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        let limits = adapter.limits();
        Self {
            max_buffer_size: limits.max_buffer_size,
            max_storage_buffer_binding_size: u64::from(limits.max_storage_buffer_binding_size),
            // There is no portable VRAM query. A max-buffer-derived aggregate
            // ceiling is conservative and deterministic across later PRDs.
            resident_budget: limits.max_buffer_size.min(MAX_RESIDENT_BUDGET),
            software_adapter: is_software(&adapter.get_info()),
        }
    }

    #[must_use]
    pub fn binding_limit(self) -> u64 {
        self.max_buffer_size
            .min(self.max_storage_buffer_binding_size)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafeSizing {
    pub requested_rows: u32,
    pub requested_groups: u32,
    pub rows: u32,
    pub groups: u32,
    pub estimated_resident_bytes: u64,
    pub downscale_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterProbe {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub driver_info: String,
    pub shader_f64: bool,
    pub sizing: SafeSizing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeError(String);

impl ProbeError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ProbeError {}

/// Selects the default high-performance adapter (falling back to a software
/// adapter), records its identity and `SHADER_F64`, and resolves a safe size.
pub async fn probe_default_adapter(
    requested_rows: u32,
    requested_groups: u32,
) -> Result<AdapterProbe, ProbeError> {
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
            .ok_or_else(|| ProbeError::new("wgpu found no default compute adapter"))?,
    };
    let info = adapter.get_info();
    let sizing = resolve_safe_sizing(
        requested_rows,
        requested_groups,
        SizingLimits::from_adapter(&adapter),
    )?;

    Ok(AdapterProbe {
        name: info.name,
        backend: format!("{:?}", info.backend),
        device_type: format!("{:?}", info.device_type),
        driver: info.driver,
        driver_info: info.driver_info,
        shader_f64: adapter.features().contains(wgpu::Features::SHADER_F64),
        sizing,
    })
}

/// Resolves `(N, G)` while preserving the requested row/group ratio as closely
/// as integer sizes allow.
///
/// Safety checks cover the largest single storage binding, a documented
/// aggregate resident-column estimate, and a functional cap for software
/// adapters. Every active constraint is included in `downscale_reason`.
pub fn resolve_safe_sizing(
    requested_rows: u32,
    requested_groups: u32,
    limits: SizingLimits,
) -> Result<SafeSizing, ProbeError> {
    if requested_rows == 0 || requested_groups == 0 {
        return Err(ProbeError::new(
            "requested rows and groups must both be non-zero",
        ));
    }
    if requested_groups > requested_rows {
        return Err(ProbeError::new(format!(
            "requested groups ({requested_groups}) cannot exceed rows ({requested_rows})"
        )));
    }

    let binding_limit = limits.binding_limit();
    let element_capacity = binding_limit / LARGEST_ELEMENT_BYTES;
    if element_capacity == 0 || limits.resident_budget < resident_bytes(1, 1) {
        return Err(ProbeError::new(
            "adapter limits cannot hold even the minimum one-row workload",
        ));
    }

    let row_binding_cap = element_capacity.min(u64::from(u32::MAX)) as u32;
    let group_binding_cap = row_binding_cap;
    let rows_from_group_cap = if requested_groups > group_binding_cap {
        ((u64::from(group_binding_cap) * u64::from(requested_rows)) / u64::from(requested_groups))
            .max(1) as u32
    } else {
        requested_rows
    };
    let software_cap = if limits.software_adapter {
        SOFTWARE_ROW_CAP
    } else {
        requested_rows
    };
    let upper = requested_rows
        .min(row_binding_cap)
        .min(rows_from_group_cap)
        .min(software_cap);

    let fits = |rows: u32| {
        let groups = scaled_groups(rows, requested_rows, requested_groups);
        rows <= row_binding_cap
            && groups <= group_binding_cap
            && resident_bytes(rows, groups) <= limits.resident_budget
    };
    if upper == 0 || !fits(1) {
        return Err(ProbeError::new(
            "adapter limits cannot hold the scaled workload",
        ));
    }

    // Binary search the largest ratio-preserving workload under the aggregate
    // resident budget. The predicate is monotone in N and scaled G.
    let mut low = 1_u32;
    let mut high = upper;
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if fits(middle) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let rows = low;
    let groups = scaled_groups(rows, requested_rows, requested_groups);
    let estimated_resident_bytes = resident_bytes(rows, groups);

    let downscale_reason = if rows == requested_rows && groups == requested_groups {
        None
    } else {
        let mut reasons = Vec::new();
        if requested_rows > row_binding_cap {
            reasons.push(format!(
                "largest 8-byte row buffer exceeds the {binding_limit}-byte storage binding limit"
            ));
        }
        if requested_groups > group_binding_cap {
            reasons.push(format!(
                "largest 8-byte group buffer exceeds the {binding_limit}-byte storage binding limit"
            ));
        }
        let requested_resident = resident_bytes(requested_rows, requested_groups);
        if requested_resident > limits.resident_budget {
            reasons.push(format!(
                "estimated resident columns require {requested_resident} bytes but the conservative budget is {} bytes",
                limits.resident_budget
            ));
        }
        if limits.software_adapter && requested_rows > SOFTWARE_ROW_CAP {
            reasons.push(format!(
                "software-adapter safety cap is {SOFTWARE_ROW_CAP} rows"
            ));
        }
        if reasons.is_empty() {
            reasons.push("integer ratio preservation required a smaller size".to_owned());
        }
        Some(format!(
            "requested ({requested_rows}, {requested_groups}) reduced to ({rows}, {groups}): {}",
            reasons.join("; ")
        ))
    };

    Ok(SafeSizing {
        requested_rows,
        requested_groups,
        rows,
        groups,
        estimated_resident_bytes,
        downscale_reason,
    })
}

#[inline]
fn scaled_groups(rows: u32, requested_rows: u32, requested_groups: u32) -> u32 {
    let numerator = u64::from(rows) * u64::from(requested_groups);
    let groups = numerator.div_ceil(u64::from(requested_rows)) as u32;
    groups.max(1).min(requested_groups).min(rows)
}

#[inline]
fn resident_bytes(rows: u32, groups: u32) -> u64 {
    u64::from(rows) * RESIDENT_BYTES_PER_ROW + u64::from(groups) * RESIDENT_BYTES_PER_GROUP
}

fn is_software(info: &wgpu::AdapterInfo) -> bool {
    let description =
        format!("{} {} {}", info.name, info.driver, info.driver_info).to_ascii_lowercase();
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
    .any(|needle| description.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workload::{Workload, WorkloadConfig};

    #[test]
    fn small_scale_sizing_and_workload_layout_are_covered_together() {
        let roomy = SizingLimits {
            max_buffer_size: 1024 * 1024 * 1024,
            max_storage_buffer_binding_size: 1024 * 1024 * 1024,
            resident_budget: 1024 * 1024 * 1024,
            software_adapter: false,
        };
        let sizing = resolve_safe_sizing(10_000, 500, roomy).unwrap();
        assert_eq!((sizing.rows, sizing.groups), (10_000, 500));
        assert!(sizing.downscale_reason.is_none());

        let workload =
            Workload::generate(WorkloadConfig::with_size(sizing.rows, sizing.groups)).unwrap();
        assert!(workload.employer.windows(2).all(|pair| pair[0] <= pair[1]));
        for group in 0..sizing.groups {
            assert!(workload.employer[workload.group_range(group)]
                .iter()
                .all(|employer| *employer == group));
        }
    }

    #[test]
    fn resident_budget_downscales_with_a_recorded_reason() {
        let constrained = SizingLimits {
            max_buffer_size: 1_000_000,
            max_storage_buffer_binding_size: 1_000_000,
            resident_budget: 160_000,
            software_adapter: false,
        };
        let sizing = resolve_safe_sizing(10_000, 500, constrained).unwrap();
        assert!(sizing.rows < 10_000);
        assert!(sizing.groups < 500);
        assert!(sizing.estimated_resident_bytes <= constrained.resident_budget);
        assert!(sizing
            .downscale_reason
            .as_deref()
            .unwrap()
            .contains("resident columns"));
    }
}
