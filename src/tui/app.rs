use crate::CliResult;
use crate::cmd::view::DisplayedColumn;
use crate::tui::{ViewStyle, ScrollTableStyle, BasicTableStyle, view_debug::ViewInfo, scroll_table::ScrollTable};

use std::time::Duration;
use simd_csv::StringRecord;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph},
    text::Text,
    crossterm::event::{self, Event, KeyEventKind, KeyCode},
    DefaultTerminal,
};

fn list_to_string<I>(list: I) -> String
where I: Iterator, I::Item: ToString,
{
    list.map(|w| w.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug)]
pub struct App {
    scroll_table: ScrollTable,
    frozen: Option<usize>,
    show_info: bool,
    quit: bool,

    infos: Vec<(&'static str, String)>,
}

impl App {
    pub fn new(headers: StringRecord, rows: Vec<StringRecord>, cols: Vec<DisplayedColumn>, style: &'static ViewStyle) -> Self {
        let scroll_table = ScrollTable::new(headers, rows, cols, &style.scroll_table);
        let infos = vec![
            ("Lignes", "".to_string()),
            ("Colonnes", "".to_string()),
            ("Largeur colonne", "".to_string()),
            ("Colonne figée", "".to_string()),
        ];
        Self { scroll_table, frozen: None, show_info: false, quit: false, infos}
    }

    pub fn get_infos(&self) -> &Vec<(&'static str, String)> {
        &self.infos
    }

    #[cfg(debug_assertions)]
    fn log(&mut self, label: &'static str, info: String) {
        self.infos.push((label, info));
    }


    fn footer(&self, frozen_name: &str) -> Text<'static> {
        Text::raw(format!(
            " {} lines | {} columns | Home/End : top/end | Arrows : scroll | q : quit | Frozen : {frozen_name}",
            self.scroll_table.n_rows(), self.scroll_table.n_cols()))
    }

    pub fn scrollable_columns(&self) -> Vec<usize> {
        (0..self.scroll_table.n_cols()).filter(|&i| Some(i) != self.frozen).collect()
    }

    fn toggle_freeze(&mut self) {
        if self.frozen.take().is_some() || self.scroll_table.n_cols() < 2 {
            return;
        }


        let mut off = self.scroll_table.offset();

        self.frozen = self.scroll_table
            .selected_col()
            .and_then(|selected_col| {
                off.x = self.scroll_table.width_until(selected_col) as u16;
                self.scroll_table.set_offset(off);
                Some(selected_col)
            });
    }

    fn get_frozen_space(&self) -> u16 {
        if let Some(frozen_col) = self.frozen {
            self.scroll_table.col_step(frozen_col).saturating_add(1) as u16
        } else { 0 }
    }

    fn update_infos(&mut self, infos: [String; 4]) {
        for ((_, info), new_info) in self.infos.iter_mut().take(4).zip(infos) {
            *info = new_info;
        }
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
                        self.handle_key(key.code, viewport_width);
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

            // KeyCode::Char('l') => self.scroll_table.scroll_right(),
            // KeyCode::Char('h') => self.scroll_table.scroll_left(),

            KeyCode::PageDown => self.scroll_table.scroll_page_down(),
            KeyCode::PageUp => self.scroll_table.scroll_page_up(),
            // KeyCode::Home => self.scroll_table.scroll_to_top(),
            // KeyCode::End => self.scroll_table.scroll_to_bottom(),

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

        self.scroll_table.draw(&scrollable, frame, right);

        let frozen_name = if let Some(frozen_col) = self.frozen {
                self.scroll_table.draw_frozen(frozen_col, frame, left);
                self.scroll_table.header(frozen_col)
            } else { None }
            .unwrap_or("-");

        frame.render_widget(Paragraph::new(self.footer(frozen_name)), footer);

        let view = ViewInfo {
            canvas: self.scroll_table.body_size(),
            viewport: self.scroll_table.viewport(),
            offset: self.scroll_table.offset(),
        };

        if self.show_info {
            let infos: [String; 4] = [
                self.scroll_table.n_rows().to_string(),
                self.scroll_table.n_cols().to_string(),
                list_to_string(self.scroll_table.col_widths()),
                frozen_name.to_string(),
            ];

            self.update_infos(infos);

            view.draw_info(frame, &self);
        }
    }

    pub fn no_rows(&self) -> bool {
        self.scroll_table.no_rows()
    }
}