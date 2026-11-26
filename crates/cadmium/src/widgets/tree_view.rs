/// Represents a hierarchical node in a [`TreeView`].
///
/// Supports recursive child nodes, collapse/expand states, and optional badge labels.
#[derive(Clone, Debug)]
pub struct TreeNode {
    /// Unique key identifier for the tree node.
    pub key: String,
    /// Display label text.
    pub label: String,
    /// Optional badge tag displayed on the node row.
    pub badge: Option<String>,
    /// Nested child nodes.
    pub children: Vec<TreeNode>,
    /// Whether the node is currently expanded to display its children.
    pub expanded: bool,
}

impl TreeNode {
    /// Creates a new tree node with a unique key and display label.
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            badge: None,
            children: Vec::new(),
            expanded: true,
        }
    }

    /// Attaches an optional badge tag.
    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Appends a single child node.
    pub fn child(mut self, node: TreeNode) -> Self {
        self.children.push(node);
        self
    }

    /// Appends multiple child nodes.
    pub fn children(mut self, nodes: impl IntoIterator<Item = TreeNode>) -> Self {
        self.children.extend(nodes);
        self
    }
}

/// A hierarchical tree navigation component.
///
/// Displays nested [`TreeNode`] items with expandable/collapsible collapse
/// arrows, indentation guides, and reactive selection binding.
pub struct TreeView {
    nodes: Vec<TreeNode>,
    selected_key: Option<Signal<String>>,
    indent: f32,
}

impl TreeView {
    /// Creates a new tree view from a vector of root tree nodes.
    pub fn new(nodes: Vec<TreeNode>) -> Self {
        Self {
            nodes,
            selected_key: None,
            indent: 16.0,
        }
    }

    /// Binds node selection to a signal holding the selected node key string.
    pub fn selected(mut self, sig: Signal<String>) -> Self {
        self.selected_key = Some(sig);
        self
    }

    /// Sets the horizontal indentation width per tree depth level.
    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = indent.max(8.0);
        self
    }
}

impl IntoElement for TreeView {
    fn into_element(self) -> Element {
        let col = Column::new().gap(2.0);

        fn render_nodes(
            nodes: &[TreeNode],
            depth: usize,
            indent: f32,
            sel: &Option<Signal<String>>,
            mut c: Container,
        ) -> Container {
            for node in nodes {
                let k = node.key.clone();
                let sel_sig = *sel;
                let is_sel = sel_sig.is_some_and(|s| s.try_get().as_ref() == Some(&k));
                let fill_key = k.clone();
                let pad_left = 8.0 + (depth as f32) * indent;

                let row = Container::row()
                    .height(Length::Px(28.0))
                    .padding(Edges::xy(pad_left, 4.0))
                    .align(AlignItems::Center)
                    .gap(6.0)
                    .radius(6.0)
                    .fill_when(
                        move || sel_sig.is_some_and(|s| s.try_get().as_ref() == Some(&fill_key)),
                        Color::rgba(0.0, 122.0, 255.0, 0.22),
                        Color::TRANSPARENT,
                    )
                    .hover_fill(Color::rgba(128.0, 128.0, 128.0, 0.15))
                    .child(
                        Text::new(if node.children.is_empty() { "• " } else { if node.expanded { "▾ " } else { "▸ " } })
                            .size(11.0)
                            .color(Color::from_rgba8(142, 142, 147, 255))
                    )
                    .child(
                        Text::new(&node.label)
                            .size(12.5)
                            .weight(if is_sel { FontWeight::Semibold } else { FontWeight::Regular })
                    );

                let row = if let Some(sig) = sel {
                    let key_cl = k.clone();
                    let sig_cl = *sig;
                    row.on_click(move || {
                        sig_cl.set(key_cl.clone());
                    })
                } else {
                    row
                };
                c = c.child(row);

                if node.expanded && !node.children.is_empty() {
                    c = render_nodes(&node.children, depth + 1, indent, sel, c);
                }
            }
            c
        }

        let container = render_nodes(&self.nodes, 0, self.indent, &self.selected_key, col);
        Element::Container(container)
    }
}