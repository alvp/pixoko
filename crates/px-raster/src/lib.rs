//! px-raster: algoritmos de dibujo deterministas. La línea de grosor 1 y el relleno por inundación.

mod draw;
mod fill;
mod line;

pub use draw::{draw_line, line_command};
pub use fill::{fill_command, fill_command_with_limit, flood_fill};
pub use line::Line;
