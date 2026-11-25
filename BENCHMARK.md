# Cadmium Benchmark & Performance Verification Specification

To establish Cadmium as an industry-grade desktop GUI framework on par with or ahead of **GPUI**, **Iced**, **Slint**, **Dioxus Native**, and **Tauri**, all performance claims must be backed by reproducible automated benchmarks in `crates/cadmium/benches/` and `apps/zedis`.

---

## 1. Suite A — Micro-Frame & State Scaling (JS-Framework-Benchmark Port)
Directly comparable to published Rust GUI benchmarks (GPUI / Iced / Slint / Dioxus Native / egui). Measures CPU state + reactive + layout + display-list recording time (`µs` / `ms`, median & p95):

| Case | Description | Target (Median) |
|---|---|---|
| `create_1k` | Mount 1,000 keyed rows into `WidgetArena` + initial layout | `< 4.0 ms` |
| `create_10k` | Mount 10,000 keyed rows + initial layout | `< 45.0 ms` |
| `update_every_10th` | Mutate 1,000 signals in a 10,000-row tree | `< 0.8 ms` |
| `select_row_10k` | Flip selected row signal in a 10,000-row tree (O(change) test) | `< 0.15 ms` |
| `swap_rows_10k` | Swap two keyed rows in 10,000-row tree | `< 0.5 ms` |
| `append_1k_to_10k` | Append 1,000 rows to existing 10,000-row container | `< 8.0 ms` |
| `scroll_frame_100k` | Advance scroll & record visible viewport in 100,000-item `VirtualizedList` | `< 0.5 ms` |

*Why this matters:* GPUI's view-notification model degrades with tree size (`~10ms` on single-row selection in 10k nodes). Cadmium's `Signal` + `DisplayList` caching must hold `select_row_10k` flat regardless of total tree size.

---

## 2. Suite B — Workstation & Glass GPU Benchmarks
Measured via GPU timestamp queries + frame-pacing histograms across **Windows (DX12/Vulkan)**, **macOS (Metal)**, and **Linux (Vulkan)**:

1. **Idle Power Gate:** Focused `TextInput` (between blinks), static Music view, and static Zedis view must emit **`0` GPU command submits** and **`< 0.1%` CPU** over a 10-second idle window.
2. **Liquid Glass Composition Budget (1920×1080 @ 100% & 200% DPI):**
   - Fullscreen photo + 1 Frosted Sidebar + 1 Liquid Glass Now-Playing Bar + scrolling content:
   - CPU frame record `< 1.5 ms`, GPU frame `< 3.0 ms` (`>= 120 FPS` sustained).
3. **Zedis Keyspace Stress Test:**
   - 100,000 keys in `TreeView` + live JSON `TextArea` (10k lines) + `Table` (5k hash fields) + live `lyon` latency chart updating at 20 Hz:
   - `p95` frame latency `< 5.0 ms`, `p99` `< 8.0 ms`, resident RSS `< 120 MB`, zero frame drops over a 5-minute soak.

---

## 3. Suite C — Allocations & Memory Discipline
- Track `#[global_allocator]` allocation count during:
  - Steady-state scroll frames (`VirtualizedList`, `ScrollView`)
  - Single-signal hover / slider scrub (`0` `HashMap` / `Vec` / `TextBlock` allocations in steady state)
  - 1,000 window resize / remount cycles (`0` leaked `GpuContext::textures` or `ReactiveRuntime` slots)