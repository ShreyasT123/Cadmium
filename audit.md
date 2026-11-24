# SteelUI Engineering Audit

**Audit date:** 2026-11-23  
**Scope:** Current on-disk working tree, including uncommitted local changes. Git status at audit time showed modified manifests, shaders, and source files plus untracked `promt.md`.  
**Method:** Static source review, targeted dependency/source verification, and the requested automated Cargo checks. No source, manifest, shader, CI, or documentation files were modified. This `audit.md` is the only file written, in response to your follow-up instruction.

---

## Executive summary

SteelUI already has the broad module layout described in `requirements.md`/`design.md`, and the core Magnum/WGSL pipeline is structurally coherent. The highest-risk issues are not missing features but correctness and lifecycle defects:

1. **Text input can panic on ordinary Unicode editing** because byte offsets are clamped only to `String::len()`, not UTF-8 character boundaries.
2. **Reactive state is thread-local and reentrant-unsafe**: nested reads/writes panic through `RefCell`, and effects are not panic-safe.
3. **Dirty notifications are global**, so a multi-window application can apply one window's widget invalidation to another window's widget with the same numeric ID.
4. **Surface loss with a `0×0` window can create a redraw loop**.
5. **Nested clipping is broken** because `pop_clip()` emits `None` instead of restoring the prior scissor.
6. **CI currently cannot pass**: `cargo fmt --all --check` fails and the clippy warning set would fail the workflow's `-D warnings`.
7. **Several public claims are ahead of the implementation**: idle rendering, AccessKit integration, 3-stage invalidation, docs/examples, and repository metadata.

The repository is a promising prototype, but it is not yet production-hardened for Unicode text entry, multi-window isolation, resilient GPU lifecycle handling, or OSS consumption.

---

## Step 1 — Automated checks

All commands were run from the repository root against the current working tree.

| Command | Exit code | Result |
|---|---:|---|
| `cargo fmt --all --check` | `1` | 64 `Diff in ...` blocks. CI format step would fail at `.github/workflows/ci.yml:37-38`. |
| `cargo clippy --workspace --all-targets` | `0` | Completed, but emitted warning diagnostics plus per-target summaries. CI uses `-- -D warnings` at `.github/workflows/ci.yml:41`, so the CI clippy step would fail. |
| `cargo test --workspace` | `0` | 7 SteelUI unit tests passed; 0 failed. Other workspace targets reported 0 tests. |

Representative formatting diffs:

- `apps/dogfood/src/main.rs:1` — import ordering/wrapping.
- `apps/dogfood/src/main.rs:22` — line wrapping.
- `crates/cadmium/src/animation/mod.rs:64`.
- `crates/cadmium/src/reactive/mod.rs:2` and `:111`.
- `crates/cadmium/src/renderer/mod.rs:7`, `:142`, `:195`, `:211`, `:290`, `:1023`, `:1033`, `:1054`, `:1061`.
- `crates/cadmium/src/widgets/mod.rs` and `crates/cadmium/src/window/mod.rs` also appeared in the check output.

Representative clippy diagnostics:

- `Cargo.toml:16` — unused field `readme` in `workspace.package`.
- `crates/cadmium/Cargo.toml:8` — `package.homepage` is redundant with `package.repository`.
- `crates/cadmium/src/widgets/mod.rs:677` — field `placeholder` is never read.
- `crates/cadmium/src/renderer/mod.rs:1134` — function has too many arguments (8/7).
- `crates/cadmium/src/widgets/mod.rs:1965` — function has too many arguments (11/7).
- `apps/dogfood/src/main.rs:23` — unused variable `glass_enabled`.
- `examples/liquid_glass/src/main.rs:228` and `:254` — manual `RangeInclusive::contains` implementation.
- Many `collapsible_if` / `collapsible_if_let` suggestions throughout `renderer/mod.rs`, `widgets/mod.rs`, and `window/mod.rs`.

Test coverage is thin relative to the feature surface: the only `#[test]` sites are `crates/cadmium/src/core/mod.rs:156,165`, `crates/cadmium/src/layout/mod.rs:321`, `crates/cadmium/src/events/mod.rs:114`, `crates/cadmium/src/debug/mod.rs:61`, `crates/cadmium/src/animation/mod.rs:55`, and `crates/cadmium/src/reactive/mod.rs:241`. There are no tests for UTF-8 editing, reactive reentrancy/panic recovery, scissor clipping, surface loss, DPI changes, or shader/render-target behavior.

---

## Severity summary

| Severity | Count | Representative themes |
|---|---:|---|
| Critical | 3 | UTF-8 panics, cross-window dirty routing, surface-loss redraw loop |
| High | 12 | Reactive reentrancy/panic-safety/leaks, nested clipping, scissor OOB, idle redraw, GPU texture growth, shader portability, CI failure |
| Medium | 14 | allocations, dropdown placement, DPI resilience, API leakage, docs/metadata, accessibility non-integration |
| Low | 6 | lint cleanup, lockfile policy, font discovery polish, MSRV check, license wording details |

---

# 1. Correctness, panics, and edge cases

## C1. Critical — TextInput cursor/selection offsets are not UTF-8 character boundaries

`TextInput` stores `cursor` and `selection` as byte offsets but clamps them only with `val.len()`. Any external signal mutation, paste, or multi-byte character can leave an offset inside a multi-byte character, after which slicing/insertion/draining panics.

Evidence:

- Cursor clamp: `crates/cadmium/src/widgets/mod.rs:1451-1452`.
- Selection clamp and `String::drain`: `crates/cadmium/src/widgets/mod.rs:1454-1458`.
- Copy selection slice: `crates/cadmium/src/widgets/mod.rs:1473-1475`.
- Cut selection slice: `crates/cadmium/src/widgets/mod.rs:1483-1485`.
- Backspace drain: `crates/cadmium/src/widgets/mod.rs:1497-1504`.
- Delete drain: `crates/cadmium/src/widgets/mod.rs:1514-1520`.
- Arrow navigation slices: `crates/cadmium/src/widgets/mod.rs:1525-1543`.
- Paste and typed-character insertion: `crates/cadmium/src/widgets/mod.rs:1561-1574`.
- Paint-time selection slices: `crates/cadmium/src/widgets/mod.rs:2128-2134`.
- Paint-time caret slice: `crates/cadmium/src/widgets/mod.rs:2151-2153`.

Example failure mode: a text signal contains `"é"` (2 bytes). A prior cursor offset of `1` passes `*cursor <= val.len()` but `&val[..1]` panics because `1` is not a char boundary.

Minimal fix: add a helper that snaps any byte offset to the nearest valid `char_indices()` boundary and use it every time a signal value is read before slicing/inserting. Selection endpoints should be normalized through the same helper.

Related accuracy bug: `estimate_char_index()` uses a hard-coded `8.1` px per character at `crates/cadmium/src/widgets/mod.rs:1952-1963`, so click-to-caret is approximate even after boundary safety is fixed. It should use measured glyph advances from the shaped `TextBlock`.

## C2. Critical — Global thread-local dirty queue is unsafe for multi-window use

`PENDING_WIDGET_DIRTY` is one process/thread-global queue keyed only by `WidgetId`:

- `crates/cadmium/src/widgets/mod.rs:31-37`.
- Drained independently by each window: `crates/cadmium/src/widgets/mod.rs:1754-1757`.

Each `WidgetArena` has its own `slotmap::SlotMap`, so identical numeric/generational IDs can exist in different windows. A dirty notification produced by a reactive binding in window A can therefore be consumed by window B's `render()` and applied to B's widget with the same ID. The drain at `crates/cadmium/src/widgets/mod.rs:1757-1777` silently ignores IDs not found in the current arena, making the bug nondeterministic and hard to diagnose.

The queue also retains stale IDs after remount:

- `WidgetArena::clear()` drains nodes and resets focus: `crates/cadmium/src/widgets/mod.rs:754-764`.
- `WidgetWindow::init()` remounts: `crates/cadmium/src/widgets/mod.rs:1357-1360`.
- The pending queue is not purged during clear.

Minimal fix: move the queue into `WidgetArena`/`WidgetWindow` and pass a dirty sink into reactive bindings, or include a window/arena identity in the queue key. Purge queued IDs during `clear()`.

## C3. Critical — Surface loss with `0×0` size can spin the event loop

`CurrentSurfaceTexture::Lost` unconditionally calls `resize()` and then `request_redraw()`:

- `crates/cadmium/src/window/mod.rs:214-218`.

But `resize()` returns immediately for zero dimensions:

- `crates/cadmium/src/window/mod.rs:140-148`.

If a window is minimized, hidden, or in a lost state where `inner_size()` is `0×0`, the redraw request produces another `Lost`/zero-size frame and another redraw request. This violates the design's explicit "skip rendering when minimized to 0x0" contract at `design.md:264-266`.

Minimal fix: skip `request_redraw()` when width or height is zero, mark the surface as needing reconfiguration, and reconfigure once a nonzero `Resized` arrives.

## H1. High — Reactive reads/writes are reentrant-unsafe

`Signal::with()` holds a mutable borrow of the thread-local `ReactiveRuntime` while invoking user code:

- `crates/cadmium/src/reactive/mod.rs:110-118`.

`Signal::update()` does the same while calling the mutation closure:

- `crates/cadmium/src/reactive/mod.rs:124-133`.

Any nested reactive access inside those closures panics with `already mutably borrowed`. This is especially likely in UI code such as `signal.update(|v| v.push(other.get()))` or an effect that updates one signal based on another.

Minimal fix: copy/clone the value or take the closure out of the runtime borrow, release the borrow, then invoke user code. Re-acquire the runtime only to commit the value and notify subscribers.

## H2. High — Effects are not panic-safe

`run_effect()` takes the closure out of the effect entry, pushes an effect ID onto `tracking_stack`, calls user code, then restores state:

- `crates/cadmium/src/reactive/mod.rs:172-193`.

If user code panics:

- the effect ID remains on `tracking_stack`;
- the closure is lost because `func` was taken and never restored;
- subsequent reads are attributed to the wrong effect.

Minimal fix: use a guard whose `Drop` pops the tracking stack and restores the closure even during unwinding, and mark the runtime as poisoned or drop the effect cleanly on panic.

## H3. High — Signal handles leak storage and can become dangling

`Signal<T>` is `Copy` and has no `Drop`:

- `crates/cadmium/src/reactive/mod.rs:89-100`.

Dropping the last handle does not remove the slotmap entry or subscriber map entry created at `crates/cadmium/src/reactive/mod.rs:144-150`. Explicit `dispose()` removes the slot but leaves all other copies dangling:

- `crates/cadmium/src/reactive/mod.rs:135-140`.

Subsequent `get()`/`with()` panics at `crates/cadmium/src/reactive/mod.rs:114-115`.

Minimal fix: introduce a non-`Copy` owner/guard with `Drop`, or use reference-counted slots with generation checks and a non-panicking access API.

## H4. High — Disposed effects leave stale queue entries and can corrupt subscriber maps

- `dispose_effect()` removes the effect and its `dirty_set` entry but not `dirty_effects`: `crates/cadmium/src/reactive/mod.rs:163-169`.
- `record_read()` inserts subscriber IDs without validating that the effect is still alive: `crates/cadmium/src/reactive/mod.rs:43-51`.
- A corrupted `tracking_stack` from H2 can therefore produce unbounded stale subscriber growth.
- `flush_effects()` has a fixed 64-wave cap: `crates/cadmium/src/reactive/mod.rs:195-212`; dirty work can remain pending after the cap with no diagnostic.

Minimal fix: remove IDs from both dirty collections, validate effects in `record_read()`, make the tracking stack panic-safe, and replace the wave cap with convergence detection plus a cycle/limit error.

## H5. High — Nested clipping does not restore the previous scissor

`push_clip()` and `pop_clip()` emit flat scissor commands rather than maintaining a stack:

- `crates/cadmium/src/renderer/mod.rs:271-277`.

A nested `ScrollView` therefore clears clipping to the full target when it exits:

- `crates/cadmium/src/widgets/mod.rs:2196-2203`.

The renderer simply replaces `active_scissor` with the emitted `Option<Rect>`:

- `crates/cadmium/src/renderer/mod.rs:992-994`.
- `crates/cadmium/src/renderer/mod.rs:1060-1063`.

After an inner `pop_clip()`, an outer scroll viewport no longer clips subsequent content. The design explicitly requires a clip stack at `design.md:179-183`.

Minimal fix: store `Vec<Rect>` in `DrawList`; `push_clip()` pushes, `pop_clip()` pops and re-emits the previous rect (or full-target when empty). Rounded clips should eventually use the planned clip storage buffer.

## H6. High — Scissor rectangles can be out of bounds or incorrectly expanded

`apply_scissor()` clamps origins with `.max(0.0)` but does not reduce width/height for negative origins. It also forces width/height to at least 1 after clamping to remaining target space:

- `crates/cadmium/src/renderer/mod.rs:1699-1720`.

Consequences:

- If `x >= target_width`, `target_width.saturating_sub(x)` is `0`, then `.max(1)` produces a 1px rect starting outside the target.
- A clip at `x = -10, width = 20` is converted to `x = 0, width = 20`, extending 10px too far.

Minimal fix: clip the rectangle interval against `[0, target_width)` / `[0, target_height)` before converting to u32, and skip the draw entirely when the clipped interval is empty.

## H7. High — Focused TextInput defeats idle-frame skipping

`WidgetWindow::tick()` returns `true` whenever a `TextInput` is focused:

- `crates/cadmium/src/widgets/mod.rs:1376-1381`.

There is no check that the caret blink phase actually needs a redraw or that the input is being edited. The focused widget therefore forces continuous animation frames while idle.

This directly contradicts:

- `README.md:22` — "0% CPU/GPU when idle".
- `CHANGELOG.md:35` — same claim.
- `ROADMAP.md:20` — same claim.
- `design.md:105-108` and `requirements.md:291`.

Minimal fix: remove the unconditional `animating = true`; schedule a redraw only on actual text/caret/selection changes, and use a timer for the blink phase.

## H8. High — Uploaded GPU textures are never released

`GpuContext` stores every uploaded texture in a growing `Vec`:

- `crates/cadmium/src/renderer/mod.rs:503-527`.
- Insertion at `crates/cadmium/src/renderer/mod.rs:665-670`.

There is no `remove`, `retain`, eviction, or LRU policy for `textures`. Repeated image loading in a long-lived editor will leak GPU memory even when widgets are unmounted.

Minimal fix: add explicit `TextureHandle` invalidation plus reference counting or LRU eviction; tie handles to image widget lifetimes.

## H9. High — Glyph atlas overflow clears the CPU cache but not the GPU texture or emitted instances

On atlas overflow, the code clears `self.cache` and resets cursors:

- `crates/cadmium/src/renderer/mod.rs:427-437`.

It does not clear the atlas texture or rewrite glyph instances already emitted for the current frame. Uniforms, bind groups, and instance buffers are built from stale atlas coordinates:

- `crates/cadmium/src/renderer/mod.rs:288` collects glyphs for the current draw list.

A full atlas can therefore render wrong glyphs for the remainder of the frame.

Minimal fix: clear/overwrite the atlas texture on reset, or defer reset to a frame boundary and rebuild all glyph instances for the new atlas layout.

## H10. High — WGSL uses `smoothstep` with reversed edges

`glass.wgsl` computes:

- `smoothstep(0.20, -0.45, refl_coord)` at `crates/cadmium/shaders/glass.wgsl:146`.

WGSL requires `edge0 < edge1`; reversed edges produce indeterminate results. This is a portability hazard: the highlight may be correct on one driver and wrong or absent on another.

Minimal fix: rewrite as `1.0 - smoothstep(-0.45, 0.20, refl_coord)`.

## H11. High — CI formatting and lint gates currently fail

- Format check: `.github/workflows/ci.yml:37-38` fails with 64 diffs locally.
- Clippy: `.github/workflows/ci.yml:41` uses `-D warnings`; local clippy emits numerous warnings, including manifest warnings at `Cargo.toml:16` and `crates/cadmium/Cargo.toml:8`.
- Test step: `.github/workflows/ci.yml:43-44` passes.

Until formatting and lint are cleaned, no PR can merge through the configured CI.

## H12. High — Surface/device robustness is incomplete

Beyond C3:

- `ScaleFactorChanged` only requests a redraw: `crates/cadmium/src/window/mod.rs:496-500`. It does not reconfigure the surface or render target; correctness depends on winit subsequently emitting `Resized`.
- Logical size is recomputed as physical config size divided by the current scale factor: `crates/cadmium/src/window/mod.rs:222-224`.
- Surface capabilities are indexed without checks: `crates/cadmium/src/window/mod.rs:370-376` and `:385`.
- Device loss has no rebuild path despite the design requirement at `design.md:264-266`.

Minimal fix: centralize surface reconfiguration, handle zero sizes explicitly, use `first()` with a documented fallback for formats/alpha modes, and add a device-lost state machine that rebuilds `GpuContext`.

## M1. Medium — Remount leaves stale interaction state

`WidgetArena::clear()` resets only its own focus manager:

- `crates/cadmium/src/widgets/mod.rs:754-764`.

`WidgetWindow` separately stores hovered/captured widget IDs and open dropdown state:

- `crates/cadmium/src/widgets/mod.rs:1295-1305`.

These are not reset during remount at `crates/cadmium/src/widgets/mod.rs:1357-1360`. Stale IDs can keep a hovered style active, route pointer capture to a different widget, or leave dropdown hit-testing active after the tree has changed.

## M2. Medium — Only one dropdown overlay is tracked

`WidgetWindow` stores a single `open_dropdown`:

- `crates/cadmium/src/widgets/mod.rs:1303`.

But `paint_node()` can push multiple overlay records:

- `crates/cadmium/src/widgets/mod.rs:2015-2021`.

The loop over overlays overwrites `self.open_dropdown` each iteration:

- `crates/cadmium/src/widgets/mod.rs:1881-1889`.

Opening a second dropdown makes the first invisible to hit-testing and closing logic. Click handling uses only the last stored dropdown at `crates/cadmium/src/widgets/mod.rs:1636-1645`.

## M3. Medium — Dropdown menus can render off-screen

Menu placement computes:

- `menu_x = trigger_bounds.x` with no right-edge clamp.
- `menu_y` clamped with `.min(ctx.height - menu_h - 8.0)`, which can become negative for tall menus or small windows.

Evidence: `crates/cadmium/src/widgets/mod.rs:1884-1888`.

Minimal fix: clamp both axes to the window bounds after computing desired size; flip above the trigger if there is insufficient space below.

## M4. Medium — `Mutex` poisoning can panic during startup

`App::run()` uses `lock().unwrap()` when installing the event-loop proxy:

- `crates/cadmium/src/window/mod.rs:305-309`.

A poisoned mutex turns a recoverable startup problem into a panic. Use `lock().unwrap_or_else(|e| e.into_inner())` or `expect()` with a meaningful message.

## M5. Medium — Capability fallback is not defensive

`caps.formats[0]` and `caps.alpha_modes[0]` assume non-empty vectors:

- `crates/cadmium/src/window/mod.rs:370-376` and `:385`.

Use `.first().copied()` with an explicit `SteelError` if the adapter reports no usable format/alpha mode.

## M6. Medium — Three-stage invalidation is only partially implemented

`DirtyFlags` defines `state`, `layout`, and `paint`:

- `crates/cadmium/src/widgets/mod.rs:25-29`.

But the render drain only checks `flags.layout`:

- `crates/cadmium/src/widgets/mod.rs:1757-1777`.

`flags.state` and `flags.paint` do not drive separate state re-evaluation or paint-only caching here. This falls short of the detailed pipeline in `requirements.md:100-105` and `design.md:113-138`.

---

# 2. Memory and steady-state allocations

## M7. Medium — Per-node per-frame allocations for accessibility records

`paint_node()` clones `a11y_label` and `children` for every node, every frame:

- `crates/cadmium/src/widgets/mod.rs:1992-1998`.

The resulting `TreeUpdate` is then immediately dropped:

- `crates/cadmium/src/widgets/mod.rs:1942`.

This is pure steady-state allocation with no functional benefit until an AccessKit adapter exists.

## M8. Medium — Text measurement rebuilds a `TextBlock` for every caret/selection query

`measure_prefix_width()` constructs and shapes a new `TextBlock` on every call:

- `crates/cadmium/src/widgets/mod.rs:2304-2318`.

It is called twice for selection plus once for the caret each frame:

- `crates/cadmium/src/widgets/mod.rs:2128-2153`.

Minimal fix: cache prefix widths per `TextBlock` generation or use the glyph metrics already produced by shaping.

## M9. Medium — Open dropdown items are cloned and reshaped every frame

The dropdown paint path clones all items:

- `crates/cadmium/src/widgets/mod.rs:2015-2021`.

The overlay renderer then constructs one `TextBlock` per item every frame:

- `crates/cadmium/src/widgets/mod.rs:1928-1938`.

Minimal fix: cache item `TextBlock`s keyed by the dropdown's signal/version and scale factor.

## M10. Medium — `collect_glyphs()` allocates a fresh vector per text block per frame

- `crates/cadmium/src/text/mod.rs:181-200`.
- Called from `crates/cadmium/src/renderer/mod.rs:288`.

Minimal fix: use a reusable scratch buffer in `GpuContext` or `DrawList`, cleared per frame.

## M11. Medium — `tick()` scans the entire arena every frame

- `crates/cadmium/src/widgets/mod.rs:1362-1384`.

Even with no animations, this iterates all nodes. Combined with C1/H7, it makes idle frames expensive. Maintain separate active-animation and focused-text sets instead.

## M12. Medium — GPU render targets eagerly allocate the full blur pyramid

`WindowRenderTarget::new()` always creates canonical/backdrop targets, three mip textures, blur uniform buffers/bind groups, and four instance buffers:

- `crates/cadmium/src/renderer/mod.rs:764-834`.

This happens even when no glass/backdrop blur is used. Resize recreates all textures:

- `crates/cadmium/src/renderer/mod.rs:869-915`.

Minimal fix: lazily create blur resources on first `GlassQuad`, and reuse textures where wgpu permits or when dimensions shrink/grow only within tolerance.

## M13. Medium — Uploaded textures have no lifetime management

See H8. This is both a correctness/leak issue and a steady-state GPU memory issue.

---

# 3. Reactive runtime and leaks

The reactive-related panic and leak findings are detailed above:

- Reentrancy: H1 (`crates/cadmium/src/reactive/mod.rs:110-133`).
- Panic safety: H2 (`crates/cadmium/src/reactive/mod.rs:172-193`).
- Handle leaks/dangling handles: H3 (`crates/cadmium/src/reactive/mod.rs:89-100`, `:135-150`).
- Stale dirty/subscriber state and wave cap: H4 (`crates/cadmium/src/reactive/mod.rs:43-51`, `:163-212`).
- Cross-window dirty routing: C2 (`crates/cadmium/src/widgets/mod.rs:31-37`, `:1754-1757`).

Additional observations:

## M14. Medium — Reactive runtime is thread-local and globally shared

`RUNTIME` is a thread-local `RefCell<ReactiveRuntime>`:

- `crates/cadmium/src/reactive/mod.rs:79-81`.

This is acceptable for a single UI thread, but the public API does not document that all windows and dispatchers must live on one thread. Combined with the global dirty queue, it prevents clean isolation of independent applications or test cases in one process/thread.

## M15. Medium — Reactive tests do not cover failure modes

The only reactive test is at `crates/cadmium/src/reactive/mod.rs:241`. There are no tests for:

- nested `get()` inside `update()`;
- effect panic recovery;
- disposal during dirty flush;
- subscriber cleanup;
- cycle/limit behavior.

Given the unsafe-state potential in H1-H4, these should be added before broader adoption.

---

# 4. Magnum rendering, GPU resources, and shaders

## Positive verification

The following were checked and are currently consistent:

- WGSL and Rust struct layouts match for `Globals`, `BlurParams`, SDF/image/glass/glyph instance data. See `crates/cadmium/shaders/sdf.wgsl:1-19`, `crates/cadmium/shaders/glass.wgsl:1-31`, `crates/cadmium/shaders/text.wgsl:1-19`, `crates/cadmium/shaders/textured.wgsl:1-20`, `crates/cadmium/shaders/blur.wgsl:23-27`, and Rust mirrors at `crates/cadmium/src/renderer/mod.rs:120-195`.
- `textureSample()` calls are in uniform control flow before `discard` in the SDF/text/textured/glass shaders (`crates/cadmium/shaders/sdf.wgsl:94-137`, `text.wgsl:52-55`, `textured.wgsl:67-72`, `glass.wgsl:128-135`).
- Premultiplied alpha blending is used for the UI pipelines at `crates/cadmium/src/renderer/mod.rs:1393`, `:1452`, `:1510`, and `:1622`.
- `glass.wgsl:121` uses `@builtin(position)` framebuffer coordinates for screen UVs; this is correct.
- The canonical `Rgba8UnormSrgb` offscreen target plus final blit exists as designed (`crates/cadmium/src/renderer/mod.rs:768-770`, blit pipeline cache at `:526`).

## Defects

See H5 (clip stack), H6 (scissor bounds), H9 (atlas overflow), H10 (reversed `smoothstep`), H12 (surface/device resilience), M12 (eager blur allocation), and M13 (texture lifetime).

Additional lower-severity shader finding:

## L1. Low — Approximate gamma curve in glass shader

`glass.wgsl:149-152` uses `pow(..., 1.0/2.2)` / `pow(..., 2.2)` instead of the exact sRGB transfer function. This is visually approximate rather than incorrect, but it undermines the "colour-space correctness" goal at `requirements.md:138`. Prefer exact sRGB encode/decode or rely on the surface format and work in linear space consistently.

## M16. Medium — Batching is frequently interrupted by glass and scissor changes

The render loop ends the normal batch when it encounters `DrawBatch::Glass` and performs a full-target copy plus three blur passes:

- `crates/cadmium/src/renderer/mod.rs:996-1044`.

Hardware scissor changes are handled inside an active render pass (`crates/cadmium/src/renderer/mod.rs:1058-1063`), but glass still forces separate passes. This is expected for the current blur approach, but it means "minimal GPU state changes" in `CHANGELOG.md:31` is optimistic for glass-heavy UI.

---

# 5. Architecture, API hygiene, and dependency isolation

## M17. Medium — Public API leaks implementation dependencies

The requirements explicitly state that `winit`, `cosmic-text`, `accesskit`, `wgpu`, and similar types must remain internal implementation details:

- `requirements.md:82`.
- `design.md:12` also requires a strict DAG with lower layers not importing higher layers.

Current leaks:

- `WindowDelegate` exposes `&winit::keyboard::Key`: `crates/cadmium/src/window/mod.rs:97-103`.
- `WindowDelegate::init()` exposes `&mut TextEngine`: `crates/cadmium/src/window/mod.rs:79-80`.
- `WindowDelegate::init_gpu()` exposes `&mut GpuContext`: `crates/cadmium/src/window/mod.rs:81`.
- `GpuContext` publicly exposes wgpu device/queue/adapter/instance fields: `crates/cadmium/src/renderer/mod.rs:503-507`.
- Internal modules and types are re-exported broadly: `crates/cadmium/src/lib.rs:1-36`.

Minimal fix: introduce SteelUI-owned key/event, text, and GPU-surface types at the public boundary; keep `winit`/`wgpu` types behind `pub(crate)` or controlled escape-hatch traits.

## M18. Medium — Logical module boundaries exist but are not enforced

All design modules are declared at `crates/cadmium/src/lib.rs:1-14`, but they are all `pub mod` rather than internal modules. Combined with broad re-exports at `crates/cadmium/src/lib.rs:16-36`, the DAG in `design.md:14-64` is not enforced by the compiler. Lower-level code can be imported directly by applications, and higher-level modules can be reached from anywhere.

## M19. Medium — AccessKit tree is built but never delivered

`build_tree_update()` correctly constructs a `TreeUpdate`:

- `crates/cadmium/src/accessibility/mod.rs:42-78`.

But `WidgetWindow::render()` stores it in `_tree_update` and drops it:

- `crates/cadmium/src/widgets/mod.rs:1942`.

There is no `AccessKit` adapter, no `update_if_active()`, and no semantic action routing anywhere in the repository. Therefore:

- `README.md` does not claim full accessibility, but `design.md:255-258` and `requirements.md:300-307` do.
- `CHANGELOG.md:43` calls it a "Semantic accessibility tree builder bridge", which is accurate but easy to misread as functional accessibility.

Minimal fix: add a per-window `accesskit::Adapter`, create it from raw window handles, and call `update_if_active()` after phase 7; route `Action` events back through `WindowDelegate`.

## M20. Medium — Dirty pipeline and global runtime conflict with multi-window design

The design calls for per-window root/focus/IME/AccessKit state and process-wide GPU/reactive state (`design.md:66-72`, `requirements.md:262-274`). The current reactive runtime is process/thread-global (`crates/cadmium/src/reactive/mod.rs:79-81`) and dirty notifications are globally queued (C2), so multi-window isolation is not actually enforced.

## L2. Low — Manifest hygiene warnings

- `Cargo.toml:16` defines workspace `readme` that is not consumed.
- `crates/cadmium/Cargo.toml:8` duplicates `repository` as `homepage`.

These are small but block CI because of `-D warnings`.

---

# 6. Cross-platform and OSS readiness

## M21. Medium — README badges point at a different repository

`README.md:8` uses `github.com/cadmium/cadmium`, while the actual repository metadata is `https://github.com/ShreyasT123/cadmium`:

- `Cargo.toml:14-15`.

Fix the badge URLs or change the repository metadata if the project has moved.

## M22. Medium — README references packages that are not workspace members

Workspace members are only `crates/cadmium`, `apps/dogfood`, and `examples/liquid_glass`:

- `Cargo.toml:3-7`.

But README instructs users to run:

- `cargo run -p sequel_showcase` at `README.md:151`.
- `cargo run -p macos_notes` at `README.md:157`.

These commands will fail. Either add the examples to the workspace or remove the sections.

## M23. Medium — Changelog and roadmap contain stale or contradictory status

Examples:

- `CHANGELOG.md:13-16` lists Material, ScrollView/Scissor, TextInput, and floating overlays as "Planned" even though implementations exist in `widgets/mod.rs` and `renderer/mod.rs`.
- `ROADMAP.md:49-66` similarly marks those items as pending/in-progress.
- `README.md:24` and `CHANGELOG.md:29` claim a 2048×2048 glyph atlas; implementation is 512×512 at `crates/cadmium/src/renderer/mod.rs:13`.
- `README.md:22`, `CHANGELOG.md:35`, and `ROADMAP.md:20` claim zero idle CPU/GPU; contradicted by H7.

This is an OSS-readiness problem because contributors will work on already-implemented features and users will rely on inaccurate capability claims.

## M24. Medium — Apache license file is only a notice, not the full license text

`LICENSE-APACHE:1-17` contains the standard short notice but not the full Apache License 2.0 body. Most tooling and legal review expects the complete license text. Replace it with the official full text.

## M25. Medium — `Cargo.lock` is ignored despite binary workspace members

`.gitignore:2` ignores `Cargo.lock`, and the workspace contains binary targets (`apps/dogfood`, `examples/liquid_glass`). For reproducible CI and downstream builds, application workspaces typically commit `Cargo.lock`.

## M26. Medium — Platform font discovery is brittle

`TextEngine::new()` hard-codes Windows font paths at `crates/cadmium/src/text/mod.rs:55-68` and a small set of macOS paths at `:70-78`. On macOS it skips `load_system_fonts()` unless both hard-coded paths fail (`crates/cadmium/src/text/mod.rs:80-82`). Modern macOS versions do not reliably ship those exact files, so fallback/family discovery can be missing.

Minimal fix: always call `db.load_system_fonts()` and then apply preferred family names via `set_sans_serif_family()` when available.

## L3. Low — Linux CI dependency set appears adequate for the current lockfile

`ci.yml:28-32` installs X11/Wayland development packages. The current `Cargo.lock` resolves `rfd 0.17.2` with Wayland dependencies (`Cargo.lock:1853-1877`) and no GTK crates, so no GTK package gap was proven. Keep an eye on this if `rfd` features change.

## L4. Low — No explicit MSRV verification

`README.md:10` advertises Rust 1.85+, and the workspace uses edition 2024 (`Cargo.toml:11`), but CI uses unpinned stable at `.github/workflows/ci.yml:23-26`. Add an MSRV job or document that stable-only is intended.

## L5. Low — Shader portability needs CI coverage

The reversed `smoothstep` (H10) would likely be caught by a real GPU or validation-focused shader test. The current test suite has no headless GPU or WGSL validation step despite the design requirement at `design.md:268-271`.

---

# 7. Prioritized action plan

## Critical — fix before any further feature work

1. **Make TextInput UTF-8 safe** — C1.  
   Add a canonical boundary-normalization helper; use it in all key, pointer, paint, and signal-update paths. Add tests with `"é"`, emoji, and external `signal.set()` shortening.

2. **Isolate dirty notifications per window/arena** — C2.  
   Move `PENDING_WIDGET_DIRTY` into `WidgetArena` and purge on `clear()`. Add a two-window test that mutates a signal in one arena and asserts no flags change in the other.

3. **Fix surface-loss/zero-size redraw loop** — C3.  
   Do not call `request_redraw()` for zero dimensions; add a `needs_reconfigure` flag. Add a testable helper for the lost/outdated/suboptimal state machine.

## High — fix before release or public beta

4. **Harden reactive runtime** — H1-H4.  
   Release the `RefCell` borrow around user closures; make effect execution guard-based; validate disposed effects; remove stale queue entries; replace the fixed wave cap with convergence diagnostics. Add reentrancy and panic tests.

5. **Implement a real clip stack** — H5.  
   Replace flat `SetScissor(None)` with stack restoration. Add nested `ScrollView` coverage.

6. **Fix scissor geometry** — H6.  
   Intersect rectangles with target bounds before converting to u32; skip empty clips.

7. **Remove unconditional focused-TextInput animation** — H7.  
   Drive redraws from actual state changes and blink timers; verify idle CPU/GPU behavior with instrumentation.

8. **Add texture lifetime management and atlas reset correctness** — H8, H9, M13.  
   Reference-count or LRU-evict uploaded textures; clear/rewrite atlas state on overflow before instances are emitted.

9. **Fix WGSL portability bug** — H10.  
   Replace reversed `smoothstep` at `glass.wgsl:146`.

10. **Make CI green** — H11.  
    Run `cargo fmt --all`, clean clippy diagnostics or scope allow-lints deliberately, then verify the exact CI commands including `-D warnings`.

11. **Complete GPU resilience path** — H12.  
    Reconfigure on DPI/size changes with explicit zero-size handling; use checked capability fallbacks; add device-lost rebuild or graceful diagnostic.

## Medium — schedule next

12. Reset `hovered`, `captured`, and `open_dropdown` on remount — M1.  
13. Support multiple dropdown overlays and clamp menu placement — M2, M3.  
14. Replace per-frame clones/TextBlock construction with caches — M7, M8, M9, M10, M11.  
15. Lazily allocate blur resources and reuse render targets where possible — M12.  
16. Introduce SteelUI-owned public wrappers to hide `winit`/`wgpu`/`TextEngine` — M17, M18.  
17. Wire an actual AccessKit adapter and semantic action routing — M19.  
18. Reconcile docs, roadmap, changelog, README commands, atlas size, and idle claims — M21, M22, M23.  
19. Replace `LICENSE-APACHE` with the full Apache 2.0 text — M24.  
20. Commit `Cargo.lock` or document why the workspace intentionally ignores it — M25.  
21. Improve font discovery and fallback — M26.  
22. Implement state/paint phases of the 3-stage dirty pipeline — M6.  
23. Add headless GPU/shader validation and Unicode/reactive regression tests — L5, M15.

## Low — cleanup

24. Clear manifest warnings — L2.  
25. Add MSRV CI or update the badge — L4.  
26. Revisit glass color-space math — L1.  
27. Replace `lock().unwrap()` with poison-tolerant handling — M4.  
28. Reduce clippy style warnings in batches where behavior is unchanged.

---

## Items checked and not treated as defects

- **`queue.write_texture` 256-byte row alignment:** Not required by the resolved wgpu 30 implementation for this use; no finding recorded.
- **`Filtering` bind-group layout accepting nearest samplers:** Verified as valid; no finding recorded.
- **`WindowContext` field drop order:** `surface` is declared before `window` at `crates/cadmium/src/window/mod.rs:123-125`; Rust drops fields in declaration order, so the surface is dropped before the window. This is correct.
- **README quick example API existence:** `Column` exists as a type alias at `crates/cadmium/src/widgets/mod.rs:223`, and the chained builder methods used in `README.md:67-117` largely exist. The README problems recorded above are about commands and claims, not that specific example's method names.
- **Linux GTK dependency gap:** Current lockfile evidence does not support adding GTK dev packages, so no finding was recorded.

---

## Closing assessment

SteelUI's architecture documents and code show the right high-level shape, and the renderer/shader data contracts are internally consistent. The main work now is hardening rather than expansion: Unicode-safe editing, reactive lifecycle safety, per-window isolation, resilient GPU state transitions, correct clipping, and truthful documentation. Until those are addressed, the project should be presented as an experimental prototype rather than a release-ready GUI framework.
