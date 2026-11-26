#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiler_computes_fps_and_counts() {
        let mut p = FrameProfiler::new(4);
        p.record_frame(16.0, 0.5, 1.0, true);
        p.record_frame(16.0, 0.0, 0.8, false);
        assert_eq!(p.metrics.relayouts, 1);
        assert_eq!(p.metrics.repaints, 2);
        assert!((p.metrics.fps - 62.5).abs() < 0.1);
    }

    #[test]
    fn cadence_records_active_frames_and_clears_on_idle_gap() {
        let mut p = FrameProfiler::new(10);
        for _ in 0..10 {
            p.record_cadence(0.008);
        }
        assert!(
            (p.metrics.effective_fps - 125.0).abs() < 1.0,
            "got {}",
            p.metrics.effective_fps
        );

        // An idle gap (>= 45 ms, e.g. caret blink or typing pause) clears cadence.
        p.record_cadence(0.530);
        assert_eq!(p.metrics.effective_fps, 0.0);
    }
}
