use sembla_ir::{ParamType, ParamValue, ValidatedModel};

use crate::error::EvalError;

/// One named per-run parameter override.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamOverride {
    pub name: String,
    pub value: ParamValue,
}

impl ParamOverride {
    pub fn new(name: impl Into<String>, value: ParamValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

/// Parameters resolved once from IR defaults and per-run overrides.
///
/// Entries remain in declaration order. Parameter values are never written
/// back into the IR (`DESIGN.md` §4.1).
#[derive(Clone, Debug, PartialEq)]
pub struct ParamEnv {
    values: Vec<(String, ParamValue)>,
}

impl ParamEnv {
    /// Resolves all defaults with no per-run overrides.
    pub fn defaults(model: &ValidatedModel) -> Self {
        Self {
            values: model
                .model()
                .params
                .iter()
                .map(|param| (param.name.clone(), param.default.clone()))
                .collect(),
        }
    }

    /// Resolves defaults overlaid by validated, uniquely named overrides.
    pub fn resolve(model: &ValidatedModel, overrides: &[ParamOverride]) -> Result<Self, EvalError> {
        let mut env = Self::defaults(model);
        for (override_index, parameter_override) in overrides.iter().enumerate() {
            if overrides[..override_index]
                .iter()
                .any(|previous| previous.name == parameter_override.name)
            {
                return Err(EvalError::new(format!(
                    "duplicate override for parameter '{}'",
                    parameter_override.name
                )));
            }
            let declaration = model
                .model()
                .params
                .iter()
                .find(|param| param.name == parameter_override.name)
                .ok_or_else(|| {
                    EvalError::new(format!(
                        "override refers to unknown parameter '{}'",
                        parameter_override.name
                    ))
                })?;
            if !parameter_value_matches(declaration.ty, &parameter_override.value) {
                return Err(EvalError::new(format!(
                    "override for parameter '{}' does not match {:?}",
                    parameter_override.name, declaration.ty
                )));
            }
            if matches!(
                parameter_override.value,
                ParamValue::Real { value } if !value.is_finite()
            ) {
                return Err(EvalError::new(format!(
                    "override for parameter '{}' must be finite",
                    parameter_override.name
                )));
            }
            let entry = env
                .values
                .iter_mut()
                .find(|(name, _)| *name == parameter_override.name)
                .ok_or_else(|| EvalError::new("validated parameter declaration disappeared"))?;
            entry.1 = parameter_override.value.clone();
        }
        Ok(env)
    }

    /// Resolved values in parameter declaration order.
    pub fn values(&self) -> impl Iterator<Item = (&str, &ParamValue)> {
        self.values
            .iter()
            .map(|(name, value)| (name.as_str(), value))
    }

    pub(crate) fn get(&self, name: &str) -> Result<&ParamValue, EvalError> {
        self.values
            .iter()
            .find(|(entry_name, _)| entry_name == name)
            .map(|(_, value)| value)
            .ok_or_else(|| {
                EvalError::new(format!("parameter environment has no value for '{name}'"))
            })
    }
}

pub(crate) fn parameter_value_matches(parameter_type: ParamType, value: &ParamValue) -> bool {
    matches!(
        (parameter_type, value),
        (ParamType::Real, ParamValue::Real { .. }) | (ParamType::Int, ParamValue::Int { .. })
    )
}
