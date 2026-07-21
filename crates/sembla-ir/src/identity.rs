use sha2::{Digest, Sha256};

pub const RULE_WORD_DOMAIN: &str = "sembla.rule-word/v1";
pub const RESERVED_RULE_WORDS: [u32; 2] = [u32::MAX - 1, u32::MAX];

/// Computes SHA-256(domain ++ 0x00 ++ payload).
pub fn domain_digest(domain: &str, payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(payload);
    hasher.finalize().into()
}

/// Derives the big-endian u32 in the first four bytes of the rule-word digest.
pub fn rule_word(identity: &str) -> u32 {
    let digest = domain_digest(RULE_WORD_DOMAIN, identity.as_bytes());
    u32::from_be_bytes(digest[..4].try_into().expect("SHA-256 has four bytes"))
}

pub fn is_reserved_rule_word(word: u32) -> bool {
    RESERVED_RULE_WORDS.contains(&word)
}

pub fn occurrence_of_leaf(box_name: &str) -> String {
    format!("occ:{box_name}")
}

pub fn transition_identity(occurrence: &str, name: &str) -> String {
    format!("{occurrence}#{name}")
}

pub fn mailbox_identity(
    wire_occurrence: &str,
    source_box: &str,
    source_port: &str,
    target_box: &str,
    target_port: &str,
) -> String {
    format!(
        "mbox:{wire_occurrence}|{}.port:{source_port}|{}.port:{target_port}",
        occurrence_of_leaf(source_box),
        occurrence_of_leaf(target_box)
    )
}

/// Returns whether `s` matches the frozen ASCII slug grammar.
pub fn is_slug(s: &str) -> bool {
    let mut bytes = s.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
