/// Convenience helper to launch a single-window Cadmium application in one call.
pub fn run<F, E>(
    title: impl Into<String>,
    width: f64,
    height: f64,
    mut build_fn: F,
) -> Result<(), CadmiumError>
where
    F: FnMut() -> E + 'static,
    E: crate::widgets::IntoElement,
{
    let light = crate::style::MacTheme::light();
    let delegate = crate::widgets::WidgetWindow::new(move || build_fn().into_element());
    App::new()
        .add_window(WindowSpec {
            title: title.into(),
            width,
            height,
            min_width: None,
            min_height: None,
            borderless: false,
            clear_color: light.window_bg,
            delegate: Box::new(delegate),
        })
        .run()
}
