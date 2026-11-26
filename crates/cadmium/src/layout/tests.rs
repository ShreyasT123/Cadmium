#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taffy_row_layout_and_pixel_snap() {
        let mut eng = LayoutEngine::new();
        let a = eng.new_leaf(LayoutStyle {
            width: Length::Px(100.0),
            height: Length::Px(40.0),
            ..Default::default()
        });
        let b = eng.new_leaf(LayoutStyle {
            flex_grow: 1.0,
            height: Length::Px(40.0),
            ..Default::default()
        });
        let root = eng.new_container(
            LayoutStyle {
                direction: FlexDir::Row,
                width: Length::Px(400.0),
                height: Length::Px(40.0),
                gap: 10.0,
                ..Default::default()
            },
            &[a, b],
        );

        eng.compute_with_measure(root, 400.0, 40.0, |_, _, _, _, _| (0.0, 0.0));
        let rects = eng.collect_logical_rects(root);
        assert_eq!(rects[&a], Rect::new(0.0, 0.0, 100.0, 40.0));
        assert_eq!(rects[&b], Rect::new(110.0, 0.0, 290.0, 40.0));

        let snapped = snap_rect_to_pixels(Rect::new(10.3, 10.3, 99.8, 20.0), 1.5);
        assert_eq!(snapped.x, 15.0);
        assert_eq!(snapped.width, 150.0);
    }
}
