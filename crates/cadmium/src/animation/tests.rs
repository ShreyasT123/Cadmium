#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_settles_at_target() {
        let mut s = Spring::new(0.0, 220.0, 20.0);
        s.set_target(100.0);
        for _ in 0..240 {
            if !s.step(1.0 / 60.0) {
                break;
            }
        }
        assert!((s.value - 100.0).abs() < 0.1);
    }

    #[test]
    fn normalized_spring_does_not_snap_early() {
        let mut s = Spring::new(0.0, 260.0, 24.0);
        s.set_target(1.0);
        // After a single frame it must still be animating, not snapped from 0.05..0.92
        assert!(s.step(1.0 / 60.0));
        assert!(s.value < 0.9);
        for _ in 0..180 {
            s.step(1.0 / 60.0);
        }
        assert_eq!(s.value, 1.0);
    }
}
