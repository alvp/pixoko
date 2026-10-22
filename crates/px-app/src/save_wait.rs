//! Guardar un documento grande con capas (specs/hito-3-capas-vista.md, unidad 3.3, condición 4):
//! componer más de 2²⁴ píxeles de capa visible congela la ventana unos instantes, así que la
//! barra de estado dice «Guardando…» y el guardado espera `WAIT_FRAMES` fotogramas (el mismo
//! mecanismo que `Convirtiendo…`). La barra de progreso es un pulido del hito 8.

use px_core::Document;
use px_ui::source::identity_cell;

use crate::convert_mode::WAIT_FRAMES;

/// El aviso mientras se espera para guardar un documento grande.
pub const SAVING_NOTICE: &str = "Guardando…";

/// El aviso mientras se espera para exportar el PNG aplanado de un documento grande.
pub const EXPORTING_NOTICE: &str = "Exportando…";

/// Por encima de estos píxeles de capa visible a componer, el guardado va en dos tiempos: 2²⁴.
pub const SAVE_SYNC_PIXELS: u64 = 1 << 24;

/// Píxeles de capa visible que hay que componer para guardar: ancho × alto × capas visibles con
/// imagen, o 0 si no se compone nada (una sola capa visible y opaca: se guarda la celda tal cual).
pub fn compose_pixels(doc: &Document) -> u64 {
    if identity_cell(doc).is_some() {
        return 0;
    }
    let visible = doc
        .layers()
        .iter()
        .filter(|layer| layer.visible() && doc.image(layer.cel()).is_some())
        .count() as u64;
    u64::from(doc.width()) * u64::from(doc.height()) * visible
}

/// Guardar ya, o esperar `WAIT_FRAMES` fotogramas para que se vea el aviso.
#[derive(Debug, PartialEq, Eq)]
pub enum StartSave {
    Now,
    Later(PendingSave),
}

/// Un guardado que espera su fotograma.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingSave {
    left: u8,
}

impl PendingSave {
    /// Un fotograma: `true` exactamente en el `WAIT_FRAMES`-ésimo, y nunca después.
    pub fn tick(&mut self) -> bool {
        if self.left == 0 {
            return false;
        }
        self.left -= 1;
        self.left == 0
    }
}

/// Con `pixels` por componer (véase [`compose_pixels`]): ya, o tras la espera.
pub fn start_save(pixels: u64) -> StartSave {
    if pixels <= SAVE_SYNC_PIXELS {
        StartSave::Now
    } else {
        StartSave::Later(PendingSave { left: WAIT_FRAMES })
    }
}
