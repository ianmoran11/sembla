# Sembla

Sembla is a simulation framework with a Lean frontend and a deterministic Rust runtime. See [DESIGN.md](DESIGN.md) for the architecture and project scope.

## Build and test

```sh
cargo build --workspace
cargo test --workspace
./scripts/check.sh
```

Print the CLI version with:

```sh
cargo run -p sembla-cli -- --version
```

## Workspace layout

- `crates/sembla-ir`: shared simulation IR types.
- `crates/sembla-runtime`: deterministic CPU interpreter.
- `crates/sembla-cli`: `sembla` command-line binary.
