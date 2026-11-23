use std::time::Duration;

use steelui::{
    AlignItems, App, Button, Clipboard, Color, Column, Container, Dropdown, Edges, FileDialog,
    FontWeight, IntoElement, JustifyContent, Length, MacTheme, Material, Row, ScrollView, Shadow,
    Spacer, Tabs, Text, TextInput, TrafficLights, WidgetWindow, WindowSpec, create_signal,
};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,wgpu=warn")
        .init();

    let app = App::new();
    let dispatcher = app.dispatcher();

    let selected_engine_mode = create_signal(0_usize);
    let engine_modes = ["Classic UI", "Frosted Glass", "Liquid Glass"];
    let search_text = create_signal("Magnum Engine".to_string());
    let active_tab = create_signal(0_usize);
    let radius = create_signal(14.0_f32);
    let async_jobs = create_signal(0_u32);
    let status_msg = create_signal("Ready — type in search box or switch tabs above".to_string());

    let theme = MacTheme::light();

    let window = WidgetWindow::new(move || {
        let search_field = TextInput::new(search_text)
            .placeholder("Search or filter...")
            .width(Length::Px(210.0))
            .material(Material::Classic {
                fill: Color::from_rgba8(255, 255, 255, 225),
                fill_end: None,
                gradient_dir: [0.0, 1.0],
                border_color: theme.hairline,
                border_width: 1.0,
                shadow: Some(Shadow::new(Color::from_rgba8(0, 0, 0, 20), 4.0, 1.0)),
            });

        let view_tabs =
            Tabs::new(active_tab, &["Workspace", "Inspector", "Feed"]).width(Length::Px(260.0));

        let mode_dropdown =
            Dropdown::new(selected_engine_mode, &engine_modes).width(Length::Px(150.0));

        let toolbar = Row::new()
            .height(Length::Px(52.0))
            .padding(Edges::xy(18.0, 0.0))
            .align(AlignItems::Center)
            .gap(14.0)
            .fill(theme.content_bg)
            .border(theme.hairline, 1.0)
            .drag_region()
            .child(TrafficLights::new())
            .child(
                Text::new("SteelUI Studio")
                    .size(14.5)
                    .weight(FontWeight::Semibold)
                    .color(theme.text_primary),
            )
            .child(Spacer::flex())
            .child(view_tabs)
            .child(search_field)
            .child(mode_dropdown)
            .child(
                Text::reactive(move || format!("Jobs: {}", async_jobs.get()))
                    .size(12.5)
                    .weight(FontWeight::Medium)
                    .color(theme.text_secondary),
            );

        let sidebar = Container::column()
            .width(Length::Px(220.0))
            .padding(Edges::all(16.0))
            .gap(10.0)
            .radius(14.0)
            .material(Material::frosted(Color::from_rgba8(245, 245, 247, 175)))
            .child(
                Text::new("SYSTEM CONTROLS")
                    .size(11.0)
                    .weight(FontWeight::Semibold)
                    .color(theme.text_tertiary),
            )
            .child(Button::new("Run Async Task").primary().on_click({
                let disp = dispatcher.clone();
                move || {
                    status_msg.set("Async worker dispatched...".into());
                    let d = disp.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(320));
                        d.dispatch(move || {
                            async_jobs.update(|n| *n += 1);
                            status_msg.set("Background task completed on UI thread".into());
                        });
                    });
                }
            }))
            .child(Button::new("Copy Summary").on_click(move || {
                let text_val = search_text.get();
                let summary = format!(
                    "Query='{text_val}' tab={} radius={:.0} jobs={}",
                    active_tab.get(),
                    radius.get(),
                    async_jobs.get()
                );
                match Clipboard::set_text(&summary) {
                    Ok(()) => status_msg.set("Copied summary to clipboard".into()),
                    Err(e) => status_msg.set(format!("Clipboard error: {e}")),
                }
            }))
            .child(Button::new("Open Asset...").on_click(move || {
                if let Some(path) = FileDialog::pick_file("Select Asset") {
                    status_msg.set(format!("File: {}", path.display()));
                }
            }));

        let preview_card = Container::column()
            .height(Length::Px(120.0))
            .padding(Edges::all(18.0))
            .gap(6.0)
            .justify(JustifyContent::Center)
            .radius(16.0)
            .bind_radius(radius)
            .material(Material::liquid(Color::from_rgba8(255, 255, 255, 60)))
            .child(
                Text::new("Liquid Glass Bezel Preview")
                    .size(16.0)
                    .weight(FontWeight::Bold)
                    .color(theme.text_primary),
            )
            .child(
                Text::reactive(move || {
                    format!(
                        "Query: \"{}\"  •  Tab: {}  •  Radius: {:.0} pt",
                        search_text.get(),
                        active_tab.get(),
                        radius.get()
                    )
                })
                .size(13.0)
                .color(theme.text_secondary),
            );

        let mut feed_scroll = ScrollView::new().gap(8.0);
        for i in 1..=12 {
            let item_card = Container::row()
                .height(Length::Px(42.0))
                .padding(Edges::xy(16.0, 0.0))
                .align(AlignItems::Center)
                .radius(8.0)
                .fill(Color::from_rgba8(255, 255, 255, 210))
                .border(theme.hairline, 1.0)
                .child(
                    Text::new(format!("Feed Item #{i}"))
                        .size(13.0)
                        .weight(FontWeight::Medium)
                        .color(theme.text_primary),
                )
                .child(Spacer::flex())
                .child(
                    Text::new(if i % 2 == 0 { "Verified" } else { "Pending" })
                        .size(12.0)
                        .color(theme.text_secondary),
                );
            feed_scroll = feed_scroll.child(item_card);
        }

        let main_area = Column::new()
            .flex_grow(1.0)
            .gap(12.0)
            .child(preview_card)
            .child(
                Text::new("SCROLLABLE COMPONENT FEED")
                    .size(11.0)
                    .weight(FontWeight::Semibold)
                    .color(theme.text_tertiary),
            )
            .child(feed_scroll)
            .child(
                Text::reactive(move || status_msg.get())
                    .size(12.5)
                    .color(theme.text_secondary),
            );

        let body = Row::new()
            .flex_grow(1.0)
            .padding(Edges::all(16.0))
            .gap(16.0)
            .child(sidebar)
            .child(main_area);

        Column::new()
            .fill(theme.window_bg)
            .child(toolbar)
            .child(body)
            .into_element()
    });

    let app = app.add_window(WindowSpec {
        title: "SteelUI Studio".into(),
        width: 980.0,
        height: 600.0,
        borderless: true,
        clear_color: theme.window_bg,
        delegate: Box::new(window),
    });

    if let Err(err) = app.run() {
        eprintln!("SteelUI Studio exited with error: {err}");
    }
}
