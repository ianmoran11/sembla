//! Sembla's JSON intermediate representation and semantic validator.

mod canonical_json;
mod error;
mod identity;
mod model;
mod plan;
mod validate;

pub use canonical_json::{
    plan_envelope_hash, plan_semantic_hash, to_canonical_string, PLAN_CORE_DOMAIN,
    PLAN_ENVELOPE_DOMAIN,
};
pub use error::{ParseError, ValidationError};
pub use identity::{
    domain_digest, is_reserved_rule_word, is_slug, mailbox_identity, occurrence_of_leaf, rule_word,
    transition_identity, RESERVED_RULE_WORDS, RULE_WORD_DOMAIN,
};
pub use model::*;
pub use plan::*;
pub use validate::{validate, ValidatedModel, ValidatedTransition};

/// The version of the Sembla IR crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Parses a Sembla JSON IR document.
pub fn parse_json(source: &str) -> Result<Model, ParseError> {
    serde_json::from_str(source).map_err(ParseError::new)
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParsedInput {
    LegacyModel(Model),
    Plan(ExecutablePlanV1),
}

/// Dispatches by the presence of the top-level `schema_version` key while
/// retaining the existing parser for unversioned legacy models.
pub fn parse_input(source: &str) -> Result<ParsedInput, String> {
    let value: serde_json::Value =
        serde_json::from_str(source).map_err(|error| ParseError::new(error).to_string())?;
    let has_schema_version = value
        .as_object()
        .is_some_and(|object| object.contains_key("schema_version"));
    if !has_schema_version {
        return parse_json(source)
            .map(ParsedInput::LegacyModel)
            .map_err(|error| error.to_string());
    }

    let schema = &value["schema_version"];
    if schema.as_str() != Some(EXECUTABLE_PLAN_SCHEMA) {
        let offending = schema
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| schema.to_string());
        return Err(format!(
            "$.schema_version: unsupported schema_version '{offending}'; supported versions: '{EXECUTABLE_PLAN_SCHEMA}'"
        ));
    }

    serde_json::from_str(source)
        .map(ParsedInput::Plan)
        .map_err(|error| ParseError::new(error).to_string())
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
