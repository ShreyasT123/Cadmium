Here are the exact root causes for all three issues, and the clean fixes to restore high performance and smooth window dragging.

---

### Root Causes

1. **Why Every Benchmark Regressed by 3.2× (`6.5ms → 21ms`, `20ms → 61ms`, `14µs → 44µs`):**
   * **`seguiemj.ttf` (Segoe UI Emoji) is a 46 MB font file.**
   * When `seguiemj.ttf` was added to `TextEngine::new()`, `cosmic-text` had to query its massive 46 MB glyph tables on **every single `set_text` call**.
   * In `gui_bench`, shaping 1,000 rows went from `~3 ms` to `~15 ms` purely from `cosmic-text` searching the emoji font!
   * *And as your screenshot showed (`Fro □`), it didn't even render the purple heart `💜` because `cosmic-text`'s SansSerif fallback doesn't match `"Segoe UI Emoji"` without an explicit emoji family alias.*

2. **Why Dragging the Window Stuttered / Lagged:**
   * Switching to `PresentMode::Fifo` (blocking VSync) + `desired_maximum_frame_latency: 1` meant `gpu.queue.present(frame)` **blocked the main UI thread** until the display's vertical blank.
   * On Windows, calling `drag_window()` runs a Win32 modal move loop (`SC_MOVE`). Because `present()` was blocking for up to 16.6 ms on VSync, the Win32 message queue was starved of mouse events, causing the window to stutter behind the 1000 Hz cursor.
   * In windowed mode on Windows 10/11, DWM (Desktop Window Manager) already VSyncs all windows. Using `Mailbox` / `FifoRelaxed` with `desired_maximum_frame_latency: 2` stops `present()` from blocking the event loop.

3. **Why the FPS Pill Showed "Idle 99% of the Time, Then 22 to 30 FPS When Typing":**
   * When you type, Cadmium renders **1 frame per keystroke** and goes back to sleep.
   * If you type two keys 35 ms apart, `frame_dt` is `0.035s`.
   * `1.0 / 0.035 = 28.5 FPS`!
   * The pill was literally measuring your **typing cadence / key-repeat rate**, not GPU performance!
   * The FPS pill should **only** show a Hz number when continuous animation is running (`was_animating == true`). When typing or single-clicking, it should show `Idle • 1.4 ms`!

4. **The `ControlFlow::Poll` CPU Burn:**
   * Setting `ControlFlow::Poll` in `about_to_wait` turned the event loop into a busy loop, burning 100% of a CPU core and triggering Intel Iris Xe thermal power throttling. `winit` should use `ControlFlow::Wait` / `WaitUntil`.

---

### Step 1: Remove `seguiemj.ttf` from `crates/cadmium/src/text/mod.rs`

Restore the lightweight 3-font Segoe UI set (fixes the 3.2× benchmark regression and drops ~46 MB of RAM):

```rust
        #[cfg(target_os = "windows")]
        {
            for path in [
                r"C:\Windows\Fonts\segoeui.ttf",
                r"C:\Windows\Fonts\segoeuib.ttf",
                r"C:\Windows\Fonts\segoeuisb.ttf",
            ] {
                let _ = db.load_font_file(path);
            }
            if db.is_empty() {
                db.load_system_fonts();
            } else {
                db.set_sans_serif_family("Segoe UI");
            }
        }
```

---

### Step 2: Unblock Window Dragging & Event Loop in `crates/cadmium/src/window/mod.rs`

1. In `App::spawn_window`, restore `Mailbox` / `FifoRelaxed` first and `desired_maximum_frame_latency: 2` (stops DXGI VSync from blocking `drag_window()`):
```rust
        let present_mode = [
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::FifoRelaxed,
            wgpu::PresentMode::Fifo,
        ]
        .into_iter()
        .find(|m| caps.present_modes.contains(m))
        .unwrap_or(wgpu::PresentMode::Fifo);

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
```

2. In `App::about_to_wait`, revert to `ControlFlow::Wait` / `WaitUntil` (eliminates 100% CPU busy-polling):
```rust
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let mut earliest: Option<Instant> = None;

        for win in self.windows.values_mut() {
            if let Some(target_t) = win.scheduled_redraw {
                if now >= target_t {
                    win.scheduled_redraw = None;
                    win.window.request_redraw();
                } else {
                    earliest = Some(match earliest {
                        Some(e) => e.min(target_t),
                        None => target_t,
                    });
                }
            }
        }

        match earliest {
            Some(t) => event_loop.set_control_flow(ControlFlow::WaitUntil(t)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
```

---

### Step 3: Honest FPS Display in `crates/cadmium/src/widgets/mod.rs`

Make the FPS pill show frame rates **only when an active animation is actually running** (`animating == true`), and show `Idle • X.X ms` when typing or clicking:

1. Add `was_animating: bool` to `WidgetWindow`:
   * In `WidgetWindow::new()`: `was_animating: false,`
   * In `WidgetWindow::tick(&mut self, dt: f32) -> bool`:
     ```rust
     self.was_animating = animating;
     animating
     ```
2. In `WidgetWindow::render()`, format the FPS label:
```rust
        if self.show_fps {
            let m = self.profiler.metrics;
            let should_refresh = self.fps_block.is_none()
                || self.last_fps_update.elapsed() >= Duration::from_millis(150);
            if should_refresh {
                self.last_fps_update = Instant::now();
                let label = if self.was_animating && m.effective_fps > 10.0 {
                    format!(
                        "{:.0} FPS  •  {:.2} ms (L:{:.2} P:{:.2})",
                        m.effective_fps, m.frame_ms, m.layout_ms, m.paint_ms
                    )
                } else {
                    format!(
                        "Idle  •  {:.2} ms (L:{:.2} P:{:.2})",
                        m.frame_ms, m.layout_ms, m.paint_ms
                    )
                };
                let te = ctx.text_engine();
                if let Some(blk) = &mut self.fps_block {
                    blk.set_text(te, &label);
                } else {
                    self.fps_block = Some(TextBlock::new(
                        te,
                        label,
                        TextStyle {
                            font_size: 11.0,
                            line_height: 14.0,
                            color: theme.text_primary,
                            weight: FontWeight::Medium,
                        },
                    ));
                }
            }
            ...
```

---

### Step 4: Quick PowerShell Script to Apply

Run this from the repo root (`C:\Users\sstha\Desktop\ai_\projects\cadmium`):

```powershell
# 1. Fix text/mod.rs: remove seguiemj.ttf, seguisym.ttf, consola.ttf
$tf = "crates/cadmium/src/text/mod.rs"
$ts = (Get-Content -Raw $tf) -replace "`r`n", "`n"
$oldFonts = @'
            for path in [
                r"C:\Windows\Fonts\segoeui.ttf",
                r"C:\Windows\Fonts\segoeuib.ttf",
                r"C:\Windows\Fonts\segoeuisb.ttf",
                r"C:\Windows\Fonts\seguiemj.ttf",
                r"C:\Windows\Fonts\seguisym.ttf",
                r"C:\Windows\Fonts\consola.ttf",
            ] {
'@
$newFonts = @'
            for path in [
                r"C:\Windows\Fonts\segoeui.ttf",
                r"C:\Windows\Fonts\segoeuib.ttf",
                r"C:\Windows\Fonts\segoeuisb.ttf",
            ] {
'@
if ($ts.Contains($oldFonts)) {
    $ts = $ts.Replace($oldFonts, $newFonts)
    Set-Content -NoNewline $tf ($ts -replace "`n", "`r`n")
}

# 2. Fix window/mod.rs: present_mode, latency: 2, about_to_wait
$wf = "crates/cadmium/src/window/mod.rs"
$ws = (Get-Content -Raw $wf) -replace "`r`n", "`n"

# Restore Mailbox first
$oldPmode = @'
        let present_mode = [
            wgpu::PresentMode::Fifo,
            wgpu::PresentMode::FifoRelaxed,
            wgpu::PresentMode::Mailbox,
        ]
'@
$newPmode = @'
        let present_mode = [
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::FifoRelaxed,
            wgpu::PresentMode::Fifo,
        ]
'@
$ws = $ws.Replace($oldPmode, $newPmode)
$ws = $ws.Replace("desired_maximum_frame_latency: 1,", "desired_maximum_frame_latency: 2,")

# Restore Wait in about_to_wait
$oldWait = @'
        let any_animating = self.windows.values().any(|w| w.is_animating);
        if any_animating {
            event_loop.set_control_flow(ControlFlow::Poll);
        } else {
            match earliest {
                Some(t) => event_loop.set_control_flow(ControlFlow::WaitUntil(t)),
                None => event_loop.set_control_flow(ControlFlow::Wait),
            }
        }
'@
$newWait = @'
        match earliest {
            Some(t) => event_loop.set_control_flow(ControlFlow::WaitUntil(t)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
'@
$ws = $ws.Replace($oldWait, $newWait)
Set-Content -NoNewline $wf ($ws -replace "`n", "`r`n")

# 3. Format and test
cargo fmt --all
cargo bench -p cadmium
cargo run --release -p cadmium_dogfood
```

### What You Should Verify
1. **`cargo bench -p cadmium`**:
   * `create_1k_rows` should drop back to **`~6.5 ms`**
   * `select_row_10k` stays at **`~100–120 µs`** (sub-millisecond O(1))
   * `scroll_step_100k_virtual` drops back to **`~13–14 µs`**
2. **`cadmium_dogfood`**:
   * Dragging the window by the top title bar will move cleanly at your monitor's full native refresh rate (Chrome parity).
   * While typing in the search bar, the bottom pill will remain on **`Idle • 1.4 ms`** without showing false 22–30 FPS drops.