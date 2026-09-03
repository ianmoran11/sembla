use std::error::Error;
use std::fmt;

use crate::state::StateError;

/// A deterministic evaluation failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalError {
    message: String,
}

impl EvalError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
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

/// A deterministic CPU tick execution failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TickError {
    UnsupportedBoxCount {
        found: usize,
    },
    Evaluation(String),
    State(String),
    InvalidRuntimeType {
        context: String,
        found: String,
    },
    EntityIdOverflow {
        rule_id: u32,
        row: usize,
    },
    IncompatibleClaimOrdering {
        table: String,
        row: u32,
    },
    DoubleWrite {
        box_name: Box<str>,
        table: Box<str>,
        attr: Box<str>,
        row: usize,
        first_rule_id: u32,
        first_transition: Box<str>,
        second_rule_id: u32,
        second_transition: Box<str>,
    },
}

impl fmt::Display for TickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedBoxCount { found } => write!(
                formatter,
                "tick executor requires exactly one box, found {found}"
            ),
            Self::Evaluation(message) => {
                write!(formatter, "expression evaluation failed: {message}")
            }
            Self::State(message) => write!(formatter, "state operation failed: {message}"),
            Self::InvalidRuntimeType { context, found } => {
                write!(formatter, "{context} evaluated to {found}")
            }
            Self::EntityIdOverflow { rule_id, row } => write!(
                formatter,
                "rule {rule_id} row {row} cannot be represented as a u32 entity ID"
            ),
            Self::IncompatibleClaimOrdering { table, row } => write!(
                formatter,
                "resource '{table}' row {row} has incompatible claim ordering modes or key types"
            ),
            Self::DoubleWrite {
                box_name,
                table,
                attr,
                row,
                first_rule_id,
                first_transition,
                second_rule_id,
                second_transition,
            } => write!(
                formatter,
                "double write to {box_name}.{table}.{attr}[{row}] by transition '{first_transition}' (rule {first_rule_id}) and transition '{second_transition}' (rule {second_rule_id})"
            ),
        }
    }
}

impl Error for TickError {}

impl From<EvalError> for TickError {
    fn from(error: EvalError) -> Self {
        Self::Evaluation(error.to_string())
    }
}

impl From<StateError> for TickError {
    fn from(error: StateError) -> Self {
        Self::State(error.to_string())
    }
}
