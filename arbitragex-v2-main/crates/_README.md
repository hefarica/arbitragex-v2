# Rust Crates (`crates/`)

This directory contains the Rust workspace crates for the ArbitrageX V2
system. Each crate is a compilable unit with a focused responsibility.

## Workspace Structure

```
crates/
  arbitragex-core/       # Shared types, traits, and domain models
  arbitragex-engine/     # Opportunity discovery engine
  arbitragex-executor/   # Transaction submission and monitoring
  arbitragex-risk/       # Risk engine and circuit breakers
  arbitragex-api/        # HTTP API server (Axum)
  arbitragex-ws/         # WebSocket server for real-time feeds
  arbitragex-common/     # Utilities, logging, error types
```

## Getting Started

```bash
# Build entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Run a specific crate
cargo run -p arbitragex-api

# Lint
cargo clippy --workspace --all-targets
```

## Dependencies

- Rust 1.75+ (edition 2021)
- Shared deps: `tokio`, `serde`, `tracing`, `thiserror`, `async-trait`

## Crate Interdependency

```
core ← [engine, executor, risk, api, ws, common]
common ← [all crates except core]
engine, executor, risk → api, ws
```

## Conventions

- Crate names use `arbitragex-` prefix.
- Public API is documented with rustdoc.
- Errors use `thiserror` and propagate via `crate::Result<T>`.