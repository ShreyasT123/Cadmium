use std::collections::VecDeque;
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
pub struct FrameMetrics {
    pub frame_ms: f32,
    pub layout_ms: f32,
    pub paint_ms: f32,
    pub fps: f32,
    pub effective_fps: f32,
    pub relayouts: u64,
    pub repaints: u64,
}

pub struct FrameProfiler {
    samples: VecDeque<f32>,
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

    pub fn record_cadence(&mut self, frame_dt_secs: f32) {
        if frame_dt_secs > 0.0005 && frame_dt_secs < 0.20 {
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
}
