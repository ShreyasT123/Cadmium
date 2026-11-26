do not run check workspace or build release
That screenshot immediately reveals the exact root cause of **both** the giant empty black gap and the crushed `player_capsule`:

### Why There Was a ~380px Black Gap Between the Sidebar and `"New"`

1. **`Switcher` forced `PositionMode::Absolute` on active children + defaulted to `flex_grow: 1.0`:**
   - Because `sidebar` (`224px`) was placed inside `Switcher::new(sidebar_mode, ...)` with `PositionMode::Absolute` and `flex_grow: 1.0`, `Switcher` reported `0px` intrinsic width and expanded to **50% of the entire window (~640px)**!
   - That squeezed `right_canvas` into the remaining `~640px`, which in turn caused `Best New Songs` (4 separate `Wrap`s of 3 items) to wrap into `2 + 1` orphan rows and caused `player_capsule` to overflow its volume slider!

---

### Fix 1 — `crates/cadmium/src/widgets/` (`views.rs`, `arena.rs`, `window_impl.rs`)

1. **Add `.flex_grow(f32)` and `.width(Length)` to `Switcher` (`views.rs`):**
   ```rust
   pub struct Switcher {
       signal: Signal<usize>,
       children: Vec<Element>,
       layout: LayoutStyle,
   }

   impl Switcher {
       pub fn new(signal: Signal<usize>, children: Vec<Element>) -> Self {
           Self {
               signal,
               children,
               layout: LayoutStyle {
                   flex_grow: 1.0,
                   ..Default::default()
               },
           }
       }

       pub fn flex_grow(mut self, g: f32) -> Self {
           self.layout.flex_grow = g;
           self
       }

       pub fn width(mut self, w: Length) -> Self {
           self.layout.width = w;
           self
       }
   }
   ```
   *(In `mount_node` (`arena.rs`) for `Element::Switcher(sw)`, use `sw.layout` instead of hardcoded `flex_grow: 1.0`, and do **not** set `PositionMode::Absolute` on active children!)*

2. **Only set `PositionMode::Absolute` on the outgoing `prev_idx` during transitions (`window_impl.rs`):**
   In `WidgetWindow::tick`'s `switcher_ids` loop:
   ```rust
   for (i, &cid) in children.iter().enumerate() {
       let hidden = i != active && Some(i) != prev_opt;
       let is_outgoing = Some(i) == prev_opt && i != active;
       if let Some(node) = self.arena.nodes.get_mut(cid) {
           let target_pos = if is_outgoing {
               PositionMode::Absolute
           } else {
               PositionMode::Relative
           };
           let target_inset = if is_outgoing {
               Inset::all(Length::Px(0.0))
           } else {
               Inset::AUTO
           };
           if node.layout_style.hidden != hidden
               || node.layout_style.position != target_pos
           {
               node.layout_style.hidden = hidden;
               node.layout_style.position = target_pos;
               node.layout_style.inset = target_inset;
               let style = node.layout_style;
               let ln = node.layout_node;
               self.arena.layout.set_style(ln, style);
               self.arena.needs_full_layout = true;
           }
       }
   }
   ```
   Now:
   - The **active** child stays `PositionMode::Relative` in normal flex flow, so `Switcher(sidebar_mode).flex_grow(0.0)` hugs **`224px`** (or **`60px`** compact) with **zero gap**!
   - The **outgoing** child floats in `PositionMode::Absolute` during the 200ms fade/slide, then hides cleanly.

---

### Fix 2 — `apps/dogfood/src/main.rs` (4 Targeted Edits)

Apply these 4 clean adjustments in `apps/dogfood/src/main.rs`:

1. **Set `.flex_grow(0.0)` on the two fixed-size `Switcher`s:**
   - Sidebar `Switcher` (line ~1066):
     ```rust
     Switcher::new(
         sidebar_mode.signal(),
         vec![sidebar.into_element(), compact_sidebar.into_element()],
     )
     .flex_grow(0.0)
     ```
   - `active_cover` `Switcher` (line ~715):
     ```rust
     .child(Switcher::new(current_track, imgs).flex_grow(1.0))
     ```
     *(Inside `Container::new().width(Px(size)).height(Px(size))`, `flex_grow(1.0)` fills the square box cleanly.)*

2. **Use ONE single `Wrap` for all 12 songs in `Best New Songs` (eliminates `2 + 1` orphan rows):**
   Replace the `for row_idx in 0..4 { let mut row = Wrap::new()... }` loop with one `Wrap`:
   ```rust
   let mut song_grid = Wrap::new().padding(Edges::xy(28.0, 0.0)).gap(14.0);
   for idx in 0..12 {
       let tr = &tracks[idx];
       ...
       song_grid = song_grid.child(
           Container::row()
               .width(Length::Px(280.0))
               .flex_grow(1.0)
               ...
       );
   }
   ```
   Now at `1280px` it flows cleanly into `3×4`, and when tiled to half-screen (`640px–800px`) it flows cleanly into `2×6` or `1×12` with **zero orphan rows**.

3. **Make `player_capsule` center tile & `ProgressBar` flexible (never overflows):**
   Inside `player_capsule`:
   - Change the center track `Container::row().width(Length::Px(260.0))` → `.width(Length::Px(0.0)).flex_grow(1.0).max_width(Length::Px(290.0))`
   - Change `ProgressBar::new(progress).width(Length::Px(206.0))` → `.width(Length::Percent(1.0))`
   - Set `let show_secondary = create_memo(move || viewport.get().0 >= 1020.0);`

4. **Show the `<` sidebar toggle button only when `Compact` (`< 760px`):**
   Wrap the `<` toggle row inside `right_canvas` with `Show::new(is_compact.signal(), ...)` (where `let is_compact = create_memo(move || breakpoint.get() == Breakpoint::Compact);`), so on full desktop (`1280px`) `"New"` sits cleanly at the top without an extra `<` button above it.

---