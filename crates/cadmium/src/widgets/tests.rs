#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactive::create_signal;

    #[test]
    fn utf8_boundary_snapping_prevents_mid_char_panics() {
        let s = "Café🦀";
        assert_eq!(snap_char_boundary(s, 4), 3);
        assert_eq!(snap_char_boundary(s, 7), 5);
        assert_eq!(snap_char_boundary(s, 99), s.len());
    }

    #[test]
    fn dirty_queues_are_isolated_per_arena() {
        let mut te = TextEngine::new();
        let sig_a = create_signal("A".to_string());
        let sig_b = create_signal("B".to_string());

        let mut arena_a = WidgetArena::new();
        let mut arena_b = WidgetArena::new();
        arena_a.mount(Text::reactive(move || sig_a.get()).into_element(), &mut te);
        arena_b.mount(Text::reactive(move || sig_b.get()).into_element(), &mut te);

        arena_a.dirty_queue.borrow_mut().clear();
        arena_b.dirty_queue.borrow_mut().clear();

        sig_a.set("A2".to_string());
        crate::reactive::flush_effects();

        assert_eq!(arena_a.dirty_queue.borrow().len(), 1);
        assert_eq!(arena_b.dirty_queue.borrow().len(), 0);

        sig_a.dispose();
        sig_b.dispose();
    }
}
