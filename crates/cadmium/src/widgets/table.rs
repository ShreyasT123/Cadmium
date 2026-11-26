/// Represents a column definition in a [`Table`].
///
/// Holds a header text label and a fixed or flexible layout width.
pub struct TableColumn {
    /// The display title shown in the table header row.
    pub header: String,
    /// The layout width constraint for the column cells and header.
    pub width: Length,
}

impl TableColumn {
    /// Creates a new table column with a header label and width constraint.
    pub fn new(header: impl Into<String>, width: Length) -> Self {
        Self {
            header: header.into(),
            width,
        }
    }
}

/// Represents a single data row in a [`Table`].
///
/// Holds a unique row identifier string and a vector of cell text values
/// corresponding 1-to-1 with the table's defined columns.
pub struct TableRow {
    /// Unique identifier for keying or tracking the row.
    pub id: String,
    /// Cell text strings ordered to match the table columns.
    pub cells: Vec<String>,
}

impl TableRow {
    /// Creates a new table row with a unique identifier and string cell values.
    pub fn new(id: impl Into<String>, cells: Vec<impl Into<String>>) -> Self {
        Self {
            id: id.into(),
            cells: cells.into_iter().map(Into::into).collect(),
        }
    }
}

/// A multi-column data grid component.
///
/// Displays a sticky header row with defined [`TableColumn`] widths followed
/// by scrollable rows of text cells. Supports optional single-row selection
/// bound to a [`Signal<usize>`].
pub struct Table {
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    selected_row: Option<Signal<usize>>,
}

impl Table {
    /// Creates a new table with given columns and rows.
    pub fn new(columns: Vec<TableColumn>, rows: Vec<TableRow>) -> Self {
        Self {
            columns,
            rows,
            selected_row: None,
        }
    }

    /// Binds row selection to a signal storing the selected row index.
    pub fn selected(mut self, sig: Signal<usize>) -> Self {
        self.selected_row = Some(sig);
        self
    }
}

impl IntoElement for Table {
    fn into_element(self) -> Element {
        let mut header_row = Row::new()
            .height(Length::Px(32.0))
            .padding(Edges::xy(12.0, 0.0))
            .align(AlignItems::Center)
            .fill(Color::from_rgba8(240, 240, 244, 180));

        for col in &self.columns {
            header_row = header_row.child(
                Container::new()
                    .width(col.width)
                    .child(Text::new(&col.header).size(12.0).weight(FontWeight::Semibold))
            );
        }

        let mut scroll = ScrollView::new().gap(2.0);
        for (i, row) in self.rows.into_iter().enumerate() {
            let mut r = Row::new()
                .height(Length::Px(34.0))
                .padding(Edges::xy(12.0, 0.0))
                .align(AlignItems::Center)
                .radius(6.0);

            for (c_idx, cell_text) in row.cells.into_iter().enumerate() {
                let col_w = self.columns.get(c_idx).map(|c| c.width).unwrap_or(Length::Auto);
                r = r.child(
                    Container::new()
                        .width(col_w)
                        .child(Text::new(cell_text).size(12.5))
                );
            }

            if let Some(sel) = self.selected_row {
                let sig_cl = sel;
                r = r
                    .fill_when(
                        move || sig_cl.try_get() == Some(i),
                        Color::rgba(0.0, 122.0, 255.0, 0.20),
                        Color::TRANSPARENT,
                    )
                    .hover_fill(Color::rgba(128.0, 128.0, 128.0, 0.12));
                r = r.on_click(move || {
                    sig_cl.set(i);
                });
            }

            scroll = scroll.child(r);
        }

        let table_container = Column::new()
            .flex_grow(1.0)
            .child(header_row)
            .child(scroll);

        Element::Container(table_container)
    }
}