//! `Imagen → Reducir colores…` (specs/hito-4-cuantizador.md): the form, the plan that comes out of
//! it, the two-step accept and the checks. No egui: the form types are `px-ui`'s and it is tested
//! without a window.

use px_core::{Document, Mode, Palette};
use px_raster::{Dither, PaletteSource, QuantizeOptions};
use px_ui::actions::Locks;
use px_ui::dialog::{DialogKind, FieldId, FieldKind, FieldSpec, FormSpec, Modal, Rows, Values};
use px_ui::quantize_es::{
    QUANTIZE_ACCEPT, QUANTIZE_CANCEL, QUANTIZE_COLORS, QUANTIZE_DITHER, QUANTIZE_DITHERS,
    QUANTIZE_ONLY_COLOR, QUANTIZE_PALETTE, QUANTIZE_PALETTES_ONE, QUANTIZE_PALETTES_TWO,
    QUANTIZE_THRESHOLD, QUANTIZE_TITLE,
};

use crate::convert_mode::{SYNC_PIXELS, WAIT_FRAMES, layer_block};

pub const FIELD_COLORS: FieldId = "colores";
pub const FIELD_PALETTE: FieldId = "paleta";
pub const FIELD_DITHER: FieldId = "tramado";
pub const FIELD_THRESHOLD: FieldId = "umbral";
pub const FIELD_REPORT: FieldId = "informe";

/// Pixels above which the reduction is announced as slow (the constant of the other dialogs).
const WARN_PIXELS: u64 = 1 << 24;

/// The dithers in the order of the options of the form.
const DITHERS: [Dither; 3] = [Dither::None, Dither::FloydSteinberg, Dither::Ordered];

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The read-only report: size, layers, saved palette and the warning of a slow reduction.
fn report(width: u32, height: u32, layers: usize, saved: Option<usize>) -> String {
    let mut text = format!(
        "Imagen de {width}×{height}, {}.",
        plural(layers, "capa", "capas")
    );
    if let Some(colors) = saved {
        text.push_str(&format!(
            " Paleta guardada: {}.",
            plural(colors, "color", "colores")
        ));
    }
    let pixels = u64::from(width) * u64::from(height) * layers as u64;
    if pixels > WARN_PIXELS {
        text.push_str(&format!(" aviso: más de {WARN_PIXELS} px; puede tardar"));
    }
    text
}

/// The form for a document of that size and number of layers; `saved` is the number of colors of
/// the saved palette of the session, if it has one.
pub fn quantize_form(width: u32, height: u32, layers: usize, saved: Option<usize>) -> FormSpec {
    let options: &'static [&'static str] = if saved.is_some() {
        &QUANTIZE_PALETTES_TWO
    } else {
        &QUANTIZE_PALETTES_ONE
    };
    FormSpec {
        title: QUANTIZE_TITLE,
        accept: QUANTIZE_ACCEPT,
        cancel: Some(QUANTIZE_CANCEL),
        fields: vec![
            FieldSpec {
                id: FIELD_COLORS,
                label: QUANTIZE_COLORS,
                kind: FieldKind::Number {
                    min: 2,
                    max: 256,
                    default: 256,
                },
                shown_when: None,
            },
            FieldSpec {
                id: FIELD_PALETTE,
                label: QUANTIZE_PALETTE,
                kind: FieldKind::Choice {
                    options,
                    default: 0,
                },
                shown_when: None,
            },
            FieldSpec {
                id: FIELD_DITHER,
                label: QUANTIZE_DITHER,
                kind: FieldKind::Choice {
                    options: &QUANTIZE_DITHERS,
                    default: 0,
                },
                shown_when: None,
            },
            FieldSpec {
                id: FIELD_THRESHOLD,
                label: QUANTIZE_THRESHOLD,
                kind: FieldKind::Number {
                    min: 1,
                    max: 255,
                    default: 128,
                },
                shown_when: None,
            },
            FieldSpec {
                id: FIELD_REPORT,
                label: "Informe",
                kind: FieldKind::ReadOnly {
                    text: report(width, height, layers, saved),
                    rows: Rows::Fixed(4),
                },
                shown_when: None,
            },
        ],
        check: None,
    }
}

/// Opens the dialog; `false` if some lock prevents it.
pub fn open_quantize(
    modal: &mut Modal,
    width: u32,
    height: u32,
    layers: usize,
    saved: Option<usize>,
    locks: Locks,
) -> bool {
    modal.open(
        DialogKind::Quantize,
        quantize_form(width, height, layers, saved),
        locks,
    )
}

/// What the accepted form asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantizePlan {
    pub colors: u16,
    pub dither: Dither,
    pub threshold: u8,
    /// Use the saved palette of the session instead of computing one.
    pub saved_palette: bool,
}

/// The plan that comes out of what was accepted.
pub fn plan_from_values(values: &Values) -> Option<QuantizePlan> {
    let colors = u16::try_from(values.number(FIELD_COLORS)?).ok()?;
    let threshold = u8::try_from(values.number(FIELD_THRESHOLD)?).ok()?;
    let dither = *DITHERS.get(values.choice(FIELD_DITHER)?)?;
    let saved_palette = values.choice(FIELD_PALETTE)? == 1;
    ((2..=256).contains(&colors) && threshold >= 1).then_some(QuantizePlan {
        colors,
        dither,
        threshold,
        saved_palette,
    })
}

/// The options of the quantizer for a plan; `None` if it asks for a saved palette that is not there.
pub fn options_of(plan: &QuantizePlan, saved: Option<&Palette>) -> Option<QuantizeOptions> {
    let palette = if plan.saved_palette {
        PaletteSource::Fixed(saved?.clone())
    } else {
        PaletteSource::Computed
    };
    Some(QuantizeOptions {
        colors: plan.colors,
        dither: plan.dither,
        alpha_threshold: plan.threshold,
        palette,
    })
}

/// Why `doc` cannot be reduced, as a notice: its mode or the properties of its layers.
pub fn can_quantize(doc: &Document) -> Result<(), String> {
    if !matches!(doc.mode(), Mode::Rgba | Mode::GrayAlpha) {
        return Err(QUANTIZE_ONLY_COLOR.to_string());
    }
    match layer_block(doc, Mode::Indexed) {
        Some(block) => Err(block.reason().to_string()),
        None => Ok(()),
    }
}

/// The notice of 5 s after a reduction.
pub fn done_notice(colors: usize, mae_milli: Option<u64>) -> String {
    let mut text = format!("colores reducidos a {colors}");
    if let Some(milli) = mae_milli {
        let tenths = (milli + 50) / 100;
        text.push_str(&format!(" (error medio {},{})", tenths / 10, tenths % 10));
    }
    text
}

/// A reduction that waits `WAIT_FRAMES` frames after `Accept` (the mechanism of `Convirtiendo…`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingQuantize {
    plan: QuantizePlan,
    remaining: u8,
}

impl PendingQuantize {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn plan(&self) -> QuantizePlan {
        self.plan
    }

    /// `Some` exactly at the `WAIT_FRAMES`-th call, and never again.
    pub fn tick(&mut self) -> Option<QuantizePlan> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        (self.remaining == 0).then_some(self.plan)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum QuantizeStart {
    Now(QuantizePlan),
    Later(PendingQuantize),
}

/// Runs now up to `SYNC_PIXELS` pixels; above, after `WAIT_FRAMES` frames, so that the notice is
/// painted before the screen freezes.
pub fn start_quantize(plan: QuantizePlan, pixels: u64) -> QuantizeStart {
    if pixels <= SYNC_PIXELS {
        QuantizeStart::Now(plan)
    } else {
        QuantizeStart::Later(PendingQuantize {
            plan,
            remaining: WAIT_FRAMES,
        })
    }
}

#[cfg(test)]
mod tests;
