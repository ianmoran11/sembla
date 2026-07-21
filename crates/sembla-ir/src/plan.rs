use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::identity::{
    is_reserved_rule_word, is_slug, mailbox_identity, occurrence_of_leaf, rule_word,
    transition_identity,
};
use crate::{validate, Model, ValidatedModel, ValidationError, Wire};

pub const EXECUTABLE_PLAN_SCHEMA: &str = "sembla.executable-plan/v1";
pub const STABLE_IDENTITY_SCHEME: &str = "sembla.identity/stable-v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutablePlanV1 {
    pub schema_version: String,
    pub identity_scheme: String,
    pub origin: PlanOrigin,
    pub model: Model,
    pub identity: IdentityMapV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_provenance: Option<LinkedProvenanceV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanOrigin {
    Linked,
    DirectStable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityMapV1 {
    pub model_id: String,
    pub enabled_features: Vec<String>,
    pub scheduler_domains: Vec<SchedulerDomainV1>,
    pub leaves: Vec<LeafIdentityV1>,
    pub transitions: Vec<TransitionIdentityV1>,
    pub mailboxes: Vec<MailboxIdentityV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchedulerDomainV1 {
    pub id: String,
    pub algorithm: String,
    pub leaves: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeafIdentityV1 {
    pub r#box: String,
    pub occurrence: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionIdentityV1 {
    pub r#box: String,
    pub name: String,
    pub identity: String,
    pub rule_word: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MailboxIdentityV1 {
    pub identity: String,
    pub source_box: String,
    pub source_port: String,
    pub target_box: String,
    pub target_port: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkedProvenanceV1 {
    pub source_hash: HashRecordV1,
    pub linker: LinkerDescriptorV1,
    pub source_map: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HashRecordV1 {
    pub algorithm: String,
    pub domain: String,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkerDescriptorV1 {
    pub semantics: String,
    pub source_schema: String,
    pub plan_schema: String,
    pub identity_scheme: String,
    pub canonical_encoding: String,
    pub source_map_schema: String,
}

/// A validated envelope plus stable rule words aligned with legacy dense IDs.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedPlan {
    model: ValidatedModel,
    identity: IdentityMapV1,
    words_by_dense_rule_id: Vec<u32>,
}

impl ValidatedPlan {
    pub fn model(&self) -> &ValidatedModel {
        &self.model
    }

    pub fn identity(&self) -> &IdentityMapV1 {
        &self.identity
    }

    pub fn words_by_dense_rule_id(&self) -> &[u32] {
        &self.words_by_dense_rule_id
    }
}

/// Checks the reserved namespaces and pairwise distinctness of rule words.
/// This is public so collision defense can be tested without finding a real
/// SHA-256 prefix collision.
pub fn validate_rule_words(words: &[u32]) -> Result<(), ValidationError> {
    let mut first_index = HashMap::new();
    for (index, &word) in words.iter().enumerate() {
        if is_reserved_rule_word(word) {
            return Err(plan_error(
                format!("$.identity.transitions[{index}].rule_word"),
                format!("rule word {word} is reserved"),
            ));
        }
        if let Some(previous) = first_index.insert(word, index) {
            return Err(plan_error(
                format!("$.identity.transitions[{index}].rule_word"),
                format!("duplicate rule word {word}; first used at transition index {previous}"),
            ));
        }
    }
    Ok(())
}

/// Validates a V1 executable-plan envelope without changing legacy validation.
pub fn validate_plan(plan: &ExecutablePlanV1) -> Result<ValidatedPlan, ValidationError> {
    if plan.schema_version != EXECUTABLE_PLAN_SCHEMA {
        return Err(plan_error(
            "$.schema_version",
            format!(
                "unsupported schema_version '{}'; expected '{EXECUTABLE_PLAN_SCHEMA}'",
                plan.schema_version
            ),
        ));
    }
    if plan.identity_scheme != STABLE_IDENTITY_SCHEME {
        return Err(plan_error(
            "$.identity_scheme",
            format!(
                "unsupported identity_scheme '{}'; expected '{STABLE_IDENTITY_SCHEME}'",
                plan.identity_scheme
            ),
        ));
    }

    if let Some(feature) = plan.identity.enabled_features.first() {
        return Err(plan_error(
            "$.identity.enabled_features[0]",
            format!("unsupported enabled feature '{feature}'; V1 requires an empty list"),
        ));
    }

    let validated_model = validate(plan.model.clone()).map_err(|error| {
        let suffix = error.path.strip_prefix('$').unwrap_or(&error.path);
        plan_error(format!("$.model{suffix}"), error.message)
    })?;

    let expected_model_id = format!("model:{}", plan.model.name);
    if plan.identity.model_id != expected_model_id {
        return Err(plan_error(
            "$.identity.model_id",
            format!(
                "model_id '{}' does not match '{expected_model_id}'",
                plan.identity.model_id
            ),
        ));
    }
    if !is_slug(&plan.model.name) {
        return Err(plan_error(
            "$.model.name",
            format!("model name '{}' is not a slug", plan.model.name),
        ));
    }

    validate_leaves(plan)?;
    validate_transitions(plan)?;
    validate_scheduler_domains(plan)?;
    validate_mailboxes(plan)?;
    validate_provenance(plan)?;
    validate_canonical_array_order(plan)?;

    let mut words_by_dense_rule_id = Vec::with_capacity(validated_model.transitions().len());
    for transition in validated_model.transitions() {
        let model_box = &plan.model.boxes[transition.box_index];
        let declaration = &model_box.transitions[transition.transition_index];
        let entry = plan
            .identity
            .transitions
            .iter()
            .find(|entry| entry.r#box == model_box.name && entry.name == declaration.name)
            .expect("transition bijection was validated");
        words_by_dense_rule_id.push(entry.rule_word);
    }

    Ok(ValidatedPlan {
        model: validated_model,
        identity: plan.identity.clone(),
        words_by_dense_rule_id,
    })
}

fn validate_leaves(plan: &ExecutablePlanV1) -> Result<(), ValidationError> {
    for (index, model_box) in plan.model.boxes.iter().enumerate() {
        if !model_box.name.split('/').all(is_slug) {
            return Err(plan_error(
                format!("$.model.boxes[{index}].name"),
                format!(
                    "box name '{}' is not a slash-separated slug path",
                    model_box.name
                ),
            ));
        }
    }

    if !is_sorted_by(&plan.identity.leaves, |leaf| leaf.r#box.as_str()) {
        return Err(plan_error(
            "$.identity.leaves",
            "leaves must be sorted by box",
        ));
    }

    let mut expected_boxes: Vec<&str> = plan
        .model
        .boxes
        .iter()
        .map(|model_box| model_box.name.as_str())
        .collect();
    expected_boxes.sort_unstable();
    let actual_boxes: Vec<&str> = plan
        .identity
        .leaves
        .iter()
        .map(|leaf| leaf.r#box.as_str())
        .collect();
    if actual_boxes != expected_boxes {
        return Err(plan_error(
            "$.identity.leaves",
            format!(
                "leaf map must be a bijection with model boxes; expected {expected_boxes:?}, found {actual_boxes:?}"
            ),
        ));
    }

    for (index, leaf) in plan.identity.leaves.iter().enumerate() {
        let expected = occurrence_of_leaf(&leaf.r#box);
        if leaf.occurrence != expected {
            return Err(plan_error(
                format!("$.identity.leaves[{index}].occurrence"),
                format!(
                    "occurrence '{}' does not match '{expected}'",
                    leaf.occurrence
                ),
            ));
        }
    }
    Ok(())
}

fn validate_transitions(plan: &ExecutablePlanV1) -> Result<(), ValidationError> {
    if !is_sorted_by(&plan.identity.transitions, |entry| entry.identity.as_str()) {
        return Err(plan_error(
            "$.identity.transitions",
            "transitions must be sorted by identity",
        ));
    }

    let mut expected_pairs = Vec::new();
    for model_box in &plan.model.boxes {
        for transition in &model_box.transitions {
            expected_pairs.push((model_box.name.as_str(), transition.name.as_str()));
        }
    }
    expected_pairs.sort_unstable();
    let mut actual_pairs: Vec<(&str, &str)> = plan
        .identity
        .transitions
        .iter()
        .map(|entry| (entry.r#box.as_str(), entry.name.as_str()))
        .collect();
    actual_pairs.sort_unstable();
    if actual_pairs != expected_pairs {
        return Err(plan_error(
            "$.identity.transitions",
            format!(
                "transition map must be a bijection with model transitions; expected {expected_pairs:?}, found {actual_pairs:?}"
            ),
        ));
    }

    let mut words = Vec::with_capacity(plan.identity.transitions.len());
    for (index, entry) in plan.identity.transitions.iter().enumerate() {
        if !is_slug(&entry.name) {
            return Err(plan_error(
                format!("$.identity.transitions[{index}].name"),
                format!("transition name '{}' is not a slug", entry.name),
            ));
        }
        let expected_identity = transition_identity(&occurrence_of_leaf(&entry.r#box), &entry.name);
        if entry.identity != expected_identity {
            return Err(plan_error(
                format!("$.identity.transitions[{index}].identity"),
                format!(
                    "transition identity '{}' does not match '{expected_identity}'",
                    entry.identity
                ),
            ));
        }
        let expected_word = rule_word(&entry.identity);
        if entry.rule_word != expected_word {
            return Err(plan_error(
                format!("$.identity.transitions[{index}].rule_word"),
                format!(
                    "rule_word {} does not match recomputed value {expected_word} for '{}'",
                    entry.rule_word, entry.identity
                ),
            ));
        }
        words.push(entry.rule_word);
    }
    validate_rule_words(&words)
}

fn validate_scheduler_domains(plan: &ExecutablePlanV1) -> Result<(), ValidationError> {
    if plan.identity.scheduler_domains.len() != 1 {
        return Err(plan_error(
            "$.identity.scheduler_domains",
            format!(
                "V1 requires exactly one scheduler domain, found {}",
                plan.identity.scheduler_domains.len()
            ),
        ));
    }
    let domain = &plan.identity.scheduler_domains[0];
    if domain.id != "domain:global" {
        return Err(plan_error(
            "$.identity.scheduler_domains[0].id",
            format!(
                "scheduler domain id '{}' must be 'domain:global'",
                domain.id
            ),
        ));
    }
    if domain.algorithm != "tau_leap" {
        return Err(plan_error(
            "$.identity.scheduler_domains[0].algorithm",
            format!(
                "scheduler algorithm '{}' must be 'tau_leap'",
                domain.algorithm
            ),
        ));
    }
    let mut expected: Vec<&str> = plan
        .model
        .boxes
        .iter()
        .map(|model_box| model_box.name.as_str())
        .collect();
    expected.sort_unstable();
    let actual: Vec<&str> = domain.leaves.iter().map(String::as_str).collect();
    if actual != expected {
        return Err(plan_error(
            "$.identity.scheduler_domains[0].leaves",
            format!(
                "scheduler leaves must equal sorted model boxes {expected:?}, found {actual:?}"
            ),
        ));
    }
    Ok(())
}

fn validate_mailboxes(plan: &ExecutablePlanV1) -> Result<(), ValidationError> {
    if !is_sorted_by(&plan.identity.mailboxes, |entry| entry.identity.as_str()) {
        return Err(plan_error(
            "$.identity.mailboxes",
            "mailboxes must be sorted by identity",
        ));
    }

    let mut expected_endpoints: Vec<(&str, &str, &str, &str)> = plan
        .model
        .wires
        .iter()
        .map(|wire| {
            (
                wire.from.r#box.as_str(),
                wire.from.port.as_str(),
                wire.to.r#box.as_str(),
                wire.to.port.as_str(),
            )
        })
        .collect();
    expected_endpoints.sort_unstable();
    let mut actual_endpoints: Vec<(&str, &str, &str, &str)> = plan
        .identity
        .mailboxes
        .iter()
        .map(|entry| {
            (
                entry.source_box.as_str(),
                entry.source_port.as_str(),
                entry.target_box.as_str(),
                entry.target_port.as_str(),
            )
        })
        .collect();
    actual_endpoints.sort_unstable();
    if actual_endpoints != expected_endpoints {
        return Err(plan_error(
            "$.identity.mailboxes",
            format!(
                "mailbox map must be a bijection with model wires; expected {expected_endpoints:?}, found {actual_endpoints:?}"
            ),
        ));
    }

    for (box_index, model_box) in plan.model.boxes.iter().enumerate() {
        for (port_index, port) in model_box.inputs.iter().enumerate() {
            if !is_slug(&port.name) {
                return Err(plan_error(
                    format!("$.model.boxes[{box_index}].inputs[{port_index}].name"),
                    format!("input port name '{}' is not a slug", port.name),
                ));
            }
        }
        for (port_index, port) in model_box.outputs.iter().enumerate() {
            if !is_slug(&port.name) {
                return Err(plan_error(
                    format!("$.model.boxes[{box_index}].outputs[{port_index}].name"),
                    format!("output port name '{}' is not a slug", port.name),
                ));
            }
        }
    }

    for (index, entry) in plan.identity.mailboxes.iter().enumerate() {
        let wire = plan
            .model
            .wires
            .iter()
            .find(|wire| mailbox_matches_wire(entry, wire))
            .expect("mailbox bijection was validated");
        match plan.origin {
            PlanOrigin::DirectStable => {
                let wire_occurrence = direct_wire_occurrence(wire);
                let expected = mailbox_identity(
                    &wire_occurrence,
                    &wire.from.r#box,
                    &wire.from.port,
                    &wire.to.r#box,
                    &wire.to.port,
                );
                if entry.identity != expected {
                    return Err(plan_error(
                        format!("$.identity.mailboxes[{index}].identity"),
                        format!(
                            "mailbox identity '{}' does not match '{expected}'",
                            entry.identity
                        ),
                    ));
                }
            }
            PlanOrigin::Linked => validate_linked_mailbox_identity(entry, index)?,
        }
    }
    Ok(())
}

fn validate_linked_mailbox_identity(
    entry: &MailboxIdentityV1,
    index: usize,
) -> Result<(), ValidationError> {
    let path = format!("$.identity.mailboxes[{index}].identity");
    let rest = entry.identity.strip_prefix("mbox:").ok_or_else(|| {
        plan_error(
            &path,
            format!("invalid mailbox identity '{}'", entry.identity),
        )
    })?;
    let (wire_occurrence, _) = rest.split_once('|').ok_or_else(|| {
        plan_error(
            &path,
            format!("invalid mailbox identity '{}'", entry.identity),
        )
    })?;
    let expected = mailbox_identity(
        wire_occurrence,
        &entry.source_box,
        &entry.source_port,
        &entry.target_box,
        &entry.target_port,
    );
    if entry.identity != expected || !valid_wire_occurrence(wire_occurrence) {
        return Err(plan_error(
            path,
            format!("invalid linked mailbox identity '{}'", entry.identity),
        ));
    }
    Ok(())
}

fn validate_provenance(plan: &ExecutablePlanV1) -> Result<(), ValidationError> {
    match (plan.origin, &plan.linked_provenance) {
        (PlanOrigin::Linked, None) => {
            return Err(plan_error(
                "$.linked_provenance",
                "linked origin requires linked_provenance",
            ));
        }
        (PlanOrigin::DirectStable, Some(_)) => {
            return Err(plan_error(
                "$.linked_provenance",
                "direct_stable origin forbids linked_provenance",
            ));
        }
        (PlanOrigin::DirectStable, None) => return Ok(()),
        (PlanOrigin::Linked, Some(_)) => {}
    }

    let provenance = plan
        .linked_provenance
        .as_ref()
        .expect("linked provenance exists");
    if provenance.source_hash.algorithm != "sha256" {
        return Err(plan_error(
            "$.linked_provenance.source_hash.algorithm",
            format!(
                "source hash algorithm '{}' must be 'sha256'",
                provenance.source_hash.algorithm
            ),
        ));
    }
    if provenance.source_hash.domain != "sembla.source-artifact/v1" {
        return Err(plan_error(
            "$.linked_provenance.source_hash.domain",
            format!(
                "source hash domain '{}' must be 'sembla.source-artifact/v1'",
                provenance.source_hash.domain
            ),
        ));
    }
    if !is_lowercase_sha256_hex(&provenance.source_hash.digest) {
        return Err(plan_error(
            "$.linked_provenance.source_hash.digest",
            "source hash digest must be exactly 64 lowercase hexadecimal characters",
        ));
    }

    let descriptor = &provenance.linker;
    validate_descriptor("semantics", &descriptor.semantics, "sembla.linker/v1")?;
    validate_descriptor(
        "source_schema",
        &descriptor.source_schema,
        "sembla.composition-source/v1",
    )?;
    validate_descriptor(
        "plan_schema",
        &descriptor.plan_schema,
        EXECUTABLE_PLAN_SCHEMA,
    )?;
    validate_descriptor(
        "identity_scheme",
        &descriptor.identity_scheme,
        STABLE_IDENTITY_SCHEME,
    )?;
    validate_descriptor(
        "canonical_encoding",
        &descriptor.canonical_encoding,
        "sembla.canonical-json/v1",
    )?;
    validate_descriptor(
        "source_map_schema",
        &descriptor.source_map_schema,
        "sembla.source-map/v1",
    )
}

fn validate_descriptor(field: &str, actual: &str, expected: &str) -> Result<(), ValidationError> {
    if actual == expected {
        Ok(())
    } else {
        Err(plan_error(
            format!("$.linked_provenance.linker.{field}"),
            format!("linker {field} '{actual}' must be '{expected}'"),
        ))
    }
}

fn validate_canonical_array_order(plan: &ExecutablePlanV1) -> Result<(), ValidationError> {
    if !is_sorted_by(&plan.model.boxes, |model_box| model_box.name.as_str()) {
        return Err(plan_error(
            "$.model.boxes",
            "model boxes must be sorted by name",
        ));
    }
    for (index, model_box) in plan.model.boxes.iter().enumerate() {
        let base = format!("$.model.boxes[{index}]");
        if !is_sorted_by(&model_box.transitions, |item| item.name.as_str()) {
            return Err(plan_error(
                format!("{base}.transitions"),
                "transitions must be sorted by name",
            ));
        }
        if !is_sorted_by(&model_box.views, |item| item.name.as_str()) {
            return Err(plan_error(
                format!("{base}.views"),
                "views must be sorted by name",
            ));
        }
        if !is_sorted_by(&model_box.inputs, |item| item.name.as_str()) {
            return Err(plan_error(
                format!("{base}.inputs"),
                "inputs must be sorted by name",
            ));
        }
        if !is_sorted_by(&model_box.outputs, |item| item.name.as_str()) {
            return Err(plan_error(
                format!("{base}.outputs"),
                "outputs must be sorted by name",
            ));
        }
        if !is_sorted_by(&model_box.tables, |item| item.name.as_str()) {
            return Err(plan_error(
                format!("{base}.tables"),
                "tables must be sorted by name",
            ));
        }
    }
    if !is_sorted_by(&plan.model.params, |item| item.name.as_str()) {
        return Err(plan_error(
            "$.model.params",
            "model params must be sorted by name",
        ));
    }
    if !is_sorted_by(&plan.model.summaries, |item| item.name.as_str()) {
        return Err(plan_error(
            "$.model.summaries",
            "model summaries must be sorted by name",
        ));
    }

    let wire_identities: Vec<String> = plan
        .model
        .wires
        .iter()
        .map(|wire| match plan.origin {
            PlanOrigin::DirectStable => mailbox_identity(
                &direct_wire_occurrence(wire),
                &wire.from.r#box,
                &wire.from.port,
                &wire.to.r#box,
                &wire.to.port,
            ),
            PlanOrigin::Linked => plan
                .identity
                .mailboxes
                .iter()
                .find(|entry| mailbox_matches_wire(entry, wire))
                .expect("mailbox bijection was validated")
                .identity
                .clone(),
        })
        .collect();
    if !wire_identities.windows(2).all(|pair| pair[0] <= pair[1]) {
        return Err(plan_error(
            "$.model.wires",
            "model wires must be sorted by mailbox identity",
        ));
    }
    Ok(())
}

fn direct_wire_occurrence(wire: &Wire) -> String {
    format!("occ:#wire:to_{}_{}", wire.to.r#box, wire.to.port)
}

fn mailbox_matches_wire(entry: &MailboxIdentityV1, wire: &Wire) -> bool {
    entry.source_box == wire.from.r#box
        && entry.source_port == wire.from.port
        && entry.target_box == wire.to.r#box
        && entry.target_port == wire.to.port
}

fn is_lowercase_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_wire_occurrence(value: &str) -> bool {
    let Some(value) = value.strip_prefix("occ:") else {
        return false;
    };
    let Some((owner, wire)) = value.split_once("#wire:") else {
        return false;
    };
    (owner.is_empty() || owner.split('/').all(is_slug)) && is_slug(wire)
}

fn is_sorted_by<T>(items: &[T], key: impl Fn(&T) -> &str) -> bool {
    items.windows(2).all(|pair| key(&pair[0]) <= key(&pair[1]))
}

fn plan_error(path: impl Into<String>, message: impl Into<String>) -> ValidationError {
    ValidationError::new(path, message)
}
