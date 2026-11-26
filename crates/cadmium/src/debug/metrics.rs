/// Timing and work counters for the most recent frames.
///
/// All durations are in milliseconds; rates are in frames per second and are
/// zero until enough samples have been recorded.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameMetrics {
    /// Duration of the last complete frame, in milliseconds.
    pub frame_ms: f32,
    /// Time spent in the layout stage of the last frame, in milliseconds.
    pub layout_ms: f32,
    /// Time spent recording paint commands in the last frame, in milliseconds.
    pub paint_ms: f32,
    /// Frames-per-second based purely on frame execution time (how fast
    /// `render()` runs), regardless of whether the loop was idle.
    ///
    /// Computed as `1000 / average(frame_ms)` over the rolling sample window.
    pub fps: f32,
    /// Frames-per-second based on the wall-clock cadence between consecutive
    /// frames that the event loop actually requested (i.e. true throughput).
    /// For interactive apps this equals `fps` during animation and is 0 / idle
    /// when no work is pending.
    pub effective_fps: f32,
    /// Cumulative number of recorded frames that required a layout pass.
    pub relayouts: u64,
    /// Cumulative number of recorded frames, including idle repaints.
    pub repaints: u64,
}

/// Rolling frame profiler that produces [`FrameMetrics`].
///
/// Keeps a bounded window of execution-time and cadence samples. The default
/// window holds 60 frames; [`FrameProfiler::new`] chooses another size.
pub struct FrameProfiler {
    /// Rolling window of frame execution times (ms).
    samples: VecDeque<f32>,
    /// Rolling window of wall-clock inter-frame intervals (s), used only for
    /// effective_fps.  We keep a *separate* counter for the "true FPS" path so
    /// that keystroke frames (dt ≈ 0) are recorded without the cadence filter
    /// dropping them.
    cadence_samples: VecDeque<f32>,
    max_samples: usize,
    /// Latest measured metrics, updated by [`Self::record_frame`] and
    /// [`Self::record_cadence`].
    pub metrics: FrameMetrics,
}

impl FrameProfiler {
    /// Creates an empty profiler with a rolling window of `max_samples` frames.
    ///
    /// `max_samples` is clamped to at least one. Metrics are initialized to
    /// [`FrameMetrics::default`], so all timings and rates start at zero.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::debug::FrameProfiler;
    ///
    /// let profiler = FrameProfiler::new(120);
    /// assert_eq!(profiler.metrics.repaints, 0);
    /// // `new(0)` still keeps the most recent sample.
    /// let profiler = FrameProfiler::new(0);
    /// let _ = &profiler.metrics;
    /// ```
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            cadence_samples: VecDeque::with_capacity(max_samples),
            max_samples: max_samples.max(1),
            metrics: FrameMetrics::default(),
        }
    }

    /// Returns an `Instant` marking the start of a timing region.
    ///
    /// Convert its elapsed time to milliseconds with
    /// `start.elapsed().as_secs_f32() * 1000.0` before passing it to
    /// [`Self::record_frame`]. This is an associated function so code can time
    /// work without owning a profiler.
    pub fn begin_timer() -> Instant {
        Instant::now()
    }

    /// Record the wall-clock interval between two consecutive presents.
    ///
    /// `frame_dt_secs` is in seconds, typically the frame's delta time.
    ///
    /// Intervals >= 0.045 s (< ~22 FPS) represent idle gaps between discrete
    /// user inputs or caret blinks rather than continuous rendering, so they
    /// clear the cadence window (`effective_fps = 0.0`).  Intervals <= 0.001 s
    /// (duplicate scheduler wakeups) are ignored so `effective_fps` never
    /// spikes to thousands of FPS.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::debug::FrameProfiler;
    ///
    /// let mut profiler = FrameProfiler::new(10);
    /// for _ in 0..10 {
    ///     profiler.record_cadence(0.008);
    /// }
    /// assert!((profiler.metrics.effective_fps - 125.0).abs() < 1.0);
    ///
    /// // A pause between user inputs clears the active cadence.
    /// profiler.record_cadence(0.530);
    /// assert_eq!(profiler.metrics.effective_fps, 0.0);
    /// ```
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

    /// Records one completed frame and updates [`Self::metrics`].
    ///
    /// `frame_ms`, `layout_ms`, and `paint_ms` are durations in milliseconds.
    /// `did_layout` should be `true` when a layout pass was recomputed for the
    /// frame; it increments [`FrameMetrics::relayouts`]. Every call increments
    /// [`FrameMetrics::repaints`] and refreshes [`FrameMetrics::fps`] from the
    /// rolling average of `frame_ms`, which is clamped to at least `0.001` ms
    /// for that calculation. The stored `frame_ms` is the value passed in.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::debug::FrameProfiler;
    ///
    /// let mut profiler = FrameProfiler::new(4);
    /// profiler.record_frame(16.0, 0.5, 1.0, true);
    /// profiler.record_frame(16.0, 0.0, 0.8, false);
    /// assert_eq!(profiler.metrics.relayouts, 1);
    /// assert_eq!(profiler.metrics.repaints, 2);
    /// assert!((profiler.metrics.fps - 62.5).abs() < 0.1);
    /// ```
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

/// Creates a profiler with a 60-frame rolling window (see [`FrameProfiler::new`]).
impl Default for FrameProfiler {
    fn default() -> Self {
        Self::new(60)
    }
}

