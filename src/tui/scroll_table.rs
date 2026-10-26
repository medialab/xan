use crate::cmd::view::DisplayedColumn;
use crate::tui::{ViewStyle, BasicTableStyle, ScrollTableStyle};

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
    pub cum_sum: Vec<usize>,
    table_state: TableState,
    scroll_state: ScrollViewState,
    body_size: Size,
    viewport: Size,
    frozen: Option<usize>,
    style: &'static ScrollTableStyle,
}

impl ScrollTable {
    pub fn new(headers: StringRecord, rows: Vec<StringRecord>, cols: Vec<DisplayedColumn>, style: &'static ScrollTableStyle) -> Self {
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
            table_state: TableState::new().with_selected_column(0).with_selected(0),
            scroll_state: ScrollViewState::new(),
            body_size, viewport: Size::new(0,0),
            style, frozen: None}
    }

    fn cum_sum_until(&self, i: usize) -> usize {
        let mut sum = 0_usize;
        for &col in self.scrollable_columns().iter() {
            if col > i { break }
            sum += self.cols[col].allowed_width() + self.style.col_spacing;
        };
        sum
    }

    fn vect_cum_sum_until(&self, i: usize) -> Vec<usize> {
        let mut sum = 0_usize;
        let mut cum_sum = Vec::new();
        for &col in self.scrollable_columns().iter() {
            cum_sum.push(sum);
            if col > i { break }
            sum += self.cols[col].allowed_width() + self.style.col_spacing;
        };
        cum_sum
    }

    pub fn scrollable_columns(&self) -> Vec<usize> {
        (0..self.n_cols()).filter(|&i| Some(i) != self.frozen).collect()
    }

    pub fn update_body_width(&self) -> u16 {
        self.scrollable_columns().iter().map(|&index| self.cols[index].allowed_width() as u16).sum::<u16>()
        + (self.style.col_spacing * (self.cols.len() - 1)) as u16
    }

    pub fn update_cum_sum(&mut self) {
        let mut sum = 0_usize;
        self.cum_sum = self.scrollable_columns().iter()
            .map(|col| {
                let tmp = sum;
                sum += self.cols[*col].allowed_width() + self.style.col_spacing;
                tmp
            })
            .collect();
    }

    pub fn frozen(&self) -> Option<usize> {
        self.frozen
    }

    pub fn set_frozen(&mut self, frozen: usize) {
        self.frozen = Some(frozen);
    }

    pub fn frozen_name(&self) -> String {
        if let Some(frozen) = self.frozen {
            self.header(frozen)
        } else { None }
        .unwrap_or("-").to_string()
    }

    pub fn n_cols(&self) -> usize {
        self.cols.len() - self.frozen.is_some() as usize
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
        self.viewport.height = area.width.saturating_sub(1);
    }

    // ---------------------------------------------------------------------------------

    fn resolve_col(&self, col_idx: usize) -> usize {
        let gap = match self.frozen {
            Some(frozen) =>  {if col_idx >= frozen { 1 } else { 0 }},
            None => 0,
        };
        col_idx + gap
    }

    pub fn selected_col(&self) -> usize {
        let selected = self.table_state.selected_column().unwrap();
        self.resolve_col(selected)
    }

    fn select_col(&mut self, i: usize) {
        let gap = match self.frozen {
            Some(frozen) =>  {
                let selected = self.table_state.selected_column().unwrap();
                if selected >= frozen { 1 } else { 0 }
            },
            None => 0,
        };
        self.table_state.select_column(Some(i + gap));
    }


    // ---------------------------------------------------------------------------------

    pub fn col_step(&self, col: usize) -> usize {
        let col_idx = self.resolve_col(col);
        if let Some(selected_col) = self.cols.get(col_idx) {
            selected_col.allowed_width() + self.style.col_spacing
        } else { 0 }
    }

    pub fn col_widths(&self) -> impl Iterator <Item=u16> {
        self.cols.iter().map(|col| col.allowed_width() as u16)
    }

    pub fn width_until(&self, col: usize) -> usize {
        self.cum_sum[col]
    }

    fn next_to_left_col_idx(&self, off_x: u16) -> usize {
        self.cum_sum.partition_point(|&x| x as u16 <= off_x)
    }

    fn next_row_idx(&self, off_y: u16) -> usize {
        off_y as usize
    }

    fn last_col_view(&self, off_x: u16) -> usize {
        self.cum_sum.partition_point(|&s| s as u16 <= off_x+self.viewport.width) - 1
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

    pub fn set_offset_on_col(&mut self, col_idx: usize) {
        let mut off = self.scroll_state.offset();
        off.x = self.width_until(col_idx) as u16;
        self.scroll_state.set_offset(off);
    }

    // ---------------------------------------------------------------------------------

    pub fn scroll_col_right(&mut self) {
        let mut off = self.scroll_state.offset();
        let next_to_left_col_idx = self.next_to_left_col_idx(off.x);

        let target_x = if let Some(&next_col) = self.cum_sum.get(next_to_left_col_idx) {
            next_col
        } else { self.width_until(self.n_cols() - 1) };

        off.x = target_x as u16;
        self.scroll_state.set_offset(off);
        let selected = self.selected_col();
        if selected < next_to_left_col_idx {
            self.select_col(selected+1);
        }
    }

    pub fn scroll_col_right_for_max_step_from(&mut self, col_idx: usize) {
        let mut off = self.scroll_state.offset();
        let col_step = self.col_step(col_idx);
        let mut i = self.next_to_left_col_idx(off.x) - 1;
        let mut target_x = off.x as usize + self.col_step(i);
        while target_x < col_step {
            i += 1;
            target_x += self.col_step(i);
        }

        off.x = target_x as u16;
        self.scroll_state.set_offset(off);
    }

    pub fn select_next_col(&mut self) {
        let selected_col = self.selected_col();

        let off_x = self.scroll_state.offset().x;
        let last_col_idx = self.last_col_view(off_x);
        if last_col_idx <= selected_col {
            self.scroll_col_right_for_max_step_from(last_col_idx);
        }

        self.select_col(selected_col.saturating_add(1).min(last_col_idx));
    }

    pub fn scroll_col_left(&mut self) {
        let mut off = self.scroll_state.offset();
        let next_to_left_col_idx = self.next_to_left_col_idx(off.x);

        let current_start = self.cum_sum[next_to_left_col_idx - 1] as u16 ;
        let target_x: u16 =
            if off.x > current_start { current_start }
            else if next_to_left_col_idx > 2 { self.cum_sum[next_to_left_col_idx - 2] as u16 }
            else { 0 };

        off.x = target_x;
        self.scroll_state.set_offset(off);
        let selected_col = (self.selected_col()).min(self.last_col_view(off.x));
        self.select_col(selected_col);
    }

    pub fn select_previous_col(&mut self) {
        let mut off = self.scroll_state.offset();
        let selected_col = self.selected_col();

        let next_to_left_col_idx = self.next_to_left_col_idx(off.x);
        if next_to_left_col_idx > selected_col {
            off.x = self.width_until(next_to_left_col_idx.saturating_sub(2)) as u16 ;
            self.scroll_state.set_offset(off);
    }

        let selected_col = selected_col.max(next_to_left_col_idx - 1);
        self.select_col(selected_col.saturating_sub(1));
    }

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

    pub fn scroll_page_down(&mut self) {
        self.table_state.scroll_down_by(self.viewport.height);
        self.scroll_state.scroll_page_down();
    }

    pub fn scroll_page_up(&mut self) {
        self.table_state.scroll_up_by(self.viewport.height);
        self.scroll_state.scroll_page_up();
    }

    // ---------------------------------------------------------------------------------

    fn headers_from_parts<'a>(&'a self,
        style: &Style,
        scrollable_cols: &[usize],
    ) -> Table<'a> {
        let head = row_of(&self.headers, scrollable_cols);
        Table::new(
            [head],
            scrollable_cols.iter().map(|&index| self.cols[index].allowed_width() as u16)
        )
        .style(*style)
    }

    // ---------------------------------------------------------------------------------

    pub fn toggle_freeze(&mut self) {
        if self.n_cols() < 2 {
            return;
        }
        if let Some(frozen_col) = self.frozen.take() {
            self.select_col(frozen_col);
            self.set_offset_on_col(frozen_col);
        } else {
            let mut off = self.offset();

            self.frozen = self.table_state
                .selected_column()
                .and_then(|selected_col| {
                    off.x = self.width_until(selected_col) as u16;
                    self.set_offset(off);
                    self.select_col((selected_col).min(self.n_cols() - 2));
                    Some(selected_col)
                });
        }

        self.update_body_width();
        self.update_cum_sum();
    }

    fn get_frozen_space(&self) -> u16 {
        if let Some(frozen_col) = self.frozen {
            self.col_step(frozen_col).saturating_add(1) as u16
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

pub fn draw<'a>(frame: &mut Frame, table: &mut ScrollTable, area: Rect) {
    let scrollable_cols = table.scrollable_columns();
    let left_w = table.get_frozen_space();
    let [left, right] =
        Layout::horizontal([Constraint::Length(left_w), Constraint::Min(0)]).areas(area);

    // scrollable table:
    let [header_area, body_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(right);

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

    let header_table = table.headers_from_parts( &table.style.table.header, &scrollable_cols)
                                    .column_spacing(table.style.col_spacing as u16);
    header.render_widget(header_table, header.area());

    let header_area = Rect { width: header_area.width.saturating_sub(1), ..header_area };
    let mut header_state = ScrollViewState::with_offset(Position::new(off.x, 0));
    frame.render_stateful_widget(header, header_area, &mut header_state);

    if let Some(frozen_col) = table.frozen() {
        draw_frozen(frame, frozen_col, table, left);
    }

    table.set_viewport(body_area);
}

pub fn draw_frozen(frame: &mut Frame, frozen: usize, table: &ScrollTable, area: Rect) {
    let separator = Block::new().borders(Borders::RIGHT).style(table.style.separator);
    let left_inner = separator.inner(area);
    frame.render_widget(separator, area);

    let [corner, left_column_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(left_inner);
    frame.render_widget(table.headers_from_parts(&table.style.frozen.header, &[frozen]), corner);

    let left_column_area = Rect { height: left_column_area.height.saturating_sub(1), ..left_column_area }; // -1 : scrollbar
    let mut left_column = ScrollView::new(Size::new(table.col_step(frozen) as u16, table.n_rows() as u16))
        .scrollbars_visibility(ScrollbarVisibility::Never);
    let frozen_table = table_from_parts(&table.rows, &table.cols, &table.style.frozen, &[frozen]);

    left_column.render_widget(frozen_table, left_column.area());
    let off_y = table.offset().y; // after ScrollView's clamp
    let mut left_column_state = ScrollViewState::with_offset(Position::new(0, off_y));
    frame.render_stateful_widget(left_column, left_column_area, &mut left_column_state);
}