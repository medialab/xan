use ratatui::style::{Color, Style};
use ratatui::widgets::BorderType;

#[derive(Debug)]
pub struct BasicTableStyle {
    pub header: Style,
    pub body_text: Style,
    pub body_row_highlight: Style,
}

#[derive(Debug)]
pub struct ScrollTableStyle {
    pub table: BasicTableStyle,
    pub body_col_highlight: Style,
    pub body_cell_highlight: Style,
    pub frozen: BasicTableStyle,
    pub separator: Style,
}

#[derive(Debug)]
pub struct ViewStyle {
    pub scroll_table: ScrollTableStyle,
}

const CLASSIC_BASIC_TABLE_STYLE: BasicTableStyle = BasicTableStyle {
    header: Style::new().bold().white().on_blue(),
    body_text: Style::new().white(),
    body_row_highlight: Style::new().bold().bg(Color::Black),
};

const CLASSIC_FROZEN_STYLE: BasicTableStyle = BasicTableStyle {
    header: Style::new().italic().bold().reversed().green(),
    body_text: Style::new().italic(),
    body_row_highlight: Style::new().on_light_yellow().bold(),
};

const CLASSIC_SCROLLTABLE_STYLE: ScrollTableStyle = ScrollTableStyle {
    table: CLASSIC_BASIC_TABLE_STYLE,
    body_col_highlight: Style::new().bg(Color::Black),
    body_cell_highlight: Style::new().reversed(),
    frozen: CLASSIC_FROZEN_STYLE,
    separator: Style::new().red(),
};

pub const CLASSIC_VIEWSTYLE: ViewStyle = ViewStyle {
    scroll_table: CLASSIC_SCROLLTABLE_STYLE,
};

#[derive(Debug, Clone)]
pub struct FrameTheme {
    pub vertical: char,
    pub horizontal: char,
    pub cross: char,
    pub tee_down: char,
    pub tee_up: char,
    pub tee_right: char,
    pub tee_left: char,
    pub padding: usize,
    pub external_borders: bool,
    pub border_type: BorderType,
    pub hr_under_headers: bool,
    pub striped: bool,
    pub style: Style,
}

impl FrameTheme {
    pub fn col_spacing(&self) -> usize {
        2 * self.padding + 1
    }
}

impl Default for FrameTheme {
    fn default() -> Self {
        Self {
            vertical: '│',
            horizontal: '─',
            cross: '┼',
            tee_down: '┬',
            tee_up: '┴',
            tee_right: '├',
            tee_left: '┤',
            padding: 1,
            external_borders: true,
            border_type: BorderType::Plain,
            hr_under_headers: true,
            striped: false,
            style: Style::new().fg(Color::DarkGray),
        }
    }
}

pub const STRIPED_ROW_STYLE: Style = Style::new().reversed();
