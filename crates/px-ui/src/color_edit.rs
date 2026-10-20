//! Lógica pura del editor de color: destino, controles, arrastre y hexadecimal, sin tipos de
//! egui (specs/hito-2-editar-color.md).

use px_core::{Hsl, Image, Mode, Rgba};

use crate::color::{ColorChoice, SlotColors};
use crate::theme::NOTICE_PREFIX;

/// Qué se edita: una entrada de la paleta (indexadas) o una ranura (los demás modos).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditTarget {
    Entry(u8),
    Slot(u8),
}

impl EditTarget {
    /// Destino de la elección: en indexadas, la entrada (`Slot(n)` vale como `Entry(n − 1)`);
    /// en los demás modos, la ranura 1 a 9. `None` si no existe.
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
    /// Color **guardado** del destino (`Palette::get` o la ranura), no el resuelto.
    pub fn stored(self, image: &Image, slots: &SlotColors) -> Option<Rgba> {
        match self {
            EditTarget::Entry(i) => image.palette()?.get(i).ok(),
            EditTarget::Slot(n) => slots.get(n),
        }
    }
    /// Cabecera del editor: `editar índice 12`, `editar color 3`, `editar color 4 gris`.
    pub fn header(self, mode: Mode) -> String {
        match self {
            EditTarget::Entry(i) => format!("editar índice {i}"),
            EditTarget::Slot(n) if mode == Mode::Gray => format!("editar color {n} gris"),
            EditTarget::Slot(n) => format!("editar color {n}"),
        }
    }
}

/// Un deslizador.
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
}

/// Deslizadores de un modo, de arriba abajo: `R G B H S L A` (indexadas y RGBA), `G A`
/// (GrayAlpha) o `G` (Gray).
pub fn controls(mode: Mode) -> &'static [Control] {
    use Control::*;
    match mode {
        Mode::Rgba | Mode::Indexed => &[Red, Green, Blue, Hue, Saturation, Lightness, Alpha],
        Mode::GrayAlpha => &[Gray, Alpha],
        Mode::Gray => &[Gray],
    }
}

impl Control {
    /// Letra del deslizador.
    pub fn label(self) -> char {
        match self {
            Control::Red => 'R',
            Control::Green => 'G',
            Control::Blue => 'B',
            Control::Hue => 'H',
            Control::Saturation => 'S',
            Control::Lightness => 'L',
            Control::Alpha => 'A',
            Control::Gray => 'G',
        }
    }
    /// Valor máximo: 255, 359 (matiz) o 100 (saturación y luminosidad).
    pub fn max(self) -> u32 {
        match self {
            Control::Hue => 359,
            Control::Saturation | Control::Lightness => 100,
            _ => 255,
        }
    }
}

/// Valor entero que muestra `control` para ese color.
pub fn control_value(rgba: Rgba, hsl: Hsl, control: Control) -> u32 {
    match control {
        Control::Red | Control::Gray => u32::from(rgba.r),
        Control::Green => u32::from(rgba.g),
        Control::Blue => u32::from(rgba.b),
        Control::Alpha => u32::from(rgba.a),
        Control::Hue => (hsl.h().round() as u32) % 360,
        Control::Saturation => (100.0 * hsl.s()).round() as u32,
        Control::Lightness => (100.0 * hsl.l()).round() as u32,
    }
}

/// Valor en 3 columnas, alineado a la derecha.
pub fn value_text(value: u32) -> String {
    format!("{value:>3}")
}

/// Valor bajo la posición `x_px` de una pista de `width_px` píxeles (se limita a la pista).
pub fn slider_value_at(x_px: i64, width_px: i64, max: u32) -> u32 {
    if width_px <= 1 {
        return 0;
    }
    let span = i128::from(width_px - 1);
    let x = i128::from(x_px.clamp(0, width_px - 1));
    ((2 * x * i128::from(max) + span) / (2 * span)) as u32
}

/// Posición, en píxeles desde el inicio de la pista, de `value`.
pub fn slider_x(value: u32, width_px: i64, max: u32) -> i64 {
    if width_px <= 1 {
        return 0;
    }
    let span = i128::from(width_px - 1);
    let m = i128::from(max.max(1));
    ((2 * i128::from(value.min(max)) * span + m) / (2 * m)) as i64
}

/// `Hsl` tras editar RGB: el de `from_rgba`, salvo gris (conserva el matiz anterior) y negro o
/// blanco (conserva también la saturación anterior).
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

/// Color provisional (y su `Hsl`) tras poner `control` a `value` (se limita al máximo) desde
/// `base`: solo depende de `base`, el control y el valor, no del camino.
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
    }
}

/// Por qué no vale un hexadecimal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HexError {
    Empty,
    Length(usize),
    Digit,
    NotGray,
    NoAlpha,
}

/// Lee un hexadecimal. Con 3 o 6 dígitos el alfa es el de `current`; con 4 u 8, el escrito.
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

/// Forma canónica de un color en el campo (`Rgba: Display`).
pub fn hex_text(color: Rgba) -> String {
    color.to_string()
}

/// Motivo del aviso (sin el prefijo `aviso: `), de 25 caracteres como mucho.
pub fn hex_notice(error: &HexError) -> &'static str {
    match error {
        HexError::Empty => "escribe un color",
        HexError::Length(_) => "usa 3, 4, 6 u 8 dígitos",
        HexError::Digit => "solo dígitos 0-9 y a-f",
        HexError::NotGray => "en modo gris: r = g = b",
        HexError::NoAlpha => "este modo no tiene alfa",
    }
}

/// Por qué se descarta un texto pendiente en vez de aplicarlo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaleReason {
    /// La entrada ya no existe (por ejemplo, tras deshacer un `Add`).
    Gone,
    /// El color guardado de la entrada cambió desde que se tecleó (deshacer, rehacer).
    Changed,
    /// La entrada cambió de índice (mover, quitar una anterior).
    Moved,
}

/// Aviso de 5 s al descartar un texto pendiente.
pub fn stale_notice(reason: StaleReason) -> &'static str {
    match reason {
        StaleReason::Gone => "texto descartado: la entrada ya no existe",
        StaleReason::Changed => "texto descartado: el color cambió",
        StaleReason::Moved => "texto descartado: la entrada se movió",
    }
}

/// Aviso al abrir otro documento con texto pendiente.
pub const DOCUMENT_CHANGED_NOTICE: &str = "texto descartado: se abrió otro documento";

/// Resultado de resolver el texto pendiente comprobando que su destino sigue igual.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    Done(HexOutcome),
    Stale(StaleReason),
}

/// Un color a aplicar a un destino.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Commit {
    pub target: EditTarget,
    pub color: Rgba,
}

/// Resultado de confirmar el texto del campo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HexOutcome {
    Nothing,
    Unchanged,
    Commit(Commit),
    Invalid(HexError),
}

/// Arrastre en curso: el control, el color del inicio y el provisional.
#[derive(Debug)]
struct Drag {
    control: Control,
    base: (Rgba, Hsl),
    prov: (Rgba, Hsl),
}

/// Texto del campo todavía sin confirmar, con su destino y el color base del momento de teclear.
#[derive(Debug)]
struct Pending {
    target: EditTarget,
    base: Rgba,
    text: String,
    error: Option<HexError>,
}

/// Estado del editor: destino, color mostrado, `Hsl` propio, arrastre y texto pendiente.
#[derive(Debug, Default)]
pub struct ColorEditor {
    target: Option<EditTarget>,
    rgba: Rgba,
    hsl: Hsl,
    drag: Option<Drag>,
    pending: Option<Pending>,
}

impl ColorEditor {
    /// Pone al día el destino y su color guardado. Recalcula el `Hsl` (`Hsl::from_rgba`) si el
    /// destino o el color guardado cambiaron; si no, lo conserva. Con un arrastre en curso no
    /// hace nada. Un aviso del campo (texto inválido tras Intro) se borra al cambiar el destino
    /// o el color.
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

    /// El destino que muestra el editor, si lo hay.
    pub fn target(&self) -> Option<EditTarget> {
        self.target
    }

    /// Lo deja como recién creado (al abrir un documento).
    pub fn reset(&mut self) {
        *self = ColorEditor::default();
    }

    /// Empieza un arrastre de `control` con el valor bajo el puntero. `false` sin destino o con
    /// otro arrastre en curso.
    pub fn begin(&mut self, control: Control, value: u32) -> bool {
        if self.target.is_none() || self.drag.is_some() {
            return false;
        }
        let base = (self.rgba, self.hsl);
        self.drag = Some(Drag {
            control,
            base,
            prov: apply_control(base, control, value),
        });
        true
    }

    /// Nuevo valor del control arrastrado; el provisional sale del color del inicio.
    pub fn drag(&mut self, value: u32) {
        if let Some(drag) = &mut self.drag {
            drag.prov = apply_control(drag.base, drag.control, value);
        }
    }

    /// Termina el arrastre. `Some` solo si el color provisional difiere del del inicio. El
    /// `Hsl` provisional se conserva aunque no haya cambio de color.
    pub fn release(&mut self) -> Option<Commit> {
        let drag = self.drag.take()?;
        self.rgba = drag.prov.0;
        self.hsl = drag.prov.1;
        let target = self.target?;
        (drag.prov.0 != drag.base.0).then_some(Commit {
            target,
            color: drag.prov.0,
        })
    }

    /// Cancela el arrastre y restaura color y `Hsl` del inicio. Después, los movimientos y el
    /// soltar no hacen nada (no se reanuda).
    pub fn cancel(&mut self) {
        self.drag = None;
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Control que se está arrastrando.
    pub fn dragging_control(&self) -> Option<Control> {
        self.drag.as_ref().map(|drag| drag.control)
    }

    /// Color provisional del destino mientras dura el arrastre.
    pub fn preview(&self) -> Option<(EditTarget, Rgba)> {
        let drag = self.drag.as_ref()?;
        Some((self.target?, drag.prov.0))
    }

    /// Color y `Hsl` que muestran los controles: el provisional o el guardado. `None` sin
    /// destino.
    pub fn shown(&self) -> Option<(Rgba, Hsl)> {
        self.target?;
        Some(match &self.drag {
            Some(drag) => drag.prov,
            None => (self.rgba, self.hsl),
        })
    }

    /// El usuario cambió el texto del campo. Recuerda su destino y el color base; borra el aviso.
    pub fn type_hex(&mut self, target: EditTarget, base: Rgba, text: String) {
        self.pending = Some(Pending {
            target,
            base,
            text,
            error: None,
        });
    }

    /// Texto pendiente del campo, si lo hay.
    pub fn hex_display(&self) -> Option<String> {
        self.pending.as_ref().map(|pending| pending.text.clone())
    }

    /// Intro (`blur` falso) o pérdida del foco (`blur` verdadero). Nada pendiente: `Nothing`.
    /// Válido: borra lo pendiente (`Unchanged` si es el color base, si no `Commit`). Inválido:
    /// con Intro conserva el texto y el aviso; con `blur` borra el texto (revierte).
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

    /// Destino del texto pendiente, si lo hay.
    pub fn pending_target(&self) -> Option<EditTarget> {
        self.pending.as_ref().map(|pending| pending.target)
    }

    /// Descarta el texto pendiente sin aplicarlo. `true` si había.
    pub fn discard_pending(&mut self) -> bool {
        self.pending.take().is_some()
    }

    /// Hay texto pendiente (tecleado y aún sin confirmar ni revertir).
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Como [`ColorEditor::confirm_hex`], pero solo si el destino del texto sigue existiendo y su
    /// color guardado (`stored`) es el que había al teclear: si no, descarta el texto sin
    /// `Commit` (`Stale`). Nada pendiente: `Done(Nothing)`.
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

    /// `Escape` en el campo: borra lo pendiente, sin aviso.
    pub fn escape_hex(&mut self) {
        self.pending = None;
    }

    /// Las dos líneas del aviso, con el prefijo `aviso: ` ya puesto (vacías si no hay aviso).
    /// El del hexadecimal inválido tapa el de la entrada transparente.
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
