//! Las operaciones de capa de la sesión (specs/hito-3-capas-interfaz.md, decisiones 9 a 13): crear,
//! duplicar, quitar, mover y cambiar propiedades con la regla de la capa activa, los avisos de
//! presupuesto y los de los modos sin transparencia. Cada una es un `Command` del núcleo (un paso
//! de historia) y devuelve un [`LayerReport`]; elegir la activa sigue sin ser un paso.

use px_core::{BlendMode, Command, Document, DocumentError, LayerId, Mode};
use px_ui::actions::LayerNav;
use px_ui::layers_panel::{format_bytes, layer_nav};
use px_ui::tool::DropperSource;

use super::Session;

/// Por qué no se crea una capa en una imagen en gris, con la oferta de convertir.
pub const GRAY_LAYER_NOTICE: &str = "una imagen en gris no admite capas nuevas: convierte a gris con alfa (repite la orden en 5 s para abrir Convertir modo…)";
/// Por qué no se crea una capa en un indexado sin color transparente.
pub const INDEXED_LAYER_NOTICE: &str =
    "esta paleta no tiene color transparente: fíjalo en el editor de la paleta o convierte a RGBA";
/// Por qué no se quita la única capa.
pub const LAST_LAYER_NOTICE: &str = "la última capa no se puede quitar";

/// Lo que pasó con una operación de capa.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LayerReport {
    /// El documento cambió (un paso de historia nuevo).
    pub changed: bool,
    /// El aviso de 5 s, si lo hay.
    pub notice: Option<String>,
    /// El aviso ofrece abrir `Convertir modo…` con «Gris con alfa» (repetir la orden en 5 s).
    pub offer_convert: bool,
}

impl LayerReport {
    fn from_result(result: Result<bool, DocumentError>) -> LayerReport {
        match result {
            Ok(changed) => LayerReport {
                changed,
                ..LayerReport::default()
            },
            Err(error) => LayerReport {
                notice: Some(error.to_string()),
                ..LayerReport::default()
            },
        }
    }

    fn refused(notice: &str, offer_convert: bool) -> LayerReport {
        LayerReport {
            changed: false,
            notice: Some(notice.to_string()),
            offer_convert,
        }
    }
}

/// El aviso al crear una capa si, con todas con píxeles, las celdas pasarían de `warn` (1 GiB):
/// `worst` es lo que ocuparía el documento con todas las celdas materializadas y `limit` el tope.
pub fn create_budget_notice(worst: usize, limit: usize, warn: usize) -> Option<String> {
    (worst > warn).then(|| {
        format!(
            "con todas las capas con píxeles ocuparían {} de {}",
            format_bytes(worst),
            format_bytes(limit)
        )
    })
}

/// La fuente del cuentagotas: con `Mayús` al pulsar, la capa activa; sin ella, lo que se ve.
pub fn dropper_source_for(shift: bool) -> DropperSource {
    if shift {
        DropperSource::Layer
    } else {
        DropperSource::Visible
    }
}

/// `name` recortado a `max` bytes en una frontera de carácter.
fn limit_bytes(name: String, max: usize) -> String {
    if name.len() <= max {
        return name;
    }
    let mut end = max;
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_string()
}

impl Session {
    pub(super) fn layer_ids(&self) -> Vec<LayerId> {
        self.document
            .layers()
            .iter()
            .map(|layer| layer.id())
            .collect()
    }

    /// Si tras deshacer o rehacer entra exactamente una capa, pasa a ser la activa.
    pub(super) fn follow_entering_layer(&mut self, before: &[LayerId]) {
        let after = self.layer_ids();
        let entering: Vec<LayerId> = after
            .iter()
            .copied()
            .filter(|id| !before.contains(id))
            .collect();
        if let [id] = entering.as_slice() {
            let _ = self.set_active_layer(*id);
        }
    }

    fn active_position(&self) -> usize {
        self.document
            .layers()
            .iter()
            .position(|layer| layer.id() == self.active_layer())
            .unwrap_or(0)
    }

    /// La pila de capas para habilitar el menú y el panel.
    pub fn layer_nav(&self) -> LayerNav {
        layer_nav(&self.document, self.active_layer())
    }

    /// Por qué no se puede crear una capa en este modo, si no se puede.
    fn creation_refusal(&self) -> Option<LayerReport> {
        match self.document.mode() {
            Mode::Gray => Some(LayerReport::refused(GRAY_LAYER_NOTICE, true)),
            Mode::Indexed
                if self
                    .document
                    .palette()
                    .and_then(|palette| palette.transparent_index())
                    .is_none() =>
            {
                Some(LayerReport::refused(INDEXED_LAYER_NOTICE, false))
            }
            _ => None,
        }
    }

    /// Tras crear una capa: la nueva pasa a ser la activa y, si el peor caso pasa de 1 GiB, el aviso.
    fn finish_creation(&mut self, at: usize) -> LayerReport {
        if let Some(id) = self.document.layers().get(at).map(|layer| layer.id()) {
            let _ = self.set_active_layer(id);
        }
        LayerReport {
            changed: true,
            notice: create_budget_notice(
                self.document.worst_case_bytes(),
                self.document.cells_limit(),
                Document::MEMORY_WARN_BYTES,
            ),
            offer_convert: false,
        }
    }

    /// `Nueva capa`: vacía, encima de la activa, que pasa a ser la activa.
    pub fn new_layer(&mut self) -> LayerReport {
        if let Some(refusal) = self.creation_refusal() {
            return refusal;
        }
        let at = self.active_position() + 1;
        let name = format!("Capa {}", self.document.layers().len() + 1);
        match self.apply_layer(Command::AddLayer { name, at }) {
            Ok(true) => self.finish_creation(at),
            other => LayerReport::from_result(other),
        }
    }

    /// `Duplicar`: la copia va encima de la original y pasa a ser la activa.
    pub fn duplicate_layer(&mut self) -> LayerReport {
        if let Some(refusal) = self.creation_refusal() {
            return refusal;
        }
        let at = self.active_position();
        let Some(layer) = self.document.layers().get(at) else {
            return LayerReport::default();
        };
        let name = limit_bytes(
            format!("{} copia", layer.name()),
            Document::MAX_LAYER_NAME_BYTES,
        );
        let id = layer.id();
        match self.apply_layer(Command::DuplicateLayer { layer: id, name }) {
            Ok(true) => self.finish_creation(at + 1),
            other => LayerReport::from_result(other),
        }
    }

    /// `Quitar`: la activa pasa a la vecina; la última capa no se quita.
    pub fn remove_layer(&mut self) -> LayerReport {
        if self.document.layers().len() < 2 {
            return LayerReport::refused(LAST_LAYER_NOTICE, false);
        }
        let layer = self.active_layer();
        LayerReport::from_result(self.apply_layer(Command::RemoveLayer { layer }))
    }

    /// `Subir` (`delta` > 0) o `Bajar` (`delta` < 0) la activa; en un extremo no hace nada.
    pub fn move_layer(&mut self, delta: i32) -> LayerReport {
        let to = self.active_position() as i64 + i64::from(delta);
        if to < 0 || to >= self.document.layers().len() as i64 {
            return LayerReport::default();
        }
        let layer = self.active_layer();
        LayerReport::from_result(self.apply_layer(Command::MoveLayer {
            layer,
            to: to as usize,
        }))
    }

    pub fn set_layer_visible(&mut self, layer: LayerId, visible: bool) -> LayerReport {
        LayerReport::from_result(self.apply_layer(Command::SetLayerVisible { layer, visible }))
    }

    pub fn set_layer_locked(&mut self, layer: LayerId, locked: bool) -> LayerReport {
        LayerReport::from_result(self.apply_layer(Command::SetLayerLocked { layer, locked }))
    }

    pub fn rename_layer(&mut self, layer: LayerId, name: String) -> LayerReport {
        LayerReport::from_result(self.apply_layer(Command::RenameLayer { layer, name }))
    }

    pub fn set_layer_opacity(&mut self, layer: LayerId, opacity: u8) -> LayerReport {
        LayerReport::from_result(self.apply_layer(Command::SetLayerOpacity { layer, opacity }))
    }

    pub fn set_layer_blend(&mut self, layer: LayerId, blend: BlendMode) -> LayerReport {
        LayerReport::from_result(self.apply_layer(Command::SetLayerBlend { layer, blend }))
    }

    /// El aviso de presupuesto al pintar: si las celdas con píxeles pasan de menos de `warn` a
    /// `warn` o más desde la última llamada. Una vez por cruce.
    pub fn memory_crossing_notice_with(&mut self, warn: usize) -> Option<String> {
        let now = self.document.materialized_bytes();
        let before = std::mem::replace(&mut self.mem_seen, now);
        (before < warn && now >= warn).then(|| {
            format!(
                "las capas con píxeles ya ocupan {} de {}",
                format_bytes(now),
                format_bytes(self.document.cells_limit())
            )
        })
    }

    /// Como [`Session::memory_crossing_notice_with`] con el umbral de 1 GiB de D04.
    pub fn memory_crossing_notice(&mut self) -> Option<String> {
        self.memory_crossing_notice_with(Document::MEMORY_WARN_BYTES)
    }
}
