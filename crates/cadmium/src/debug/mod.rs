use std::collections::VecDeque;
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
pub struct FrameMetrics {
    pub frame_ms: f32,
    pub layout_ms: f32,
    pub paint_ms: f32,
    /// Frames-per-second based purely on frame execution time (how fast
    /// `render()` runs), regardless of whether the loop was idle.
    pub fps: f32,
    /// Frames-per-second based on the wall-clock cadence between consecutive
    /// frames that the event loop actually requested (i.e. true throughput).
    /// For interactive apps this equals `fps` during animation and is 0 / idle
    /// when no work is pending.
    pub effective_fps: f32,
    pub relayouts: u64,
    pub repaints: u64,
}

pub struct FrameProfiler {
    /// Rolling window of frame execution times (ms).
    samples: VecDeque<f32>,
    /// Rolling window of wall-clock inter-frame intervals (s), used only for
    /// effective_fps.  We keep a *separate* counter for the "true FPS" path so
    /// that keystroke frames (dt ≈ 0) are recorded without the cadence filter
    /// dropping them.
    cadence_samples: VecDeque<f32>,
    max_samples: usize,
    pub metrics: FrameMetrics,
}

impl FrameProfiler {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            cadence_samples: VecDeque::with_capacity(max_samples),
            max_samples: max_samples.max(1),
            metrics: FrameMetrics::default(),
        }
    }

    pub fn begin_timer() -> Instant {
        Instant::now()
    }

    /// Record the wall-clock interval between two consecutive presents.
    ///
    /// Intervals >= 0.045 s (< ~22 FPS) represent idle gaps between discrete
    /// user inputs or caret blinks rather than continuous rendering, so they
    /// clear the cadence window (`effective_fps = 0.0`).  Intervals <= 0.001 s
    /// (duplicate scheduler wakeups) are ignored so `effective_fps` never
    /// spikes to thousands of FPS.
    pub fn record_cadence(&mut self, frame_dt_secs: f32) {
        if frame_dt_secs >= 0.045 {
            self.cadence_samples.clear();
            self.metrics.effective_fps = 0.0;
            return;
        }
        if frame_dt_secs > 0.001 {
            if self.cadence_samples.len() == self.max_samples {
                self.cadence_samples.pop_front();
            }
            self.cadence_samples.push_back(frame_dt_secs);
            let avg_dt =
                self.cadence_samples.iter().sum::<f32>() / self.cadence_samples.len() as f32;
            self.metrics.effective_fps = if avg_dt > 0.0 { 1.0 / avg_dt } else { 0.0 };
        }
    }

    pub fn record_frame(&mut self, frame_ms: f32, layout_ms: f32, paint_ms: f32, did_layout: bool) {
        if self.samples.len() == self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(frame_ms.max(0.001));
        let avg_ms = self.samples.iter().sum::<f32>() / self.samples.len() as f32;

        self.metrics.frame_ms = frame_ms;
        self.metrics.layout_ms = layout_ms;
        self.metrics.paint_ms = paint_ms;
        self.metrics.fps = if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 };
        if did_layout {
            self.metrics.relayouts += 1;
        }
        self.metrics.repaints += 1;
    }
}

impl Default for FrameProfiler {
    fn default() -> Self {
        Self::new(60)
    }
}

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
        assert!((p.metrics.effective_fps - 125.0).abs() < 1.0, "got {}", p.metrics.effective_fps);

        // An idle gap (>= 45 ms, e.g. caret blink or typing pause) clears cadence.
        p.record_cadence(0.530);
        assert_eq!(p.metrics.effective_fps, 0.0);
    }
}
