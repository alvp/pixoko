//! px-raster: algoritmos de dibujo deterministas. Por ahora, la línea de grosor 1.

mod draw;
mod line;

pub use draw::{draw_line, line_command};
pub use line::Line;
