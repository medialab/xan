use ratatui::style::{Color, Style};

#[derive(Debug)]
pub struct BasicTableStyle {
    pub header: Style,
    pub body_text: Style,
    pub body_row_highlight: Style,
}

const CLASSIC_BASIC_TABLE_STYLE: BasicTableStyle = BasicTableStyle {
    header: Style::new().bold().reversed().blue(),
    body_text: Style::new().white(),
    body_row_highlight: Style::new().on_gray().bold(),
};

const CLASSIC_FROZEN_STYLE: BasicTableStyle = BasicTableStyle {
    header: Style::new().bold().reversed().green(),
    body_text: Style::new().fg(Color::Green),
    body_row_highlight: Style::new().on_yellow().bold(),

};

#[derive(Debug)]
pub struct ScrollTableStyle {
    pub table: BasicTableStyle,
    pub col_spacing: usize,
    pub body_col_highlight: Style,
    pub body_cell_highlight: Style,
    pub highlight_symbol: &'static str,
}

const CLASSIC_SCROLLTABLE_STYLE: ScrollTableStyle = ScrollTableStyle {
    table: CLASSIC_BASIC_TABLE_STYLE,
    col_spacing: 1,
    body_col_highlight: Style::new().red(),
    body_cell_highlight: Style::new().reversed().yellow(),
    highlight_symbol: "# ",
};

#[derive(Debug)]
pub struct ViewStyle {
    pub scroll_table: ScrollTableStyle,
    pub frozen: BasicTableStyle,
}

pub const CLASSIC_VIEWSTYLE: ViewStyle = ViewStyle {
    scroll_table: CLASSIC_SCROLLTABLE_STYLE,
    frozen: CLASSIC_FROZEN_STYLE,
};



