use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cadmium::{
    AlignItems, App, Button, CircularProgressIndicator, Clipboard, Color, Column, CommandItem,
    CommandPalette, Container, DockArea, DockGroup, DockNode, DockTab, Dropdown, FontWeight,
    GridView, IntoElement, Length, MacTheme, Material, MenuAction, MenuBar, MenuGroup, Row,
    ShaderCanvas, Spacer, Stack, Switch, Table, TableColumn, TableRow, Text, TextArea, TextInput,
    TextTone, ThemeMode, ToastHost, ToastItem, ToastLevel, Toolbar, TrafficLights, TreeNode,
    TreeView, VirtualRow, VirtualizedList, WidgetWindow, WindowSpec, create_signal,
};

const OPS_WAVEFORM_WGSL: &str = r#"
@group(0) @binding(0)
var<uniform> u: vec4<f32>; // x: time, y: live_toggle, z: w, w: h

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = u.x * mix(0.0, 2.2, u.y);

    // Subtle dark grid
    let gx = abs(fract(uv.x * 12.0) - 0.5);
    let gy = abs(fract(uv.y * 6.0) - 0.5);
    let grid = (1.0 - smoothstep(0.0, 0.03, min(gx, gy))) * 0.08;

    let bg = vec3<f32>(0.07, 0.08, 0.11) + vec3<f32>(grid);

    // Live ops/sec curve
    let wave1 = 0.50
        + 0.22 * sin(uv.x * 9.0 - t * 1.8)
        + 0.11 * cos(uv.x * 21.0 + t * 2.7);
    let d1 = abs((1.0 - uv.y) - wave1);
    let line1 = exp(-d1 * 55.0);
    let fill1 = smoothstep(wave1, wave1 - 0.25, 1.0 - uv.y) * 0.22;

    // P99 latency curve
    let wave2 = 0.24 + 0.08 * sin(uv.x * 14.0 - t * 1.1);
    let d2 = abs((1.0 - uv.y) - wave2);
    let line2 = exp(-d2 * 65.0);

    let cyan = vec3<f32>(0.18, 0.78, 0.98);
    let emerald = vec3<f32>(0.22, 0.88, 0.56);
    let col = bg + cyan * (line1 + fill1) + emerald * line2 * 0.85;
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
"#;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,wgpu=warn")
        .init();

    let next_toast_id = std::sync::Arc::new(AtomicU64::new(1));

    let app = App::new();
    let dispatcher = app.dispatcher();

    let theme_mode = create_signal(ThemeMode::Dark);
    let active_menu = create_signal(None::<usize>);
    let palette_open = create_signal(false);
    let palette_query = create_signal(String::new());
    let key_filter = create_signal(String::new());
    let selected_key = create_signal("users:profile:10042".to_string());
    let selected_db = create_signal(0_usize);
    let selected_server = create_signal(0_usize);
    let active_center_tab = create_signal(0_usize);
    let active_hash_row = create_signal(0_usize);
    let selected_bulk_row = create_signal(0_usize);
    let live_metrics = create_signal(false);
    let syncing = create_signal(false);
    let left_split = create_signal(0.24_f32);
    let bottom_split = create_signal(0.68_f32);

    let json_editor_val = create_signal(
        "{\n  \"user_id\": 10042,\n  \"handle\": \"@shreyas\",\n  \"plan\": \"enterprise\",\n  \"region\": \"ap-south-1\",\n  \"active_sessions\": 4,\n  \"flags\": [\"gpu_native\", \"liquid_glass\", \"zero_idle\"]\n}".to_string(),
    );
    let cli_input = create_signal("HGETALL users:profile:10042".to_string());
    let cli_output = create_signal(
        "127.0.0.1:6379[0]> PING\nPONG (0.14 ms)\n127.0.0.1:6379[0]> DBSIZE\n(integer) 100000"
            .to_string(),
    );

    let toasts = create_signal(vec![
        ToastItem::new(0, "Connected to prod-redis-01", ToastLevel::Success)
            .message("TLS 1.3 • 100,000 keys indexed in 4.2 ms"),
    ]);

    let push_toast = {
        let counter = next_toast_id.clone();
        move |title: &str, msg: &str, level: ToastLevel| {
            let id = counter.fetch_add(1, Ordering::Relaxed);
            toasts.update(|list| {
                if list.len() >= 3 {
                    list.remove(0);
                }
                list.push(ToastItem::new(id, title, level).message(msg));
            });
        }
    };

    let window = WidgetWindow::new(move || {
        let th = MacTheme::dark();

        // 1. Top Desktop MenuBar + Window Controls
        let menu_bar = MenuBar::new(
            active_menu,
            vec![
                MenuGroup::new(
                    "Connection",
                    vec![
                        MenuAction::new("Test Connection", {
                            let pt = push_toast.clone();
                            move || pt("PING -> PONG", "Latency: 0.12 ms", ToastLevel::Success)
                        })
                        .shortcut("Ctrl+T"),
                        MenuAction::new("Sync 100k Keyspace", {
                            let disp = dispatcher.clone();
                            let pt = push_toast.clone();
                            move || {
                                syncing.set(true);
                                let d = disp.clone();
                                let pt2 = pt.clone();
                                std::thread::spawn(move || {
                                    std::thread::sleep(Duration::from_millis(700));
                                    d.dispatch(move || {
                                        syncing.set(false);
                                        pt2(
                                            "Keyspace Synced",
                                            "100,000 keys verified",
                                            ToastLevel::Info,
                                        );
                                    });
                                });
                            }
                        })
                        .shortcut("Ctrl+R"),
                        MenuAction::separator(),
                        MenuAction::new("Open Command Palette", move || palette_open.set(true))
                            .shortcut("Ctrl+K"),
                    ],
                ),
                MenuGroup::new(
                    "View",
                    vec![
                        MenuAction::new("Key Inspector", move || active_center_tab.set(0)),
                        MenuAction::new("Cluster Metrics", move || active_center_tab.set(1)),
                        MenuAction::new("100k Virtual Browser", move || active_center_tab.set(2)),
                        MenuAction::separator(),
                        MenuAction::new("Toggle Live Telemetry", move || {
                            live_metrics.update(|v| *v = !*v);
                        }),
                    ],
                ),
            ],
        );

        // 2. Workstation Toolbar
        let toolbar = Toolbar::new()
            .child(TrafficLights::new())
            .child(
                Text::new("Cadmium Zedis")
                    .size(13.5)
                    .weight(FontWeight::Bold)
                    .tone(TextTone::Primary),
            )
            .child(
                Dropdown::new(
                    selected_server,
                    &[
                        "prod-redis-01 (6379)",
                        "staging-cluster (6380)",
                        "local-wsl (6379)",
                    ],
                )
                .width(Length::Px(190.0)),
            )
            .child(
                Dropdown::new(selected_db, &["db0 (100k)", "db1 (4.2k)", "db2 (0)"])
                    .width(Length::Px(110.0)),
            )
            .child(CircularProgressIndicator::spinner(18.0, syncing))
            .child(Spacer::flex())
            .child(
                Button::new("Command Palette (Ctrl+K)")
                    .glass()
                    .on_click(move || palette_open.set(true)),
            )
            .child(Button::new("Theme").ghost().on_click(move || {
                theme_mode.update(|m| {
                    *m = match *m {
                        ThemeMode::Dark => ThemeMode::Light,
                        ThemeMode::Light => ThemeMode::Dark,
                    };
                });
            }));

        // 3. Left Sidebar: Prefix TreeView
        let key_tree = TreeView::new(vec![
            TreeNode::new("ns:users", "users:* (24,100)").children([
                TreeNode::new("users:profile:10042", "users:profile:10042 [HASH]"),
                TreeNode::new("users:profile:10043", "users:profile:10043 [HASH]"),
                TreeNode::new("users:roles:admin", "users:roles:admin [SET]"),
            ]),
            TreeNode::new("ns:sessions", "sessions:* (48,200)").children([
                TreeNode::new("sessions:tok:8f91a", "sessions:tok:8f91a [STR]"),
                TreeNode::new("sessions:tok:4c22b", "sessions:tok:4c22b [STR]"),
            ]),
            TreeNode::new("ns:metrics", "metrics:* (27,700)").children([
                TreeNode::new("metrics:p99:api", "metrics:p99:api [ZSET]"),
                TreeNode::new("metrics:streams:events", "metrics:streams:events [STREAM]"),
            ]),
        ])
        .selected(selected_key);

        let left_pane = Container::column()
            .flex_grow(1.0)
            .p(10.0)
            .gap(8.0)
            .material(Material::Frosted {
                tint: th.sidebar_glass_tint,
                frost: 0.45,
                specular: 0.75,
                border_color: th.hairline,
                border_width: 1.0,
                shadow: None,
            })
            .radius(10.0)
            .child(
                TextInput::new(key_filter)
                    .placeholder("Filter prefix (e.g. users:*)")
                    .width(Length::Percent(1.0)),
            )
            .child(key_tree);

        // 4A. Center Tab 0: Key Inspector (JSON TextArea + Hash Field Table)
        let inspector_tab = Column::new()
            .flex_grow(1.0)
            .p(14.0)
            .gap(12.0)
            .child(
                Row::new()
                    .align(AlignItems::Center)
                    .gap(10.0)
                    .child(
                        Text::reactive(move || format!("Key: {}", selected_key.get()))
                            .size(15.0)
                            .weight(FontWeight::Bold)
                            .tone(TextTone::Primary),
                    )
                    .child(Spacer::flex())
                    .child(
                        Text::new("TTL: 3592s • 1.4 KB")
                            .size(12.0)
                            .tone(TextTone::Secondary),
                    )
                    .child(Button::new("Save Value").primary().on_click({
                        let pt = push_toast.clone();
                        move || {
                            pt("SET Committed", "Updated key in Redis", ToastLevel::Success);
                        }
                    }))
                    .child(Button::new("Copy JSON").on_click({
                        let pt = push_toast.clone();
                        move || {
                            let _ = Clipboard::set_text(&json_editor_val.get());
                            pt("Copied", "JSON copied to clipboard", ToastLevel::Info);
                        }
                    })),
            )
            .child(TextArea::new(json_editor_val).height(Length::Px(170.0)))
            .child(
                Text::new("Hash Fields (HGETALL)")
                    .size(12.5)
                    .weight(FontWeight::Semibold)
                    .tone(TextTone::Secondary),
            )
            .child(
                Table::new(
                    vec![
                        TableColumn::new("Field", Length::Px(180.0)),
                        TableColumn::new("Value", Length::Px(320.0)),
                        TableColumn::new("Type", Length::Px(90.0)),
                    ],
                    vec![
                        TableRow::new("r1", vec!["user_id", "10042", "int"]),
                        TableRow::new("r2", vec!["handle", "@shreyas", "string"]),
                        TableRow::new("r3", vec!["plan", "enterprise", "string"]),
                        TableRow::new("r4", vec!["region", "ap-south-1", "string"]),
                        TableRow::new("r5", vec!["active_sessions", "4", "int"]),
                    ],
                )
                .selected(active_hash_row),
            );

        // 4B. Center Tab 1: Live Cluster Metrics (GridView + 8-line ShaderCanvas)
        let stat_card = |label: &'static str, val: &'static str| {
            Container::column()
                .p(14.0)
                .gap(4.0)
                .radius(10.0)
                .fill(Color::from_rgba8(38, 38, 44, 210))
                .border(th.hairline, 1.0)
                .child(Text::new(label).size(11.5).tone(TextTone::Secondary))
                .child(
                    Text::new(val)
                        .size(20.0)
                        .weight(FontWeight::Bold)
                        .tone(TextTone::Primary),
                )
        };

        let metrics_tab = Column::new()
            .flex_grow(1.0)
            .p(14.0)
            .gap(12.0)
            .child(
                Row::new()
                    .align(AlignItems::Center)
                    .child(
                        Text::new("Live Ops/sec & Latency Telemetry")
                            .size(15.0)
                            .weight(FontWeight::Bold)
                            .tone(TextTone::Primary),
                    )
                    .child(Spacer::flex())
                    .child(
                        Text::new("Animate Waveform")
                            .size(12.0)
                            .tone(TextTone::Secondary),
                    )
                    .child(Switch::new(live_metrics)),
            )
            .child(
                ShaderCanvas::new(OPS_WAVEFORM_WGSL, move |time, w, h| {
                    [
                        time,
                        if live_metrics.try_get().unwrap_or(false) {
                            1.0
                        } else {
                            0.0
                        },
                        w as f32,
                        h as f32,
                    ]
                })
                .height(Length::Px(190.0))
                .radius(12.0)
                .animate_when(live_metrics),
            )
            .child(
                GridView::new(4)
                    .gap(10.0)
                    .child(stat_card("Used Memory", "1.84 GB"))
                    .child(stat_card("Ops / sec", "142,890"))
                    .child(stat_card("Hit Rate", "99.42%"))
                    .child(stat_card("Clients", "384")),
            );

        // 4C. Center Tab 2: 100,000-Key Virtualized Keyspace
        let bulk_tab = Column::new()
            .flex_grow(1.0)
            .p(14.0)
            .gap(8.0)
            .child(
                Text::new("100,000 Keys (O(visible) Virtualized Browser)")
                    .size(14.0)
                    .weight(FontWeight::Bold)
                    .tone(TextTone::Primary),
            )
            .child(
                VirtualizedList::new(100_000, 44.0, |idx| VirtualRow {
                    title: format!("cache:session:user:{idx:06}"),
                    subtitle: "TTL 3600s • 512 bytes".into(),
                    badge: if idx % 3 == 0 { "HASH" } else { "STR" }.into(),
                })
                .selected(selected_bulk_row),
            );

        // 5. Bottom CLI Console Dock Pane
        let cli_pane = Column::new()
            .flex_grow(1.0)
            .p(10.0)
            .gap(8.0)
            .child(
                Row::new()
                    .gap(8.0)
                    .child(
                        TextInput::new(cli_input)
                            .placeholder("Enter Redis command...")
                            .width(Length::Percent(0.85)),
                    )
                    .child(Button::new("Execute").primary().on_click(move || {
                        let cmd = cli_input.get();
                        cli_output.update(|out| {
                            out.push_str(&format!("\n127.0.0.1:6379[0]> {cmd}\nOK (0.11 ms)"));
                        });
                    })),
            )
            .child(TextArea::new(cli_output).height(Length::Px(120.0)));

        let right_dock = DockNode::split_vertical(
            bottom_split,
            DockNode::pane(DockGroup::new(
                active_center_tab,
                vec![
                    DockTab::new("inspector", "Key Inspector", inspector_tab).closable(false),
                    DockTab::new("metrics", "Cluster Metrics", metrics_tab).closable(false),
                    DockTab::new("bulk", "100k Keys", bulk_tab).closable(false),
                ],
            )),
            DockNode::pane(DockGroup::new(
                create_signal(0_usize),
                vec![DockTab::new("cli", "Redis CLI Console", cli_pane).closable(false)],
            )),
        );

        let dock_area = DockArea::new(DockNode::split_horizontal(
            left_split,
            DockNode::pane(DockGroup::new(
                create_signal(0_usize),
                vec![DockTab::new("keys", "Keyspace Tree", left_pane).closable(false)],
            )),
            right_dock,
        ));

        // 6. Command Palette + ToastHost Overlays
        let cmd_palette = CommandPalette::new(
            palette_open,
            palette_query,
            vec![
                CommandItem::new("Switch to Key Inspector", move || active_center_tab.set(0))
                    .shortcut("Ctrl+1"),
                CommandItem::new("Switch to Cluster Metrics", move || {
                    active_center_tab.set(1)
                })
                .shortcut("Ctrl+2"),
                CommandItem::new("Open 100k Keyspace Browser", move || {
                    active_center_tab.set(2)
                })
                .shortcut("Ctrl+3"),
                CommandItem::new("Toggle Live Telemetry Waveform", move || {
                    live_metrics.update(|v| *v = !*v)
                }),
                CommandItem::new("Send Test Toast Notification", {
                    let pt = push_toast.clone();
                    move || {
                        pt(
                            "Zedis Command",
                            "Executed from CommandPalette",
                            ToastLevel::Info,
                        )
                    }
                }),
            ],
        );

        Stack::new()
            .flex_grow(1.0)
            .fill(th.window_bg)
            .child(
                Column::new()
                    .w_full()
                    .h_full()
                    .child(menu_bar)
                    .child(toolbar)
                    .child(dock_area),
            )
            .child(cmd_palette)
            .child(ToastHost::new(toasts))
            .into_element()
    })
    .with_theme(theme_mode)
    .show_fps(true);

    let app = app.add_window(WindowSpec {
        title: "Cadmium Zedis — Redis Workstation".into(),
        width: 1320.0,
        height: 840.0,
        min_width: Some(640.0),
        min_height: Some(480.0),
        borderless: true,
        clear_color: MacTheme::dark().window_bg,
        delegate: Box::new(window),
    });

    if let Err(err) = app.run() {
        eprintln!("Cadmium Zedis error: {err}");
    }
}
