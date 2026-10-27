//! px-raster: algoritmos de dibujo deterministas. La línea de grosor 1, el relleno por inundación,
//! la composición de capas (`compose`, `compose_rect`, `compose_layers`, `composite_pixel`) y los
//! el pincel y los trazos (`Brush`, `stamp_pixels`, `spaced_centers`, `curve_path`), los
//! comandos de combinar y aplanar capas (`merge_down_command`, `merge_visible_command`,
//! `flatten_command`) y el cuantizador de color (`quantize_cels`, `quantize_command`).

mod brush;
mod compose;
mod draw;
mod fill;
mod image_ops;
mod line;
mod merge;
mod quantize;
mod shapes;

pub use brush::{
    Brush, BrushShape, Spacing, curve_path, line_path, line_path_near, spaced_centers, stamp_pixels,
};
pub use compose::{ComposeError, Rect, compose, compose_layers, compose_rect, composite_pixel};
pub use draw::{draw_line, line_command};
pub use fill::{fill_command, fill_command_with_limit, flood_fill};
pub use image_ops::{
    Anchor, ImageOp, ImageOpError, flip_horizontal, flip_vertical, image_command, resize_canvas,
    rotate_180, rotate_ccw, rotate_cw,
};
pub use line::Line;
pub use merge::{MergeError, flatten_command, merge_down_command, merge_visible_command};
pub use quantize::{
    Dither, ErrorStats, PaletteSource, QuantizeError, QuantizeOptions, QuantizeStats, Quantized,
    mean_abs_error, quantize_cels, quantize_command,
};
pub use shapes::{
    MAX_RADIUS, circle_radius, ellipse_ring, ellipse_spans, polygon_ring, polygon_spans, rect_ring,
    rect_spans, spaced_centers_closed,
};
