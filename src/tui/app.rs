use std::time::Duration;

use simd_csv::StringRecord;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{Block, Borders, Paragraph},
    text::Text,
    crossterm::event::{self, Event, KeyEventKind, KeyCode},
    DefaultTerminal,
};

use crate::CliResult;
use crate::cmd::view::DisplayedColumn;
use crate::utils::DataType;
use crate::tui::{FrameTheme, ViewStyle, CLASSIC_VIEWSTYLE, view_infos::ViewInfo, scroll_table, scroll_table::ScrollTable};


#[derive(Debug)]
pub struct ViewApp {
    name: String,
    scroll_table: ScrollTable,
    show_info: bool,
    quit: bool,
}

impl ViewApp {
    pub fn new(name: String, headers: StringRecord,
        rows: Vec<(StringRecord, Vec<DataType>)>, cols: Vec<DisplayedColumn>,
        frame: FrameTheme, style: Option<&'static ViewStyle>) -> Self {
        let viewstyle = style.unwrap_or(&CLASSIC_VIEWSTYLE);
        let scroll_table = ScrollTable::new(headers, rows, cols, &viewstyle.scroll_table, frame);
        Self { name, scroll_table, show_info: false, quit: false }
    }

    pub fn n_rows(&self) -> usize {
        self.scroll_table.n_rows()
    }

    pub fn n_cols(&self) -> usize {
        self.scroll_table.n_cols()
    }

    fn footer(&self) -> Text<'static> {
        Text::raw(format!(
            " {} lines | {} columns | Home/End : top/end | Arrows : scroll | Tab/BackTab : next/previous column | \
            f: freeze | i : infos | q : quit", self.scroll_table.n_rows(), self.scroll_table.n_cols()))
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> CliResult<()> {
        while !self.quit {
            terminal.draw(|f| self.draw(f))?;

            // treating all recorded events, and *then* drawing
            let mut has_event = event::poll(Duration::from_millis(250))?;

            while has_event {
                if let Event::Key(key) = event::read()? {
                    #[cfg(not(windows))]
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code);
                    }
                    #[cfg(windows)]
                    if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                        self.handle_key(key.code);
                    }
                }
                has_event = event::poll(Duration::ZERO)?;
            }
        }
        Ok(())
    }

    pub fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,

            KeyCode::Down => self.scroll_table.select_next_row(),
            KeyCode::Up => self.scroll_table.select_previous_row(),
            KeyCode::Tab => self.scroll_table.scroll_col_right(),
            KeyCode::BackTab => self.scroll_table.scroll_col_left(),

            KeyCode::Right => self.scroll_table.select_next_col(),
            KeyCode::Left => self.scroll_table.select_previous_col(),


            KeyCode::Char('l') => self.scroll_table.scroll_right(),
            KeyCode::Char('h') => self.scroll_table.scroll_left(),

            KeyCode::PageDown => self.scroll_table.scroll_page_down(),
            KeyCode::PageUp => self.scroll_table.scroll_page_up(),
            KeyCode::Home => self.scroll_table.scroll_to_top(),
            KeyCode::End => self.scroll_table.scroll_to_bottom(),

            KeyCode::Char('f') => self.scroll_table.toggle_freeze(),
            KeyCode::Char('i') => self.show_info = !self.show_info,

            _ => {}
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {

        let [main, footer] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());

        let frame_theme = self.scroll_table.frame();
        let block = if frame_theme.external_borders {
            Block::bordered()
                .border_type(frame_theme.border_type)
                .border_style(frame_theme.style)
        } else {
            Block::new().borders(Borders::NONE)
        }
        .title(format!(" {} ", self.name));
        let inner = block.inner(main);
        frame.render_widget(block, main);

        scroll_table::draw(frame, &mut self.scroll_table, inner);

        frame.render_widget(Paragraph::new(self.footer()), footer);

        let view = ViewInfo {
            canvas: self.scroll_table.body_size(),
            viewport: self.scroll_table.viewport(),
            offset: self.scroll_table.offset(),
        };

        if self.show_info {
            view.draw_info(frame, &self);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.scroll_table.is_empty()
    }

    pub fn selected_row(&self) -> Option<usize> {
        if self.is_empty() {
            return None;
        }
        self.scroll_table.selected_row()
    }

    pub fn selected_column(&self) -> Option<usize> {
        if self.scroll_table.n_cols() == 0 {
            return None;
        }
        Some(self.scroll_table.selected_col())
    }

    pub fn frozen_column(&self) -> Option<usize> {
        self.scroll_table.frozen()
    }

    pub fn next_col_idx(&self, off_x: u16) -> usize {
        self.scroll_table.next_col_idx(off_x)
    }

    pub fn last_col_view(&self, off_x: u16, view_w: u16) -> usize {
        self.scroll_table.last_col_view(off_x, view_w)
    }

    pub fn scrollable_columns(&self) -> Vec<usize> {
        self.scroll_table.scrollable_columns()
    }
}