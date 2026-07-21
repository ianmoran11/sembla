use serde::Serialize;

use crate::identity::domain_digest;
use crate::plan::{ExecutablePlanV1, HashRecordV1};

pub const PLAN_CORE_DOMAIN: &str = "sembla.plan-core/v1";
pub const PLAN_ENVELOPE_DOMAIN: &str = "sembla.plan-envelope/v1";

/// Serializes through `serde_json::Value` so object keys are recursively
/// sorted, then emits compact JSON without a trailing newline.
pub fn to_canonical_string<T: Serialize>(value: &T) -> Result<String, String> {
    let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    serde_json::to_string(&value).map_err(|error| error.to_string())
}

#[derive(Serialize)]
struct SemanticPlanPayload<'a> {
    identity: &'a crate::plan::IdentityMapV1,
    identity_scheme: &'a str,
    model: &'a crate::Model,
    schema_version: &'a str,
}

pub fn plan_semantic_hash(plan: &ExecutablePlanV1) -> Result<HashRecordV1, String> {
    let payload = SemanticPlanPayload {
        identity: &plan.identity,
        identity_scheme: &plan.identity_scheme,
        model: &plan.model,
        schema_version: &plan.schema_version,
    };
    hash_record(PLAN_CORE_DOMAIN, to_canonical_string(&payload)?.as_bytes())
}

pub fn plan_envelope_hash(plan: &ExecutablePlanV1) -> Result<HashRecordV1, String> {
    hash_record(PLAN_ENVELOPE_DOMAIN, to_canonical_string(plan)?.as_bytes())
}

fn hash_record(domain: &str, payload: &[u8]) -> Result<HashRecordV1, String> {
    let digest = domain_digest(domain, payload);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").map_err(|error| error.to_string())?;
    }
    Ok(HashRecordV1 {
        algorithm: "sha256".to_owned(),
        domain: domain.to_owned(),
        digest: hex,
    })
}

#[cfg(test)]
mod tests {
    use super::to_canonical_string;
    use serde::Serialize;

    #[derive(Serialize)]
    struct UnsortedFields {
        z_key: u8,
        a_key: u8,
    }

    #[test]
    fn serde_json_objects_are_btree_sorted() {
        let value = UnsortedFields { z_key: 1, a_key: 2 };
        assert_eq!(
            to_canonical_string(&value).unwrap(),
            r#"{"a_key":2,"z_key":1}"#
        );
    }
}
