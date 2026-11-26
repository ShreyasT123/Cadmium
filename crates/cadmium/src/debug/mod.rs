//! Frame timings and profiling for Cadmium's render loop.
//!
//! [`FrameProfiler`] records two kinds of information each frame: CPU timings
//! passed to [`FrameProfiler::record_frame`], and the wall-clock interval
//! between presented frames passed to [`FrameProfiler::record_cadence`]. The
//! resulting [`FrameMetrics`] are exposed to widgets through
//! [`crate::widgets::WidgetWindow::profiler`] and can be rendered as an overlay
//! or used for adaptive behavior.
//!
//! Two different frame rates are reported. [`FrameMetrics::fps`] is derived only
//! from measured render execution time over a rolling window, while
//! [`FrameMetrics::effective_fps`] is derived from the actual cadence requested
//! by the event loop and drops to zero when frames are sparse or idle. Use
//! `effective_fps` to tell continuous animation from a few reaction frames.
//!
//! # Examples
//!
//! ```rust
//! use cadmium::debug::{FrameMetrics, FrameProfiler};
//!
//! let mut profiler = FrameProfiler::new(60);
//! let start = FrameProfiler::begin_timer();
//! // ... render the frame ...
//! let frame_ms = start.elapsed().as_secs_f32() * 1000.0;
//! profiler.record_frame(frame_ms, 0.4, 1.2, false);
//! profiler.record_cadence(1.0 / 60.0);
//! let metrics: &FrameMetrics = &profiler.metrics;
//! assert_eq!(metrics.repaints, 1);
//! ```

use std::collections::VecDeque;
use std::time::Instant;

include!("metrics.rs");
include!("tests.rs");
