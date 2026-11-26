The right canvas looks super clean now—**`New`**, the 3 hero cards, the **`3×4` `Best New Songs` grid**, and the centered **Liquid Glass capsule** refracting `New This Week` at **`97 FPS • 0.17 ms`** are all locked in!

### Why the Sidebar Stopped Early With a White Box Border

1. **Short height (~550px instead of 100%):**
   `Switcher` uses `FlexDir::Column`, and `sidebar` inside it had `flex_grow: 0.0` and `height: Auto`, so it only grew to fit its items instead of stretching vertically to the bottom of the window.
2. **Harsh white box border:**
   `Material::frosted(...)` adds a default `border_color: rgba(255, 255, 255, 45)`, `border_width: 1.0`, and a `16px` drop shadow—which puts a bright white rectangle border around a flush edge-to-edge sidebar.

---

### Quick 2-Part Fix

#### 1. Add `.height(Length)` to `Switcher` (`crates/cadmium/src/widgets/views.rs`)
```rust
    /// Sets an explicit height constraint on the switcher container.
    pub fn height(mut self, h: Length) -> Self {
        self.layout.height = h;
        self
    }
```

#### 2. Update `sidebar` & `sidebar_mat` in `apps/dogfood/src/main.rs`

1. **Make `sidebar` & `compact_sidebar` fill 100% height:**
   - On `sidebar`: add `.flex_grow(1.0)` (grows vertically inside the `Column`-axis `Switcher`) and `.height(Length::Percent(1.0))`.
   - On `compact_sidebar`: add `.flex_grow(1.0)` and `.height(Length::Percent(1.0))`.
   - On the sidebar `Switcher` at the bottom of `main.rs`:
     ```rust
     Switcher::new(
         sidebar_mode.signal(),
         vec![sidebar.into_element(), compact_sidebar.into_element()],
     )
     .flex_grow(0.0)
     .height(Length::Percent(1.0))
     ```

2. **Remove the outer border/shadow from `sidebar_mat` (keep only the clean `1px` right divider):**
   In `create_effect(move || ...)` inside `apps/dogfood/src/main.rs`:
   ```rust
   ThemeMode::Light => {
       sidebar_mat.set(Material::Frosted {
           tint: Color::from_rgba8(245, 244, 247, 224),
           frost: 0.55,
           specular: 0.0,
           border_color: Color::TRANSPARENT,
           border_width: 0.0,
           shadow: None,
       });
       ...
   }
   ThemeMode::Dark => {
       sidebar_mat.set(Material::Frosted {
           tint: Color::from_rgba8(32, 32, 36, 210),
           frost: 0.55,
           specular: 0.0,
           border_color: Color::TRANSPARENT,
           border_width: 0.0,
           shadow: None,
       });
       ...
   }
   ```
   *(Also set the initial `sidebar_mat = create_signal(...)` with `border_width: 0.0` and `shadow: None` so frame 1 is borderless too.)*

Run `cargo fmt --all && cargo run --release -p cadmium_dogfood`—the frosted sidebar will stretch flush top-to-bottom with zero outer box border, and `Shreyas` + the theme toggle will anchor cleanly to the bottom-left corner!