//! `Imagen → Reducir colores…` on the session (specs/hito-4-cuantizador.md): the whole document in
//! a single step, with a notice that says how many colors and how much error.

use px_raster::{QuantizeOptions, quantize_command};
use px_ui::image_ops_es::IMAGE_OP_NOTHING;

use super::{LayerReport, Session, default_background};
use crate::quantize_flow::done_notice;

impl Session {
    /// Reduces the colors of every layer in a single step. A refusal (mode, layer properties,
    /// memory) gives the notice and leaves the document intact; after a change the choice of
    /// color is adjusted to the new mode like after an undo.
    pub fn quantize(&mut self, options: &QuantizeOptions) -> LayerReport {
        let (command, stats) = match quantize_command(&self.document, options) {
            Ok(done) => done,
            Err(error) => return LayerReport::refused(&error.to_string(), false),
        };
        let before = self.choice;
        match self.document.apply(command) {
            Ok(true) => {
                self.note_applied(self.full());
                let layers = self.layer_ids();
                self.after_history_move(before, &layers);
                self.background = default_background(self.document.canvas_image());
                LayerReport {
                    changed: true,
                    notice: Some(done_notice(stats.colors, stats.mae_milli())),
                    offer_convert: false,
                }
            }
            Ok(false) => LayerReport::refused(IMAGE_OP_NOTHING, false),
            Err(error) => LayerReport::refused(&error.to_string(), false),
        }
    }
}
