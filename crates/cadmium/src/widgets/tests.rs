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

    #[test]
    fn mount_deep_container_tree_without_recursion() {
        let depth = 20_000;
        let mut element = Text::new("leaf").into_element();
        for _ in 0..depth {
            element = Container::new().child(element).into_element();
        }

        let mut arena = WidgetArena::new();
        let mut text = TextEngine::new();
        let root = arena.mount(element, &mut text);

        assert_eq!(arena.root, Some(root));
        assert_eq!(arena.node_count(), depth + 1);
    }

    #[test]
    fn mount_nested_foreach_tree_without_recursion() {
        let depth = 4_000;
        let mut element = Text::new("leaf").into_element();
        for _ in 0..depth {
            let child = Rc::new(RefCell::new(Some(element)));
            element = ForEach::new(
                create_signal(vec![0_u32]),
                |item| *item as u64,
                move |_| child.borrow_mut().take().expect("ForEach child built once"),
            )
            .into_element();
        }

        let mut arena = WidgetArena::new();
        let mut text = TextEngine::new();
        arena.mount(element, &mut text);

        assert_eq!(arena.node_count(), depth + 1);
    }

    #[test]
    fn foreach_reuses_keyed_children_during_reconciliation() {
        let items = create_signal(vec![1_u32, 2]);
        let mut arena = WidgetArena::new();
        let mut text = TextEngine::new();
        let root = arena.mount(
            ForEach::new(
                items,
                |item| *item as u64,
                |item| Text::new(item.to_string()).into_element(),
            )
            .into_element(),
            &mut text,
        );
        let old_children = arena.nodes[root].children.clone();
        arena.dirty_queue.borrow_mut().clear();

        items.set(vec![2, 3]);
        crate::reactive::flush_effects();
        arena.bench_sync_and_layout(300.0, 100.0, &mut text);

        let new_children = &arena.nodes[root].children;
        assert_eq!(new_children[0], old_children[1]);
        assert!(arena.nodes.get(old_children[0]).is_none());
        assert_eq!(arena.node_count(), 3);
    }
}
