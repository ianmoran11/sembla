use super::VERSION;

#[test]
fn version_matches_package_version() {
    assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
}
