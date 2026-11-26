#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_edges() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(110.0, 70.0));
        assert!(!r.contains(9.9, 20.0));
        assert!(!r.contains(50.0, 71.0));
    }

    #[test]
    fn srgb_conversion_preserves_black_and_white() {
        let black = Color::from_rgba8(0, 0, 0, 255);
        assert_eq!(black.r, 0.0);
        assert_eq!(black.a, 1.0);
        let white = Color::from_rgba8(255, 255, 255, 255);
        assert!((white.r - 1.0).abs() < 1e-5);
    }
}
