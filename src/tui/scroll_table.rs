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

#[derive(Debug)]
pub struct ScrollTable {
    headers: StringRecord,
    rows: Vec<StringRecord>,
    cols: Vec<DisplayedColumn>,
    cum_sum: Vec<usize>, // beginning of each scrollable column
    table_state: TableState,
    scroll_state: ScrollViewState,
    body_size: Size,
    viewport: Size,
    frozen: Option<usize>,
    style: &'static ScrollTableStyle,
}

impl ScrollTable {
    pub fn new(headers: StringRecord, rows: Vec<StringRecord>, cols: Vec<DisplayedColumn>, style: &'static ScrollTableStyle) -> Self {
        let body_size = Size::new(0, rows.len() as u16);
        let mut table = Self {
            headers, rows, cols,
            cum_sum: Vec::new(),
            table_state: TableState::new().with_selected_column(0).with_selected(0),
            scroll_state: ScrollViewState::new(),
            body_size,
            viewport: Size::new(0, 0),
            style,
            frozen: None,
        };
        table.update_cum_sum();
        table.update_body_width();
        table
    }

    pub fn update_body_width(&mut self) -> u16 {
        let n = self.n_cols();
        let widths: usize = (0..n).map(|s| self.scroll_width(s)).sum();
        let total = widths + self.style.col_spacing * n.saturating_sub(1);
        self.body_size.width = total.min(u16::MAX as usize) as u16;
        self.body_size.width
    }

    pub fn update_cum_sum(&mut self) {
        let mut sum = 0_usize;
        self.cum_sum = (0..self.n_cols())
            .map(|s| {
                let start = sum;
                sum += self.scroll_width(s) + self.style.col_spacing;
                start
            })
            .collect();
    }

    pub fn scrollable_columns(&self) -> Vec<usize> {
        (0..self.cols.len()).filter(|&i| Some(i) != self.frozen).collect()
    }

    pub fn frozen(&self) -> Option<usize> {
        self.frozen
    }

    pub fn frozen_name(&self) -> String {
        self.frozen
            .and_then(|f| self.header(f))
            .unwrap_or("-")
            .to_string()
    }

    pub fn n_cols(&self) -> usize {
        self.cols.len().saturating_sub(self.frozen.is_some() as usize)
    }

    pub fn n_cols_original(&self) -> usize {
        self.cols.len()
    }

    pub fn n_rows(&self) -> usize {
        self.rows.len()
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
        self.viewport.height = area.height.saturating_sub(1);
    }

    // ---------------------------------------------------------------------------------

    fn resolve_col(&self, s: usize) -> usize {
        match self.frozen {
            Some(f) if s >= f => s + 1,
            _ => s,
        }
    }

    fn scroll_width(&self, s: usize) -> usize {
        self.cols[self.resolve_col(s)].allowed_width()
    }

    fn col_start(&self, s: usize) -> usize {
        self.cum_sum[s]
    }

    fn col_end(&self, s: usize) -> usize {
        self.cum_sum[s] + self.scroll_width(s)
    }

    pub fn col_step(&self, col: usize) -> usize {
        if col < self.n_cols() {
            self.scroll_width(col) + self.style.col_spacing
        } else { 0 }
    }

    fn frozen_step(&self) -> usize {
        self.frozen
            .map_or(0, |f| self.cols[f].allowed_width() + self.style.col_spacing)
    }

    pub fn col_widths(&self) -> impl Iterator<Item = u16> + '_ {
        self.cols.iter().map(|col| col.allowed_width() as u16)
    }

    pub fn width_until(&self, col: usize) -> usize {
        self.cum_sum.get(col).or(self.cum_sum.last()).copied().unwrap_or(0)
    }

    fn next_row_idx(&self, off_y: u16) -> usize {
        off_y as usize
    }

    fn n_rows_view(&self) -> u16 {
        (self.viewport.height.saturating_sub(1)) / 4 - 2
    }

    fn last_row_view(&self, off_y: u16) -> u16 {
        off_y + self.n_rows_view()
    }

    // ---------------------------------------------------------------------------------

    pub fn offset(&self) -> Position {
        self.scroll_state.offset()
    }

    pub fn set_offset(&mut self, off: Position) {
        self.scroll_state.set_offset(off);
    }

    fn offset_x(&self) -> usize {
        self.scroll_state.offset().x as usize
    }

    fn max_offset_x(&self) -> usize {
        (self.body_size.width as usize).saturating_sub(self.viewport.width as usize)
    }

    fn set_offset_x(&mut self, x: usize) {
        let mut off = self.scroll_state.offset();
        off.x = x.min(self.max_offset_x()) as u16;
        self.scroll_state.set_offset(off);
    }

    pub fn set_offset_on_col(&mut self, col_idx: usize) {
        let x = self.width_until(col_idx);
        self.set_offset_x(x);
    }

    // ---------------------------------------------------------------------------------

    fn selected_scroll_col(&self) -> usize {
        self.table_state.selected_column().unwrap_or(0)
    }

    pub fn selected_col(&self) -> usize {
        self.resolve_col(self.selected_scroll_col())
    }

    fn select_col(&mut self, s: usize) {
        let last = self.n_cols().saturating_sub(1);
        self.table_state.select_column(Some(s.min(last)));
    }

    // ---------------------------------------------------------------------------------

    fn first_visible_col(&self, off: usize) -> usize {
        self.cum_sum
            .partition_point(|&x| x < off)
            .min(self.n_cols().saturating_sub(1))
    }

    fn last_visible_col(&self, off: usize) -> usize {
        let first = self.first_visible_col(off);
        let right = off + self.viewport.width as usize;
        (first..self.n_cols())
            .rev()
            .find(|&s| self.col_end(s) <= right)
            .unwrap_or(first)
    }

    fn ensure_col_visible(&mut self, s: usize) {
        if s >= self.n_cols() { return; }
        let off = self.offset_x();
        let view_w = self.viewport.width as usize;
        let start = self.col_start(s);
        let end = self.col_end(s);

        if start < off {
            self.set_offset_x(start);
        } else if end > off + view_w {
            let k = self.cum_sum[..=s]
                .partition_point(|&x| x + view_w < end)
                .min(s);
            self.set_offset_x(self.cum_sum[k]);
        }
    }

    fn ensure_selected_visible(&mut self) {
        let s = self.selected_scroll_col();
        self.ensure_col_visible(s);
    }

    fn clamp_selection_to_view(&mut self) {
        if self.n_cols() == 0 { return; }
        let off = self.offset_x();
        let first = self.first_visible_col(off);
        let last = self.last_visible_col(off).max(first);
        let s = self.selected_scroll_col().clamp(first, last);
        self.select_col(s);
    }

    // ---------------------------------------------------------------------------------

    pub fn scroll_right(&mut self) {
        self.scroll_state.scroll_right();
        self.clamp_selection_to_view();
    }

    pub fn scroll_left(&mut self) {
        self.scroll_state.scroll_left();
        self.clamp_selection_to_view();
    }

    pub fn scroll_col_right(&mut self) {
        let off = self.offset_x();
        let next = self.cum_sum.partition_point(|&x| x <= off);
        if next >= self.n_cols() { return; }

        self.set_offset_x(self.cum_sum[next]);
        if self.offset_x() == off { return; }
        self.clamp_selection_to_view();
    }

    pub fn scroll_col_left(&mut self) {
        let off = self.offset_x();
        let prev = self.cum_sum.partition_point(|&x| x < off);
        let target = if prev == 0 { 0 } else { self.cum_sum[prev - 1] };

        self.set_offset_x(target);
        if self.offset_x() == off { return; }
        self.clamp_selection_to_view();
    }

    pub fn select_next_col(&mut self) {
        let next = self.selected_scroll_col() + 1;
        if next < self.n_cols() {
            self.select_col(next);
            self.ensure_col_visible(next);
        }
    }

    pub fn select_previous_col(&mut self) {
        let selected = self.selected_scroll_col();
        if selected > 0 {
            self.select_col(selected - 1);
            self.ensure_col_visible(selected - 1);
        }
    }

    // ---------------------------------------------------------------------------------

    pub fn select_previous_row(&mut self) {
        // safe: usize cannot be negative
        let selected = self.table_state.selected().unwrap();
        let off_y = self.scroll_state.offset().y;
        if self.next_row_idx(off_y) + 1 > selected {
            self.scroll_state.scroll_up();
        }
        self.table_state.scroll_up_by(1);
    }

    pub fn select_next_row(&mut self) {
        let selected = self.table_state.selected().unwrap();
        if selected < self.n_rows() - 1 {
            let off_y = self.scroll_state.offset().y;
            if self.last_row_view(off_y) < selected as u16 {
                self.scroll_state.scroll_down();
            }
            self.table_state.scroll_down_by(1);
        }
    }

    pub fn scroll_page_up(&mut self) {
        self.table_state.scroll_up_by(self.viewport.height);
        self.scroll_state.scroll_page_up();
        self.scroll_state.scroll_up();
    }

    pub fn scroll_page_down(&mut self) {
        self.table_state.scroll_down_by(self.viewport.height);
        self.scroll_state.scroll_page_down();
        self.scroll_state.scroll_down();
    }

    pub fn scroll_to_top(&mut self) {
        self.table_state.select(Some(0));
        self.scroll_state.scroll_to_top();
    }

    pub fn scroll_to_bottom(&mut self) {
        self.table_state.select(Some(self.n_rows()-1));
        self.scroll_state.scroll_to_bottom();
    }

    // ---------------------------------------------------------------------------------

    pub fn toggle_freeze(&mut self) {
        if self.cols.len() < 2 {
            return;
        }
        match self.frozen {
            Some(frozen_col) => self.apply_frozen(None, frozen_col),
            None => {
                let selected = self.selected_col();
                self.apply_frozen(Some(selected), selected);
            }
        }
    }

    pub fn set_frozen(&mut self, frozen: usize) {
        if self.cols.len() < 2 || frozen >= self.cols.len() {
            return;
        }
        let focus = self.selected_col();
        self.apply_frozen(Some(frozen), focus);
    }

    fn apply_frozen(&mut self, new_frozen: Option<usize>, focus: usize) {
        self.frozen = new_frozen;
        self.update_cum_sum();
        self.update_body_width();

        let selected = match new_frozen {
            Some(f) if focus > f => focus - 1,
            _ => focus,
        };
        self.select_col(selected);
        let s = self.selected_scroll_col();
        self.set_offset_on_col(s);
    }

    fn get_frozen_space(&self) -> u16 {
        if self.frozen.is_some() {
            (self.frozen_step() + 1).min(u16::MAX as usize) as u16 // +1 : bord droit du séparateur
        } else { 0 }
    }
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
        scrollable_cols.iter().map(|&index| cols[index].allowed_width() as u16)
    )
    .style(style.body_text)
    .row_highlight_style(style.body_row_highlight)
}

fn headers_from_parts<'a>(
    table: &'a ScrollTable,
    style: &Style,
    scrollable_cols: &[usize],
) -> Table<'a> {
    let head = row_of(&table.headers, scrollable_cols);
    Table::new(
        [head],
        scrollable_cols
            .iter()
            .map(|&index| table.cols[index].allowed_width() as u16),
    )
    .style(*style)
}

pub fn draw(frame: &mut Frame, table: &mut ScrollTable, area: Rect) {
    let scrollable_cols = table.scrollable_columns();
    let left_w = table.get_frozen_space();
    let [left, right] =
        Layout::horizontal([Constraint::Length(left_w), Constraint::Min(0)]).areas(area);

    // scrollable table:
    let [header_area, body_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(right);

    table.set_viewport(body_area);
    table.ensure_selected_visible();

    let mut body = ScrollView::new(table.body_size)
        .scrollbars_visibility(ScrollbarVisibility::Always);

    let body_table = table_from_parts(&table.rows, &table.cols, &table.style.table, &scrollable_cols)
                                    .column_spacing(table.style.col_spacing as u16)
                                    .column_highlight_style(table.style.body_col_highlight)
                                    .cell_highlight_style(table.style.body_cell_highlight);

    body.render_stateful_widget(body_table, body.area(), &mut table.table_state);
    frame.render_stateful_widget(body, body_area, &mut table.scroll_state);

    let off = table.scroll_state.offset(); // after ScrollView's clamp

    let mut header = ScrollView::new(Size::new(table.body_size.width, 1))
        .scrollbars_visibility(ScrollbarVisibility::Never);

    let header_table = headers_from_parts(table, &table.style.table.header, &scrollable_cols)
                                    .column_spacing(table.style.col_spacing as u16);
    header.render_widget(header_table, header.area());

    let header_area = Rect { width: header_area.width.saturating_sub(1), ..header_area };
    let mut header_state = ScrollViewState::with_offset(Position::new(off.x, 0));
    frame.render_stateful_widget(header, header_area, &mut header_state);

    if let Some(frozen_col) = table.frozen() {
        draw_frozen(frame, frozen_col, table, left);
    }
}

pub fn draw_frozen(frame: &mut Frame, frozen: usize, table: &ScrollTable, area: Rect) {
    let separator = Block::new().borders(Borders::RIGHT).style(table.style.separator);
    let left_inner = separator.inner(area);
    frame.render_widget(separator, area);

    let [corner, left_column_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(left_inner);
    frame.render_widget(headers_from_parts(table, &table.style.frozen.header, &[frozen]), corner);

    let left_column_area = Rect { height: left_column_area.height.saturating_sub(1), ..left_column_area }; // -1 : scrollbar
    let mut left_column = ScrollView::new(Size::new(table.frozen_step() as u16, table.n_rows() as u16))
        .scrollbars_visibility(ScrollbarVisibility::Never);
    let frozen_table = table_from_parts(&table.rows, &table.cols, &table.style.frozen, &[frozen]);

    let mut frozen_row_state = TableState::new().with_selected(table.table_state.selected());
    left_column.render_stateful_widget(frozen_table, left_column.area(), &mut frozen_row_state);

    let off_y = table.offset().y; // after ScrollView's clamp
    let mut left_column_state = ScrollViewState::with_offset(Position::new(0, off_y));
    frame.render_stateful_widget(left_column, left_column_area, &mut left_column_state);
}