# Sembla

Sembla is a simulation framework with a Lean frontend and a deterministic Rust runtime. See [DESIGN.md](DESIGN.md) for the architecture and project scope.

## Build and test

```sh
cargo build --workspace
cargo test --workspace
./scripts/check.sh
```

Print the CLI version or validate an IR document with:

```sh
cargo run -p sembla-cli -- --version
cargo run -p sembla-cli -- validate examples/two_state.json
```

## IR JSON conventions

IR enums use snake-case `kind` tags, declarations retain source order, and the canonical serializer emits compact JSON with one trailing newline. Validation assigns zero-based `rule_id` values to transitions in declaration order across all boxes; IDs are derived metadata on `ValidatedModel` and are not serialized into the wire format. Parameter expressions always retain symbolic names rather than inlining per-run values.

## Workspace layout

- `crates/sembla-ir`: shared simulation IR types.
- `crates/sembla-runtime`: deterministic CPU interpreter.
- `crates/sembla-cli`: `sembla` command-line binary.
