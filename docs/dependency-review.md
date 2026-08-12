# Dependency Review

Date: 2026-06-24

## Selected Core Slice

The implementation keeps only the server/runtime crates needed for the Rust MCP
bridge and pins every direct crate exactly:

- `axum = 0.8.9`
- `base64 = 0.22.1`
- `tokio = 1.52.3`
- `serde = 1.0.228`
- `serde_json = 1.0.150`
- `sha2 = 0.11.0`
- `thiserror = 2.0.18`
- `tracing = 0.1.44`
- `tracing-subscriber = 0.3.23`

Test-only direct crates:

- `tower-service = 0.3.3`; already present transitively through Axum, promoted
  to a direct dev-dependency so route tests can call the Axum service
  in-process without opening loopback sockets.

Deferred crates:

- SQLite/state: `rusqlite`
- CLI/config: `clap`, `toml`, `dirs`
- Tool schema generation: `schemars`
- HTTP middleware: `tower-http`

## Checks Run

- `python3 scripts/test_check_deps.py`: dependency-checker unit tests.
- `python3 scripts/check-deps.py --dry-run`: parse every crates.io
  package/version in `Cargo.lock` without network access.
- `cargo audit --no-fetch`: RustSec advisory scan for `Cargo.lock` using the
  local advisory database.
- `cargo fmt --check`
- `cargo check`
- `cargo test`
- `cargo check --locked`
- `cargo test --locked`

## Results

- `scripts/test_check_deps.py`: passed 2 tests.
- Current `Cargo.lock`: 74 crates.io packages parsed by the OSV checker dry
  run.
- Live OSV query: not rerun in this increment.
- RustSec advisories: `cargo audit --no-fetch` loaded 1138 local advisories,
  scanned 75 crate dependencies, and exited 0. It also warned that it could not
  open the crates.io index cache because that path is read-only in this
  sandbox.
- Rust verification: `cargo fmt --check`, `cargo check`, `cargo test`,
  `cargo check --locked`, and `cargo test --locked` passed.
- `local-sec`: not rerun in this increment because no `lsec` executable is
  currently on `PATH`, under `/tmp`, or in a local `local-sec` distribution
  directory such as `/path/to/local-sec/dist`.

## Not Used

- Socket package score: not applicable because Socket CLI package scoring lists
  npm, PyPI, NuGet, gem, Go, and Maven support, but not Cargo crates.
- `local-sec` preflight: not applicable because current `local-sec` guards npm,
  PyPI, uv, pipx, curl, and wget workflows, not Cargo installs.
