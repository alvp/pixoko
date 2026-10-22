//! Combinar y aplanar sin ventana (specs/hito-3-combinar.md, unidad 3.5): qué se hace, cuántos
//! píxeles se componen, la espera de «Combinando…» / «Aplanando…» antes de empezar (el mismo
//! mecanismo de dos fotogramas que «Guardando…» y «Convirtiendo…») y la confirmación por
//! repetición de Aplanar cuando descarta capas ocultas con píxeles.

use px_core::{Document, LayerId};

use crate::confirm::{Confirm, Confirmation, Request};
use crate::convert_mode::WAIT_FRAMES;

/// Qué se une.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeKind {
    /// La capa activa con la de debajo.
    Down,
    /// Todas las visibles en la visible más baja; las ocultas se quedan.
    Visible,
    /// Una sola capa con lo que se ve; las ocultas se quitan.
    Flatten,
}

/// Por encima de estos píxeles de capa visible a componer, la orden va en dos tiempos: 2²⁴.
pub const MERGE_SYNC_PIXELS: u64 = 1 << 24;

/// El aviso mientras se espera para combinar o aplanar un documento grande.
pub fn wait_notice(kind: MergeKind) -> &'static str {
    match kind {
        MergeKind::Down | MergeKind::Visible => "Combinando…",
        MergeKind::Flatten => "Aplanando…",
    }
}

/// Píxeles de capa visible con imagen que hay que componer: ancho × alto × esas capas. `active`
/// es la capa activa (la de arriba de Combinar abajo, que se combina con la de justo debajo).
pub fn merge_pixels(doc: &Document, kind: MergeKind, active: LayerId) -> u64 {
    let layers = doc.layers();
    let counted = |layer: &px_core::Layer| layer.visible() && doc.image(layer.cel()).is_some();
    let count = match kind {
        MergeKind::Down => layers
            .iter()
            .position(|layer| layer.id() == active)
            .map_or(0, |at| {
                let below = at.checked_sub(1);
                [Some(at), below]
                    .into_iter()
                    .flatten()
                    .filter_map(|i| layers.get(i))
                    .filter(|layer| counted(layer))
                    .count()
            }),
        MergeKind::Visible | MergeKind::Flatten => {
            layers.iter().filter(|layer| counted(layer)).count()
        }
    };
    u64::from(doc.width()) * u64::from(doc.height()) * count as u64
}

/// Hacerlo ya, o esperar `WAIT_FRAMES` fotogramas para que se vea el aviso.
#[derive(Debug, PartialEq, Eq)]
pub enum StartMerge {
    Now,
    Later(PendingMerge),
}

/// Una combinación que espera su fotograma.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingMerge {
    kind: MergeKind,
    left: u8,
}

impl PendingMerge {
    pub fn kind(&self) -> MergeKind {
        self.kind
    }

    /// Un fotograma: `true` exactamente en el `WAIT_FRAMES`-ésimo, y nunca después.
    pub fn tick(&mut self) -> bool {
        if self.left == 0 {
            return false;
        }
        self.left -= 1;
        self.left == 0
    }
}

/// Con `pixels` por componer (véase [`merge_pixels`]): ya, o tras la espera.
pub fn start_merge(kind: MergeKind, pixels: u64) -> StartMerge {
    if pixels <= MERGE_SYNC_PIXELS {
        StartMerge::Now
    } else {
        StartMerge::Later(PendingMerge {
            kind,
            left: WAIT_FRAMES,
        })
    }
}

/// La regla de Aplanar con `hidden` capas ocultas con píxeles: sin ellas se hace a la primera; con
/// ellas, la primera petición queda armada (y avisa) y la repetida en 5 s la ejecuta.
pub fn flatten_gate(hidden: usize, confirm: &mut Confirm, now: f64) -> bool {
    hidden == 0 || confirm.request(Request::FlattenHidden, now) == Confirmation::Confirmed
}

/// El aviso al armar Aplanar con ocultas con píxeles.
pub fn flatten_warning(hidden: usize) -> String {
    let capas = if hidden == 1 {
        "1 capa oculta con píxeles".to_string()
    } else {
        format!("{hidden} capas ocultas con píxeles")
    };
    format!(
        "Aplanar descarta {capas}: repite la orden en {:.0} s",
        crate::confirm::CONFIRM_SECS
    )
}
