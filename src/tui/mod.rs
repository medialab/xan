mod utils;
mod app;
mod style;
mod view_infos;
mod scroll_table;

pub use utils::print_ratatui_frame_to_stdout;
pub use app::ViewApp;
pub use style::{CLASSIC_VIEWSTYLE, ViewStyle, ScrollTableStyle, BasicTableStyle};