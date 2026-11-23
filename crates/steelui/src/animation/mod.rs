#[derive(Debug, Clone, Copy)]
pub struct Spring {
    pub value: f32,
    pub target: f32,
    pub velocity: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl Spring {
    pub fn new(initial: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            value: initial,
            target: initial,
            velocity: 0.0,
            stiffness,
            damping,
        }
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

        if (self.target - self.value).abs() < 0.08 && self.velocity.abs() < 0.25 {
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
}