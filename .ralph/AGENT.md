# Ralph Agent Configuration

## Build Instructions

```bash
cargo build
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

## Test Instructions

```bash
cargo test
```

## Run Instructions

```bash
# CLI
cargo run -- --help
cargo run -- detect <file>
cargo run -- clean <file>

# GUI (requires gui feature)
cd gui && npm run build
cargo run -- gui
```

## Notes
- Rust project with `gui` feature flag for web interface
- Frontend is React/TypeScript in `gui/` directory, built with Vite
- Always run clippy and fmt check before committing
- Use conventional commit format (enforced by .githooks/commit-msg)
