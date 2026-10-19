//! Operaciones sobre las entradas de la paleta: añadir, quitar, mover y rotar, sin tipos de egui
//! (specs/hito-2-paleta-operaciones.md). Aquí vive lo que se puede probar sin ventana: qué le
//! pasa a una entrada con cada operación, la selección resultante, el destino por defecto de una
//! quita, el estado del arrastre y de la quita pendiente y los textos.

use px_core::{Image, Palette};

use crate::edit_layout::PaletteButton;
use crate::palette_grid::DRAG_START_U;
use crate::theme::NOTICE_PREFIX;

/// Aviso de 5 s al cancelarse una quita pendiente porque el documento cambió.
pub const REMOVAL_CANCELLED_NOTICE: &str = "quitar cancelado: el documento cambió";
/// Aviso de 5 s al rotar una paleta o un rango de una sola entrada.
pub const NOTHING_TO_ROTATE_NOTICE: &str = "nada que rotar: una sola entrada";

/// Una edición de la paleta pedida desde el panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteOp {
    Add,
    Remove { index: u8, into: u8 },
    Move { from: u8, to: u8 },
    Rotate { start: u8, len: u16, steps: i32 },
}

/// Qué le pasa a una entrada (por su índice) con una operación.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fate {
    /// Sigue en su índice con su color.
    Same,
    /// Cambia de índice.
    Moved,
    /// Sigue en su índice con otro color.
    Changed,
    /// Ya no existe.
    Gone,
}

/// Destino de la entrada `entry` tras `op` (para el texto pendiente del campo hexadecimal).
pub fn entry_fate(op: &PaletteOp, entry: u8) -> Fate {
    match *op {
        PaletteOp::Add => Fate::Same,
        PaletteOp::Move { from, to } => {
            if from != to && (from.min(to)..=from.max(to)).contains(&entry) {
                Fate::Moved
            } else {
                Fate::Same
            }
        }
        PaletteOp::Remove { index, .. } => match entry.cmp(&index) {
            std::cmp::Ordering::Less => Fate::Same,
            std::cmp::Ordering::Equal => Fate::Gone,
            std::cmp::Ordering::Greater => Fate::Moved,
        },
        PaletteOp::Rotate { start, len, steps } => {
            let rotates = len >= 2 && i64::from(steps).rem_euclid(i64::from(len)) != 0;
            let end = usize::from(start) + usize::from(len);
            if rotates && (usize::from(start)..end).contains(&usize::from(entry)) {
                Fate::Changed
            } else {
                Fate::Same
            }
        }
    }
}

/// Entrada activa tras operar (índice de la selección): `Add` la nueva, `Remove` el mismo
/// índice recortado a la última, `Move` el destino y `Rotate` la misma.
pub fn choice_after(op: &PaletteOp, active: u8, old_len: usize) -> u8 {
    match *op {
        PaletteOp::Add => u8::try_from(old_len).unwrap_or(u8::MAX),
        PaletteOp::Remove { index, .. } => {
            let last = old_len.saturating_sub(2);
            u8::try_from(usize::from(index).min(last)).unwrap_or(u8::MAX)
        }
        PaletteOp::Move { to, .. } => to,
        PaletteOp::Rotate { .. } => active,
    }
}

/// Rango a rotar `(start, len)`: sin rango, toda la paleta; con rango, de la activa al otro
/// extremo (en cualquier orden).
pub fn rotation_range(len: usize, active: u8, range_end: Option<u8>) -> (u8, u16) {
    match range_end {
        None => (0, u16::try_from(len).unwrap_or(u16::MAX)),
        Some(end) => {
            let (low, high) = (active.min(end), active.max(end));
            (low, u16::from(high - low) + 1)
        }
    }
}

/// Destino por defecto al quitar `index`: `Palette::nearest` sobre el resto (con el transparente
/// reajustado) del color resuelto de la quitada, en la numeración de antes de quitar. Sin
/// destino para una paleta de menos de dos entradas ni para la entrada transparente.
pub fn default_destination(palette: &Palette, index: u8) -> Option<u8> {
    if palette.len() < 2 || palette.transparent_index() == Some(index) {
        return None;
    }
    let skipped = usize::from(index);
    let rest = palette
        .colors()
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != skipped)
        .map(|(_, color)| *color);
    let mut rest = Palette::from_colors(rest).ok()?;
    if let Some(transparent) = palette.transparent_index() {
        let moved = if transparent > index {
            transparent - 1
        } else {
            transparent
        };
        rest.set_transparent_index(Some(moved)).ok()?;
    }
    let query = palette.resolved(index).ok()?;
    let nearest = usize::from(rest.nearest(query)?);
    let old = if nearest >= skipped {
        nearest + 1
    } else {
        nearest
    };
    u8::try_from(old).ok()
}

/// Algún píxel usa la entrada `index` (`Image::uses_index`: una búsqueda por bytes, sin
/// construir un `Pixel` por píxel).
pub fn uses_entry(image: &Image, index: u8) -> bool {
    image.uses_index(index)
}

/// Aviso de una quita pendiente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovalWarning {
    /// Se quita la transparente: sus píxeles pasan a ser opacos.
    PixelsOpaque,
    /// El destino es la transparente: los píxeles quedarán transparentes.
    DestinationTransparent,
}

pub fn removal_warning(palette: &Palette, index: u8, into: Option<u8>) -> Option<RemovalWarning> {
    let transparent = palette.transparent_index()?;
    if transparent == index {
        Some(RemovalWarning::PixelsOpaque)
    } else if into == Some(transparent) {
        Some(RemovalWarning::DestinationTransparent)
    } else {
        None
    }
}

/// Las dos líneas del editor mientras hay una quita pendiente.
pub fn removal_prompt(index: u8, into: Option<u8>, warn: Option<RemovalWarning>) -> [String; 2] {
    let first = match into {
        Some(into) => format!("quitar {index}; destino {into}"),
        None => format!("quitar {index}: pulsa un destino"),
    };
    let second = match warn {
        Some(RemovalWarning::PixelsOpaque) => format!("{NOTICE_PREFIX}sus píxeles quedan opacos"),
        Some(RemovalWarning::DestinationTransparent) => {
            format!("{NOTICE_PREFIX}destino transparente")
        }
        None => "elige otra muestra o confirma".to_string(),
    };
    [first, second]
}

/// Resumen de 5 s de lo ocurrido. `active` es la entrada activa antes de operar; `old_len`, el
/// tamaño de la paleta antes; `unused`: ningún píxel usaba la quitada.
pub fn summary(
    op: &PaletteOp,
    active: u8,
    old_len: usize,
    was_transparent: bool,
    unused: bool,
) -> String {
    match *op {
        PaletteOp::Add => format!("entrada {old_len} añadida (copia de la {active})"),
        PaletteOp::Remove { index, into } => {
            if unused && was_transparent {
                format!("entrada {index} quitada; era la transparente")
            } else if unused {
                format!("entrada {index} quitada (sin píxeles)")
            } else {
                let after = if into < index {
                    into
                } else {
                    into.saturating_sub(1)
                };
                format!("entrada {index} quitada; píxeles pasan a la {after}")
            }
        }
        PaletteOp::Move { from, to } => format!("entrada {from} movida a la posición {to}"),
        PaletteOp::Rotate { start, len, steps } => {
            let way = if steps > 0 { "derecha" } else { "izquierda" };
            if start == 0 && usize::from(len) == old_len {
                format!("paleta rotada a la {way}")
            } else {
                let last = usize::from(start) + usize::from(len).saturating_sub(1);
                format!("rotadas entradas {start} a {last} a la {way}")
            }
        }
    }
}

/// Por qué no se puede pulsar un botón (aviso de 5 s al pulsarlo). Rotar nunca se desactiva.
pub fn blocked_reason(
    button: PaletteButton,
    len: usize,
    has_destination: bool,
) -> Option<&'static str> {
    match button {
        PaletteButton::Add if len >= 256 => Some("paleta llena: 256 entradas"),
        PaletteButton::Remove if len <= 1 => Some("no se puede quitar la única entrada"),
        PaletteButton::Confirm if !has_destination => Some("elige un destino pulsando una muestra"),
        _ => None,
    }
}

/// Las dos líneas del panel mientras se arrastra una muestra.
pub fn drag_labels(from: u8, to: Option<u8>) -> [String; 2] {
    let first = match to {
        Some(to) => format!("mover {from} a {to}"),
        None => format!("mover {from}: suelta en la rejilla"),
    };
    [first, "Escape cancela".to_string()]
}

/// Cómo termina pulsar una muestra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragEnd {
    /// No se pasó el umbral: es un clic.
    Click,
    /// Soltar en otra posición.
    Move { from: u8, to: u8 },
    /// Se arrastró, pero no hay nada que mover (misma posición o sin destino).
    Nothing,
}

/// Pulsación de una muestra que puede acabar en arrastre. Posiciones en píxeles físicos.
#[derive(Clone, Copy, Debug)]
pub struct MoveDrag {
    from: u8,
    start: (i64, i64),
    dragging: bool,
    target: Option<u8>,
}

impl MoveDrag {
    pub fn press(from: u8, at: (i64, i64)) -> MoveDrag {
        MoveDrag {
            from,
            start: at,
            dragging: false,
            target: None,
        }
    }

    /// Posición actual y destino bajo el puntero. Empieza a arrastrar al recorrer `4n`
    /// píxeles (distancia euclídea) y ya no vuelve a ser un clic.
    pub fn update(&mut self, at: (i64, i64), n: u32, target: Option<u8>) {
        if !self.dragging {
            let dx = i128::from(at.0) - i128::from(self.start.0);
            let dy = i128::from(at.1) - i128::from(self.start.1);
            let limit = i128::from(DRAG_START_U) * i128::from(n.max(1));
            self.dragging = dx * dx + dy * dy >= limit * limit;
        }
        if self.dragging {
            self.target = target;
        }
    }

    pub fn dragging(&self) -> bool {
        self.dragging
    }

    /// Entrada que se arrastra.
    pub fn from(&self) -> u8 {
        self.from
    }

    /// Último destino bajo el puntero (solo con arrastre vivo).
    pub fn target(&self) -> Option<u8> {
        self.target
    }

    /// `(from, to)` si se arrastra, hay destino y es otra posición.
    pub fn preview(&self) -> Option<(u8, u8)> {
        self.target
            .filter(|to| self.dragging && *to != self.from)
            .map(|to| (self.from, to))
    }

    pub fn release(self) -> DragEnd {
        if !self.dragging {
            return DragEnd::Click;
        }
        match self.target {
            Some(to) if to != self.from => DragEnd::Move {
                from: self.from,
                to,
            },
            _ => DragEnd::Nothing,
        }
    }
}

/// Quita pendiente de destino: la entrada y el destino elegido (ninguno para la transparente
/// hasta que se pulse una muestra).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Removal {
    pub index: u8,
    pub into: Option<u8>,
}

/// Estado del panel entre fotogramas para estas operaciones: arrastre, quita pendiente y rango.
#[derive(Debug, Default)]
pub struct PaletteOps {
    drag: Option<MoveDrag>,
    removal: Option<(Removal, u64)>,
    range_end: Option<u8>,
    doc_state: u64,
    text_focus: bool,
}

impl PaletteOps {
    /// Cada fotograma, antes del panel: recorta el rango a la paleta y cancela la quita si el
    /// documento cambió o sus índices ya no valen. `true`: se canceló (hay que avisar con
    /// [`REMOVAL_CANCELLED_NOTICE`]).
    pub fn sync(&mut self, doc_state: u64, len: usize) -> bool {
        self.doc_state = doc_state;
        // El otro extremo del rango se recorta a la última entrada.
        self.range_end = match (self.range_end, len.checked_sub(1)) {
            (Some(end), Some(last)) => Some(u8::try_from(last).map_or(end, |last| end.min(last))),
            _ => None,
        };
        let stale = self.removal.is_some_and(|(removal, state)| {
            state != doc_state
                || usize::from(removal.index) >= len
                || removal.into.is_some_and(|into| usize::from(into) >= len)
        });
        if stale {
            self.removal = None;
        }
        stale
    }

    pub fn begin_drag(&mut self, from: u8, at: (i64, i64)) {
        self.drag = Some(MoveDrag::press(from, at));
    }

    pub fn drag(&mut self) -> Option<&mut MoveDrag> {
        self.drag.as_mut()
    }

    /// El arrastre en curso, sin poder modificarlo (para pintar).
    pub fn drag_state(&self) -> Option<&MoveDrag> {
        self.drag.as_ref()
    }

    /// Termina el arrastre (soltar) y dice cómo acabó.
    pub fn end_drag(&mut self) -> Option<DragEnd> {
        self.drag.take().map(MoveDrag::release)
    }

    pub fn cancel_drag(&mut self) {
        self.drag = None;
    }

    pub fn range_end(&self) -> Option<u8> {
        self.range_end
    }

    pub fn set_range_end(&mut self, end: u8) {
        self.range_end = Some(end);
    }

    pub fn clear_range(&mut self) {
        self.range_end = None;
    }

    pub fn removal(&self) -> Option<Removal> {
        self.removal.map(|(removal, _)| removal)
    }

    /// Empieza una quita pendiente, atada al estado del documento de la última `sync`.
    pub fn begin_removal(&mut self, index: u8, into: Option<u8>) {
        self.removal = Some((Removal { index, into }, self.doc_state));
    }

    /// Cambia el destino de la quita pendiente. `false` sin quita, con la propia entrada o con
    /// una muestra que no existe.
    pub fn set_destination(&mut self, cell: u8, len: usize) -> bool {
        match &mut self.removal {
            Some((removal, _)) if cell != removal.index && usize::from(cell) < len => {
                removal.into = Some(cell);
                true
            }
            _ => false,
        }
    }

    pub fn cancel_removal(&mut self) {
        self.removal = None;
    }

    /// Hay un arrastre de verdad (pasó el umbral); el botón solo armado no cuenta.
    pub fn busy(&self) -> bool {
        self.drag.as_ref().is_some_and(MoveDrag::dragging)
    }

    /// Había un campo de texto enfocado al terminar el fotograma anterior. egui le quita el foco
    /// al principio del fotograma de un `Escape`, así que el panel recuerda el del anterior.
    pub fn text_focus(&self) -> bool {
        self.text_focus
    }

    pub fn set_text_focus(&mut self, focused: bool) {
        self.text_focus = focused;
    }

    pub fn reset(&mut self) {
        *self = PaletteOps::default();
    }
}
