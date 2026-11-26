use ratatui::style::{Color, Style};

#[derive(Debug)]
pub struct BasicTableStyle {
    pub header: Style,
    pub body_text: Style,
    pub body_row_highlight: Style,
}

#[derive(Debug)]
pub struct ScrollTableStyle {
    pub table: BasicTableStyle,
    pub col_spacing: usize,
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
    body_row_highlight: Style::new().on_light_yellow().bold(),
};

const CLASSIC_FROZEN_STYLE: BasicTableStyle = BasicTableStyle {
    header: Style::new().italic().bold().reversed().green(),
    body_text: Style::new().italic(),
    body_row_highlight: Style::new().on_light_yellow().bold(),
};

const CLASSIC_SCROLLTABLE_STYLE: ScrollTableStyle = ScrollTableStyle {
    table: CLASSIC_BASIC_TABLE_STYLE,
    col_spacing: 1,
    body_col_highlight: Style::new().bg(Color::DarkGray),
    body_cell_highlight: Style::new().bold().on_yellow(),
    frozen: CLASSIC_FROZEN_STYLE,
    separator: Style::new().red(),
};

pub const CLASSIC_VIEWSTYLE: ViewStyle = ViewStyle {
    scroll_table: CLASSIC_SCROLLTABLE_STYLE,
};
