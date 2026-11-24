use accesskit::{Action, ActionRequest, Node, NodeId, Role, Toggled, TreeId, TreeInfo, TreeUpdate};
use slotmap::{Key, KeyData};

use crate::core::Rect;
use crate::events::WidgetId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A11yRole {
    Window,
    Container,
    Label,
    Button,
    Switch,
    Slider,
    TextInput,
}

impl From<A11yRole> for Role {
    fn from(r: A11yRole) -> Self {
        match r {
            A11yRole::Window => Role::Window,
            A11yRole::Container => Role::GenericContainer,
            A11yRole::Label => Role::Label,
            A11yRole::Button => Role::Button,
            A11yRole::Switch => Role::Switch,
            A11yRole::Slider => Role::Slider,
            A11yRole::TextInput => Role::TextInput,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A11yAction {
    Click(WidgetId),
    Focus(WidgetId),
}

#[derive(Debug, Clone)]
pub struct A11yNodeRecord {
    pub id: WidgetId,
    pub role: A11yRole,
    pub label: Option<String>,
    pub value: Option<String>,
    pub numeric_value: Option<f64>,
    pub toggled: Option<bool>,
    pub focusable: bool,
    pub clickable: bool,
    pub bounds: Rect,
    pub children: Vec<WidgetId>,
}

#[derive(Debug, Clone)]
pub struct SemanticTree {
    pub root: WidgetId,
    pub focus: Option<WidgetId>,
    pub nodes: Vec<A11yNodeRecord>,
}

pub(crate) fn widget_to_a11y_id(id: WidgetId) -> NodeId {
    NodeId(id.data().as_ffi())
}

pub(crate) fn a11y_id_to_widget(id: NodeId) -> WidgetId {
    WidgetId::from(KeyData::from_ffi(id.0))
}

pub(crate) fn action_from_request(req: &ActionRequest) -> Option<A11yAction> {
    let target = a11y_id_to_widget(req.target_node);
    match req.action {
        Action::Click => Some(A11yAction::Click(target)),
        Action::Focus => Some(A11yAction::Focus(target)),
        _ => None,
    }
}

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
