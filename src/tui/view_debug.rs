use ratatui::{
    layout::{Constraint, Flex, Layout, Position, Rect, Size},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

use crate::tui::app::App;

pub struct ViewInfo {
    /// Taille du contenu virtuel du body.
    pub canvas: Size,
    /// Partie visible du body (zone moins les scrollbars).
    pub viewport: Size,
    /// Offset du body après clamp par le ScrollView.
    pub offset: Position,
}

fn popup_area(area: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Start)
        .areas(area);
    area
}

fn kv(label: &str, value: impl std::fmt::Display) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<19}"), Style::new().green()),
        Span::raw(value.to_string()),
    ])
}

impl ViewInfo {
    fn get_all_infos(&self, app: &App) -> Vec<Line<'static>> {
        let scrollable = app.scrollable_columns();

        let (x, y) = (self.offset.x, self.offset.y);
        let (vw, vh) = (self.viewport.width, self.viewport.height);
        let max_x = self.canvas.width.saturating_sub(self.viewport.width);
        let max_y = self.canvas.height.saturating_sub(self.viewport.height);

        let visible_rows = if app.no_rows() {
            "-".to_string()
        } else {
            format!("{}-{} / {}", y + 1, (y + vh).min(app.n_rows()), app.n_rows())
        };
        let first = app.get_left_col(x);
        //let last = ((x + vw).saturating_sub(1) / step).min(scrollable.len().saturating_sub(1));
        let last = first + 3;
        let visible_cols: Vec<String> = scrollable
            .get(first..=last)
            .unwrap_or_default()
            .iter()
            .map(|c| c.to_string())
            .collect();
        let visible_cols = if visible_cols.is_empty() { "-".to_string() } else { visible_cols.join(", ") };

        vec![
                Line::raw(""),
                kv("Canvas body", format!("{}x{}", self.canvas.width, self.canvas.height)),
                kv("Viewport body", format!("{vw}x{vh}")),
                kv("Offset", format!("x={x} y={y}")),
                kv("Offset max", format!("x={max_x} y={max_y}")),
                Line::raw(""),
                kv("Lignes visibles", visible_rows),
                kv("Colonnes visibles", visible_cols),
            ]
    }

    pub fn draw_info(&self, frame: &mut Frame, app: &App) {

        let lines = app.get_infos().into_iter()
                 .map(|(label, info)| kv(label, info))
                 .chain(self.get_all_infos(app))
                 .collect::<Vec<_>>();

        let area = popup_area(frame.area(), 162, lines.len() as u16 + 2);
        let block = Block::bordered()
            .title(" Infos · i pour fermer ")
            .border_style(Style::new().fg(Color::Yellow));
        frame.render_widget(Clear, area); // efface ce qui est dessous
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }
}
