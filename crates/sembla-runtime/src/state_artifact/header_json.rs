use super::*;

pub(super) fn canonical_header(header: &Header) -> Result<String, StateArtifactError> {
    to_canonical_string(header).map_err(StateArtifactError::InvalidHeaderJson)
}

pub(super) fn parse_header(source: &str) -> Result<Header, StateArtifactError> {
    serde_json::from_str(source).map_err(|error| {
        // Preserve the established quote style in diagnostics while delegating
        // syntax, duplicate-field, and closed-schema checks to serde_json.
        StateArtifactError::InvalidHeaderJson(error.to_string().replace('`', "'"))
    })
}
