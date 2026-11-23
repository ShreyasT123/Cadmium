use accesskit::{Node, NodeId, Role, TreeId, TreeInfo, TreeUpdate};
use slotmap::Key;

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
        }
    }
}

pub struct A11yNodeRecord {
    pub id: WidgetId,
    pub role: A11yRole,
    pub label: Option<String>,
    pub bounds: Rect,
    pub children: Vec<WidgetId>,
}

pub fn widget_to_a11y_id(id: WidgetId) -> NodeId {
    NodeId(id.data().as_ffi())
}

pub fn build_tree_update(
    root: WidgetId,
    focus: Option<WidgetId>,
    records: &[A11yNodeRecord],
) -> TreeUpdate {
    let root_nid = widget_to_a11y_id(root);
    let focus_nid = focus.map(widget_to_a11y_id).unwrap_or(root_nid);

    let nodes = records
        .iter()
        .map(|rec| {
            let mut node = Node::new(rec.role.into());
            if let Some(lbl) = &rec.label {
                node.set_label(lbl.as_str());
            }
            node.set_bounds(accesskit::Rect::new(
                rec.bounds.x as f64,
                rec.bounds.y as f64,
                (rec.bounds.x + rec.bounds.width) as f64,
                (rec.bounds.y + rec.bounds.height) as f64,
            ));
            if !rec.children.is_empty() {
                let child_ids: Vec<NodeId> =
                    rec.children.iter().copied().map(widget_to_a11y_id).collect();
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
