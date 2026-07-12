//! Deterministic prior sampling for prior-predictive sweeps.
//!
//! Parameter draws use a namespace disjoint from transition and population
//! randomness: `rule_id = u32::MAX`, `tick = draw_index`, `entity_id` is the
//! parameter declaration index, and `draw_idx` is internal to the sampler.
//! Uniform uses counter 0. Normal uses Box--Muller with counters 0 and 1, and
//! LogNormal exponentiates that same Normal draw. The mapping is frozen by the
//! tests in `tests/prior.rs`; extending a sweep cannot alter an earlier draw.

use std::error::Error;
use std::fmt;

use sembla_ir::{ParamType, ParamValue, Prior, PriorFamily, ValidatedModel};

use crate::eval::{ParamEnv, ParamOverride};
use crate::rng::uniform_f64;

/// Reserved `rule_id` for parameter/prior draws.
pub const PRIOR_DRAW_RULE_ID: u32 = u32::MAX;

/// A deterministic prior-sampling failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorError(String);

impl PriorError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for PriorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for PriorError {}

/// Samples one real-valued prior at the reserved parameter coordinates.
///
/// Normal draws use the cosine branch of Box--Muller:
/// `z = sqrt(-2 ln(u0)) * cos(2 pi u1)`. This exact choice and lane ordering
/// are part of the deterministic sweep contract.
pub fn sample_prior(
    prior: &Prior,
    seed: u64,
    draw_index: u32,
    parameter_index: u32,
) -> Result<f64, PriorError> {
    if prior.args.len() != 2 || prior.args.iter().any(|value| !value.is_finite()) {
        return Err(PriorError::new("prior requires two finite arguments"));
    }
    let first = prior.args[0];
    let second = prior.args[1];
    let value = match prior.family {
        PriorFamily::Uniform => {
            if first >= second {
                return Err(PriorError::new("Uniform prior requires lo < hi"));
            }
            let uniform = coordinate_uniform(seed, draw_index, parameter_index, 0);
            first + (second - first) * uniform
        }
        PriorFamily::Normal | PriorFamily::LogNormal => {
            let u0 = coordinate_uniform(seed, draw_index, parameter_index, 0);
            let u1 = coordinate_uniform(seed, draw_index, parameter_index, 1);
            let standard = (-2.0 * u0.ln()).sqrt() * (2.0 * std::f64::consts::PI * u1).cos();
            let normal = first + second * standard;
            if prior.family == PriorFamily::LogNormal {
                normal.exp()
            } else {
                normal
            }
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(PriorError::new("prior draw produced a non-finite value"))
    }
}

/// Resolves one sweep draw in parameter declaration order.
///
/// Pinned values win over priors. Prior-less, unpinned declarations retain
/// their IR defaults. All values are returned as the ordinary per-run
/// [`ParamEnv`], so a sweep introduces no new simulation semantics.
pub fn sample_parameters_for_draw(
    model: &ValidatedModel,
    seed: u64,
    draw_index: u32,
    pinned: &[ParamOverride],
) -> Result<ParamEnv, PriorError> {
    // Validate unknown, duplicate, and mistyped pins before sampling anything.
    ParamEnv::resolve(model, pinned).map_err(|error| PriorError::new(error.to_string()))?;
    let mut values = Vec::with_capacity(model.model().params.len());
    for (parameter_index, declaration) in model.model().params.iter().enumerate() {
        let value = if let Some(pin) = pinned.iter().find(|pin| pin.name == declaration.name) {
            pin.value.clone()
        } else if let Some(prior) = &declaration.prior {
            if declaration.ty != ParamType::Real {
                return Err(PriorError::new(format!(
                    "integer parameter '{}' cannot have a prior",
                    declaration.name
                )));
            }
            ParamValue::Real {
                value: sample_prior(
                    prior,
                    seed,
                    draw_index,
                    u32::try_from(parameter_index)
                        .map_err(|_| PriorError::new("parameter declaration index exceeds u32"))?,
                )?,
            }
        } else {
            declaration.default.clone()
        };
        values.push(ParamOverride::new(&declaration.name, value));
    }
    ParamEnv::resolve(model, &values).map_err(|error| PriorError::new(error.to_string()))
}

fn coordinate_uniform(seed: u64, draw_index: u32, parameter_index: u32, counter: u32) -> f64 {
    uniform_f64(
        seed,
        draw_index,
        PRIOR_DRAW_RULE_ID,
        parameter_index,
        counter,
    )
}
