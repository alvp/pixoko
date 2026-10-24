//! px-raster: algoritmos de dibujo deterministas. La línea de grosor 1, el relleno por inundación,
//! la composición de capas (`compose`, `compose_rect`, `compose_layers`, `composite_pixel`) y los
//! comandos de combinar y aplanar capas (`merge_down_command`, `merge_visible_command`,
//! `flatten_command`).

mod compose;
mod draw;
mod fill;
mod image_ops;
mod line;
mod merge;

pub use compose::{ComposeError, Rect, compose, compose_layers, compose_rect, composite_pixel};
pub use draw::{draw_line, line_command};
pub use fill::{fill_command, fill_command_with_limit, flood_fill};
pub use image_ops::{
    Anchor, ImageOp, ImageOpError, flip_horizontal, flip_vertical, image_command, resize_canvas,
    rotate_180, rotate_ccw, rotate_cw,
};
pub use line::Line;
pub use merge::{MergeError, flatten_command, merge_down_command, merge_visible_command};
