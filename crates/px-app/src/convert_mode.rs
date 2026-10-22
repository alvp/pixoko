//! Convertir modo (specs/hito-2-convertir-modo.md): el formulario por modo de origen, el informe
//! de `convert_image` sin tocar el documento (solo al cambiar de destino, en dos tiempos si la
//! imagen es grande), los permisos, el motivo que deja `Aceptar` atenuado, el aviso de historia y
//! la reserva previa de memoria. Sin egui: los tipos del formulario son de `px-ui` y se prueba sin
//! ventana.

use px_core::{
    Allowed, BlendMode, CelsAnalysis, Conversion, ConversionReport, ConvertError, Document, Image,
    Mode, Rgba, analyze_cels, convert_image, target_byte_len,
};
use px_ui::actions::{AppAction, Locks};
use px_ui::color_edit::parse_hex;
use px_ui::dialog::{
    Dialog, DialogKind, FOOTER_COLUMNS, FieldId, FieldKind, FieldSpec, FormSpec, MISSING_PREFIX,
    Modal, Rows, ShowWhen, Values,
};

use crate::new_image::{ImageAlloc, WARN_PIXELS};

/// Por encima de estos píxeles el informe (y el aceptar) va en dos tiempos: 2²².
pub const SYNC_PIXELS: u64 = 1 << 22;
/// Fotogramas de espera antes de calcular, para que se vea «Calculando…» antes de congelarse.
pub const WAIT_FRAMES: u8 = 2;
/// Cota de cabecera y paleta de un paso de la historia (spec 9, fila 15).
pub const HISTORY_MARGIN: u64 = 2048;

pub const FIELD_TARGET: FieldId = "destino";
pub const FIELD_REPORT: FieldId = "informe";
pub const FIELD_LUMA_GRAY_ALPHA: FieldId = "luma_gris_alfa";
pub const FIELD_LUMA_GRAY: FieldId = "luma_gris";
pub const FIELD_ALPHA: FieldId = "alfa_gris";
pub const FIELD_ALPHA_HEX: FieldId = "fondo_hex";
/// El botón `Aplanar` que ofrece el motivo «con varias capas: aplana antes» (3.5, condición (a)).
pub const FIELD_FLATTEN: FieldId = "aplanar";

/// Los modos en el orden de las opciones del destino.
const MODES: [Mode; 4] = [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray];
const OPTIONS_FROM_INDEXED: &[&str] = &["RGBA", "Gris con alfa", "Gris"];
const OPTIONS_FROM_RGBA: &[&str] = &["Indexado", "Gris con alfa", "Gris"];
const OPTIONS_FROM_GRAY_ALPHA: &[&str] = &["Indexado", "RGBA", "Gris"];
const OPTIONS_FROM_GRAY: &[&str] = &["Indexado", "RGBA", "Gris con alfa"];
const ALPHA_OPTIONS: &[&str] = &[
    "No perder el alfa",
    "Fondo blanco",
    "Fondo negro",
    "Fondo: el color activo",
    "Fondo: un color hexadecimal",
];
/// Posición de cada opción del grupo del alfa.
const ALPHA_KEEP: usize = 0;
const ALPHA_WHITE: usize = 1;
const ALPHA_BLACK: usize = 2;
const ALPHA_ACTIVE: usize = 3;
const ALPHA_HEX: usize = 4;
const DEFAULT_HEX: Rgba = Rgba::opaque(0x80, 0x80, 0x80);

const PENDING_REASON: &str = "calculando el informe";
const REPORT_PENDING: &str = "Calculando el informe…";
const NO_ALPHA_IN_HEX: &str = "Color: el fondo no lleva alfa";

/// Los modos de destino de `from`: los otros tres, en el orden Indexado · RGBA · Gris con alfa ·
/// Gris. El mismo modo no se ofrece.
pub fn targets(from: Mode) -> [Mode; 3] {
    let mut out = [Mode::Indexed; 3];
    let mut slot = out.iter_mut();
    for mode in MODES.into_iter().filter(|mode| *mode != from) {
        if let Some(place) = slot.next() {
            *place = mode;
        }
    }
    out
}

fn target_options(from: Mode) -> &'static [&'static str] {
    match from {
        Mode::Indexed => OPTIONS_FROM_INDEXED,
        Mode::Rgba => OPTIONS_FROM_RGBA,
        Mode::GrayAlpha => OPTIONS_FROM_GRAY_ALPHA,
        Mode::Gray => OPTIONS_FROM_GRAY,
    }
}

/// El nombre de un modo en los avisos.
fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Indexed => "indexado",
        Mode::Rgba => "RGBA",
        Mode::GrayAlpha => "gris con alfa",
        Mode::Gray => "gris",
    }
}

/// El formulario de convertir desde `from`: el destino, el informe y los permisos que ese origen
/// puede necesitar (ninguno marcado por defecto).
pub fn convert_form(from: Mode) -> FormSpec {
    let to = targets(from);
    let index_of = |mode: Mode| to.iter().position(|candidate| *candidate == mode);
    let shown = |mode: Mode| {
        index_of(mode).map(|choice| ShowWhen {
            field: FIELD_TARGET,
            choice,
        })
    };
    let mut fields = vec![
        FieldSpec {
            id: FIELD_TARGET,
            label: "Modo de destino",
            kind: FieldKind::Choice {
                options: target_options(from),
                default: 0,
            },
            shown_when: None,
        },
        FieldSpec {
            id: FIELD_REPORT,
            label: "Informe",
            kind: FieldKind::ReadOnly {
                text: REPORT_PENDING.to_string(),
                rows: Rows::Fixed(7),
            },
            shown_when: None,
        },
    ];
    if matches!(from, Mode::Rgba | Mode::Indexed) {
        for (id, mode) in [
            (FIELD_LUMA_GRAY_ALPHA, Mode::GrayAlpha),
            (FIELD_LUMA_GRAY, Mode::Gray),
        ] {
            fields.push(FieldSpec {
                id,
                label: "Permitir perder el color (pasar a gris)",
                kind: FieldKind::Check { default: false },
                shown_when: shown(mode),
            });
        }
    }
    if from != Mode::Gray {
        fields.push(FieldSpec {
            id: FIELD_ALPHA,
            label: "Si hay alfa, componer sobre:",
            kind: FieldKind::Choice {
                options: ALPHA_OPTIONS,
                default: ALPHA_KEEP,
            },
            shown_when: shown(Mode::Gray),
        });
        fields.push(FieldSpec {
            id: FIELD_ALPHA_HEX,
            label: "Color",
            kind: FieldKind::Hex {
                mode: Mode::Rgba,
                default: DEFAULT_HEX,
            },
            shown_when: Some(ShowWhen {
                field: FIELD_ALPHA,
                choice: ALPHA_HEX,
            }),
        });
    }
    FormSpec {
        title: "Convertir modo",
        accept: "Aceptar",
        cancel: Some("Cancelar"),
        fields,
        check: Some(check_values),
    }
}

/// El fondo hexadecimal es opaco: con alfa distinto de `ff` no vale.
fn check_values(values: &Values) -> Option<String> {
    values
        .color(FIELD_ALPHA_HEX)
        .filter(|color| color.a != 255)
        .map(|_| NO_ALPHA_IN_HEX.to_string())
}

/// Abre el diálogo de convertir desde `from`; hasta el primer informe, `Aceptar` queda atenuado.
#[cfg_attr(not(test), allow(dead_code))]
pub fn open_convert_mode(modal: &mut Modal, from: Mode, locks: Locks) -> bool {
    if !modal.open(DialogKind::ConvertMode, convert_form(from), locks) {
        return false;
    }
    if let Modal::Open(dialog) = modal {
        dialog.set_block(Some(PENDING_REASON.to_string()));
    }
    true
}

/// Como [`open_convert_mode`] con `target` ya elegido como modo de destino (si es uno de los
/// destinos de `from`).
#[cfg_attr(not(test), allow(dead_code))]
pub fn open_convert_mode_to(modal: &mut Modal, from: Mode, target: Mode, locks: Locks) -> bool {
    open_convert_mode_layers(modal, from, 1, Some(target), locks)
}

/// El formulario de convertir `layers` capas desde `from`: el de siempre y, con varias, un botón
/// `Aplanar` visible solo con el destino Gris (que con varias capas no existe: el motivo ofrece
/// aplanar antes).
pub fn convert_form_layers(from: Mode, layers: usize) -> FormSpec {
    let mut spec = convert_form(from);
    if layers > 1
        && let Some(gray) = targets(from).iter().position(|mode| *mode == Mode::Gray)
    {
        spec.fields.push(FieldSpec {
            id: FIELD_FLATTEN,
            label: "Capas",
            kind: FieldKind::Button {
                text: "Aplanar".to_string(),
                aside: None,
            },
            shown_when: Some(ShowWhen {
                field: FIELD_TARGET,
                choice: gray,
            }),
        });
    }
    spec
}

/// La acción que lanza el botón de un campo del diálogo de convertir, si lo es de una acción.
pub fn flatten_offer(field: FieldId) -> Option<AppAction> {
    (field == FIELD_FLATTEN).then_some(AppAction::FlattenLayers)
}

/// Abre el diálogo de convertir `layers` capas desde `from`, con `target` ya elegido si se da (y
/// es uno de los destinos de `from`).
pub fn open_convert_mode_layers(
    modal: &mut Modal,
    from: Mode,
    layers: usize,
    target: Option<Mode>,
    locks: Locks,
) -> bool {
    let mut spec = convert_form_layers(from, layers);
    if let Some(target) = target
        && let Some(index) = targets(from).iter().position(|mode| *mode == target)
        && let Some(field) = spec.fields.iter_mut().find(|f| f.id == FIELD_TARGET)
        && let FieldKind::Choice { default, .. } = &mut field.kind
    {
        *default = index;
    }
    if !modal.open(DialogKind::ConvertMode, spec, locks) {
        return false;
    }
    if let Modal::Open(dialog) = modal {
        dialog.set_block(Some(PENDING_REASON.to_string()));
    }
    true
}

/// Lo que dice `convert_image(.., NOTHING)` de una conversión.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteInfo {
    pub colors: usize,
    pub transparent: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Analysis {
    /// Sin pérdidas; a indexado, la paleta que saldría.
    Exact { palette: Option<PaletteInfo> },
    /// Con pérdida: los dos contadores completos.
    Lossy(ConversionReport),
    /// Más de 256 colores a indexado (sin cuantizador hasta el hito 4).
    TooManyColors,
    /// La imagen convertida no cabe.
    TooLarge,
}

/// Por qué un documento con varias capas no puede convertirse a un modo, antes de mirar los
/// píxeles (specs/hito-3-combinar.md, D8 y condición (a)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerBlock {
    /// Destino Gris con varias capas: el gris sin alfa no admite capas (se ofrece aplanar antes).
    MultiLayerGray,
    /// Destino indexado con capas de opacidad distinta de 255 o mezcla distinta de Normal.
    LayerProps,
}

impl LayerBlock {
    /// El motivo de `Aceptar` atenuado (≤ 37 caracteres).
    pub fn reason(self) -> &'static str {
        match self {
            LayerBlock::MultiLayerGray => "con varias capas: aplana antes",
            LayerBlock::LayerProps => "hay capas con opacidad o mezcla",
        }
    }

    /// La explicación del informe.
    fn explanation(self) -> &'static str {
        match self {
            LayerBlock::MultiLayerGray => {
                "El gris sin alfa no admite varias capas: aplana antes (botón Aplanar) o elige otro modo."
            }
            LayerBlock::LayerProps => {
                "El indexado no tiene opacidad ni mezcla: pon las capas a 255 y Normal, o aplana."
            }
        }
    }
}

/// De un resultado de `convert_image` a un `Analysis`. Pura: permite probar `TooLarge` sin reservar
/// memoria.
pub fn classify(result: Result<Conversion, ConvertError>) -> Analysis {
    match result {
        Ok(conversion) => Analysis::Exact {
            palette: conversion.image.palette().map(|palette| PaletteInfo {
                colors: palette.len(),
                transparent: palette.transparent_index().is_some(),
            }),
        },
        Err(ConvertError::LossNotAllowed { report }) => Analysis::Lossy(report),
        Err(ConvertError::TooManyColors { .. }) => Analysis::TooManyColors,
        Err(ConvertError::TooLarge { .. }) => Analysis::TooLarge,
    }
}

/// El informe de convertir `image` a `target`, sin tocar nada.
pub fn analyze(image: &Image, target: Mode) -> Analysis {
    classify(convert_image(image, target, Allowed::NOTHING))
}

/// Quien calcula el informe (la real, o una que cuenta las llamadas en las pruebas).
pub trait Analyzer {
    fn analyze(&mut self, image: &Image, target: Mode) -> Analysis;
}

/// El analizador real: `convert_image(.., NOTHING)`.
pub struct SystemAnalyzer;

impl Analyzer for SystemAnalyzer {
    fn analyze(&mut self, image: &Image, target: Mode) -> Analysis {
        analyze(image, target)
    }
}

/// El fondo con el que se compone el alfa al pasar a gris.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaChoice {
    Keep,
    White,
    Black,
    Active,
    Hex(Rgba),
}

/// Lo marcado en el formulario para el destino elegido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permissions {
    pub luma: bool,
    pub alpha: AlphaChoice,
}

/// Lo que haría falta permitir para convertir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Needs {
    pub luma: bool,
    pub alpha: bool,
}

pub fn needs(analysis: &Analysis) -> Needs {
    match analysis {
        Analysis::Lossy(report) => Needs {
            luma: report.color_lost > 0,
            alpha: report.alpha_lost > 0,
        },
        _ => Needs {
            luma: false,
            alpha: false,
        },
    }
}

/// El fondo opaco de `perms`, si lo hay y vale: `None` con un color activo o hexadecimal que no
/// es opaco (o sin color activo).
fn background(perms: &Permissions, active: Option<Rgba>) -> Option<Option<[u8; 3]>> {
    let opaque = |color: Rgba| (color.a == 255).then_some(Some([color.r, color.g, color.b]));
    match perms.alpha {
        AlphaChoice::Keep => Some(None),
        AlphaChoice::White => Some(Some([255, 255, 255])),
        AlphaChoice::Black => Some(Some([0, 0, 0])),
        AlphaChoice::Active => active.and_then(opaque),
        AlphaChoice::Hex(color) => opaque(color),
    }
}

/// El `Allowed` de lo marcado; `None` si el fondo no es opaco.
pub fn allowed_of(perms: &Permissions, active: Option<Rgba>) -> Option<Allowed> {
    background(perms, active).map(|drop_alpha| Allowed {
        luma: perms.luma,
        drop_alpha,
    })
}

/// Un requisito que falta para poder aceptar, en el orden en que se lista.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Missing {
    /// Marcar «permitir perder el color».
    Luma,
    /// Elegir el fondo del alfa.
    AlphaBackground,
    /// Un valor inválido (hexadecimal mal escrito o con alfa; color activo sin color o con alfa).
    Value,
}

impl Missing {
    /// Rótulo largo, para cuando es el único.
    fn long(self) -> &'static str {
        match self {
            Missing::Luma => "permitir perder el color",
            Missing::AlphaBackground => "elegir el fondo del alfa",
            Missing::Value => "fondo válido",
        }
    }

    /// Rótulo corto, para la lista de dos o más.
    fn short(self) -> &'static str {
        match self {
            Missing::Luma => "perder el color",
            Missing::AlphaBackground => "fondo del alfa",
            Missing::Value => "fondo válido",
        }
    }

    /// Una palabra, para la forma con cuenta.
    fn word(self) -> &'static str {
        match self {
            Missing::Luma => "color",
            Missing::AlphaBackground => "fondo",
            Missing::Value => "valor",
        }
    }
}

/// El texto del pie para una lista de requisitos (ya en orden): `falta: …` con el rótulo largo si
/// hay uno, con los cortos si hay varios y, si no caben en el pie, `faltan N: …`. `None` si no
/// falta nada.
pub fn missing_text(items: &[Missing]) -> Option<String> {
    match items {
        [] => None,
        [one] => Some(format!("{MISSING_PREFIX}{}", one.long())),
        _ => {
            let labels: Vec<&str> = items.iter().map(|item| item.short()).collect();
            let list = format!("{MISSING_PREFIX}{}", labels.join("; "));
            if list.chars().count() <= FOOTER_COLUMNS {
                return Some(list);
            }
            let words: Vec<&str> = items.iter().map(|item| item.word()).collect();
            Some(format!("faltan {}: {}", items.len(), words.join(", ")))
        }
    }
}

/// Lo que falta para aceptar, en el orden color, fondo, valor. Los análisis imposibles no tienen
/// requisitos que cubrir (su motivo va solo). `value_problem`: hay un valor inválido en un campo.
pub fn pending(
    analysis: &Analysis,
    perms: &Permissions,
    active: Option<Rgba>,
    value_problem: bool,
) -> Vec<Missing> {
    if matches!(analysis, Analysis::TooManyColors | Analysis::TooLarge) {
        return Vec::new();
    }
    let need = needs(analysis);
    let mut missing = Vec::new();
    if need.luma && !perms.luma {
        missing.push(Missing::Luma);
    }
    if need.alpha && perms.alpha == AlphaChoice::Keep {
        missing.push(Missing::AlphaBackground);
    }
    // Un fondo «color activo» no opaco se rechaza siempre que esté elegido.
    let bad_active = perms.alpha == AlphaChoice::Active && active.is_none_or(|color| color.a < 255);
    if value_problem || bad_active {
        missing.push(Missing::Value);
    }
    missing
}

/// Por qué `Aceptar` está atenuado; `analysis = None` es «pendiente». Una lista de pendientes
/// lleva `falta: ` (≤ 44 columnas); lo demás, ≤ 37 caracteres (el pie les pone `error: `).
pub fn block_reason(
    analysis: Option<&Analysis>,
    perms: &Permissions,
    active: Option<Rgba>,
) -> Option<String> {
    let Some(analysis) = analysis else {
        return Some(PENDING_REASON.to_string());
    };
    match analysis {
        Analysis::TooManyColors => return Some("más de 256 colores: sin cuantizador".to_string()),
        Analysis::TooLarge => return Some("la imagen convertida no cabe".to_string()),
        Analysis::Exact { .. } | Analysis::Lossy(_) => {}
    }
    let missing = pending(analysis, perms, active, false);
    if missing == [Missing::Value] {
        // Solo el valor: su motivo concreto, con `error: ` en el pie.
        return match active {
            None => Some("no hay color activo".to_string()),
            Some(color) => Some(format!(
                "el color activo #{:02x}{:02x}{:02x}{:02x} tiene alfa",
                color.r, color.g, color.b, color.a
            )),
        };
    }
    missing_text(&missing)
}

/// Lo que se aplica al aceptar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub target: Mode,
    pub allowed: Allowed,
}

/// El plan de los valores del formulario (solo campos visibles). `None` si faltan campos o el
/// fondo no es opaco.
pub fn plan_from_values(values: &Values, from: Mode, active: Option<Rgba>) -> Option<Plan> {
    let target = *targets(from).get(values.choice(FIELD_TARGET)?)?;
    let luma = match target {
        Mode::GrayAlpha => values.check(FIELD_LUMA_GRAY_ALPHA),
        Mode::Gray => values.check(FIELD_LUMA_GRAY),
        _ => None,
    }
    .unwrap_or(false);
    let alpha = match values.choice(FIELD_ALPHA) {
        None | Some(ALPHA_KEEP) => AlphaChoice::Keep,
        Some(ALPHA_WHITE) => AlphaChoice::White,
        Some(ALPHA_BLACK) => AlphaChoice::Black,
        Some(ALPHA_ACTIVE) => AlphaChoice::Active,
        Some(_) => AlphaChoice::Hex(values.color(FIELD_ALPHA_HEX)?),
    };
    let allowed = allowed_of(&Permissions { luma, alpha }, active)?;
    Some(Plan { target, allowed })
}

/// El límite y el uso de la historia del documento.
#[derive(Clone, Copy, Debug)]
pub struct History {
    pub limit: usize,
    pub used: usize,
}

const HISTORY_ALL: &str = "aviso: convertir descarta el historial anterior; solo se podrá deshacer hasta tu próxima edición";
const HISTORY_PART: &str = "aviso: convertir descarta parte del historial anterior";

/// El aviso de historia de una imagen con `pixel_bytes` bytes de píxeles (D7): solo si el paso
/// cuesta más de la mitad del límite, y según lo que de verdad pasa.
pub fn history_warning_for_bytes(pixel_bytes: u64, history: History) -> Option<String> {
    let cost = pixel_bytes.saturating_add(HISTORY_MARGIN);
    let limit = history.limit as u64;
    let used = history.used as u64;
    if cost > limit {
        Some(HISTORY_ALL.to_string())
    } else if cost.saturating_mul(2) > limit && used.saturating_add(cost) > limit {
        Some(HISTORY_PART.to_string())
    } else {
        None
    }
}

/// Como [`history_warning_for_bytes`], con los bytes de píxeles de `image`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn history_warning(image: &Image, history: History) -> Option<String> {
    let bytes = Image::byte_len(image.width(), image.height(), image.mode())
        .map_or(u64::MAX, |bytes| bytes as u64);
    history_warning_for_bytes(bytes, history)
}

fn plural(count: u64, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// El texto del informe (como mucho 7 líneas de 42 columnas): el estado, el fondo del alfa, el
/// aviso de tamaño y el de historia.
pub fn report_text(
    analysis: Option<&Analysis>,
    size: (u32, u32),
    perms: &Permissions,
    active: Option<Rgba>,
    image: &Image,
    history: History,
) -> String {
    let bytes = Image::byte_len(image.width(), image.height(), image.mode())
        .map_or(u64::MAX, |bytes| bytes as u64);
    report_body(analysis, size, perms, active, bytes, history)
}

/// Como [`report_text`] para `layers` capas: lleva la línea «Capas: N (se convierten todas)», las
/// pérdidas son las sumadas de todas las celdas y el aviso de historia mide `pixel_bytes`, la
/// suma de las imágenes que guardará el paso.
pub fn report_text_all(
    analysis: Option<&Analysis>,
    layers: usize,
    size: (u32, u32),
    perms: &Permissions,
    active: Option<Rgba>,
    pixel_bytes: u64,
    history: History,
) -> String {
    let body = report_body(analysis, size, perms, active, pixel_bytes, history);
    format!("Capas: {layers} (se convierten todas)\n{body}")
}

fn report_body(
    analysis: Option<&Analysis>,
    size: (u32, u32),
    perms: &Permissions,
    active: Option<Rgba>,
    pixel_bytes: u64,
    history: History,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    match analysis {
        None => lines.push(REPORT_PENDING.to_string()),
        Some(Analysis::Exact { palette: None }) => {
            lines.push("Sin pérdidas: la conversión es exacta.".to_string());
        }
        Some(Analysis::Exact {
            palette: Some(palette),
        }) => {
            let colors = plural(palette.colors as u64, "color", "colores");
            let transparent = if palette.transparent {
                ", el 0 transparente"
            } else {
                ""
            };
            lines.push(format!("Sin pérdidas. Paleta nueva de {colors}{transparent}."));
        }
        Some(Analysis::Lossy(report)) => {
            if report.color_lost > 0 {
                let pixels = plural(report.color_lost, "píxel", "píxeles");
                lines.push(format!("Pierde el color en {pixels}."));
            }
            if report.alpha_lost > 0 {
                let pixels = plural(report.alpha_lost, "píxel", "píxeles");
                lines.push(format!("Pierde el alfa en {pixels}."));
            }
        }
        Some(Analysis::TooManyColors) => lines.push(
            "Esta imagen tiene más de 256 colores y una paleta indexada no los admite. Más de 256 colores: el cuantizador llega en el hito 4; hasta entonces no se puede pasar a Indexado."
                .to_string(),
        ),
        Some(Analysis::TooLarge) => lines
            .push("La imagen convertida no cabe en memoria (máximo 1 GiB).".to_string()),
    }
    if let Some(Analysis::Lossy(report)) = analysis
        && report.alpha_lost > 0
        && perms.alpha != AlphaChoice::Keep
        && let Some(Allowed {
            drop_alpha: Some([r, g, b]),
            ..
        }) = allowed_of(perms, active)
    {
        lines.push(format!("Fondo del alfa: #{r:02x}{g:02x}{b:02x}"));
    }
    if u64::from(size.0) * u64::from(size.1) > WARN_PIXELS {
        lines.push(format!("aviso: más de {WARN_PIXELS} px; puede tardar"));
    }
    if matches!(analysis, Some(Analysis::Exact { .. } | Analysis::Lossy(_)))
        && let Some(warning) = history_warning_for_bytes(pixel_bytes, history)
    {
        lines.push(warning);
    }
    lines.join("\n")
}

/// El estado del diálogo entre fotogramas: el informe vigente y la espera de un cálculo.
pub struct ConvertSync {
    from: Mode,
    state_id: u64,
    active: Option<Rgba>,
    shown: Option<(Mode, Analysis)>,
    waiting: Option<(Mode, u8)>,
}

impl ConvertSync {
    pub fn new(from: Mode, state_id: u64, active: Option<Rgba>) -> ConvertSync {
        ConvertSync {
            from,
            state_id,
            active,
            shown: None,
            waiting: None,
        }
    }

    /// El informe vigente.
    pub fn analysis(&self) -> Option<&Analysis> {
        self.shown.as_ref().map(|(_, analysis)| analysis)
    }

    /// El color activo al abrir.
    pub fn active(&self) -> Option<Rgba> {
        self.active
    }
}

/// Lo marcado en el diálogo para `target`, leído aunque el formulario sea inválido.
fn read_permissions(dialog: &Dialog, target: Mode) -> Permissions {
    let luma_id = match target {
        Mode::GrayAlpha => Some(FIELD_LUMA_GRAY_ALPHA),
        Mode::Gray => Some(FIELD_LUMA_GRAY),
        _ => None,
    };
    let luma = luma_id.is_some_and(|id| dialog.is_shown(id) && dialog.checked(id).unwrap_or(false));
    let alpha = if dialog.is_shown(FIELD_ALPHA) {
        match dialog.chosen(FIELD_ALPHA) {
            Some(ALPHA_WHITE) => AlphaChoice::White,
            Some(ALPHA_BLACK) => AlphaChoice::Black,
            Some(ALPHA_ACTIVE) => AlphaChoice::Active,
            Some(ALPHA_HEX) => {
                let text = dialog.text(FIELD_ALPHA_HEX).unwrap_or_default();
                AlphaChoice::Hex(parse_hex(&text, Mode::Rgba, DEFAULT_HEX).unwrap_or(DEFAULT_HEX))
            }
            _ => AlphaChoice::Keep,
        }
    } else {
        AlphaChoice::Keep
    };
    Permissions { luma, alpha }
}

fn current_report(dialog: &Dialog) -> Option<&str> {
    dialog
        .spec()
        .fields
        .iter()
        .find(|field| field.id == FIELD_REPORT)
        .and_then(|field| match &field.kind {
            FieldKind::ReadOnly { text, .. } => Some(text.as_str()),
            _ => None,
        })
}

/// Pone el diálogo al día tras un fotograma: calcula el informe solo si cambió el destino (o el
/// documento), en dos tiempos si la imagen es grande, y recompone el texto y el motivo con lo
/// marcado. `true` si hay que repintar (algo cambió o hay un cálculo en espera).
pub fn sync_convert(
    dialog: &mut Dialog,
    sync: &mut ConvertSync,
    image: &Image,
    state_id: u64,
    history: History,
    analyzer: &mut dyn Analyzer,
) -> bool {
    let Some(target) = dialog
        .chosen(FIELD_TARGET)
        .and_then(|option| targets(sync.from).get(option).copied())
    else {
        return false;
    };
    if sync.state_id != state_id {
        sync.state_id = state_id;
        sync.shown = None;
        sync.waiting = None;
    }
    if !matches!(&sync.shown, Some((shown, _)) if *shown == target) {
        let pixels = u64::from(image.width()) * u64::from(image.height());
        let ready = if pixels <= SYNC_PIXELS {
            true
        } else {
            match &mut sync.waiting {
                Some((waiting, left)) if *waiting == target => {
                    *left = left.saturating_sub(1);
                    *left == 0
                }
                _ => {
                    sync.waiting = Some((target, WAIT_FRAMES));
                    false
                }
            }
        };
        if ready {
            sync.shown = Some((target, analyzer.analyze(image, target)));
            sync.waiting = None;
        } else {
            sync.shown = None;
        }
    }
    let analysis = sync.shown.as_ref().map(|(_, analysis)| analysis);
    let perms = read_permissions(dialog, target);
    let report = report_text(
        analysis,
        (image.width(), image.height()),
        &perms,
        sync.active,
        image,
        history,
    );
    let reason = block_reason(analysis, &perms, sync.active);
    let mut changed = sync.waiting.is_some();
    if current_report(dialog) != Some(report.as_str()) {
        changed |= dialog.set_report(FIELD_REPORT, report);
    }
    if dialog.block() != reason.as_deref() {
        dialog.set_block(reason);
        changed = true;
    }
    // Con un valor inválido de campo y otros pendientes, el pie los lista todos (el error del
    // campo sigue junto al campo).
    let summary = analysis.and_then(|analysis| {
        let problem = !dialog.problems().is_empty();
        let missing = pending(analysis, &perms, sync.active, problem);
        (problem && missing.len() >= 2)
            .then(|| missing_text(&missing))
            .flatten()
    });
    if dialog.summary() != summary.as_deref() {
        dialog.set_summary(summary);
        changed = true;
    }
    changed
}

/// El aviso mientras se espera para convertir una imagen grande.
pub const CONVERTING_NOTICE: &str = "Convirtiendo…";

/// Convertir ya, o esperar `WAIT_FRAMES` fotogramas para que se vea el aviso.
#[derive(Debug, PartialEq, Eq)]
pub enum Start {
    Now(Plan),
    Later(PendingConvert),
}

/// Una conversión que espera su fotograma.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingConvert {
    plan: Plan,
    left: u8,
}

impl PendingConvert {
    pub fn plan(&self) -> Plan {
        self.plan
    }

    /// Un fotograma: `Some(plan)` exactamente en el `WAIT_FRAMES`-ésimo, y nunca después.
    pub fn tick(&mut self) -> Option<Plan> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        (self.left == 0).then_some(self.plan)
    }
}

pub fn start_convert(plan: Plan, pixels: u64) -> Start {
    if pixels <= SYNC_PIXELS {
        Start::Now(plan)
    } else {
        Start::Later(PendingConvert {
            plan,
            left: WAIT_FRAMES,
        })
    }
}

/// Por qué no se pudo convertir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConvertModeError {
    /// No hay memoria: los bytes que hacían falta.
    Memory { bytes: u64 },
}

/// Reserva previa: comprueba (y suelta) lo que hará falta tras `apply`, la imagen convertida y la
/// subida RGBA de la textura, antes de tocar nada.
pub fn reserve_for_convert(
    image: &Image,
    target: Mode,
    alloc: &mut dyn ImageAlloc,
) -> Result<(), ConvertModeError> {
    let staging = 4 * u64::from(image.width()) * u64::from(image.height());
    let converted = target_byte_len(image.width(), image.height(), target)
        .map_or(u64::MAX / 2, |bytes| bytes as u64);
    let bytes = converted.saturating_add(staging);
    if alloc.probe(usize::try_from(bytes).unwrap_or(usize::MAX)) {
        Ok(())
    } else {
        Err(ConvertModeError::Memory { bytes })
    }
}

/// Aviso de 5 s cuando no hay memoria.
pub fn convert_memory_notice(width: u32, height: u32, target: Mode, bytes: u64) -> String {
    let mib = bytes.div_ceil(1 << 20).max(1);
    format!(
        "sin memoria para convertir {width}×{height} a {} (~{mib} MiB)",
        mode_label(target)
    )
}

/// El aviso de 5 s tras convertir: el modo y, si hubo pérdida, cuánta.
pub fn describe_converted(target: Mode, analysis: Option<&Analysis>) -> String {
    let mut text = format!("imagen convertida a {}", mode_label(target));
    if let Some(Analysis::Lossy(report)) = analysis {
        let mut lost = Vec::new();
        if report.color_lost > 0 {
            lost.push(format!("color {} px", report.color_lost));
        }
        if report.alpha_lost > 0 {
            lost.push(format!("alfa {} px", report.alpha_lost));
        }
        if !lost.is_empty() {
            text.push_str(&format!(" · perdido: {}", lost.join(", ")));
        }
    }
    text
}

// ---------------------------------------------------------------------------------------------
// Varias capas (specs/hito-3-combinar.md, D8): `ConvertAll`
// ---------------------------------------------------------------------------------------------

/// De un resultado de `analyze_cels` a un `Analysis`. Pura: permite probar sin imágenes grandes.
pub fn classify_cels(result: Result<CelsAnalysis, ConvertError>) -> Analysis {
    match result {
        Ok(analysis) => Analysis::Exact {
            palette: analysis.palette.map(|palette| PaletteInfo {
                colors: palette.len(),
                transparent: palette.transparent_index().is_some(),
            }),
        },
        Err(ConvertError::LossNotAllowed { report }) => Analysis::Lossy(report),
        Err(ConvertError::TooManyColors { .. }) => Analysis::TooManyColors,
        Err(ConvertError::TooLarge { .. }) => Analysis::TooLarge,
    }
}

/// Por qué `doc` no puede convertirse a `target` por sus capas (antes de mirar los píxeles).
pub fn layer_block(doc: &Document, target: Mode) -> Option<LayerBlock> {
    let layers = doc.layers();
    if target == Mode::Gray && layers.len() > 1 {
        return Some(LayerBlock::MultiLayerGray);
    }
    (target == Mode::Indexed
        && layers
            .iter()
            .any(|layer| layer.opacity() != 255 || layer.blend() != BlendMode::Normal))
    .then_some(LayerBlock::LayerProps)
}

/// El informe de convertir **todas** las capas de `doc` a `target`, sin tocar nada: las pérdidas
/// sumadas de todas las celdas y, a indexado, la paleta común. Solo si [`layer_block`] no lo
/// impide.
pub fn analyze_document(doc: &Document, target: Mode) -> Analysis {
    let layers = doc.layers();
    let cels: Vec<Option<&Image>> = layers.iter().map(|layer| doc.image(layer.cel())).collect();
    classify_cels(analyze_cels(
        &cels,
        (doc.width(), doc.height()),
        doc.mode(),
        target,
        Allowed::NOTHING,
    ))
}

/// Píxeles de las celdas con imagen de `doc`: lo que cuesta calcular el informe y convertir.
pub fn convert_pixels(doc: &Document) -> u64 {
    let cells = doc
        .layers()
        .iter()
        .filter(|layer| doc.image(layer.cel()).is_some())
        .count() as u64;
    u64::from(doc.width()) * u64::from(doc.height()) * cells
}

/// Bytes de píxeles de las celdas con imagen de `doc`: lo que guardará el paso de historia.
pub fn convert_bytes(doc: &Document) -> u64 {
    doc.materialized_bytes() as u64
}

/// Quien calcula el informe de un documento con capas (el real, o uno que cuenta las llamadas en
/// las pruebas).
pub trait DocAnalyzer {
    fn analyze(&mut self, doc: &Document, target: Mode) -> Analysis;
}

/// El analizador real: `analyze_cels` sobre todas las celdas.
pub struct SystemDocAnalyzer;

impl DocAnalyzer for SystemDocAnalyzer {
    fn analyze(&mut self, doc: &Document, target: Mode) -> Analysis {
        analyze_document(doc, target)
    }
}

/// Como [`sync_convert`] para un documento con varias capas (o con la celda activa vacía): el
/// informe se calcula sobre todas las celdas, solo al cambiar de destino y en dos tiempos si la
/// suma de sus píxeles pasa de `SYNC_PIXELS`.
pub fn sync_convert_all(
    dialog: &mut Dialog,
    sync: &mut ConvertSync,
    doc: &Document,
    history: History,
    analyzer: &mut dyn DocAnalyzer,
) -> bool {
    let Some(target) = dialog
        .chosen(FIELD_TARGET)
        .and_then(|option| targets(sync.from).get(option).copied())
    else {
        return false;
    };
    let state_id = doc.state_id();
    if sync.state_id != state_id {
        sync.state_id = state_id;
        sync.shown = None;
        sync.waiting = None;
    }
    // Con varias capas hay destinos que no existen: no hay informe que calcular, solo el motivo.
    let block = layer_block(doc, target);
    if block.is_some() {
        sync.shown = None;
        sync.waiting = None;
    } else if !matches!(&sync.shown, Some((shown, _)) if *shown == target) {
        let ready = if convert_pixels(doc) <= SYNC_PIXELS {
            true
        } else {
            match &mut sync.waiting {
                Some((waiting, left)) if *waiting == target => {
                    *left = left.saturating_sub(1);
                    *left == 0
                }
                _ => {
                    sync.waiting = Some((target, WAIT_FRAMES));
                    false
                }
            }
        };
        if ready {
            sync.shown = Some((target, analyzer.analyze(doc, target)));
            sync.waiting = None;
        } else {
            sync.shown = None;
        }
    }
    let analysis = sync.shown.as_ref().map(|(_, analysis)| analysis);
    let perms = read_permissions(dialog, target);
    let layers = doc.layers().len();
    let report = match block {
        Some(block) => format!(
            "Capas: {layers} (se convierten todas)\n{}",
            block.explanation()
        ),
        None => report_text_all(
            analysis,
            layers,
            (doc.width(), doc.height()),
            &perms,
            sync.active,
            convert_bytes(doc),
            history,
        ),
    };
    let reason = match block {
        Some(block) => Some(block.reason().to_string()),
        None => block_reason(analysis, &perms, sync.active),
    };
    let mut changed = sync.waiting.is_some();
    if current_report(dialog) != Some(report.as_str()) {
        changed |= dialog.set_report(FIELD_REPORT, report);
    }
    if dialog.block() != reason.as_deref() {
        dialog.set_block(reason);
        changed = true;
    }
    let summary = analysis.and_then(|analysis| {
        let problem = !dialog.problems().is_empty();
        let missing = pending(analysis, &perms, sync.active, problem);
        (problem && missing.len() >= 2)
            .then(|| missing_text(&missing))
            .flatten()
    });
    if dialog.summary() != summary.as_deref() {
        dialog.set_summary(summary);
        changed = true;
    }
    changed
}

#[cfg(test)]
mod all_tests;
#[cfg(test)]
mod dialog_tests;
#[cfg(test)]
mod tests;
