use std::error::Error;
use std::fmt;

use crate::state::StateError;

/// A deterministic evaluation failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalError {
    message: String,
}

impl EvalError {
    #[doc(hidden)]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for EvalError {}

impl From<StateError> for EvalError {
    fn from(error: StateError) -> Self {
        Self::new(error.to_string())
    }
}
