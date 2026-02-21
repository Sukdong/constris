# Repository Guidelines

## Project Structure & Module Organization
- `src/main.rs` contains the full game implementation (single-binary crate).
- `Cargo.toml` defines dependencies (`crossterm`, `rand`) and package metadata.
- `Cargo.lock` is committed for reproducible builds.
- `target/` is build output and should not be edited by hand.

## Build, Test, and Development Commands
- `cargo build --release` builds an optimized binary.
- `cargo run --release` runs the game in a release build.
- `cargo run` runs a debug build (slower but easier to debug).
- `cargo test` runs unit tests.
- `cargo fmt` formats Rust code.
- `cargo clippy` runs lint checks (fix warnings before PRs).

## Coding Style & Naming Conventions
- Follow standard Rust formatting; run `cargo fmt` before committing.
- Prefer explicit, descriptive names (`board`, `current`, `clear_lines`).
- Constants use `SCREAMING_SNAKE_CASE` and live near the top of `src/main.rs`.
- Keep functions short and focused; avoid large, nested blocks when possible.

## Testing Guidelines
- Unit tests live in `src/main.rs` under `#[cfg(test)]` until the code is split into modules.
- Name tests with behavior-oriented names (e.g., `clears_full_rows`, `rotates_with_wall_kick`).
- Run `cargo test` locally before opening a PR.

## Commit & Pull Request Guidelines
- Commit subjects are short, imperative, and sentence case (e.g., `Add README with game description` or `Fix line clear scoring`).
- Keep commits scoped and focused; avoid mixing refactors with gameplay changes.
- PRs should include:
  - A clear description of gameplay or UI changes.
  - Terminal screenshots or a short capture for visual changes.
  - Any relevant issue links.

## Security & Configuration Tips
- Requires Rust 1.70+ and a terminal that supports ANSI control sequences.
- When changing input handling or terminal modes, ensure raw mode is always restored on exit.
