//! Spring-physics animation for continuously changing scalar values.
//!
//! A [`Spring`] holds a `value` that is pulled toward `target` by a force
//! proportional to their distance (`stiffness`), opposed by a force proportional
//! to `velocity` (`damping`). Each call to [`Spring::step`] integrates the
//! motion for a time delta in **seconds**, using four semi-implicit Euler
//! substeps for stability, and reports whether the spring is still moving.
//! Once both the remaining distance and the velocity are below their tolerances,
//! the spring snaps exactly to its target and stops.
//!
//! Springs are typically advanced from [`crate::window::WindowDelegate::tick`];
//! built-in widgets use them for scroll offsets and tab indicators. The value can
//! represent any caller-chosen scalar, and the result is consumed by widgets in
//! [`crate::widgets`] before drawing through [`crate::renderer`]. `stiffness` and
//! `damping` are coefficients applied per second of simulated time, and `dt` must
//! be in seconds.
//!
//! # Examples
//!
//! ```rust
//! use cadmium::animation::Spring;
//!
//! let mut opacity = Spring::new(0.0, 260.0, 24.0);
//! opacity.set_target(1.0);
//! for _ in 0..240 {
//!     if !opacity.step(1.0 / 60.0) {
//!         break;
//!     }
//! }
//! assert_eq!(opacity.value, 1.0);
//! ```

include!("spring.rs");
include!("tests.rs");
