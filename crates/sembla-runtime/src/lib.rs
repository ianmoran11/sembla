//! Deterministic runtime for Sembla simulations.

pub mod eval;
pub mod executor;
pub mod population;
pub mod prior;
pub mod rng;
pub mod state;

/// The version of the Sembla runtime crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_matches_package_version() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }
}
