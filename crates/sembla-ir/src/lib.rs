//! Sembla's JSON intermediate representation and semantic validator.

mod error;
mod model;
mod validate;

pub use error::{ParseError, ValidationError};
pub use model::*;
pub use validate::{validate, ValidatedModel, ValidatedTransition};

/// The version of the Sembla IR crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Parses a Sembla JSON IR document.
pub fn parse_json(source: &str) -> Result<Model, ParseError> {
    serde_json::from_str(source).map_err(ParseError::new)
}

/// Serializes a model into the canonical compact JSON representation.
///
/// Canonical documents contain struct fields and declarations in their Rust
/// and source order, contain no insignificant whitespace, and end in one
/// newline. Because the IR uses ordered vectors rather than maps, repeated
/// parse/serialize cycles are byte-stable.
pub fn to_canonical_json(model: &Model) -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string(model)?;
    json.push('\n');
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_matches_package_version() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }
}
