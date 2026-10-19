//! Nueva imagen (specs/hito-2-nueva-imagen.md): el formulario, el fondo por modo y su traslado al
//! cambiar de modo, la paleta inicial, la estimación de memoria y la construcción de la imagen con
//! toda la memoria reservada **antes** de reemplazar el documento. Sin egui: los tipos del
//! formulario son de `px-ui` y se prueba sin ventana.

use px_core::{Hsl, Image, ImageError, Mode, Palette, Pixel, Rgba};
use px_ui::actions::Locks;
use px_ui::dialog::{
    Dialog, DialogKind, FieldId, FieldKind, FieldSpec, FormSpec, Modal, Rows, ShowWhen, Values,
    resolve_hex,
};
use px_ui::number::parse_number;

use crate::files::MAX_OPEN_PIXELS;

/// Lado máximo del formulario (antes del límite de la GPU).
pub const MAX_SIDE: u32 = 8192;
/// Lado por defecto.
pub const DEFAULT_SIDE: u32 = 64;
/// Por encima de estos píxeles (estrictamente) se avisa de que puede ir lento; no impide crear.
pub const WARN_PIXELS: u64 = 1 << 24;
/// Entradas de la paleta inicial del indexado.
pub const PALETTE_LEN: usize = 32;
/// Entrada del negro y del blanco en la paleta inicial.
pub const BLACK_INDEX: u8 = 25;
pub const WHITE_INDEX: u8 = 31;
/// Cómo dice el motivo del límite de la GPU: `máximo 4096 (tarjeta gráfica)`.
pub const GPU_LIMIT_NOTE: &str = "tarjeta gráfica";

pub const FIELD_WIDTH: FieldId = "ancho";
pub const FIELD_HEIGHT: FieldId = "alto";
pub const FIELD_MODE: FieldId = "modo";
pub const FIELD_BG_INDEXED: FieldId = "fondo_indexado";
pub const FIELD_BG_RGBA: FieldId = "fondo_rgba";
pub const FIELD_COLOR_RGBA: FieldId = "color_rgba";
pub const FIELD_BG_GRAY_ALPHA: FieldId = "fondo_gris_alfa";
pub const FIELD_COLOR_GRAY_ALPHA: FieldId = "color_gris_alfa";
pub const FIELD_BG_GRAY: FieldId = "fondo_gris";
pub const FIELD_COLOR_GRAY: FieldId = "color_gris";
pub const FIELD_REPORT: FieldId = "informe";

/// Los modos en el orden del campo `modo`.
const MODES: [Mode; 4] = [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray];
const MODE_OPTIONS: &[&str] = &["Indexado", "RGBA", "Gris con alfa", "Gris"];
const BG_INDEXED_OPTIONS: &[&str] = &["Transparente", "Blanco", "Negro"];
const BG_ALPHA_OPTIONS: &[&str] = &["Transparente", "Blanco", "Negro", "Color…"];
const BG_GRAY_OPTIONS: &[&str] = &["Blanco", "Negro", "Color…"];
/// El hexadecimal nace en un gris medio: vale en los tres modos con color.
const DEFAULT_COLOR: Rgba = Rgba::opaque(0x80, 0x80, 0x80);

/// El fondo de la imagen nueva.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Background {
    Transparent,
    White,
    Black,
    Color(Rgba),
}

/// Lo que pide el formulario.
pub struct NewImageSpec {
    pub width: u32,
    pub height: u32,
    pub mode: Mode,
    pub background: Background,
}

fn mode_index(mode: Mode) -> usize {
    MODES.iter().position(|m| *m == mode).unwrap_or(0)
}

fn mode_from_index(index: usize) -> Mode {
    MODES.get(index).copied().unwrap_or(Mode::Indexed)
}

/// Campo del grupo de fondo de un modo.
fn bg_field(mode: Mode) -> FieldId {
    match mode {
        Mode::Indexed => FIELD_BG_INDEXED,
        Mode::Rgba => FIELD_BG_RGBA,
        Mode::GrayAlpha => FIELD_BG_GRAY_ALPHA,
        Mode::Gray => FIELD_BG_GRAY,
    }
}

/// Campo hexadecimal del fondo de un modo, si el modo tiene «Color…».
fn color_field(mode: Mode) -> Option<FieldId> {
    match mode {
        Mode::Indexed => None,
        Mode::Rgba => Some(FIELD_COLOR_RGBA),
        Mode::GrayAlpha => Some(FIELD_COLOR_GRAY_ALPHA),
        Mode::Gray => Some(FIELD_COLOR_GRAY),
    }
}

/// Índice de la opción «Color…» del grupo de fondo de un modo.
fn color_option(mode: Mode) -> Option<usize> {
    match mode {
        Mode::Indexed => None,
        Mode::Rgba | Mode::GrayAlpha => Some(3),
        Mode::Gray => Some(2),
    }
}

/// Índice de la opción de `background` en el grupo del modo (sin el color, que va aparte).
fn option_of(mode: Mode, background: Background) -> Option<usize> {
    match (mode, background) {
        (Mode::Gray, Background::Transparent) => None,
        (Mode::Gray, Background::White) => Some(0),
        (Mode::Gray, Background::Black) => Some(1),
        (_, Background::Transparent) => Some(0),
        (_, Background::White) => Some(1),
        (_, Background::Black) => Some(2),
        (_, Background::Color(_)) => color_option(mode),
    }
}

/// El fondo de una opción de un grupo; el color, si es «Color…», lo da `color`.
fn background_of(mode: Mode, option: usize, color: Option<Rgba>) -> Option<Background> {
    if color_option(mode) == Some(option) {
        return color.map(Background::Color);
    }
    match (mode, option) {
        (Mode::Gray, 0) => Some(Background::White),
        (Mode::Gray, 1) => Some(Background::Black),
        (Mode::Gray, _) => None,
        (_, 0) => Some(Background::Transparent),
        (_, 1) => Some(Background::White),
        (_, 2) => Some(Background::Black),
        _ => None,
    }
}

/// El formulario. `max_side` es el lado máximo de textura de la GPU: el de los campos es
/// `min(8192, max_side)` (como mínimo 1).
pub fn new_image_form(max_side: usize) -> FormSpec {
    let max = u32::try_from(max_side)
        .unwrap_or(u32::MAX)
        .clamp(1, MAX_SIDE);
    let default = DEFAULT_SIDE.min(max);
    let number = |id, label| FieldSpec {
        id,
        label,
        kind: FieldKind::Number {
            min: 1,
            max,
            default,
        },
        shown_when: None,
    };
    let when = |field, choice| Some(ShowWhen { field, choice });
    let background = |id, options: &'static [&'static str], mode: Mode| FieldSpec {
        id,
        label: "Fondo",
        kind: FieldKind::Choice {
            options,
            default: 0,
        },
        shown_when: when(FIELD_MODE, mode_index(mode)),
    };
    let color = |id, mode: Mode, control| FieldSpec {
        id,
        label: "Color",
        kind: FieldKind::Hex {
            mode,
            default: DEFAULT_COLOR,
        },
        shown_when: when(control, color_option(mode).unwrap_or(0)),
    };
    FormSpec {
        title: "Nueva imagen",
        accept: "Crear",
        cancel: Some("Cancelar"),
        fields: vec![
            number(FIELD_WIDTH, "Ancho"),
            number(FIELD_HEIGHT, "Alto"),
            FieldSpec {
                id: FIELD_MODE,
                label: "Modo",
                kind: FieldKind::Choice {
                    options: MODE_OPTIONS,
                    default: 0,
                },
                shown_when: None,
            },
            background(FIELD_BG_INDEXED, BG_INDEXED_OPTIONS, Mode::Indexed),
            background(FIELD_BG_RGBA, BG_ALPHA_OPTIONS, Mode::Rgba),
            color(FIELD_COLOR_RGBA, Mode::Rgba, FIELD_BG_RGBA),
            background(FIELD_BG_GRAY_ALPHA, BG_ALPHA_OPTIONS, Mode::GrayAlpha),
            color(FIELD_COLOR_GRAY_ALPHA, Mode::GrayAlpha, FIELD_BG_GRAY_ALPHA),
            background(FIELD_BG_GRAY, BG_GRAY_OPTIONS, Mode::Gray),
            color(FIELD_COLOR_GRAY, Mode::Gray, FIELD_BG_GRAY),
            FieldSpec {
                id: FIELD_REPORT,
                label: "Informe",
                kind: FieldKind::ReadOnly {
                    text: report_text(Some((default, default)), Mode::Indexed, None),
                    rows: Rows::Fixed(4),
                },
                shown_when: None,
            },
        ],
        check: Some(check_values),
    }
}

/// La comprobación cruzada: producto y bytes (hoy inalcanzable desde el formulario, D5).
fn check_values(values: &Values) -> Option<String> {
    let width = values.number(FIELD_WIDTH)?;
    let height = values.number(FIELD_HEIGHT)?;
    let mode = mode_from_index(values.choice(FIELD_MODE)?);
    size_error(width, height, mode)
}

/// Abre el diálogo de nueva imagen; con la GPU por debajo de 8192, el motivo del tope lo dice.
pub fn open_new_image(modal: &mut Modal, max_side: usize, locks: Locks) -> bool {
    if !modal.open(DialogKind::NewImage, new_image_form(max_side), locks) {
        return false;
    }
    if max_side < MAX_SIDE as usize
        && let Modal::Open(dialog) = modal
    {
        for field in [FIELD_WIDTH, FIELD_HEIGHT] {
            dialog.set_limit_note(field, Some(GPU_LIMIT_NOTE));
        }
    }
    true
}

/// Lo que dejó el formulario, o `None` si faltan campos visibles.
pub fn spec_from_values(values: &Values) -> Option<NewImageSpec> {
    let width = values.number(FIELD_WIDTH)?;
    let height = values.number(FIELD_HEIGHT)?;
    let mode = mode_from_index(values.choice(FIELD_MODE)?);
    let option = values.choice(bg_field(mode))?;
    let color = color_field(mode).and_then(|id| values.color(id));
    let background = background_of(mode, option, color)?;
    Some(NewImageSpec {
        width,
        height,
        mode,
        background,
    })
}

const SIZE_ERROR: &str = "ancho × alto: máx. 67108864 píxeles";

/// Por qué no vale un tamaño: producto por encima de `MAX_OPEN_PIXELS`, o una dimensión nula, o
/// más bytes que `Image::MAX_BYTES`. Texto de ≤ 37 caracteres.
pub fn size_error(width: u32, height: u32, mode: Mode) -> Option<String> {
    let too_many = u64::from(width) * u64::from(height) > MAX_OPEN_PIXELS;
    (too_many || Image::byte_len(width, height, mode).is_err()).then(|| SIZE_ERROR.to_string())
}

/// La paleta inicial del indexado (D2): la entrada 0 transparente, 24 colores (ocho matices por
/// tres luminosidades) y siete grises de negro a blanco. Determinista.
pub fn initial_palette() -> Palette {
    let mut colors = Vec::with_capacity(PALETTE_LEN);
    colors.push(Rgba::TRANSPARENT);
    for lightness in [0.25_f32, 0.5, 0.75] {
        for step in 0..8 {
            colors.push(Hsl::new(45.0 * step as f32, 1.0, lightness).to_rgba(255));
        }
    }
    for step in 0..7_u16 {
        let gray = ((255 * step + 3) / 6) as u8;
        colors.push(Rgba::opaque(gray, gray, gray));
    }
    let mut palette = Palette::from_colors(colors).expect("32 entradas caben en la paleta");
    palette
        .set_transparent_index(Some(0))
        .expect("la entrada 0 existe");
    palette
}

/// El fondo por defecto de un modo: Gris no tiene transparente.
pub fn default_background(mode: Mode) -> Background {
    if mode == Mode::Gray {
        Background::White
    } else {
        Background::Transparent
    }
}

/// El píxel de un fondo en un modo; `None` si el modo no lo admite.
pub fn background_pixel(mode: Mode, background: Background) -> Option<Pixel> {
    let gray = |c: Rgba| (c.r == c.g && c.g == c.b).then_some(c.r);
    match (mode, background) {
        (Mode::Rgba, Background::Transparent) => Some(Pixel::Rgba(Rgba::TRANSPARENT)),
        (Mode::Rgba, Background::White) => Some(Pixel::Rgba(Rgba::opaque(255, 255, 255))),
        (Mode::Rgba, Background::Black) => Some(Pixel::Rgba(Rgba::opaque(0, 0, 0))),
        (Mode::Rgba, Background::Color(c)) => Some(Pixel::Rgba(c)),
        (Mode::GrayAlpha, Background::Transparent) => Some(Pixel::GrayAlpha { gray: 0, alpha: 0 }),
        (Mode::GrayAlpha, Background::White) => Some(Pixel::GrayAlpha {
            gray: 255,
            alpha: 255,
        }),
        (Mode::GrayAlpha, Background::Black) => Some(Pixel::GrayAlpha {
            gray: 0,
            alpha: 255,
        }),
        (Mode::GrayAlpha, Background::Color(c)) => {
            gray(c).map(|gray| Pixel::GrayAlpha { gray, alpha: c.a })
        }
        (Mode::Gray, Background::Transparent) => None,
        (Mode::Gray, Background::White) => Some(Pixel::Gray(255)),
        (Mode::Gray, Background::Black) => Some(Pixel::Gray(0)),
        (Mode::Gray, Background::Color(c)) => gray(c).filter(|_| c.a == 255).map(Pixel::Gray),
        (Mode::Indexed, Background::Transparent) => Some(Pixel::Indexed(0)),
        (Mode::Indexed, Background::White) => Some(Pixel::Indexed(WHITE_INDEX)),
        (Mode::Indexed, Background::Black) => Some(Pixel::Indexed(BLACK_INDEX)),
        (Mode::Indexed, Background::Color(_)) => None,
    }
}

/// Qué le pasa al fondo al cambiar de modo.
pub enum Carry {
    /// El modo nuevo lo admite sin cambiar el color resultante.
    Kept(Background),
    /// No lo admite: vuelve al de por defecto del modo nuevo, con este aviso.
    Reset {
        to: Background,
        notice: &'static str,
    },
}

/// Traslada un fondo de `from` a `to` (tabla D1b).
pub fn carry_background(from: Mode, background: Background, to: Mode) -> Carry {
    if from == to || background_pixel(to, background).is_some() {
        return Carry::Kept(background);
    }
    let gray = |c: Rgba| c.r == c.g && c.g == c.b;
    let notice = match (background, to) {
        (Background::Transparent, _) => "Gris sin Transparente: pasa a Blanco",
        (Background::Color(_), Mode::Indexed) => "Indexado sin Color: pasa a Transparente",
        (Background::Color(c), Mode::GrayAlpha) if !gray(c) => "Color no gris: pasa a Transparente",
        (Background::Color(c), _) if !gray(c) => "Color no gris: pasa a Blanco",
        (Background::Color(_), _) => "Color con alfa: pasa a Blanco",
        _ => "Fondo no válido: pasa al de por defecto",
    };
    Carry::Reset {
        to: default_background(to),
        notice,
    }
}

/// Lo que ocupa una imagen: la del documento, la copia del lienzo y la subida a RGBA (RAM), y la
/// textura RGBA8 (GPU).
pub struct Memory {
    pub ram_bytes: u64,
    pub gpu_bytes: u64,
}

fn bytes_per_pixel(mode: Mode) -> u64 {
    match mode {
        Mode::Indexed | Mode::Gray => 1,
        Mode::GrayAlpha => 2,
        Mode::Rgba => 4,
    }
}

pub fn memory_estimate(width: u32, height: u32, mode: Mode) -> Memory {
    let pixels = u64::from(width) * u64::from(height);
    Memory {
        ram_bytes: pixels * (2 * bytes_per_pixel(mode) + 4),
        gpu_bytes: pixels * 4,
    }
}

const MIB: u64 = 1 << 20;

fn mib_ceil(bytes: u64) -> u64 {
    bytes.div_ceil(MIB)
}

/// El informe del diálogo: memoria, la paleta (indexado), el aviso de tamaño y el aviso de fondo,
/// una línea cada uno (≤ 4 líneas de ≤ 42 caracteres).
pub fn report_text(size: Option<(u32, u32)>, mode: Mode, notice: Option<&str>) -> String {
    let mut lines = Vec::new();
    match size {
        None => lines.push("Memoria: —".to_string()),
        Some((width, height)) => {
            let memory = memory_estimate(width, height, mode);
            lines.push(if memory.ram_bytes < MIB {
                "Memoria: menos de 1 MiB".to_string()
            } else {
                format!(
                    "Memoria: ~{} MiB + {} MiB de GPU",
                    mib_ceil(memory.ram_bytes),
                    mib_ceil(memory.gpu_bytes)
                )
            });
        }
    }
    if mode == Mode::Indexed {
        lines.push(format!("Paleta: {PALETTE_LEN} colores, el 0 transparente"));
    }
    if let Some((width, height)) = size
        && u64::from(width) * u64::from(height) > WARN_PIXELS
    {
        lines.push(format!("aviso: más de {WARN_PIXELS} px; puede ir lento"));
    }
    if let Some(notice) = notice {
        lines.push(notice.to_string());
    }
    lines.join("\n")
}

/// Aviso de 5 s cuando no hay memoria.
pub fn memory_notice(width: u32, height: u32, ram_bytes: u64) -> String {
    format!(
        "sin memoria para una imagen de {width}×{height} (~{} MiB)",
        mib_ceil(ram_bytes).max(1)
    )
}

/// Por qué no se pudo crear la imagen.
pub enum NewImageError {
    /// El tamaño o el fondo no valen (el motivo).
    Size(String),
    /// No hay memoria: los bytes de RAM estimados.
    Memory { bytes: u64 },
}

/// Reserva de memoria, tras un trait para probar el fallo en cada paso sin agotarla.
pub trait ImageAlloc {
    /// Una imagen completa con el fondo ya escrito.
    fn image(&mut self, spec: &NewImageSpec) -> Result<Image, ImageError>;
    /// Si caben `bytes` más (reserva y suelta).
    fn probe(&mut self, bytes: usize) -> bool;
}

/// La reserva real: `Image::new` / `new_indexed` (`try_reserve_exact`) y rellenar con `set_pixel`.
pub struct SystemAlloc;

impl ImageAlloc for SystemAlloc {
    fn image(&mut self, spec: &NewImageSpec) -> Result<Image, ImageError> {
        let mut image = match spec.mode {
            Mode::Indexed => Image::new_indexed(spec.width, spec.height, initial_palette())?,
            mode => Image::new(spec.width, spec.height, mode)?,
        };
        let pixel =
            background_pixel(spec.mode, spec.background).ok_or(ImageError::ModeMismatch {
                expected: spec.mode,
                found: spec.mode,
            })?;
        // Una imagen nueva está a ceros: solo se rellena si el fondo no es ese.
        if image.get_pixel(0, 0)? != pixel {
            for y in 0..spec.height {
                for x in 0..spec.width {
                    image.set_pixel(x, y, pixel)?;
                }
            }
        }
        Ok(image)
    }

    fn probe(&mut self, bytes: usize) -> bool {
        Vec::<u8>::new().try_reserve_exact(bytes).is_ok()
    }
}

/// Las dos imágenes iguales: la del documento y la que se entrega al lienzo.
pub struct NewImages {
    pub document: Image,
    pub canvas: Image,
}

/// Construye todo antes de tocar nada: la del documento, la del lienzo y la reserva de la subida a
/// RGBA. Solo si las tres salen bien devuelve `Ok`.
pub fn build_new_images(
    spec: &NewImageSpec,
    alloc: &mut dyn ImageAlloc,
) -> Result<NewImages, NewImageError> {
    if let Some(reason) = size_error(spec.width, spec.height, spec.mode) {
        return Err(NewImageError::Size(reason));
    }
    if background_pixel(spec.mode, spec.background).is_none() {
        return Err(NewImageError::Size(
            "fondo no válido para el modo".to_string(),
        ));
    }
    let memory = memory_estimate(spec.width, spec.height, spec.mode);
    let no_memory = |_| NewImageError::Memory {
        bytes: memory.ram_bytes,
    };
    let document = alloc.image(spec).map_err(no_memory)?;
    let canvas = alloc.image(spec).map_err(no_memory)?;
    let staging = usize::try_from(memory.gpu_bytes).unwrap_or(usize::MAX);
    if !alloc.probe(staging) {
        return Err(NewImageError::Memory {
            bytes: memory.ram_bytes,
        });
    }
    Ok(NewImages { document, canvas })
}

/// Estado de `sync_form` entre fotogramas: el modo anterior, el aviso pendiente y la memoria del
/// último texto inválido de un hexadecimal.
#[derive(Default)]
pub struct FormSync {
    mode: Option<Mode>,
    notice: Option<String>,
    /// Lo escrito cuando se puso el aviso: si cambia, el aviso se va.
    signature: String,
    /// Lo que queda del texto inválido que el usuario escribió. El clic en el selector de modo
    /// quita el foco al pulsar y el campo se revierte antes de que se vea el cambio de modo; esta
    /// memoria dice que había un texto inválido. Se olvida si el usuario cambia otra cosa, escribe
    /// un valor válido o cambia de modo.
    invalid_seen: Option<InvalidSeen>,
}

/// Un texto inválido visto en el hexadecimal de `mode`, con lo que valía el campo y el formulario
/// (sin los textos de color) en ese momento.
struct InvalidSeen {
    mode: Mode,
    committed: Rgba,
    shape: String,
}

fn signature(dialog: &Dialog) -> String {
    let mut text = String::new();
    for id in [FIELD_WIDTH, FIELD_HEIGHT] {
        text.push_str(&dialog.text(id).unwrap_or_default());
        text.push('|');
    }
    for mode in MODES {
        text.push_str(&format!("{:?}|", dialog.chosen(bg_field(mode))));
        if let Some(id) = color_field(mode) {
            text.push_str(&dialog.text(id).unwrap_or_default());
            text.push('|');
        }
    }
    text
}

/// Tamaño y opciones de fondo, sin los textos de color: lo que el usuario cambia fuera del campo.
fn shape(dialog: &Dialog) -> String {
    let mut text = String::new();
    for id in [FIELD_WIDTH, FIELD_HEIGHT] {
        text.push_str(&dialog.text(id).unwrap_or_default());
        text.push('|');
    }
    for mode in MODES {
        text.push_str(&format!("{:?}|", dialog.chosen(bg_field(mode))));
    }
    text
}

/// El color del hexadecimal de un modo y si su texto pendiente no vale. Con un texto válido (aunque
/// no esté confirmado) es el que daría `Crear`; con uno inválido, el último confirmado. Una sola
/// regla, `resolve_hex`, la misma de la validez y de los valores del diálogo.
fn read_color(dialog: &Dialog, mode: Mode) -> Option<(Rgba, bool)> {
    let id = color_field(mode)?;
    let committed = dialog.hex_committed(id)?;
    let text = dialog.hex_pending(id).then(|| dialog.text(id)).flatten();
    Some(match resolve_hex(text.as_deref(), mode, committed) {
        Ok(color) => (color, false),
        Err(_) => (committed, true),
    })
}

/// El fondo que tiene el grupo de un modo, leído aunque el formulario sea inválido.
fn read_background(dialog: &Dialog, mode: Mode) -> Option<Background> {
    let option = dialog.chosen(bg_field(mode))?;
    background_of(
        mode,
        option,
        read_color(dialog, mode).map(|(color, _)| color),
    )
}

/// Si el modo tiene «Color…» elegido y su texto escrito no vale.
fn has_invalid_text(dialog: &Dialog, mode: Mode) -> bool {
    matches!(read_background(dialog, mode), Some(Background::Color(_)))
        && read_color(dialog, mode).is_some_and(|(_, invalid)| invalid)
}

/// El color en ocho dígitos (`#rrggbbaa`): el alfa forma parte del valor.
fn hex8(color: Rgba) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        color.r, color.g, color.b, color.a
    )
}

fn write_background(dialog: &mut Dialog, mode: Mode, background: Background) {
    let Some(option) = option_of(mode, background) else {
        return;
    };
    dialog.event(px_ui::dialog::Event::Choice {
        field: bg_field(mode),
        index: option,
    });
    if let (Background::Color(color), Some(id)) = (background, color_field(mode)) {
        let text = hex8(color);
        dialog.event(px_ui::dialog::Event::Hex { field: id, text });
        dialog.event(px_ui::dialog::Event::HexConfirm {
            field: id,
            blur: false,
        });
    }
}

fn current_report(dialog: &Dialog) -> Option<&str> {
    dialog
        .spec()
        .fields
        .iter()
        .find(|f| f.id == FIELD_REPORT)
        .and_then(|f| match &f.kind {
            FieldKind::ReadOnly { text, .. } => Some(text.as_str()),
            _ => None,
        })
}

/// Al acabar el fotograma: recuerda el texto inválido del hexadecimal del modo actual, o conserva
/// el recuerdo mientras nada cambie.
fn remember_invalid(dialog: &Dialog, sync: &mut FormSync, mode: Mode) {
    let id = color_field(mode);
    if has_invalid_text(dialog, mode) {
        sync.invalid_seen =
            id.and_then(|id| dialog.hex_committed(id))
                .map(|committed| InvalidSeen {
                    mode,
                    committed,
                    shape: shape(dialog),
                });
        return;
    }
    let still = sync.invalid_seen.as_ref().is_some_and(|seen| {
        let Some(id) = color_field(seen.mode) else {
            return false;
        };
        seen.mode == mode
            && seen.shape == shape(dialog)
            && dialog.hex_committed(id) == Some(seen.committed)
            && !dialog.hex_pending(id)
    });
    if !still {
        sync.invalid_seen = None;
    }
}

/// Pone el diálogo al día tras un fotograma: si cambió el modo, traslada el fondo (y avisa si no
/// pudo); retira el aviso cuando cambia otro valor; y actualiza el informe. `true` si algo cambió
/// que haya que pintar.
pub fn sync_form(dialog: &mut Dialog, sync: &mut FormSync) -> bool {
    let Some(option) = dialog.chosen(FIELD_MODE) else {
        return false;
    };
    let mode = mode_from_index(option);
    let mut changed = false;
    match sync.mode {
        Some(previous) if previous != mode => {
            let discarded = sync
                .invalid_seen
                .as_ref()
                .is_some_and(|seen| seen.mode == previous)
                || has_invalid_text(dialog, previous);
            sync.notice = match read_background(dialog, previous)
                .map(|background| carry_background(previous, background, mode))
            {
                Some(Carry::Kept(background)) => {
                    write_background(dialog, mode, background);
                    match background {
                        Background::Color(color) if discarded => {
                            Some(format!("Color no válido: pasa a {}", hex8(color)))
                        }
                        _ => None,
                    }
                }
                Some(Carry::Reset { to, notice }) => {
                    write_background(dialog, mode, to);
                    Some(notice.to_string())
                }
                None => None,
            };
            sync.mode = Some(mode);
            sync.signature = signature(dialog);
            changed = true;
        }
        None => sync.mode = Some(mode),
        Some(_) => {
            if sync.notice.is_some() && sync.signature != signature(dialog) {
                sync.notice = None;
                changed = true;
            }
        }
    }
    let side = |id| {
        dialog
            .text(id)
            .and_then(|text| parse_number(&text, 1, u32::MAX).ok())
    };
    let size = side(FIELD_WIDTH).zip(side(FIELD_HEIGHT));
    let report = report_text(size, mode, sync.notice.as_deref());
    if current_report(dialog) != Some(report.as_str()) {
        changed |= dialog.set_report(FIELD_REPORT, report);
    }
    remember_invalid(dialog, sync, mode);
    changed
}

/// El aviso de 5 s tras crear.
pub fn describe_new(spec: &NewImageSpec) -> String {
    let mode = match spec.mode {
        Mode::Indexed => "indexado",
        Mode::Gray => "gris",
        Mode::GrayAlpha => "gris con alfa",
        Mode::Rgba => "RGBA",
    };
    let mut text = format!("imagen nueva {}×{} · {mode}", spec.width, spec.height);
    if spec.mode == Mode::Indexed {
        text.push_str(&format!(" · {PALETTE_LEN} colores"));
    }
    text
}

#[cfg(test)]
mod dialog_tests;
#[cfg(test)]
mod tests;
