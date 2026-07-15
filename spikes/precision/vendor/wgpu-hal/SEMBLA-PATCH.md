# Sembla strict-Metal patch

This directory vendors the published `wgpu-hal 0.21.1` crate from crates.io
(checksum `172e490a87295564f3fcc0f165798d87386f6231b04d4548bca458cbbfd63222`).
Its upstream Git provenance is recorded in `.cargo_vcs_info.json` as commit
`14a7698d16f0f5bcdf8cd6d515952441d4bd2585` from `gfx-rs/wgpu`.

The only functional change is in `src/metal/device.rs`: immediately after
creating `metal::CompileOptions`, the backend calls
`set_fast_math_enabled(false)`. PRD 0002 requires the WGSL double-single
transforms to compile without reassociation or implicit FMA contraction, but
wgpu 0.20 has no public API for this Metal option.

Keep this fork pinned at 0.21.1 while the precision spike uses wgpu 0.20. Remove
it when wgpu exposes an equivalent strict-math control or when the throwaway
spike is retired.
