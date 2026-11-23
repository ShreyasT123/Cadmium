# Contributing to SteelUI

Thank you for your interest in contributing to SteelUI!

## Development Philosophy

1. **GPU-Native & Lightweight:** Keep the core engine clean. Avoid adding dependencies that duplicate existing crates or add unnecessary bloat.
2. **Predictable Rust Ownership:** All public APIs must adhere to clear ownership semantics (handles, signals, arena identifiers). Never introduce circular `Rc<RefCell<...>>` patterns across widget trees.
3. **No Redundant Draws:** Features must respect idle-frame skipping and the 3-stage invalidation pipeline.

## Pull Request Guidelines

1. **Format & Clippy:** Ensure your changes pass formatting and linting:
   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```
2. **Tests:** Ensure all unit tests pass:
   ```bash
   cargo test --workspace
   ```
3. **Branching:** Create feature branches off `main` (e.g. `feat/bezier-paths` or `fix/swapchain-resize`).
4. **Commit Messages:** Use clear, descriptive commit messages following standard conventional commit guidelines.
