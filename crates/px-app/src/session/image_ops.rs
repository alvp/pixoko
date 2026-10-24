//! The image operations of `Imagen` (specs/hito-4-imagen.md): flips, turns and canvas resizing
//! over every layer, one undo step each.

use px_core::{Mode, Pixel, Rgba};
use px_raster::{ImageOp, image_command};
use px_ui::image_ops_es::IMAGE_OP_NOTHING;

use super::{LayerReport, Role, Session};

impl Session {
    /// What the new area of a canvas resize takes (decision D4): transparent where the mode has
    /// it, and otherwise the background color of the session.
    fn canvas_fill(&self) -> Pixel {
        let image = self.document.canvas_image();
        match image.mode() {
            Mode::Rgba => Pixel::Rgba(Rgba::TRANSPARENT),
            Mode::GrayAlpha => Pixel::GrayAlpha { gray: 0, alpha: 0 },
            Mode::Indexed => image
                .palette()
                .and_then(|palette| palette.transparent_index())
                .map(Pixel::Indexed)
                .or_else(|| self.pixel_of(Role::Background))
                .unwrap_or(Pixel::Indexed(0)),
            Mode::Gray => self.pixel_of(Role::Background).unwrap_or(Pixel::Gray(255)),
        }
    }

    /// Applies `op` to the whole document in a single step. An operation that changes nothing says
    /// so; a refusal (memory, size) gives the notice and leaves the document intact.
    pub fn image_op(&mut self, op: ImageOp) -> LayerReport {
        let command = match image_command(&self.document, op, self.canvas_fill()) {
            Ok(command) => command,
            Err(error) => return LayerReport::refused(&error.to_string(), false),
        };
        match self.apply_layer(command) {
            Ok(false) => LayerReport::refused(IMAGE_OP_NOTHING, false),
            other => LayerReport::from_result(other),
        }
    }
}
