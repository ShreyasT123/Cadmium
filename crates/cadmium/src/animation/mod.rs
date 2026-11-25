#[derive(Debug, Clone, Copy)]
pub struct Spring {
    pub value: f32,
    pub target: f32,
    pub velocity: f32,
    pub stiffness: f32,
    pub damping: f32,
    pos_eps: f32,
    vel_eps: f32,
}

impl Spring {
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

    pub fn with_tolerance(mut self, pos_eps: f32, vel_eps: f32) -> Self {
        self.pos_eps = pos_eps.max(1e-5);
        self.vel_eps = vel_eps.max(1e-5);
        self
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn snap_to(&mut self, val: f32) {
        self.value = val;
        self.target = val;
        self.velocity = 0.0;
    }

    /// Advances the spring by `dt` seconds. Returns `true` if still animating.
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
