//! El panel de capas, la parte pura (specs/hito-3-capas-interfaz.md, decisiones 1 a 4 y 12): la
//! geometría en `u`, `hit`, la colocación dentro de la ventana, la lista (qué filas se ven), el pie
//! de memoria y las reglas de los campos. Sin egui; el pintado está en `layers_panel_view`.
//!
//! Todo se mide en `u` (píxeles de interfaz) desde la esquina del panel y cae en píxeles físicos
//! enteros a cualquier escala `n`.

use px_core::{Document, LayerId};

use crate::actions::LayerNav;
use crate::chrome::window_layout;
use crate::float_window;
use crate::view::ScreenRect;

/// Ancho y alto del panel, en `u`.
pub const PANEL_W_U: i64 = 188;
pub const PANEL_H_U: i64 = 277;
/// Filas de la lista que se ven a la vez.
pub const VISIBLE_ROWS: usize = 6;
/// Caracteres del nombre en una fila (más: 15 y `…`) y del campo del nombre.
pub const NAME_CHARS: usize = 16;
pub const NAME_EDIT_CHARS: usize = 60;
/// Filas que desplaza una muesca de la rueda.
pub const SCROLL_ROWS_PER_NOTCH: i64 = 3;
/// Margen de la posición inicial respecto al borde derecho del lienzo y al superior, en `u`.
pub const DEFAULT_MARGIN_U: i64 = 8;
/// Ancho del pie de memoria, en caracteres.
pub const MEMORY_CHARS: usize = 25;
/// Paso entre filas (24 de botón y 1 de hueco), en `u`.
const ROW_PITCH_U: i64 = 25;
const LIST_Y_U: i64 = 21;

/// Un rectángulo en `u` desde la esquina del panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RectU {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

impl RectU {
    const fn new(x: i64, y: i64, w: i64, h: i64) -> RectU {
        RectU { x, y, w, h }
    }

    /// Si el punto (en `u`) cae dentro.
    pub fn contains(&self, x: i64, y: i64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Los cinco botones de la fila de acciones, de izquierda a derecha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelButton {
    New,
    Duplicate,
    Remove,
    Up,
    Down,
}

impl PanelButton {
    pub const ALL: [PanelButton; 5] = [
        PanelButton::New,
        PanelButton::Duplicate,
        PanelButton::Remove,
        PanelButton::Up,
        PanelButton::Down,
    ];

    fn index(self) -> i64 {
        self as i64
    }
}

/// Qué hay bajo un punto del panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelHit {
    Title,
    Close,
    Eye(usize),
    Lock(usize),
    Name(usize),
    Scrollbar,
    Button(PanelButton),
    Count,
    NameField,
    OpacitySlider,
    OpacityField,
    BlendButton,
    Memory,
    /// Dentro del panel, sin ningún control.
    Body,
}

pub fn title_u() -> RectU {
    RectU::new(1, 1, 170, 16)
}

pub fn close_u() -> RectU {
    RectU::new(171, 1, 16, 16)
}

fn row_y(row: usize) -> i64 {
    LIST_Y_U + ROW_PITCH_U * row as i64
}

/// La fila `row` (0 es la de arriba) entera: ojo, candado y nombre.
pub fn row_u(row: usize) -> RectU {
    RectU::new(5, row_y(row), 167, 24)
}

pub fn eye_u(row: usize) -> RectU {
    RectU::new(5, row_y(row), 24, 24)
}

pub fn lock_u(row: usize) -> RectU {
    RectU::new(30, row_y(row), 24, 24)
}

pub fn name_u(row: usize) -> RectU {
    RectU::new(58, row_y(row), 114, 24)
}

pub fn scrollbar_u() -> RectU {
    RectU::new(175, LIST_Y_U, 8, 149)
}

pub fn button_u(button: PanelButton) -> RectU {
    RectU::new(5 + ROW_PITCH_U * button.index(), 174, 24, 24)
}

/// Donde va `N layers`, a la derecha de los botones.
pub fn count_u() -> RectU {
    RectU::new(135, 174, 48, 24)
}

/// La etiqueta `Nombre` ocupa de `x` = 5 a 47; el campo, el resto.
pub fn name_field_u() -> RectU {
    RectU::new(51, 202, 132, 16)
}

pub fn opacity_slider_u() -> RectU {
    RectU::new(65, 220, 74, 16)
}

pub fn opacity_field_u() -> RectU {
    RectU::new(143, 220, 40, 16)
}

pub fn blend_u() -> RectU {
    RectU::new(51, 238, 132, 16)
}

pub fn memory_u() -> RectU {
    RectU::new(5, 254, 178, 14)
}

/// Dónde se pintan las etiquetas fijas `Nombre`, `Opacidad` y `Mezcla` (esquina superior izquierda).
pub fn label_pos_u(row: LabelRow) -> (i64, i64) {
    match row {
        LabelRow::Name => (5, 202),
        LabelRow::Opacity => (5, 220),
        LabelRow::Blend => (5, 238),
    }
}

/// Las tres filas de opciones con etiqueta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelRow {
    Name,
    Opacity,
    Blend,
}

/// Qué hay bajo el punto `(x_px, y_px)` (píxeles físicos desde la esquina del panel) a escala `n`:
/// `None` fuera del panel y `Body` dentro sin ningún control. Las filas de la lista solo valen
/// para las seis visibles; un control (no fila) que se solapara con otro no existe: la propiedad
/// de recubrimiento lo comprueba.
pub fn hit(x_px: i64, y_px: i64, n: u32) -> Option<PanelHit> {
    let k = i64::from(n.max(1));
    if x_px < 0 || y_px < 0 || x_px >= PANEL_W_U * k || y_px >= PANEL_H_U * k {
        return None;
    }
    let (x, y) = (x_px / k, y_px / k);
    let single = [
        (title_u(), PanelHit::Title),
        (close_u(), PanelHit::Close),
        (scrollbar_u(), PanelHit::Scrollbar),
        (count_u(), PanelHit::Count),
        (name_field_u(), PanelHit::NameField),
        (opacity_slider_u(), PanelHit::OpacitySlider),
        (opacity_field_u(), PanelHit::OpacityField),
        (blend_u(), PanelHit::BlendButton),
        (memory_u(), PanelHit::Memory),
    ];
    if let Some((_, found)) = single.iter().find(|(rect, _)| rect.contains(x, y)) {
        return Some(*found);
    }
    for button in PanelButton::ALL {
        if button_u(button).contains(x, y) {
            return Some(PanelHit::Button(button));
        }
    }
    for row in 0..VISIBLE_ROWS {
        if eye_u(row).contains(x, y) {
            return Some(PanelHit::Eye(row));
        }
        if lock_u(row).contains(x, y) {
            return Some(PanelHit::Lock(row));
        }
        if name_u(row).contains(x, y) {
            return Some(PanelHit::Name(row));
        }
    }
    Some(PanelHit::Body)
}

/// La franja central de la ventana (entre el menú y la barra de estado), en píxeles físicos.
pub fn band(window_px: (i64, i64), n: u32) -> ScreenRect {
    float_window::band(window_px, n)
}

/// Dónde cae el panel si su esquina está en `pos_u` (`u` desde la esquina de la franja central): la
/// posición se recorta para que quepa entero en la ventana y se alinea a la rejilla de `u`. Con una
/// ventana menor que el panel se ancla arriba a la izquierda.
pub fn place(pos_u: (i64, i64), window_px: (i64, i64), n: u32) -> ScreenRect {
    float_window::place(pos_u, (PANEL_W_U, PANEL_H_U), window_px, n)
}

/// La posición inicial: pegado a la derecha del lienzo con 8 u de margen y 8 u bajo el menú.
pub fn default_pos_u(window_px: (i64, i64), n: u32) -> (i64, i64) {
    let k = i64::from(n.max(1));
    let canvas = window_layout(window_px, n).canvas;
    (
        (canvas.width / k - PANEL_W_U - DEFAULT_MARGIN_U).max(0),
        DEFAULT_MARGIN_U,
    )
}

/// Estado del panel: si está abierto, dónde está (en `u` desde la franja central; `None` hasta el
/// primer fotograma: la posición inicial), cuántas filas se desplazó la lista y el arrastre y el
/// campo del nombre en curso. No es del documento ni se guarda (preferencias: hito 8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PanelState {
    pub open: bool,
    pub pos_u: Option<(i64, i64)>,
    pub scroll: usize,
    /// La posición al empezar el arrastre de la barra de título.
    pub drag_from: Option<(i64, i64)>,
    /// El texto del campo del nombre y de qué capa es; vacío si el campo no se está editando.
    pub name: String,
    pub name_for: Option<LayerId>,
    /// Pide el foco para el campo del nombre en el próximo fotograma (`Renombrar`).
    pub focus_name: bool,
    /// El valor del deslizador de opacidad mientras se arrastra.
    pub opacity_drag: Option<u8>,
    /// El texto del campo numérico de la opacidad mientras se edita.
    pub opacity_text: Option<String>,
    /// La última capa activa vista: al cambiar, la lista la lleva a la vista.
    pub seen_active: Option<LayerId>,
}

impl PanelState {
    /// Abre o cierra el panel.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// El rectángulo del panel en esta ventana sin guardar nada (para `F1` y los clics).
    pub fn current_rect(&self, window_px: (i64, i64), n: u32) -> ScreenRect {
        place(
            self.pos_u.unwrap_or_else(|| default_pos_u(window_px, n)),
            window_px,
            n,
        )
    }

    /// El rectángulo del panel en esta ventana; guarda la posición ya recortada, así que cambiar
    /// de escala o de tamaño de ventana la conserva en `u`.
    pub fn rect(&mut self, window_px: (i64, i64), n: u32) -> ScreenRect {
        let pos = self.pos_u.unwrap_or_else(|| default_pos_u(window_px, n));
        let rect = place(pos, window_px, n);
        self.pos_u = Some(float_window::pos_of(rect, window_px, n));
        rect
    }

    /// Mueve el panel `dx_px`, `dy_px` píxeles físicos (a pasos de `u`: se redondea).
    pub fn drag(&mut self, dx_px: i64, dy_px: i64, n: u32) {
        self.pos_u = Some(float_window::dragged(
            self.pos_u.unwrap_or((0, 0)),
            dx_px,
            dy_px,
            n,
        ));
    }

    /// Lleva el panel a `from_2` más el desplazamiento total `dx_px`, `dy_px` (sin acumular
    /// redondeos fotograma a fotograma).
    pub fn drag_total(&mut self, from: (i64, i64), dx_px: i64, dy_px: i64, n: u32) {
        self.pos_u = Some(from);
        self.drag(dx_px, dy_px, n);
    }

    /// Desplaza la lista `notches` muescas de la rueda (positivo: hacia abajo) con `count` capas.
    pub fn scroll_by(&mut self, notches: i64, count: usize) {
        self.scroll_rows(notches * SCROLL_ROWS_PER_NOTCH, count);
    }

    /// Desplaza la lista `rows` filas, sin pasarse de los extremos.
    pub fn scroll_rows(&mut self, rows: i64, count: usize) {
        let max = max_scroll(count) as i64;
        self.scroll = (self.scroll as i64 + rows).clamp(0, max) as usize;
    }
}

/// Mayor desplazamiento de la lista con `count` capas.
pub fn max_scroll(count: usize) -> usize {
    count.saturating_sub(VISIBLE_ROWS)
}

/// El desplazamiento que deja a la vista la capa activa (`active_pos` desde abajo): el actual si ya
/// se ve, y si no el mínimo que la muestra.
pub fn scroll_for(count: usize, active_pos: usize, scroll: usize) -> usize {
    let top = count.saturating_sub(1).saturating_sub(active_pos);
    let scroll = scroll.min(max_scroll(count));
    if top < scroll {
        top
    } else if top >= scroll + VISIBLE_ROWS {
        top + 1 - VISIBLE_ROWS
    } else {
        scroll
    }
}

/// Una fila de la lista.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerRow {
    pub id: LayerId,
    /// El nombre ya recortado a [`NAME_CHARS`].
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub active: bool,
}

/// Las filas que se ven con `scroll` filas desplazadas: la de arriba primero, a lo sumo
/// [`VISIBLE_ROWS`]. Solo se construyen estas: el coste no depende del número de capas.
pub fn visible_rows(doc: &Document, active: LayerId, scroll: usize) -> Vec<LayerRow> {
    let layers = doc.layers();
    let scroll = scroll.min(max_scroll(layers.len()));
    layers
        .iter()
        .rev()
        .skip(scroll)
        .take(VISIBLE_ROWS)
        .map(|layer| LayerRow {
            id: layer.id(),
            name: truncate_name(layer.name()),
            visible: layer.visible(),
            locked: layer.locked(),
            active: layer.id() == active,
        })
        .collect()
}

/// La pila de capas para habilitar las acciones.
pub fn layer_nav(doc: &Document, active: LayerId) -> LayerNav {
    let layers = doc.layers();
    LayerNav {
        count: layers.len(),
        active_pos: layers.iter().position(|l| l.id() == active).unwrap_or(0),
        can_add: layers.len() < Document::MAX_LAYERS,
    }
}

/// El nombre de una fila: hasta [`NAME_CHARS`] caracteres; si tiene más, los 15 primeros y `…`.
pub fn truncate_name(name: &str) -> String {
    if name.chars().count() <= NAME_CHARS {
        return name.to_string();
    }
    let mut out: String = name.chars().take(NAME_CHARS - 1).collect();
    out.push('…');
    out
}

const KIB: usize = 1 << 10;
const MIB: usize = 1 << 20;
const GIB: usize = 1 << 30;

/// Un tamaño en KiB y MiB enteros (hacia abajo) o GiB con un decimal y coma (hacia abajo; sin
/// `,0`): `16 KiB`, `768 MiB`, `1,3 GiB`, `2 GiB`.
pub fn format_bytes(bytes: usize) -> String {
    if bytes < MIB {
        format!("{} KiB", bytes / KIB)
    } else if bytes < GIB {
        format!("{} MiB", bytes / MIB)
    } else {
        let tenths = (bytes / (GIB / 10)) as u128;
        let (whole, tenth) = (tenths / 10, tenths % 10);
        if tenth == 0 {
            format!("{whole} GiB")
        } else {
            format!("{whole},{tenth} GiB")
        }
    }
}

/// El pie del panel y si va en aviso: `Memoria: {cels with píxeles} / {tope}`, recortado a
/// [`MEMORY_CHARS`]; aviso desde 1 GiB (`Document::MEMORY_WARN_BYTES`).
pub fn memory_line(materialized: usize, limit: usize) -> (String, bool) {
    let text = format!(
        "Memoria: {} / {}",
        format_bytes(materialized),
        format_bytes(limit)
    );
    (
        text.chars().take(MEMORY_CHARS).collect(),
        materialized >= Document::MEMORY_WARN_BYTES,
    )
}

/// Lo que hace confirmar el campo del nombre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameCommit {
    /// El mismo nombre: nada que hacer.
    Unchanged,
    /// Vacío: se rechaza y el campo vuelve al nombre de la capa.
    Empty,
    /// Un nombre nuevo (recortado a [`NAME_EDIT_CHARS`] caracteres, que caben en el límite del
    /// núcleo).
    Rename(String),
}

/// Qué hacer al confirmar `text` con `current` como nombre de la capa.
pub fn commit_name(text: &str, current: &str) -> NameCommit {
    let limited: String = text.chars().take(NAME_EDIT_CHARS).collect();
    if limited.trim().is_empty() {
        NameCommit::Empty
    } else if limited == current {
        NameCommit::Unchanged
    } else {
        NameCommit::Rename(limited)
    }
}

/// Los ocho modos de mezcla, con su rótulo, en el orden del núcleo.
pub const BLEND_LABELS: [(px_core::BlendMode, &str); 8] = [
    (px_core::BlendMode::Normal, "Normal"),
    (px_core::BlendMode::Multiply, "Multiplicar"),
    (px_core::BlendMode::Screen, "Trama"),
    (px_core::BlendMode::Overlay, "Superponer"),
    (px_core::BlendMode::Darken, "Oscurecer"),
    (px_core::BlendMode::Lighten, "Aclarar"),
    (px_core::BlendMode::Add, "Sumar"),
    (px_core::BlendMode::Difference, "Diferencia"),
];

/// El rótulo de un modo de mezcla.
pub fn blend_label(blend: px_core::BlendMode) -> &'static str {
    BLEND_LABELS
        .iter()
        .find(|(mode, _)| *mode == blend)
        .map_or("Normal", |(_, label)| label)
}
