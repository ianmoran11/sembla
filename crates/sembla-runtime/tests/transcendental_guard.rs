use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_METHODS: &[&str] = &[
    "ln", "ln_1p", "log", "log2", "log10", "exp", "exp2", "exp_m1", "powf", "sin", "cos", "tan",
    "asin", "acos", "atan", "atan2", "sin_cos", "sinh", "cosh", "tanh", "asinh", "acosh", "atanh",
    "cbrt", "hypot",
];

struct Exemption {
    path: &'static str,
    method: &'static str,
    source: &'static str,
    reason: &'static str,
}

// PRD 0001 is deliberately sampler-only. The audit found this pre-existing
// result-bearing simulation call after the PRD's authoring-time grep had said
// there were none. Pinning it would change racing-clock results and downstream
// fixtures, so it remains an exact, visible exemption rather than silently
// expanding this PRD beyond its non-goals.
const EXEMPTIONS: &[Exemption] = &[Exemption {
    path: "crates/sembla-runtime/src/rng.rs",
    method: "ln",
    source: "-uniform_f64(seed, tick, rule_id, entity_id, draw_idx).ln() / lambda",
    reason: "existing racing-clock transform; simulation transcendental pinning is out of PRD 0001 scope",
}];

#[derive(Debug)]
struct Finding {
    line: usize,
    method: &'static str,
    source: String,
}

fn scan(source: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (index, line) in source.lines().enumerate() {
        for &method in FORBIDDEN_METHODS {
            if line.contains(&format!(".{method}(")) || line.contains(&format!("f64::{method}(")) {
                findings.push(Finding {
                    line: index + 1,
                    method,
                    source: line.trim().to_owned(),
                });
            }
        }
    }
    findings
}

fn rust_sources(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
    {
        let path = entry.expect("failed to read source-directory entry").path();
        if path.is_dir() {
            rust_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn scanner_detects_every_forbidden_method() {
    for fixture in [
        FORBIDDEN_METHODS
            .iter()
            .map(|method| format!("value.{method}();"))
            .collect::<Vec<_>>()
            .join("\n"),
        FORBIDDEN_METHODS
            .iter()
            .map(|method| format!("f64::{method}(value);"))
            .collect::<Vec<_>>()
            .join("\n"),
    ] {
        let detected = scan(&fixture)
            .into_iter()
            .map(|finding| finding.method)
            .collect::<Vec<_>>();
        assert_eq!(detected, FORBIDDEN_METHODS);
    }
}

#[test]
fn result_sources_use_only_documented_platform_transcendentals() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist");
    let mut files = Vec::new();
    rust_sources(&workspace.join("crates/sembla-runtime/src"), &mut files);
    rust_sources(&workspace.join("crates/sembla-cli/src"), &mut files);
    files.sort();

    let mut exemption_hits = vec![0_usize; EXEMPTIONS.len()];
    let mut unexpected = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&workspace)
            .expect("source must be inside workspace")
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", file.display()));
        for finding in scan(&source) {
            let exemption = EXEMPTIONS.iter().position(|exemption| {
                exemption.path == relative
                    && exemption.method == finding.method
                    && exemption.source == finding.source
            });
            if let Some(index) = exemption {
                exemption_hits[index] += 1;
            } else {
                unexpected.push(format!(
                    "{relative}:{}: platform-backed .{} call: {}",
                    finding.line, finding.method, finding.source
                ));
            }
        }
    }

    assert!(
        unexpected.is_empty(),
        "result-bearing source introduced platform transcendental calls:\n{}",
        unexpected.join("\n")
    );
    for (exemption, hits) in EXEMPTIONS.iter().zip(exemption_hits) {
        assert_eq!(
            hits, 1,
            "documented exemption {} ({}) must match exactly once",
            exemption.path, exemption.reason
        );
    }
}
