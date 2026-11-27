use ratatui::{
    layout::{Constraint, Flex, Layout, Position, Rect, Size},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Padding, Paragraph},
    Frame,
};

use crate::tui::app::ViewApp;

pub struct ViewInfo {
    pub canvas: Size,
    pub viewport: Size,
    pub offset: Position,
}

fn popup_area(area: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    area
}

/// Largeur des labels (le plus long : "Number of columns").
const LABEL_WIDTH: usize = 18;
/// Largeur de la première valeur d'un `double_kv`.
const VALUE_WIDTH: usize = 10;

const KEYS: &[(&str, &str)] = &[
    ("↑ / ↓", "select previous / next row"),
    ("← / →", "select previous / next column"),
    ("Tab / Shift+Tab", "scroll to next / previous column"),
    ("h / l", "scroll left / right"),
    ("PgUp / PgDn", "scroll one page up / down"),
    ("Home / End", "go to top / bottom"),
    ("f", "freeze / unfreeze selected column"),
    ("i", "show / hide infos"),
    ("q / Esc", "quit"),
];

fn kv(label: &str, value: impl std::fmt::Display) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<LABEL_WIDTH$}"), Style::new().green()),
        Span::raw(value.to_string()),
    ])
}

fn double_kv(
    label1: &str, value1: impl std::fmt::Display,
    label2: &str, value2: impl std::fmt::Display,
) -> Line<'static> {
    let label_style = Style::new().green();
    Line::from(vec![
        Span::styled(format!("{label1:<LABEL_WIDTH$}"), label_style),
        Span::raw(format!("{:<VALUE_WIDTH$}", value1.to_string())),
        Span::styled(format!("{label2:<LABEL_WIDTH$}"), label_style),
        Span::raw(value2.to_string()),
    ])
}

fn keys_help() -> Vec<Line<'static>> {
    let key_style = Style::new().fg(Color::Cyan);
    std::iter::once(Line::styled("Keys", Style::new().yellow()))
        .chain(KEYS.iter().map(|(key, action)| {
            Line::from(vec![
                Span::styled(format!("{key:<LABEL_WIDTH$}"), key_style),
                Span::raw(*action),
            ])
        }))
        .collect()
}

fn opt(value: Option<usize>) -> String {
    value.map_or_else(|| "-".to_string(), |v| v.to_string())
}

impl ViewInfo {
    fn get_all_infos(&self, app: &ViewApp) -> Vec<Line<'static>> {
        let (x, y) = (self.offset.x, self.offset.y);
        let (vw, vh) = (self.viewport.width, self.viewport.height);
        let max_x = self.canvas.width.saturating_sub(self.viewport.width);
        let max_y = self.canvas.height.saturating_sub(self.viewport.height);

        let visible_rows = if app.is_empty() {
            "-".to_string()
        } else {
            format!("{}-{}", y, (y + vh).min(app.n_rows() as u16).saturating_sub(1))
        };
        let visible_cols = if app.is_empty() {
            "-".to_string()
        } else {
            format!("{}-{}", app.next_col_idx(x) - 1, app.last_col_view(x, vw))
        };

        let mut lines = vec![
            double_kv("Number of rows", app.n_rows(),
                      "Number of columns", app.n_cols()),
            double_kv("Visible rows", visible_rows,
                      "Visible columns", visible_cols),
            double_kv("Selected row", opt(app.selected_row().map(|r| r + 1)),
                      "Selected column", opt(app.selected_column())),
            Line::raw(""),
            kv("Frozen column", opt(app.frozen_column())),
            Line::raw(""),
        ];
        #[cfg(debug_assertions)]
        lines.extend(vec![double_kv("Canvas body", format!("{}x{}", self.canvas.width, self.canvas.height),
                                         "Viewport body", format!("{vw}x{vh}")),
                                double_kv("Offset", format!("x={x} y={y}"),
                                        "Offset max", format!("x={max_x} y={max_y}")),
                                Line::raw("")]);
        lines.extend(keys_help()); lines
    }

    pub fn draw_info(&self, frame: &mut Frame, app: &ViewApp) {

        let lines = self.get_all_infos(app);

        let title = " Infos · i pour fermer ";
        // largeur du contenu + bordures (2) + padding horizontal (2)
        let content_width = lines.iter().map(Line::width).max().unwrap_or(0)
            .max(title.chars().count());
        let width = (content_width + 4).min(u16::MAX as usize) as u16;
        let height = (lines.len() + 2).min(u16::MAX as usize) as u16;

        let area = popup_area(frame.area(), width, height);
        let block = Block::bordered()
            .title(title)
            .padding(Padding::horizontal(1))
            .border_style(Style::new().fg(Color::Yellow));
        frame.render_widget(Clear, area); // efface ce qui est dessous
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }
}
