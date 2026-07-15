//! Conservative fp64 throughput classification for native GPU results.
//!
//! wgpu exposes the adapter model but not NVIDIA's fp32:fp64 performance
//! attribute. CUDA supplies that attribute directly. The model lookup here is
//! therefore only a documented fallback; unknown models are conservatively
//! treated as rate-limited and can never be extrapolated as full-rate hardware.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fp64ThroughputClass {
    /// Datacenter-compute class, approximately one fp64 operation per two fp32.
    FullRate,
    /// Commodity or unknown class; must not be extrapolated as full-rate.
    RateLimited,
}

impl fmt::Display for Fp64ThroughputClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FullRate => formatter.write_str("full-rate"),
            Self::RateLimited => formatter.write_str("rate-limited"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fp64Throughput {
    pub device_name: String,
    /// fp32 throughput divided by fp64 throughput, rendered as `1:N`.
    pub fp32_to_fp64_ratio: Option<u32>,
    pub class: Fp64ThroughputClass,
    pub evidence: String,
}

impl Fp64Throughput {
    /// CUDA's runtime attribute is authoritative when available. Ratios at most
    /// 1:4 are conservatively considered full-rate; everything slower is
    /// rate-limited. A zero/missing attribute falls back to the model table.
    #[must_use]
    pub fn from_cuda_ratio(device_name: impl Into<String>, ratio: u32) -> Self {
        let device_name = device_name.into();
        if ratio == 0 {
            return Self::from_model_name(device_name);
        }
        let class = if ratio <= 4 {
            Fp64ThroughputClass::FullRate
        } else {
            Fp64ThroughputClass::RateLimited
        };
        Self {
            device_name,
            fp32_to_fp64_ratio: Some(ratio),
            class,
            evidence: "CUDA cudaDevAttrSingleToDoublePrecisionPerfRatio".to_owned(),
        }
    }

    /// Fallback lookup for the NVIDIA families in the precision-spike plan.
    /// Exact normalized tokens prevent the `A10` family from matching `A100`.
    #[must_use]
    pub fn from_model_name(device_name: impl Into<String>) -> Self {
        let device_name = device_name.into();
        let tokens: Vec<String> = device_name
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|token| !token.is_empty())
            .map(str::to_ascii_uppercase)
            .collect();
        let has = |needle: &str| tokens.iter().any(|token| token == needle);

        let (ratio, class, evidence) = if ["A100", "V100", "H100", "H200", "GH200"]
            .iter()
            .any(|model| has(model))
        {
            (
                Some(2),
                Fp64ThroughputClass::FullRate,
                "documented NVIDIA datacenter-compute model lookup",
            )
        } else if has("T4") {
            (
                Some(32),
                Fp64ThroughputClass::RateLimited,
                "documented NVIDIA T4 model lookup",
            )
        } else if ["L4", "A10", "A10G", "RTX", "GTX", "GEFORCE"]
            .iter()
            .any(|model| has(model))
        {
            (
                Some(64),
                Fp64ThroughputClass::RateLimited,
                "documented NVIDIA commodity-model lookup",
            )
        } else {
            (
                None,
                Fp64ThroughputClass::RateLimited,
                "conservative fallback for an unrecognized model",
            )
        };

        Self {
            device_name,
            fp32_to_fp64_ratio: ratio,
            class,
            evidence: evidence.to_owned(),
        }
    }

    #[must_use]
    pub fn permits_full_rate_extrapolation(&self) -> bool {
        self.class == Fp64ThroughputClass::FullRate
    }
}

impl fmt::Display for Fp64Throughput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ratio = self
            .fp32_to_fp64_ratio
            .map_or_else(|| "unknown".to_owned(), |ratio| format!("1:{ratio}"));
        write!(
            formatter,
            "device={}; fp64:fp32={}; class={}; evidence={}; full-rate-extrapolation={}",
            self.device_name,
            ratio,
            self.class,
            self.evidence,
            if self.permits_full_rate_extrapolation() {
                "allowed"
            } else {
                "refused"
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_lookup_distinguishes_full_rate_and_rate_limited_families() {
        for model in [
            "NVIDIA A100-SXM4-40GB",
            "Tesla V100 PCIe",
            "NVIDIA H100 80GB",
        ] {
            let profile = Fp64Throughput::from_model_name(model);
            assert_eq!(profile.class, Fp64ThroughputClass::FullRate);
            assert_eq!(profile.fp32_to_fp64_ratio, Some(2));
            assert!(profile.permits_full_rate_extrapolation());
        }
        for model in ["Tesla T4", "NVIDIA L4", "NVIDIA A10", "GeForce RTX 4090"] {
            let profile = Fp64Throughput::from_model_name(model);
            assert_eq!(profile.class, Fp64ThroughputClass::RateLimited);
            assert!(!profile.permits_full_rate_extrapolation());
        }
        assert_eq!(
            Fp64Throughput::from_model_name("NVIDIA A100").class,
            Fp64ThroughputClass::FullRate
        );
        assert_eq!(
            Fp64Throughput::from_model_name("NVIDIA A10").class,
            Fp64ThroughputClass::RateLimited
        );
    }

    #[test]
    fn cuda_ratio_is_authoritative_and_unknown_models_are_conservative() {
        let full = Fp64Throughput::from_cuda_ratio("future GPU", 2);
        assert_eq!(full.class, Fp64ThroughputClass::FullRate);
        assert!(full.permits_full_rate_extrapolation());

        let limited = Fp64Throughput::from_cuda_ratio("NVIDIA A100", 32);
        assert_eq!(limited.class, Fp64ThroughputClass::RateLimited);
        assert!(!limited.permits_full_rate_extrapolation());

        let unknown = Fp64Throughput::from_model_name("mystery accelerator");
        assert_eq!(unknown.fp32_to_fp64_ratio, None);
        assert_eq!(unknown.class, Fp64ThroughputClass::RateLimited);
        assert!(!unknown.permits_full_rate_extrapolation());
    }
}
