/// Converts a [`WidgetId`] into the AccessKit `NodeId` with the same slot-map key bits.
pub(crate) fn widget_to_a11y_id(id: WidgetId) -> NodeId {
    NodeId(id.data().as_ffi())
}

/// Converts an AccessKit `NodeId` back into the [`WidgetId`] it was derived from.
pub(crate) fn a11y_id_to_widget(id: NodeId) -> WidgetId {
    WidgetId::from(KeyData::from_ffi(id.0))
}

/// Translates a click or focus `ActionRequest` into an [`A11yAction`].
///
/// Returns `None` for actions Cadmium does not handle.
pub(crate) fn action_from_request(req: &ActionRequest) -> Option<A11yAction> {
    let target = a11y_id_to_widget(req.target_node);
    match req.action {
        Action::Click => Some(A11yAction::Click(target)),
        Action::Focus => Some(A11yAction::Focus(target)),
        _ => None,
    }
}

/// Builds an AccessKit `TreeUpdate` from a [`SemanticTree`].
///
/// The tree is registered under `TreeId::ROOT`, and focus falls back to the root
/// node when [`SemanticTree::focus`] is `None`.
pub(crate) fn to_accesskit_update(tree: &SemanticTree) -> TreeUpdate {
    let root_nid = widget_to_a11y_id(tree.root);
    let focus_nid = tree.focus.map(widget_to_a11y_id).unwrap_or(root_nid);

    let nodes = tree
        .nodes
        .iter()
        .map(|rec| {
            let mut node = Node::new(rec.role.into());
            if let Some(lbl) = &rec.label {
                node.set_label(lbl.as_str());
            }
            if let Some(val) = &rec.value {
                node.set_value(val.as_str());
            }
            if let Some(num) = rec.numeric_value {
                node.set_numeric_value(num);
            }
            if let Some(t) = rec.toggled {
                node.set_toggled(if t { Toggled::True } else { Toggled::False });
            }
            if rec.focusable {
                node.add_action(Action::Focus);
            }
            if rec.clickable {
                node.add_action(Action::Click);
            }
            node.set_bounds(accesskit::Rect::new(
                rec.bounds.x as f64,
                rec.bounds.y as f64,
                (rec.bounds.x + rec.bounds.width) as f64,
                (rec.bounds.y + rec.bounds.height) as f64,
            ));
            if !rec.children.is_empty() {
                let child_ids: Vec<NodeId> = rec
                    .children
                    .iter()
                    .copied()
                    .map(widget_to_a11y_id)
                    .collect();
                node.set_children(child_ids);
            }
            (widget_to_a11y_id(rec.id), node)
        })
        .collect();

    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(root_nid)),
        tree_id: TreeId::ROOT,
        focus: focus_nid,
    }
}

