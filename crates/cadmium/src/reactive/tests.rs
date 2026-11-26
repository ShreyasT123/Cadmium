#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn signal_tracks_and_reruns_effects() {
        let count = create_signal(1);
        let observed = Rc::new(Cell::new(0));
        let obs_clone = observed.clone();

        let eff = create_effect(move || {
            obs_clone.set(count.get() * 10);
        });
        assert_eq!(observed.get(), 10);

        count.set(4);
        assert_eq!(observed.get(), 10);
        assert_eq!(flush_effects(), 1);
        assert_eq!(observed.get(), 40);

        dispose_effect(eff);
        count.set(9);
        assert_eq!(flush_effects(), 0);
        assert_eq!(observed.get(), 40);
        count.dispose();
    }

    #[test]
    fn owned_signal_cleans_up_on_drop_and_try_get_is_safe() {
        let handle = {
            let owned = OwnedSignal::new(42);
            let sig = owned.signal();
            assert_eq!(sig.try_get(), Some(42));
            assert!(sig.is_alive());
            sig
        };
        assert!(!handle.is_alive());
        assert_eq!(handle.try_get(), None);
    }

    #[test]
    fn nested_reads_inside_update_and_with_do_not_panic() {
        let a = create_signal(5);
        let b = create_signal(10);
        a.update(|v| *v += b.get());
        assert_eq!(a.get(), 15);

        let sum = a.with(|av| *av + b.get());
        assert_eq!(sum, 25);
        a.dispose();
        b.dispose();
    }

    #[test]
    fn effect_panic_restores_tracking_stack() {
        let trigger = create_signal(0);
        let clean_sig = create_signal(1);
        let observed = Rc::new(Cell::new(0));

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            create_effect(move || {
                let _ = trigger.get();
                panic!("boom inside effect");
            });
        }));
        assert!(res.is_err());

        let obs = observed.clone();
        let eff = create_effect(move || {
            obs.set(clean_sig.get());
        });
        assert_eq!(observed.get(), 1);

        clean_sig.set(7);
        flush_effects();
        assert_eq!(observed.get(), 7);

        dispose_effect(eff);
        trigger.dispose();
        clean_sig.dispose();
    }
}
