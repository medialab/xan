use crate::cmd::view::DisplayedColumn;
use crate::tui::{BasicTableStyle, ScrollTableStyle};

use simd_csv::StringRecord;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Size, Rect},
    style::Style,
    widgets::{Block, Borders, Row, Table, TableState},
};
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};

macro_rules! derive_scroll_state_methods {
    ($($method:ident),* $(,)?) => {
        $(
            pub fn $method(&mut self) {
                self.scroll_state.$method();
            }
        )*
    };
}

fn row_of<'a>(record: &'a StringRecord, cols: &[usize]) -> Row<'a> {
    Row::new(cols
        .iter()
        .map(|&c| record.get(c)
        .unwrap_or("")))
}

fn table_from_parts<'a>(
    rows: &'a [StringRecord],
    cols: &[DisplayedColumn],
    style: &BasicTableStyle,
    scrollable_cols: &[usize],
) -> Table<'a> {
    let rows = rows.iter().map(|record| row_of(record, scrollable_cols));
    Table::new(
        rows,
        cols.iter().map(|col| col.allowed_width() as u16)
    )
    .style(style.body_text)
    .row_highlight_style(style.body_row_highlight)
}

fn headers_from_parts<'a>(
    headers: &'a StringRecord,
    cols: &[DisplayedColumn],
    style: &Style,
    scrollable_cols: &[usize],
) -> Table<'a> {
    let head = row_of(headers, scrollable_cols);
    Table::new(
        [head],
        cols.iter().map(|col| col.allowed_width() as u16)
    )
    .style(*style)
}



#[derive(Debug)]
pub struct ScrollTable {
    headers: StringRecord,
    rows: Vec<StringRecord>,
    cols: Vec<DisplayedColumn>,
    cum_sum: Vec<usize>,
    table_state: TableState,
    scroll_state: ScrollViewState,
    body_size: Size,
    viewport: Size,
    style: ScrollTableStyle,
}

impl ScrollTable {
    pub fn new(headers: StringRecord, rows: Vec<StringRecord>, cols: Vec<DisplayedColumn>, style: ScrollTableStyle) -> Self {
        let mut sum = 0_usize;
        let cum_sum = cols.iter()
            .map(|col| {
                let tmp = sum;
                sum += col.allowed_width() + style.col_spacing;
                tmp
            })
            .collect();

        let body_size= Size::new(
            cols.iter().map(|col| col.allowed_width() as u16).sum::<u16>() + (style.col_spacing * (cols.len() - 1)) as u16,
            rows.len() as u16
        );

        Self {headers, rows, cols, cum_sum,
            table_state: TableState::new(),
            scroll_state: ScrollViewState::new(),
            body_size, viewport: Size::new(0,0),
            style}
    }

    pub fn selected_col(&self) -> Option<usize> {
        self.table_state.selected_column()
    }

    pub fn n_cols(&self) -> usize {
        self.cols.len()
    }

    pub fn n_rows(&self) -> u16 {
        self.rows.len() as u16
    }

    pub fn no_rows(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn header(&self, i: usize) -> Option<&str> {
        self.headers.get(i)
    }

    pub fn body_size(&self) -> Size {
        self.body_size
    }

    pub fn viewport(&self) -> Size {
        self.viewport
    }

    fn set_viewport(&mut self, area: Rect) {
        self.viewport.width = area.width.saturating_sub(1);
        self.viewport.height = area.width.saturating_sub(1);
    }


    pub fn offset(&self) -> Position {
        self.scroll_state.offset()
    }

    pub fn set_offset(&mut self, off: Position) {
        self.scroll_state.set_offset(off);
    }



    pub fn col_step(&self, col: usize) -> usize {
        if let Some(selected_col) = self.cols.get(col) {
            selected_col.allowed_width() + self.style.col_spacing
        } else { 0 }
    }

    pub fn col_widths(&self) -> impl Iterator <Item=u16> {
        self.cols.iter().map(|col| col.allowed_width() as u16)
    }

    pub fn width_until(&self, col: usize) -> usize {
        self.cum_sum[col]
    }


    pub fn next_col_idx(&self, off_x: u16) -> usize {
        self.cum_sum.partition_point(|&x| x as u16 <= off_x)
    }

    pub fn last_col_view(&self, off_x: u16) -> usize {
        self.cum_sum.partition_point(|&s| s as u16 <= off_x+self.viewport.width) - 1
    }

    // ---------------------------------------------------------------------------------


    derive_scroll_state_methods!(scroll_down, scroll_up,
                                scroll_right, scroll_left,
                                scroll_page_down, scroll_page_up,
                                scroll_to_top, scroll_to_bottom);



    pub fn scroll_col_right(&mut self) {
        let off_x = self.scroll_state.offset().x;
        let next_col_idx = self.next_col_idx(off_x);

        let target_x = if let Some(&next_col) = self.cum_sum.get(next_col_idx) {
            next_col
        } else { self.cum_sum[self.n_cols() - 1] };

        let mut off = self.scroll_state.offset();
        off.x = target_x as u16;
        self.scroll_state.set_offset(off);
    }

    pub fn scroll_col_left(&mut self) {
        let off_x = self.scroll_state.offset().x;
        let next_col_idx = self.next_col_idx(off_x);

        let current_start = self.cum_sum[next_col_idx - 1] as u16;
        let target_x: u16 =
            if off_x > current_start { current_start }
            else if next_col_idx > 2 { self.cum_sum[next_col_idx - 2] as u16 }
            else { 0 };

        let mut off = self.scroll_state.offset();
        off.x = target_x;
        self.scroll_state.set_offset(off);
    }



    pub fn select_previous_col(&mut self) {
        let off_x = self.scroll_state.offset().x;
        if let Some(selected_col) = self.table_state.selected_column() {
            if self.next_col_idx(off_x) - 1 == selected_col {
                self.scroll_col_left();
            }

            self.table_state.select_previous_column();
        }
    }

    pub fn select_next_col(&mut self) -> String {
        let off_x = self.scroll_state.offset().x;
        let text = if let Some(selected_col) = self.table_state.selected_column() {
            if self.last_col_view(off_x) == selected_col {
                self.scroll_col_right();
            }

            self.table_state.select_next_column();
            format!("{}", selected_col)
        } else { "-".to_string() };
        text
    }

    pub fn draw(&mut self, cols_idx: &Vec<usize>, frame: &mut Frame, area: Rect) {

        let [header_area, body_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);

        let mut body = ScrollView::new(self.body_size)
            .scrollbars_visibility(ScrollbarVisibility::Always);

        let body_table = table_from_parts(&self.rows, &self.cols, &self.style.table, cols_idx)
                                        .column_spacing(self.style.col_spacing as u16)
                                        .column_highlight_style(self.style.body_col_highlight)
                                        .cell_highlight_style(self.style.body_cell_highlight)
                                        .highlight_symbol(self.style.highlight_symbol);

        body.render_stateful_widget(body_table, body.area(), &mut self.table_state);
        frame.render_stateful_widget(body, body_area, &mut self.scroll_state);

        let off = self.scroll_state.offset(); // after ScrollView's clamp

        let mut header = ScrollView::new(Size::new(self.body_size.width, 1))
            .scrollbars_visibility(ScrollbarVisibility::Never);

        let header_table = headers_from_parts(&self.headers, &self.cols, &self.style.table.header, cols_idx)
                                        .column_spacing(self.style.col_spacing as u16);
        header.render_widget(header_table, header.area());

        let header_area = Rect { width: header_area.width.saturating_sub(1), ..header_area };
        let mut header_state = ScrollViewState::with_offset(Position::new(off.x, 0));
        frame.render_stateful_widget(header, header_area, &mut header_state);

        self.set_viewport(body_area);
    }

    pub fn draw_frozen(&self, cols_idx: usize, frame: &mut Frame, area: Rect, style: &BasicTableStyle) {
        let separator = Block::new().borders(Borders::RIGHT).style(Style::new().red());
        let left_inner = separator.inner(area);
        frame.render_widget(separator, area);

        let [corner, column_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(left_inner);
        frame.render_widget(headers_from_parts(&self.headers, &self.cols, &style.header, &[cols_idx]), corner);

        let column_area = Rect { height: column_area.height.saturating_sub(1), ..column_area }; // -1 : scrollbar
        let mut column = ScrollView::new(Size::new(self.col_step(cols_idx) as u16, self.n_rows()))
            .scrollbars_visibility(ScrollbarVisibility::Never);
        let table = table_from_parts(&self.rows, &self.cols, style, &[cols_idx]);

        column.render_widget(table, column.area());
        let off = self.scroll_state.offset(); // after ScrollView's clamp
        let mut column_state = ScrollViewState::with_offset(Position::new(0, off.y));
        frame.render_stateful_widget(column, column_area, &mut column_state);
    }
}