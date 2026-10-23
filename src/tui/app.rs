use crate::CliResult;
use crate::cmd::view::DisplayedColumn;
use crate::tui::{ViewStyle, view_debug::ViewInfo};

use simd_csv::StringRecord;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Size, Rect},
    style::Style,
    widgets::{Block, Paragraph, Borders, Row, Table},
    text::Text,
    crossterm::event::{self, Event, KeyEventKind, KeyCode},
    DefaultTerminal,
};
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};

fn row_of<'a>(record: &'a StringRecord, cols: &[usize]) -> Row<'a> {
    Row::new(cols
        .iter()
        .map(|&c| record.get(c)
        .unwrap_or("")))
}

fn list_to_string<I>(list: I) -> String
where
    I: Iterator,
    I::Item: ToString,
{
    list.map(|w| w.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug)]
pub struct App {
    headers: StringRecord,
    rows: Vec<StringRecord>,
    cols: Vec<DisplayedColumn>,
    cum_sum: Vec<usize>,
    style: ViewStyle,
    selected_col: usize,
    frozen: Option<usize>,
    show_info: bool,
    quit: bool,
    scroll: ScrollViewState,
    infos: Vec<(&'static str, String)>,
}

impl App {
    pub fn new(headers: StringRecord, rows: Vec<StringRecord>, cols: Vec<DisplayedColumn>, style: ViewStyle) -> Self {
        let mut sum = 0_usize;
        let cum_sum = cols.iter().map(|col| {let tmp = sum; sum += col.get_allowed_width() + style.col_spacing; tmp}).collect();
        let infos = vec![
            ("Lignes", "".to_string()),
            ("Colonnes", "".to_string()),
            ("Largeur colonne", "".to_string()),
            ("Colonne figée", "".to_string()),
        ];
        Self { headers, rows, cols, cum_sum, style, selected_col: 0, frozen: None, show_info: false, quit: false, scroll: ScrollViewState::new(), infos}
    }

    pub fn get_infos(&self) -> &Vec<(&'static str, String)> {
        &self.infos
    }

    fn log(&mut self, label: &'static str, info: String) {
        self.infos.push((label, info));
    }

    fn n_cols(&self) -> usize {
        self.cols.len()
    }

    pub fn n_rows(&self) -> u16 {
        self.rows.len() as u16
    }

    fn get_col_step(&self, col: usize) -> usize {
        if let Some(selected_col) = self.cols.iter().nth(col) {
            selected_col.get_allowed_width() + self.style.col_spacing
        } else { 0 }
    }

    fn get_widths(&self) -> impl Iterator <Item=u16> {
        self.cols.iter().map(|col| col.get_allowed_width() as u16)
    }

    // width of 'col' is included
    fn get_widths_until_col(&self, col: usize) -> usize {
        self.cum_sum[col]
    }

    pub fn get_left_col(&self, off_x: u16) -> usize {
        (0..self.cols.len())
            .find(|&i| off_x < self.get_widths_until_col(i + 1) as u16)
            .unwrap_or(self.cols.len() - 1)
    }

    fn table_from(&self, scrollable_cols: &[usize]) -> Table<'_> {
        let rows = self.rows.iter().map(|record| row_of(record, scrollable_cols));
        Table::new(
            rows,
            self.get_widths()
        )
        .column_spacing(self.style.col_spacing as u16)
        .style(self.style.body)
    }

    fn headers_from(&self, scrollable_cols: &[usize]) -> Table<'_> {
        let head = row_of(&self.headers, scrollable_cols);
        Table::new(
            [head],
            self.get_widths()
        )
        .column_spacing(self.style.col_spacing as u16)
        .style(self.style.header)
    }

    fn footer(&self, frozen_name: &str) -> Text<'static> {
        Text::raw(format!(
            " {} lines | {} columns | Home/End : top/end | Arrows : scroll | q : quit | Frozen : {frozen_name}",
            self.rows.len(),self.n_cols()))
    }

    pub fn scrollable_columns(&self) -> Vec<usize> {
        (0..self.n_cols()).filter(|&i| Some(i) != self.frozen).collect()
    }


    pub fn scroll_col(&mut self, right: bool) {
        // self.log("Displayed:", list_to_string(self.cols.iter()));
        // self.log("Displayed:", list_to_string(self.cum_sum.iter()));
        let off_x = self.scroll.offset().x;

        let next_col_idx = self.cum_sum
            .partition_point(|&x| x as u16 <= off_x);


        let mut target_x: u16 = if right {
            self.cum_sum[next_col_idx] as u16
        } else {
        let current_start = self.cum_sum[next_col_idx - 1] as u16;
            if off_x > current_start {
                current_start
            } else if next_col_idx > 2 {
                if let Some(val) = self.cum_sum.get(next_col_idx - 2) {
                    *val as u16
                } else {0}
            } else { 0 }
        };

        let mut off = self.scroll.offset();
        off.x = target_x;
        self.scroll.set_offset(off);
    }

    fn select_col_beside(&mut self, right: bool) {
        self.selected_col = if right {
            if self.selected_col < self.n_cols() {
                self.selected_col + 1
            } else { self.selected_col }
        } else {
            if self.selected_col > 0 {
                self.selected_col - 1
            } else { self.selected_col }
        };
    }

    /// Fige la colonne située au bord gauche de la zone scrollable (ou défige).
    fn toggle_freeze(&mut self) {
        if self.frozen.take().is_some() || self.n_cols() < 2 {
            return;
        }

        self.frozen = Some(self.selected_col);


        // On cale l'offset sur une frontière de colonne.
        let mut off = self.scroll.offset();
        off.x = self.get_widths_until_col(self.selected_col) as u16;
        self.scroll.set_offset(off);
    }

    fn get_frozen_space(&self) -> u16 {
        if let Some(frozen_col) = self.frozen {
            self.get_col_step(frozen_col).saturating_add(1) as u16
        } else { 0 }
    }

    fn update_infos(&mut self, infos: [String; 4]) {
        for ((_, info), new_info) in self.infos.iter_mut().take(4).zip(infos) {
            *info = new_info;
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> CliResult<()> {
        while !self.quit {
            //terminal.clear();
            terminal.draw(|f| self.draw(f))?;
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.handle_key(key.code);
                }
            }
        }
        Ok(())
    }

    pub fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,

            KeyCode::Down => self.scroll.scroll_down(),
            KeyCode::Up => self.scroll.scroll_up(),
            KeyCode::Tab => self.scroll_col(true),
            KeyCode::BackTab => self.scroll_col(false),

            KeyCode::Right => self.scroll.scroll_right(),
            KeyCode::Left => self.scroll.scroll_left(),

            KeyCode::Char('l') => self.select_col_beside(true),
            KeyCode::Char('h') => self.select_col_beside(false),


            KeyCode::PageDown => self.scroll.scroll_page_down(),
            KeyCode::PageUp => self.scroll.scroll_page_up(),
            KeyCode::Home => self.scroll.scroll_to_top(),
            KeyCode::End => self.scroll.scroll_to_bottom(),

            KeyCode::Char('f') => self.toggle_freeze(),
            KeyCode::Char('i') => self.show_info = !self.show_info,
            _ => {}
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let scrollable = self.scrollable_columns();

        let [main, footer] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());

        let block = Block::bordered().title(" XAN-V-UI ");
        let inner = block.inner(main);
        frame.render_widget(block, main);

        let left_w = self.get_frozen_space();
        let [left, right] =
            Layout::horizontal([Constraint::Length(left_w), Constraint::Min(0)]).areas(inner);
        let [header_area, body_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(right);

        let body_size = Size::new(self.get_widths().sum(), self.rows.len() as u16);
        let mut body = ScrollView::new(body_size)
                                            .scrollbars_visibility(ScrollbarVisibility::Always);

        let body_table = self.table_from(&scrollable);
        body.render_widget(body_table, body.area());

        frame.render_stateful_widget(body, body_area, &mut self.scroll);

        let off = self.scroll.offset(); // after ScrollView's clamp

        let mut header = ScrollView::new(Size::new(body_size.width, 1))
                                            .scrollbars_visibility(ScrollbarVisibility::Never);
        header.render_widget(self.headers_from(&scrollable), header.area());

        let header_area = Rect { width: header_area.width.saturating_sub(1), ..header_area };
        let mut header_state = ScrollViewState::with_offset(Position::new(off.x, 0));
        frame.render_stateful_widget(header, header_area, &mut header_state);


        if let Some(fc) = self.frozen {
            let separator = Block::new().borders(Borders::RIGHT).style(Style::new().red());
            let left_inner = separator.inner(left);
            frame.render_widget(separator, left);

            let [corner, column_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(left_inner);
            frame.render_widget(self.headers_from(&[fc]), corner);

            let column_area = Rect { height: column_area.height.saturating_sub(1), ..column_area }; // -1 : scrollbar
            let mut column = ScrollView::new(Size::new(self.get_col_step(fc) as u16, self.n_rows()))
                .scrollbars_visibility(ScrollbarVisibility::Never);
            column.render_widget(self.table_from(&[fc]), column.area());
            let mut column_state = ScrollViewState::with_offset(Position::new(0, off.y));
            frame.render_stateful_widget(column, column_area, &mut column_state);
        }

        let frozen_name = self.frozen.and_then(|i| self.headers.get(i)).unwrap_or("-");
        frame.render_widget(Paragraph::new(self.footer(frozen_name)), footer);

        let view = ViewInfo {
            canvas: body_size,
            viewport: Size::new(body_area.width.saturating_sub(1), body_area.height.saturating_sub(1)),
            offset: off,
        };

        if self.show_info {
            let infos: [String; 4] = [
                self.rows.len().to_string(),
                self.n_cols().to_string(),
                self.get_widths()
                    .map(|w| w.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                frozen_name.to_string(),
            ];

            self.update_infos(infos);

            view.draw_info(frame, &self);
        }
    }

    pub fn no_rows(&self) -> bool {
        self.rows.is_empty()
    }
}