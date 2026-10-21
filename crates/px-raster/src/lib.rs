//! px-raster: algoritmos de dibujo deterministas. La línea de grosor 1, el relleno por inundación y
//! la composición de capas (`compose`, `compose_rect`, `composite_pixel`).

mod compose;
mod draw;
mod fill;
mod line;

pub use compose::{ComposeError, Rect, compose, compose_rect, composite_pixel};
pub use draw::{draw_line, line_command};
pub use fill::{fill_command, fill_command_with_limit, flood_fill};
pub use line::Line;
