# SteelUI Engineering Audit — Patch Reconciliation

**Original audit date:** 2026-11-23  
**Updated:** 2026-11-23, after Batch 1 + Batch 2 patches  
**Scope:** Current on-disk working tree, including uncommitted local changes. No commits were made.

This file previously recorded the baseline audit. The patches applied since then resolved most findings, but several items remain unpatched or only partially patched. The current status is recorded below.

---

## Automated checks after patches

All commands were run from the repository root.

| Command | Result |
|---|---|
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed with one Cargo manifest warning (`unused field homepage` at `Cargo.toml:15`) |
| `cargo test --workspace` | Passed: 12 SteelUI unit tests, 0 failed |
| `cargo run --release -p steelui_dogfood` | Passed |

CI formatting and lint gates are now green despite the remaining manifest warning, because that warning does not fail the Clippy command.

---

## Status summary

### Resolved findings

- **C1** UTF-8 cursor/selection safety — fixed with `snap_char_boundary()` and regression coverage (`crates/steelui/src/widgets/mod.rs:38`, `:1582`, `:1593`, `:1735`, `:2261`).
- **C2** Global dirty queue — replaced with a per-arena `DirtyQueue` (`crates/steelui/src/widgets/mod.rs:32`) and isolation test.
- **C3** Zero-size surface-loss redraw loop — fixed with `needs_reconfigure` and zero-size short-circuit (`crates/steelui/src/window/mod.rs:132`, `:145`, `:159`).
- **H1/H2/H4** Reactive reentrancy, panic safety, and stale effect state — fixed with released borrows, `EffectRunGuard`, validation, both dirty collections cleared, and cycle diagnostic (`crates/steelui/src/reactive/mod.rs:136`, `:179`, `:189`, `:235`).
- **H5/H6** Clip stack and scissor bounds — fixed with `clip_stack` and interval intersection (`crates/steelui/src/renderer/mod.rs:240`, `:291`, `:1285`).
- **H7** Focused TextInput idle redraw — fixed with blink timer + `next_redraw_at()` (`crates/steelui/src/widgets/mod.rs:1393`, `:1497`, `:1509`).
- **H9** Glyph atlas overflow — deferred to frame boundary with clean GPU texture recreation (`crates/steelui/src/renderer/mod.rs:453`, `:693`).
- **H10/L1** Shader portability/color space — fixed in `crates/steelui/shaders/glass.wgsl`.
- **H11** CI gate failure — formatting and Clippy now pass.
- **M1-M5** Remount state reset, multiple dropdowns, dropdown clamping, poison-safe mutex, checked capability fallbacks — fixed.
- **M7-M10** Per-frame accessibility clones, prefix remeasurement, dropdown reshaping, and glyph vector allocation — removed or replaced with cached/char-offset/scratch paths.
- **M12** Lazy glass/blur targets — fixed (`crates/steelui/src/renderer/mod.rs:821`, `:910`, `:1109`).
- **M21-M26** Badge repo URL, nonexistent README commands, stale docs/changelog/roadmap, full Apache license, `Cargo.lock` policy, and font discovery — fixed. `Cargo.lock` is no longer ignored; it remains untracked only because no commit was made.

### Still open or partially patched

| ID | Severity | Status |
|---|---:|---|
| **H3** | High | Partial |
| **H8 / M13** | High | Partial |
| **H12** | High | Partial |
| **M6** | Medium | Open |
| **M11** | Medium | Partial |
| **M14 / M20** | Medium | Open |
| **M15** | Medium | Partial |
| **M16** | Medium | Open |
| **M17 / M18** | Medium | Open |
| **M19** | Medium | Open |
| **L2** | Low | Partial |
| **L4** | Low | Open |
| **L5** | Low | Open |

---

## Remaining unpatched findings

## H3. High (partial) — Signal handles still leak storage and can dangle after `dispose()`

`Signal<T>` is still `Copy` with no owner/`Drop` guard (`crates/steelui/src/reactive/mod.rs:112-130`). `dispose()` removes the signal slot and subscriber map entry, but does not detach that signal ID from effect dependency lists (`crates/steelui/src/reactive/mod.rs:150-156`). `try_with()` now avoids a raw panic, but `with()` still panics with `"Signal accessed after disposal"` for stale handles.

Remaining work:
- add a non-`Copy` owner/guard with `Drop`, or refcounted slots with generation checks;
- detach disposed signal IDs from effect dependencies;
- define a documented non-panicking access policy for stale handles.

## H8 / M13. High (partial) — Texture release API exists, but lifetimes are not wired

`GpuContext::release_texture()` recycles slots (`crates/steelui/src/renderer/mod.rs:765-771`), and `free_textures` allows slot reuse, but there are no call sites outside the renderer itself. Uploaded textures are still retained in `textures` (`crates/steelui/src/renderer/mod.rs:755-762`) with no reference counting, LRU eviction, or tie-in to image widget unmounting.

Remaining work:
- give `TextureHandle` a lifetime-aware owner or refcount;
- call `release_texture()` when image widgets are unmounted or replaced;
- optionally add bounded LRU eviction.

## H12. High (partial) — Surface/DPI handling improved, device-loss rebuild still missing

Zero-size surface handling, `needs_reconfigure`, checked capability fallbacks, and DPI-triggered reconfiguration are in place (`crates/steelui/src/window/mod.rs:145`, `:159`, `:399`, `:553`). However, there is still no `wgpu::Device::add_lost_closure` / device-lost state machine or `GpuContext` rebuild path. Only surface loss is handled (`crates/steelui/src/window/mod.rs:235`).

Remaining work:
- register a device-lost callback;
- pause rendering and report a recoverable diagnostic;
- rebuild `GpuContext`, render targets, pipelines, and bind groups, or exit gracefully with a clear error.

## M6. Medium (open) — State/paint stages of the 3-stage invalidation pipeline are still unused

`DirtyFlags` still defines `state`, `layout`, and `paint` (`crates/steelui/src/widgets/mod.rs:26-30`), but the render drain only reacts to `flags.layout` (`crates/steelui/src/widgets/mod.rs:1902-1910`). `flags.state` and `flags.paint` do not drive separate state re-evaluation or paint-only caching.

Remaining work:
- split the drain into explicit state/layout/paint phases;
- avoid layout work for paint-only invalidations;
- document which bindings emit which flag.

## M11. Medium (partial) — Tick still scans all scroll/tab IDs instead of active sets

`tick()` no longer scans the entire arena, but it still iterates every `scroll_ids` and `tab_ids` entry each frame (`crates/steelui/src/widgets/mod.rs:1475-1495`). Caret blink is correctly timer-driven, but spring/settled widgets remain in the scanned lists.

Remaining work:
- maintain active-animation sets or remove settled IDs from the scan lists;
- stop calling `set_target()` for tabs whose signal has not changed.

## M14 / M20. Medium (open) — Reactive runtime remains thread-local and globally shared

The reactive runtime is still a thread-local `RefCell<ReactiveRuntime>` (`crates/steelui/src/reactive/mod.rs:85-87`). Per-arena dirty queues fixed cross-window widget routing, but independent applications/test cases on one UI thread still share signal/effect storage.

Remaining work:
- document the single-UI-thread requirement, or
- introduce scoped runtimes with explicit ownership and isolate signal/effect storage per app/window.

## M15. Medium (partial) — Reactive failure-mode tests are improved but incomplete

Three tests now cover basic tracking, nested reads, and effect panic recovery (`crates/steelui/src/reactive/mod.rs:294-357`), and a cycle limit diagnostic exists (`crates/steelui/src/reactive/mod.rs:260`). There are still no explicit tests for disposal during dirty flush, subscriber cleanup after signal disposal, or cycle detection behavior.

## M16. Medium (open) — Glass rendering still interrupts batching

`DrawBatch::Glass` still ends the normal batch and performs a full-target copy plus three blur passes (`crates/steelui/src/renderer/mod.rs:1099-1153`). Lazy targets fixed allocation cost, not pass count. Glass-heavy UIs will still see more GPU state changes than the changelog's "minimal GPU state changes" claim implies.

## M17 / M18. Medium (open) — Public API still leaks implementation dependencies

`WindowDelegate` still exposes `&mut TextEngine`, `&mut GpuContext`, and `&winit::keyboard::Key` (`crates/steelui/src/window/mod.rs:79-105`). `GpuContext` is publicly re-exported (`crates/steelui/src/lib.rs:27`), and all internal modules are `pub mod` with broad re-exports (`crates/steelui/src/lib.rs:1-37`).

Remaining work:
- introduce SteelUI-owned key/event, text, and GPU-surface boundary types;
- make implementation modules `pub(crate)` where possible;
- keep wgpu/winit/accesskit/cosmic-text types internal or behind controlled escape hatches.

## M19. Medium (open) — AccessKit tree is built but never delivered

`build_tree_update()` still constructs a `TreeUpdate` (`crates/steelui/src/accessibility/mod.rs:42`), but no widget/window code calls it, no `accesskit::Adapter` exists, and there is no `update_if_active()` or semantic action routing. The per-frame builder allocation was removed, but functional accessibility remains unimplemented.

## L2. Low (partial) — One manifest warning remains

`readme` was removed from `[workspace.package]` and `homepage.workspace` from `crates/steelui/Cargo.toml`, but `Cargo.toml:15` still defines `homepage = "https://github.com/ShreyasT123/steelui"`, which Cargo reports as unused. Remove it if the warning-free manifest is desired.

## L4. Low (open) — No explicit MSRV verification

README advertises Rust 1.85+, and the workspace uses edition 2024, but CI uses unpinned stable with no MSRV job (`.github/workflows/ci.yml:23-26`).

## L5. Low (open) — No shader validation or headless GPU test

The CI workflow runs fmt, Clippy, and tests only (`.github/workflows/ci.yml:37-44`). There is still no WGSL validation or headless GPU pass that would catch shader portability regressions.

---

## Updated action plan

1. **Close the remaining High items:** signal disposal ownership (H3), texture lifetime wiring (H8/M13), and device-loss rebuild (H12).
2. **Finish the invalidation model:** implement state/paint phases (M6) and active-animation sets (M11).
3. **Document or isolate the reactive runtime:** thread-local requirement or scoped runtimes (M14/M20), plus the missing reactive tests (M15).
4. **Decide on API boundary work:** SteelUI-owned wrappers and module visibility (M17/M18) are a larger refactor but are required by the design's dependency-isolation goal.
5. **Wire AccessKit for real:** adapter creation, `update_if_active()`, and semantic action routing (M19).
6. **Final cleanup:** remove the unused workspace `homepage` (L2), add MSRV verification or update the badge (L4), and add shader validation coverage (L5).

---

## Items checked and not treated as defects

- `queue.write_texture` 256-byte row alignment is not required for this use.
- The `Filtering` bind-group layout accepts nearest samplers.
- `WindowContext` field drop order is correct: `surface` is dropped before `window`.
- The README quick example API largely exists; remaining doc work was about commands/claims and is now patched.
- Linux GTK dependency gap was not proven from the lockfile.

---

## Closing assessment

The applied patches addressed all Critical findings and most High/Medium correctness issues. The remaining work is narrower but still meaningful: resource lifetime wiring, device-loss recovery, full invalidation staging, API-boundary isolation, functional accessibility, and verification tooling. The project is now much closer to a hardened prototype, but these open items should be resolved before calling it release-ready.
