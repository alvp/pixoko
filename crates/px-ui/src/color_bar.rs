//! The color bar under the grid (specs/ui-palette-handle.md, decision 8), without egui types: the
//! text it shows and the ink of that text. The bar is painted with the **stored** color of what the
//! editor edits and says which entry or slot it is.

use egui::Color32;
use px_core::Rgba;

use crate::color_edit::EditTarget;
use crate::theme::{SURFACE_0, TEXT, contrast_ratio};

const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
const WHITE: Color32 = Color32::from_rgb(255, 255, 255);

/// The text of the bar: `Idx-10` for an entry (counted from 0, like the rest of the panel),
/// `Idx-3 transp.` for the transparent one and `Col-3` for a slot.
pub fn bar_label(target: EditTarget, transparent: bool) -> String {
    match target {
        EditTarget::Entry(index) if transparent => format!("Idx-{index} transp."),
        EditTarget::Entry(index) => format!("Idx-{index}"),
        EditTarget::Slot(slot) => format!("Col-{slot}"),
    }
}

/// The ink of the text of the bar for a stored color: what is behind the text and the color of the
/// text. An opaque color is the background of the text and the text is black or white, the one
/// with more contrast (at least 4,58:1 for any opaque color). With alpha under 255 the bar carries
/// the checker of the swatches and the text goes in an opaque box of `SURFACE_0` with `TEXT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarInk {
    pub text: Color32,
    pub behind: Color32,
    /// The text goes in a box (the color has alpha).
    pub boxed: bool,
}

pub fn bar_ink(stored: Rgba) -> BarInk {
    if stored.a < 255 {
        return BarInk {
            text: TEXT,
            behind: SURFACE_0,
            boxed: true,
        };
    }
    let behind = Color32::from_rgb(stored.r, stored.g, stored.b);
    let text = if contrast_ratio(BLACK, behind) >= contrast_ratio(WHITE, behind) {
        BLACK
    } else {
        WHITE
    };
    BarInk {
        text,
        behind,
        boxed: false,
    }
}
