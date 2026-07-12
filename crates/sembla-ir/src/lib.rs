//! Intermediate representation types for Sembla.

/// The version of the Sembla IR crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_matches_package_version() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }
}
