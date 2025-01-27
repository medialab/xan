use std::io::Result;

use colored::{ColoredString, Colorize};

use ratatui::backend::TestBackend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::style::{Color, Modifier};
use ratatui::{Frame, Terminal};

fn print_buffer_to_stdout(buffer: &Buffer, cols: usize) {
    let contents = &buffer.content;

    let mut i: usize = 0;

    fn group_cells_by_color(cells: &[Cell]) -> Vec<Vec<Cell>> {
        let mut groups: Vec<Vec<Cell>> = Vec::new();
        let mut current_run: Vec<Cell> = Vec::new();

        for cell in cells {
            if current_run.is_empty() || (current_run[0].style() == cell.style()) {
                current_run.push(cell.clone());
                continue;
            }

            groups.push(current_run);

            current_run = vec![cell.clone()];
        }

        if !current_run.is_empty() {
            groups.push(current_run);
        }

        groups
    }

    fn colorize(string: &str, color: Color, modifer: Modifier) -> ColoredString {
        let string = match color {
            Color::Reset | Color::White => Colorize::normal(string),
            Color::Red => Colorize::red(string),
            Color::Blue => Colorize::blue(string),
            Color::Cyan => Colorize::cyan(string),
            Color::Green => Colorize::green(string),
            Color::Yellow => Colorize::yellow(string),
            Color::Magenta => Colorize::magenta(string),
            _ => unimplemented!(),
        };

        if modifer.is_empty() {
            return string;
        }

        match modifer {
            Modifier::DIM => Colorize::dimmed(string),
            _ => unimplemented!(),
        }
    }

    while i < contents.len() {
        let line = group_cells_by_color(&contents[i..(i + cols)])
            .iter()
            .map(|cells| {
                colorize(
                    &cells.iter().map(|cell| cell.symbol()).collect::<String>(),
                    cells[0].fg,
                    cells[0].modifier,
                )
                .to_string()
            })
            .collect::<String>();

        println!("{}", line);

        i += cols;
    }
}

pub fn print_ratatui_frame_to_stdout<F>(cols: usize, rows: usize, callback: F) -> Result<()>
where
    F: FnOnce(&mut Frame),
{
    let mut terminal = Terminal::new(TestBackend::new(cols as u16, rows as u16))?;

    terminal.draw(callback)?;

    let buffer = terminal.backend().buffer();

    print_buffer_to_stdout(buffer, cols);

    Ok(())
}
