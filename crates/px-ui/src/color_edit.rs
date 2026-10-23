//! Pure logic of the color editor: target, controls, drag and hexadecimal, without egui types
//! (specs/hito-2-editar-color.md; the HSV model, the square and the bars are C4,
//! specs/ui-color-block.md).

use px_core::{Hsl, Image, Mode, Rgba};

use crate::color::{ColorChoice, SlotColors};
use crate::color_hsv::{Hsv, hsv_after_rgb};
use crate::theme::NOTICE_PREFIX;

/// What is edited: an entry of the palette (indexed images) or a slot (the other modes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditTarget {
    Entry(u8),
    Slot(u8),
}

impl EditTarget {
    /// Target of the choice: in indexed images, the entry (`Slot(n)` counts as `Entry(n − 1)`); in
    /// the other modes, slot 1 to 9. `None` if it does not exist.
    pub fn of(image: &Image, choice: ColorChoice) -> Option<EditTarget> {
        match image.mode() {
            Mode::Indexed => {
                let len = image.palette()?.len();
                let idx = match choice {
                    ColorChoice::Entry(i) => usize::from(i),
                    ColorChoice::Slot(n) => {
                        if !(1..=9).contains(&n) {
                            return None;
                        }
                        usize::from(n) - 1
                    }
                };
                if idx < len {
                    u8::try_from(idx).ok().map(EditTarget::Entry)
                } else {
                    None
                }
            }
            _ => match choice {
                ColorChoice::Slot(n) if (1..=9).contains(&n) => Some(EditTarget::Slot(n)),
                _ => None,
            },
        }
    }
    /// **Stored** color of the target (`Palette::get` or the slot), not the resolved one.
    pub fn stored(self, image: &Image, slots: &SlotColors) -> Option<Rgba> {
        match self {
            EditTarget::Entry(i) => image.palette()?.get(i).ok(),
            EditTarget::Slot(n) => slots.get(n),
        }
    }
    /// Header of the editor: `editar índice 12`, `editar color 3`, `editar color 4 gris`.
    pub fn header(self, mode: Mode) -> String {
        match self {
            EditTarget::Entry(i) => format!("editar índice {i}"),
            EditTarget::Slot(n) if mode == Mode::Gray => format!("editar color {n} gris"),
            EditTarget::Slot(n) => format!("editar color {n}"),
        }
    }
}

/// A slider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Red,
    Green,
    Blue,
    Hue,
    Saturation,
    Lightness,
    Alpha,
    Gray,
    /// Hue, saturation and value of the HSV model (C4); `Hue`, `Saturation` and `Lightness` stay
    /// the ones of HSL.
    HueV,
    SaturationV,
    Value,
}

/// Which model the numeric sliders of indexed and RGBA images show (specs/ui-color-block.md,
/// decision 15). It is remembered in the session, not saved to disk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SliderMode {
    #[default]
    Rgb,
    Hsl,
    Hsv,
}

/// The sliders that are seen with the selector in `slider_mode`: `R G B A`, `H S L A` or `H S V A`
/// (indexed and RGBA); `G A` and `G` in the gray modes, which have no selector.
pub fn visible_controls(mode: Mode, slider_mode: SliderMode) -> &'static [Control] {
    use Control::*;
    match (mode, slider_mode) {
        (Mode::Rgba | Mode::Indexed, SliderMode::Rgb) => &[Red, Green, Blue, Alpha],
        (Mode::Rgba | Mode::Indexed, SliderMode::Hsl) => &[Hue, Saturation, Lightness, Alpha],
        (Mode::Rgba | Mode::Indexed, SliderMode::Hsv) => &[HueV, SaturationV, Value, Alpha],
        (Mode::GrayAlpha, _) => &[Gray, Alpha],
        (Mode::Gray, _) => &[Gray],
    }
}

/// Sliders of a mode, top to bottom, with the three models together: `R G B H S L A` (indexed and
/// RGBA), `G A` (GrayAlpha) or `G` (Gray). The editor shows [`visible_controls`].
pub fn controls(mode: Mode) -> &'static [Control] {
    use Control::*;
    match mode {
        Mode::Rgba | Mode::Indexed => &[Red, Green, Blue, Hue, Saturation, Lightness, Alpha],
        Mode::GrayAlpha => &[Gray, Alpha],
        Mode::Gray => &[Gray],
    }
}

impl Control {
    /// Letter of the slider.
    pub fn label(self) -> char {
        match self {
            Control::Red => 'R',
            Control::Green => 'G',
            Control::Blue => 'B',
            Control::Hue | Control::HueV => 'H',
            Control::Saturation | Control::SaturationV => 'S',
            Control::Lightness => 'L',
            Control::Alpha => 'A',
            Control::Gray => 'G',
            Control::Value => 'V',
        }
    }
    /// Maximum value: 255, 359 (hue) or 100 (saturation, lightness and value).
    pub fn max(self) -> u32 {
        match self {
            Control::Hue | Control::HueV => 359,
            Control::Saturation | Control::Lightness | Control::SaturationV | Control::Value => 100,
            _ => 255,
        }
    }
}

/// Integer value that `control` shows for that color. The HSV controls are derived from the
/// `Rgba` here; the editor uses [`control_value_hsv`] with its own `Hsv`.
pub fn control_value(rgba: Rgba, hsl: Hsl, control: Control) -> u32 {
    match control {
        Control::Red | Control::Gray => u32::from(rgba.r),
        Control::Green => u32::from(rgba.g),
        Control::Blue => u32::from(rgba.b),
        Control::Alpha => u32::from(rgba.a),
        Control::Hue => (hsl.h().round() as u32) % 360,
        Control::Saturation => (100.0 * hsl.s()).round() as u32,
        Control::Lightness => (100.0 * hsl.l()).round() as u32,
        Control::HueV | Control::SaturationV | Control::Value => {
            control_value_hsv(Hsv::from_rgba(rgba), control)
        }
    }
}

/// Integer value that a control of the HSV model shows for that `Hsv` (0 for the others).
pub fn control_value_hsv(hsv: Hsv, control: Control) -> u32 {
    match control {
        Control::HueV => (hsv.h().round() as u32) % 360,
        Control::SaturationV => (100.0_f32 * hsv.s()).round() as u32,
        Control::Value => (100.0_f32 * hsv.v()).round() as u32,
        _ => 0,
    }
}

/// Value in 3 columns, right aligned.
pub fn value_text(value: u32) -> String {
    format!("{value:>3}")
}

/// Value under position `x_px` of a track of `width_px` pixels (clamped to the track).
pub fn slider_value_at(x_px: i64, width_px: i64, max: u32) -> u32 {
    if width_px <= 1 {
        return 0;
    }
    let span = i128::from(width_px - 1);
    let x = i128::from(x_px.clamp(0, width_px - 1));
    ((2 * x * i128::from(max) + span) / (2 * span)) as u32
}

/// Position, in pixels from the start of the track, of `value`.
pub fn slider_x(value: u32, width_px: i64, max: u32) -> i64 {
    if width_px <= 1 {
        return 0;
    }
    let span = i128::from(width_px - 1);
    let m = i128::from(max.max(1));
    ((2 * i128::from(value.min(max)) * span + m) / (2 * m)) as i64
}

/// `Hsl` after editing RGB: the one of `from_rgba`, except gray (keeps the previous hue) and black
/// or white (keep the previous saturation too).
pub fn hsl_after_rgb(previous: Hsl, new: Rgba) -> Hsl {
    let h = Hsl::from_rgba(new);
    if new.r == new.g && new.g == new.b {
        let s = if new.r == 0 || new.r == 255 {
            previous.s()
        } else {
            0.0
        };
        Hsl::new(previous.h(), s, h.l())
    } else {
        h
    }
}

/// Provisional color (and its `Hsl`) after setting `control` to `value` (clamped to the maximum)
/// from `base`: it only depends on `base`, the control and the value, not on the path.
pub fn apply_control(base: (Rgba, Hsl), control: Control, value: u32) -> (Rgba, Hsl) {
    let (rgba, hsl) = base;
    let v = value.min(control.max());
    let c = u8::try_from(v).unwrap_or(u8::MAX);
    match control {
        Control::Red => {
            let n = Rgba::new(c, rgba.g, rgba.b, rgba.a);
            (n, hsl_after_rgb(hsl, n))
        }
        Control::Green => {
            let n = Rgba::new(rgba.r, c, rgba.b, rgba.a);
            (n, hsl_after_rgb(hsl, n))
        }
        Control::Blue => {
            let n = Rgba::new(rgba.r, rgba.g, c, rgba.a);
            (n, hsl_after_rgb(hsl, n))
        }
        Control::Gray => {
            let n = Rgba::new(c, c, c, rgba.a);
            (n, hsl_after_rgb(hsl, n))
        }
        Control::Alpha => (Rgba::new(rgba.r, rgba.g, rgba.b, c), hsl),
        Control::Hue => {
            let h = Hsl::new(v as f32, hsl.s(), hsl.l());
            (h.to_rgba(rgba.a), h)
        }
        Control::Saturation => {
            let h = Hsl::new(hsl.h(), v as f32 / 100.0, hsl.l());
            (h.to_rgba(rgba.a), h)
        }
        Control::Lightness => {
            let h = Hsl::new(hsl.h(), hsl.s(), v as f32 / 100.0);
            (h.to_rgba(rgba.a), h)
        }
        Control::HueV | Control::SaturationV | Control::Value => {
            unreachable!("the HSV controls go through `apply_control_aux`")
        }
    }
}

/// The `Hsv` after an edit of the `Hsl` hue: the same hue in both models (decision 10, rule c)
/// whenever that keeps `hsv.to_rgba(a) == rgba`; if rounding to 8 bits moved the color enough that
/// it does not, the one derived from the `Rgba`.
fn hsv_with_hue(hue: f32, previous: Hsv, rgba: Rgba) -> Hsv {
    let derived = hsv_after_rgb(Hsv::new(hue, previous.s(), previous.v()), rgba);
    let same_hue = Hsv::new(hue, derived.s(), derived.v());
    if same_hue.to_rgba(rgba.a) == rgba {
        same_hue
    } else {
        derived
    }
}

/// Like [`hsv_with_hue`] for the `Hsl` after an edit of the `Hsv` hue.
fn hsl_with_hue(hue: f32, previous: Hsl, rgba: Rgba) -> Hsl {
    let derived = hsl_after_rgb(Hsl::new(hue, previous.s(), previous.l()), rgba);
    let same_hue = Hsl::new(hue, derived.s(), derived.l());
    if same_hue.to_rgba(rgba.a) == rgba {
        same_hue
    } else {
        derived
    }
}

/// Like [`apply_control`] with the HSV model next to the other two (decisions 10 and 11): the three
/// are brought up to date together and `hsv.to_rgba(a) == rgba == hsl.to_rgba(a)` holds. For the
/// controls of 11c the `Rgba` and the `Hsl` are those of `apply_control`.
pub fn apply_control_aux(base: (Rgba, Hsl, Hsv), control: Control, value: u32) -> (Rgba, Hsl, Hsv) {
    let (rgba, hsl, hsv) = base;
    let v = value.min(control.max());
    match control {
        Control::HueV => {
            let h = Hsv::new(v as f32, hsv.s(), hsv.v());
            let n = h.to_rgba(rgba.a);
            (n, hsl_with_hue(v as f32, hsl, n), h)
        }
        Control::SaturationV => {
            let h = Hsv::new(hsv.h(), v as f32 / 100.0, hsv.v());
            let n = h.to_rgba(rgba.a);
            (n, hsl_after_rgb(hsl, n), h)
        }
        Control::Value => {
            let h = Hsv::new(hsv.h(), hsv.s(), v as f32 / 100.0);
            let n = h.to_rgba(rgba.a);
            (n, hsl_after_rgb(hsl, n), h)
        }
        Control::Hue => {
            let (n, h) = apply_control((rgba, hsl), control, value);
            (n, h, hsv_with_hue(v as f32, hsv, n))
        }
        Control::Alpha => {
            let (n, h) = apply_control((rgba, hsl), control, value);
            (n, h, hsv)
        }
        Control::Red
        | Control::Green
        | Control::Blue
        | Control::Gray
        | Control::Saturation
        | Control::Lightness => {
            let (n, h) = apply_control((rgba, hsl), control, value);
            (n, h, hsv_after_rgb(hsv, n))
        }
    }
}

/// The three models after the square puts saturation `s` and brightness `v` (both in `[0, 1]`):
/// only `s` and `v` of the `Hsv` change, never its hue nor the alpha (decision 10, rule a).
fn apply_square(base: (Rgba, Hsl, Hsv), s: f32, v: f32) -> (Rgba, Hsl, Hsv) {
    let (rgba, hsl, hsv) = base;
    let h = Hsv::new(hsv.h(), s, v);
    let n = h.to_rgba(rgba.a);
    (n, hsl_after_rgb(hsl, n), h)
}

/// Why a hexadecimal is not valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HexError {
    Empty,
    Length(usize),
    Digit,
    NotGray,
    NoAlpha,
}

/// Reads a hexadecimal. With 3 or 6 digits the alpha is the one of `current`; with 4 or 8, the
/// typed one.
pub fn parse_hex(text: &str, mode: Mode, current: Rgba) -> Result<Rgba, HexError> {
    let t = text.trim();
    let d = t.strip_prefix('#').unwrap_or(t);
    if d.is_empty() {
        return Err(HexError::Empty);
    }
    let n = d.chars().count();
    if !matches!(n, 3 | 4 | 6 | 8) {
        return Err(HexError::Length(n));
    }
    if !d.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(HexError::Digit);
    }
    let c = Rgba::from_hex(d).map_err(|_| HexError::Digit)?;
    let writes_alpha = n == 4 || n == 8;
    let color = if writes_alpha {
        c
    } else {
        Rgba::new(c.r, c.g, c.b, current.a)
    };
    let gray = color.r == color.g && color.g == color.b;
    match mode {
        Mode::Gray => {
            if writes_alpha && color.a != 255 {
                return Err(HexError::NoAlpha);
            }
            if !gray {
                return Err(HexError::NotGray);
            }
        }
        Mode::GrayAlpha if !gray => return Err(HexError::NotGray),
        _ => {}
    }
    Ok(color)
}

/// Canonical form of a color in the field (`Rgba: Display`).
pub fn hex_text(color: Rgba) -> String {
    color.to_string()
}

/// Reason of the notice (without the `aviso: ` prefix), at most 25 characters.
pub fn hex_notice(error: &HexError) -> &'static str {
    match error {
        HexError::Empty => "escribe un color",
        HexError::Length(_) => "usa 3, 4, 6 u 8 dígitos",
        HexError::Digit => "solo dígitos 0-9 y a-f",
        HexError::NotGray => "en modo gris: r = g = b",
        HexError::NoAlpha => "este modo no tiene alfa",
    }
}

/// Why a pending text is discarded instead of applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaleReason {
    /// The entry no longer exists (for example after undoing an `Add`).
    Gone,
    /// The stored color of the entry changed since the text was typed (undo, redo).
    Changed,
    /// The entry changed index (move, removing an earlier one).
    Moved,
}

/// 5 s notice when a pending text is discarded.
pub fn stale_notice(reason: StaleReason) -> &'static str {
    match reason {
        StaleReason::Gone => "texto descartado: la entrada ya no existe",
        StaleReason::Changed => "texto descartado: el color cambió",
        StaleReason::Moved => "texto descartado: la entrada se movió",
    }
}

/// Notice on opening another document with a pending text.
pub const DOCUMENT_CHANGED_NOTICE: &str = "texto descartado: se abrió otro documento";

/// Result of resolving the pending text after checking that its target is still the same.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    Done(HexOutcome),
    Stale(StaleReason),
}

/// A color to apply to a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Commit {
    pub target: EditTarget,
    pub color: Rgba,
}

/// Result of confirming the text of the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HexOutcome {
    Nothing,
    Unchanged,
    Commit(Commit),
    Invalid(HexError),
}

/// What a drag in progress moves: a slider (or a bar, which is a slider with another shape) or
/// the saturation and brightness square.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragKind {
    Slider(Control),
    Square,
}

/// A drag in progress: what moves, the models at the start and the provisional ones.
#[derive(Debug)]
struct Drag {
    kind: DragKind,
    base: (Rgba, Hsl, Hsv),
    prov: (Rgba, Hsl, Hsv),
}

/// Text of the field not yet confirmed, with its target and the base color of the moment of typing.
#[derive(Debug)]
struct Pending {
    target: EditTarget,
    base: Rgba,
    text: String,
    error: Option<HexError>,
}

/// State of the editor: target, shown color, its own `Hsl` and `Hsv`, drag and pending text. The
/// `Rgba` is the real color; the other two models only keep the hue from jumping.
#[derive(Debug, Default)]
pub struct ColorEditor {
    target: Option<EditTarget>,
    rgba: Rgba,
    hsl: Hsl,
    hsv: Hsv,
    drag: Option<Drag>,
    pending: Option<Pending>,
}

impl ColorEditor {
    /// Brings the target and its stored color up to date. Recomputes the `Hsl` and the `Hsv`
    /// (`from_rgba`) if the target or the stored color changed; otherwise it keeps them. With a
    /// drag in progress it does nothing. A warning of the field (invalid text after Enter) is
    /// cleared when the target or the color changes.
    pub fn sync(&mut self, current: Option<(EditTarget, Rgba)>) {
        if self.drag.is_some() {
            return;
        }
        let changed = match current {
            None => self.target.take().is_some(),
            Some((target, color)) => {
                let changed = self.target != Some(target) || self.rgba != color;
                if changed {
                    self.hsl = Hsl::from_rgba(color);
                    self.hsv = Hsv::from_rgba(color);
                    self.rgba = color;
                    self.target = Some(target);
                }
                changed
            }
        };
        if (changed || current.is_none())
            && self.pending.as_ref().is_some_and(|p| p.error.is_some())
        {
            self.pending = None;
        }
    }

    /// The target that the editor shows, if any.
    pub fn target(&self) -> Option<EditTarget> {
        self.target
    }

    /// Leaves it as just created (on opening a document).
    pub fn reset(&mut self) {
        *self = ColorEditor::default();
    }

    /// Starts a drag of `control` with the value under the pointer. `false` without a target or
    /// with another drag in progress.
    pub fn begin(&mut self, control: Control, value: u32) -> bool {
        if self.target.is_none() || self.drag.is_some() {
            return false;
        }
        let base = (self.rgba, self.hsl, self.hsv);
        self.drag = Some(Drag {
            kind: DragKind::Slider(control),
            base,
            prov: apply_control_aux(base, control, value),
        });
        true
    }

    /// New value of the dragged control; the provisional color comes from the one at the start.
    pub fn drag(&mut self, value: u32) {
        if let Some(drag) = &mut self.drag
            && let DragKind::Slider(control) = drag.kind
        {
            drag.prov = apply_control_aux(drag.base, control, value);
        }
    }

    /// Starts a drag in the saturation and brightness square with the values under the pointer
    /// (`s` and `v` in `[0, 1]`). `false` without a target or with another drag in progress.
    pub fn begin_square(&mut self, s: f32, v: f32) -> bool {
        if self.target.is_none() || self.drag.is_some() {
            return false;
        }
        let base = (self.rgba, self.hsl, self.hsv);
        self.drag = Some(Drag {
            kind: DragKind::Square,
            base,
            prov: apply_square(base, s, v),
        });
        true
    }

    /// New saturation and brightness of the square drag; the provisional color comes from the one
    /// at the start.
    pub fn drag_square(&mut self, s: f32, v: f32) {
        if let Some(drag) = &mut self.drag
            && drag.kind == DragKind::Square
        {
            drag.prov = apply_square(drag.base, s, v);
        }
    }

    /// Ends the drag. `Some` only if the provisional color differs from the one at the start. The
    /// provisional models are kept even if the color did not change.
    pub fn release(&mut self) -> Option<Commit> {
        let drag = self.drag.take()?;
        self.rgba = drag.prov.0;
        self.hsl = drag.prov.1;
        self.hsv = drag.prov.2;
        let target = self.target?;
        (drag.prov.0 != drag.base.0).then_some(Commit {
            target,
            color: drag.prov.0,
        })
    }

    /// Cancels the drag and restores color and models from the start. Afterwards the movements and
    /// the release do nothing (it does not resume).
    pub fn cancel(&mut self) {
        self.drag = None;
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The drag in progress is the one of the square.
    pub fn dragging_square(&self) -> bool {
        self.drag
            .as_ref()
            .is_some_and(|drag| drag.kind == DragKind::Square)
    }

    /// Control being dragged (`None` for the square).
    pub fn dragging_control(&self) -> Option<Control> {
        match self.drag.as_ref()?.kind {
            DragKind::Slider(control) => Some(control),
            DragKind::Square => None,
        }
    }

    /// Provisional color of the target while the drag lasts.
    pub fn preview(&self) -> Option<(EditTarget, Rgba)> {
        let drag = self.drag.as_ref()?;
        Some((self.target?, drag.prov.0))
    }

    /// Color and `Hsl` that the controls show: the provisional or the stored one. `None` without a
    /// target.
    pub fn shown(&self) -> Option<(Rgba, Hsl)> {
        self.target?;
        Some(match &self.drag {
            Some(drag) => (drag.prov.0, drag.prov.1),
            None => (self.rgba, self.hsl),
        })
    }

    /// The `Hsv` that the square and the bars show: the provisional or the stored one. `None`
    /// without a target.
    pub fn shown_hsv(&self) -> Option<Hsv> {
        self.target?;
        Some(match &self.drag {
            Some(drag) => drag.prov.2,
            None => self.hsv,
        })
    }

    /// The user changed the text of the field. It remembers its target and the base color; it
    /// clears the warning.
    pub fn type_hex(&mut self, target: EditTarget, base: Rgba, text: String) {
        self.pending = Some(Pending {
            target,
            base,
            text,
            error: None,
        });
    }

    /// Pending text of the field, if any.
    pub fn hex_display(&self) -> Option<String> {
        self.pending.as_ref().map(|pending| pending.text.clone())
    }

    /// Enter (`blur` false) or loss of the focus (`blur` true). Nothing pending: `Nothing`. Valid:
    /// clears what is pending (`Unchanged` if it is the base color, otherwise `Commit`). Invalid:
    /// with Enter it keeps the text and the warning; with `blur` it clears the text (reverts).
    pub fn confirm_hex(&mut self, mode: Mode, blur: bool) -> HexOutcome {
        let Some(pending) = self.pending.take() else {
            return HexOutcome::Nothing;
        };
        match parse_hex(&pending.text, mode, pending.base) {
            Ok(color) if color == pending.base => HexOutcome::Unchanged,
            Ok(color) => HexOutcome::Commit(Commit {
                target: pending.target,
                color,
            }),
            Err(error) => {
                if !blur {
                    self.pending = Some(Pending {
                        error: Some(error.clone()),
                        ..pending
                    });
                }
                HexOutcome::Invalid(error)
            }
        }
    }

    /// Target of the pending text, if any.
    pub fn pending_target(&self) -> Option<EditTarget> {
        self.pending.as_ref().map(|pending| pending.target)
    }

    /// Discards the pending text without applying it. `true` if there was any.
    pub fn discard_pending(&mut self) -> bool {
        self.pending.take().is_some()
    }

    /// There is pending text (typed and not yet confirmed or reverted).
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Like [`ColorEditor::confirm_hex`], but only if the target of the text still exists and its
    /// stored color (`stored`) is the one it had when typed: otherwise it discards the text
    /// without a `Commit` (`Stale`). Nothing pending: `Done(Nothing)`.
    pub fn confirm_hex_checked(
        &mut self,
        mode: Mode,
        blur: bool,
        stored: impl Fn(EditTarget) -> Option<Rgba>,
    ) -> Resolved {
        if let Some(pending) = &self.pending {
            let reason = match stored(pending.target) {
                None => Some(StaleReason::Gone),
                Some(color) if color != pending.base => Some(StaleReason::Changed),
                Some(_) => None,
            };
            if let Some(reason) = reason {
                self.pending = None;
                return Resolved::Stale(reason);
            }
        }
        Resolved::Done(self.confirm_hex(mode, blur))
    }

    /// The pending text of the field is invalid (after Enter): the field warns with its border.
    pub fn hex_error(&self) -> Option<&HexError> {
        self.pending
            .as_ref()
            .and_then(|pending| pending.error.as_ref())
    }

    /// `Escape` in the field: clears what is pending, without a warning.
    pub fn escape_hex(&mut self) {
        self.pending = None;
    }

    /// The two lines of the warning, with the `aviso: ` prefix already in place (empty if there is
    /// none). The one of an invalid hexadecimal covers the one of the transparent entry.
    pub fn notice(&self, transparent: bool) -> [String; 2] {
        if let Some(error) = self
            .pending
            .as_ref()
            .and_then(|pending| pending.error.as_ref())
        {
            return [
                format!("{NOTICE_PREFIX}{}", hex_notice(error)),
                String::new(),
            ];
        }
        if transparent {
            return [
                format!("{NOTICE_PREFIX}entrada transparente"),
                "se guarda con alfa 0".to_string(),
            ];
        }
        [String::new(), String::new()]
    }
}
