/// A damped scalar spring advancing toward [`Spring::target`] over time.
///
/// See the [module docs](crate::animation) for the integration model and a
/// complete animation loop.
#[derive(Debug, Clone, Copy)]
pub struct Spring {
    /// Current value of the spring, in caller-defined units.
    pub value: f32,
    /// Value the spring is moving toward.
    pub target: f32,
    /// Current rate of change, in value-units per second.
    pub velocity: f32,
    /// Strength of the pull toward [`Spring::target`].
    pub stiffness: f32,
    /// Resistance that removes energy from the motion.
    pub damping: f32,
    pos_eps: f32,
    vel_eps: f32,
}

impl Spring {
    /// Creates a settled spring at `initial` with the given motion coefficients.
    ///
    /// `target` is set to `initial` and `velocity` to zero, so the spring does
    /// not move until [`Self::set_target`] is called. `stiffness` and `damping`
    /// are per-second coefficients: larger stiffness converges faster, while
    /// larger damping suppresses overshoot.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::animation::Spring;
    ///
    /// let spring = Spring::new(0.0, 220.0, 20.0);
    /// assert_eq!(spring.value, 0.0);
    /// assert_eq!(spring.target, 0.0);
    /// assert_eq!(spring.velocity, 0.0);
    /// ```
    pub fn new(initial: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            value: initial,
            target: initial,
            velocity: 0.0,
            stiffness,
            damping,
            pos_eps: 0.002,
            vel_eps: 0.01,
        }
    }

    /// Sets custom settling tolerances and returns the modified spring.
    ///
    /// `pos_eps` is the maximum distance from [`Spring::target`] considered
    /// settled, in the same units as `value`; `vel_eps` is the maximum absolute
    /// velocity. Both are clamped to at least `1e-5` to avoid a spring that can
    /// never satisfy its stopping condition. The defaults are `0.002` and
    /// `0.01`, respectively.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::animation::Spring;
    ///
    /// let spring = Spring::new(0.0, 220.0, 20.0).with_tolerance(0.001, 0.005);
    /// assert_eq!(spring.value, 0.0);
    /// ```
    pub fn with_tolerance(mut self, pos_eps: f32, vel_eps: f32) -> Self {
        self.pos_eps = pos_eps.max(1e-5);
        self.vel_eps = vel_eps.max(1e-5);
        self
    }

    /// Changes the value the spring is animating toward.
    ///
    /// The current position and velocity are preserved, so retargeting mid-flight
    /// produces a smooth reversal. Call [`Self::step`] afterwards to advance.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Instantly moves the spring to `val` with zero velocity.
    ///
    /// Both [`Self::value`] and [`Spring::target`] become `val`, so no animation
    /// occurs until a new target is set.
    pub fn snap_to(&mut self, val: f32) {
        self.value = val;
        self.target = val;
        self.velocity = 0.0;
    }

    /// Advances the spring by `dt` seconds. Returns `true` if still animating.
    ///
    /// Integration is split into four equal semi-implicit Euler substeps. `dt`
    /// should be non-negative and in seconds (for example `1.0 / 60.0`); no
    /// validation is performed. When both the distance to [`Spring::target`] and
    /// the absolute velocity are below the tolerances set by
    /// [`Spring::with_tolerance`], `value` is set exactly to `target`, velocity is
    /// zeroed, and `false` is returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cadmium::animation::Spring;
    ///
    /// let mut spring = Spring::new(0.0, 260.0, 24.0);
    /// spring.set_target(1.0);
    /// assert!(spring.step(1.0 / 60.0));
    /// assert!(spring.value > 0.0);
    ///
    /// while spring.step(1.0 / 60.0) {}
    /// assert_eq!(spring.value, 1.0);
    /// ```
    pub fn step(&mut self, dt: f32) -> bool {
        let substeps = 4;
        let sub_dt = dt / substeps as f32;
        for _ in 0..substeps {
            let force = (self.target - self.value) * self.stiffness;
            let accel = force - self.velocity * self.damping;
            self.velocity += accel * sub_dt;
            self.value += self.velocity * sub_dt;
        }

        if (self.target - self.value).abs() < self.pos_eps && self.velocity.abs() < self.vel_eps {
            self.value = self.target;
            self.velocity = 0.0;
            return false;
        }
        true
    }
}

