use ratatui::style::{Color, Style                   }  ;

#[derive(Debug)]
pub struct ViewStyle {
    pub col_spacing: usize,
    pub header: Style,
    pub selected: Style,
    pub frozen: Style,
    pub body: Style,
}

pub const CLASSIC_VIEWSTYLE: ViewStyle = ViewStyle {
    col_spacing: 1,
    header: Style::new().bold().reversed().blue(),
    selected: Style::new().magenta().reversed(),
    frozen: Style::new().fg(Color::Green),
    body: Style::new().red(),
};



