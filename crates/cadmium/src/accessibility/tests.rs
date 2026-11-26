#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::SlotMap;

    #[test]
    fn semantic_tree_roundtrips_ids_and_actions() {
        let mut sm: SlotMap<WidgetId, ()> = SlotMap::with_key();
        let root = sm.insert(());
        let btn = sm.insert(());

        let st = SemanticTree {
            root,
            focus: Some(btn),
            nodes: vec![
                A11yNodeRecord {
                    id: root,
                    role: A11yRole::Window,
                    label: Some("Main".into()),
                    value: None,
                    numeric_value: None,
                    toggled: None,
                    focusable: false,
                    clickable: false,
                    bounds: Rect::new(0.0, 0.0, 800.0, 600.0),
                    children: vec![btn],
                },
                A11yNodeRecord {
                    id: btn,
                    role: A11yRole::Button,
                    label: Some("Run".into()),
                    value: None,
                    numeric_value: None,
                    toggled: None,
                    focusable: true,
                    clickable: true,
                    bounds: Rect::new(20.0, 20.0, 100.0, 30.0),
                    children: vec![],
                },
            ],
        };

        let update = to_accesskit_update(&st);
        assert_eq!(update.nodes.len(), 2);
        assert_eq!(update.focus, widget_to_a11y_id(btn));

        let req = ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: widget_to_a11y_id(btn),
            data: None,
        };
        assert_eq!(action_from_request(&req), Some(A11yAction::Click(btn)));
    }
}
