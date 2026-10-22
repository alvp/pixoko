//! Combinar, aplanar, exportar y convertir todas las capas en la sesión
//! (specs/hito-3-combinar.md, unidad 3.5): las reglas de ocultas y bloqueadas (D4), la capa activa
//! que vuelve al deshacer y rehacer (D2), el aviso de historia (D3) y la exportación del PNG
//! aplanado, que no toca el estado del documento (D9).

use std::path::Path;

use px_core::{Allowed, Command, Document, DocumentError, LayerId, Mode};
use px_raster::{MergeError, flatten_command, merge_down_command, merge_visible_command};
use px_ui::layers_panel::format_bytes;

use crate::dialogs::with_png_extension;
use crate::files::{FileError, copy_path, file_label, write_atomic};
use crate::merge_flow::MergeKind;

use super::{LayerReport, Saved, Session, default_background, flatten_to_save, same_file};

/// Por qué no se combina.
pub const NO_LAYER_BELOW_NOTICE: &str = "no hay capa debajo con la que combinar";
pub const ACTIVE_HIDDEN_MERGE_NOTICE: &str = "la capa activa está oculta: no se combina";
pub const BELOW_HIDDEN_NOTICE: &str = "la capa de abajo está oculta: no se combina";
pub const BELOW_LOCKED_NOTICE: &str = "la capa de abajo está bloqueada: no se combina";
pub const ONE_VISIBLE_NOTICE: &str = "hace falta más de una capa visible para combinar";
pub const NO_VISIBLE_NOTICE: &str = "no hay capas visibles que aplanar";
pub const ONE_LAYER_NOTICE: &str = "hace falta más de una capa para aplanar";

/// Cuántas combinaciones recuerda la sesión para devolver la capa activa.
const MAX_NOTES: usize = 32;

/// La capa activa antes y después de una combinación, con el `state_id` de cada lado.
#[derive(Clone, Copy, Debug)]
pub(super) struct MergeNote {
    before: u64,
    active_before: LayerId,
    after: u64,
    active_after: LayerId,
}

impl Session {
    /// Por qué no se puede hacer `kind` ahora (D4), si no se puede.
    pub fn merge_refusal(&self, kind: MergeKind) -> Option<&'static str> {
        let layers = self.document.layers();
        let visible = layers.iter().filter(|layer| layer.visible()).count();
        match kind {
            MergeKind::Down => {
                let at = layers.iter().position(|layer| layer.id() == self.active)?;
                let (Some(below), Some(top)) = (
                    at.checked_sub(1).and_then(|i| layers.get(i)),
                    layers.get(at),
                ) else {
                    return Some(NO_LAYER_BELOW_NOTICE);
                };
                if !top.visible() {
                    Some(ACTIVE_HIDDEN_MERGE_NOTICE)
                } else if !below.visible() {
                    Some(BELOW_HIDDEN_NOTICE)
                } else if below.locked() {
                    Some(BELOW_LOCKED_NOTICE)
                } else {
                    None
                }
            }
            MergeKind::Visible => (visible < 2).then_some(ONE_VISIBLE_NOTICE),
            MergeKind::Flatten => {
                if layers.len() < 2 {
                    Some(ONE_LAYER_NOTICE)
                } else if visible == 0 {
                    Some(NO_VISIBLE_NOTICE)
                } else {
                    None
                }
            }
        }
    }

    /// Capas ocultas con píxeles: lo que Aplanar descartaría y pide confirmar.
    pub fn flatten_hidden_with_pixels(&self) -> usize {
        self.document
            .layers()
            .iter()
            .filter(|layer| !layer.visible() && self.document.image(layer.cel()).is_some())
            .count()
    }

    /// Combina o aplana (un paso de historia). Con un rechazo (D4) o un error del núcleo, aviso y
    /// el documento queda intacto.
    pub fn merge(&mut self, kind: MergeKind) -> LayerReport {
        if let Some(notice) = self.merge_refusal(kind) {
            return LayerReport::refused(notice, false);
        }
        let built = match kind {
            MergeKind::Down => merge_down_command(&self.document, self.active),
            MergeKind::Visible => merge_visible_command(&self.document),
            MergeKind::Flatten => flatten_command(&self.document),
        };
        let command = match built {
            Ok(command) => command,
            Err(error) => return LayerReport::refused(&merge_error_text(&error), false),
        };
        let Command::MergeLayers { target, remove, .. } = &command else {
            return LayerReport::default();
        };
        let (target, remove) = (*target, remove.clone());
        let stored = self
            .document
            .plan_merge(target, &remove)
            .map_or(0, |plan| plan.stored_bytes);
        let hidden = remove
            .iter()
            .filter(|id| self.document.layer(**id).is_some_and(|l| !l.visible()))
            .count();
        let (before, active_before) = (self.document.state_id(), self.active);
        match self.apply_layer(command) {
            Ok(true) => {
                let _ = self.set_active_layer(target);
                self.remember_merge(MergeNote {
                    before,
                    active_before,
                    after: self.document.state_id(),
                    active_after: target,
                });
                LayerReport {
                    changed: true,
                    notice: Some(self.merged_text(kind, remove.len() + 1, hidden, stored)),
                    offer_convert: false,
                }
            }
            other => LayerReport::from_result(other),
        }
    }

    fn merged_text(&self, kind: MergeKind, layers: usize, hidden: usize, stored: usize) -> String {
        let mut text = match kind {
            MergeKind::Down => "combinada con la de abajo".to_string(),
            MergeKind::Visible => format!("combinadas {layers} capas visibles"),
            MergeKind::Flatten => {
                let mut text = format!("aplanadas {layers} capas");
                match hidden {
                    0 => {}
                    1 => text.push_str(" (1 oculta descartada)"),
                    n => text.push_str(&format!(" ({n} ocultas descartadas)")),
                }
                text
            }
        };
        if stored > self.document.history_limit() {
            text.push_str(&format!(
                " · las capas originales ({}) siguen en la historia hasta tu próxima edición",
                format_bytes(stored)
            ));
        }
        text
    }

    fn remember_merge(&mut self, note: MergeNote) {
        self.merge_notes.push(note);
        if self.merge_notes.len() > MAX_NOTES {
            self.merge_notes.remove(0);
        }
    }

    /// Tras deshacer o rehacer, si se llegó al estado de antes o de después de una combinación
    /// recordada, la capa activa vuelve a la de entonces (si todavía existe).
    pub(super) fn follow_merge_notes(&mut self) {
        let state = self.document.state_id();
        let wanted = self.merge_notes.iter().rev().find_map(|note| {
            if note.before == state {
                Some(note.active_before)
            } else if note.after == state {
                Some(note.active_after)
            } else {
                None
            }
        });
        if let Some(id) = wanted {
            let _ = self.set_active_layer(id);
        }
    }

    /// Convierte el modo de **todas** las capas como un solo paso (`ConvertAll`); la elección de
    /// color se ajusta al modo nuevo como tras deshacer o rehacer.
    pub fn convert_all(&mut self, target: Mode, allowed: Allowed) -> Result<bool, DocumentError> {
        let before = self.choice;
        let changed = self
            .document
            .apply(Command::ConvertAll { target, allowed })?;
        if changed {
            self.note_applied(self.full());
            let layers = self.layer_ids();
            self.after_history_move(before, &layers);
            self.background = default_background(self.document.canvas_image());
        }
        Ok(changed)
    }

    /// Exporta el PNG aplanado (`Archivo → Exportar PNG aplanado…`): escribe lo que se ve sin tocar
    /// la ruta del documento, lo guardado, «modificado» ni la conversión. Mismas reglas de ruta
    /// que `Guardar como…`: extensión `.png`, no pisa la que se añadió y no sobrescribe el
    /// original que se convierte al abrir (escribe su copia).
    pub fn export_flat(&mut self, chosen: &Path) -> Result<Saved, FileError> {
        let dest = with_png_extension(chosen.to_path_buf());
        if dest != chosen && std::fs::symlink_metadata(&dest).is_ok() {
            return Err(FileError::Exists(dest));
        }
        let dest = match (&self.path, &self.conversion) {
            (Some(path), Some(_)) if same_file(path, &dest) => copy_path(path),
            _ => dest,
        };
        let image = flatten_to_save(&self.document)?;
        let bytes = px_formats::write_png(&image).map_err(FileError::Png)?;
        write_atomic(&dest, &bytes)?;
        Ok(Saved {
            path: dest,
            copy_reason: None,
        })
    }

    /// El aviso al exportar: el fichero y de cuántas capas sale (y cuántas ocultas no entran).
    pub fn describe_exported(&self, saved: &Saved) -> String {
        describe_exported_document(&self.document, saved)
    }
}

/// Como [`Session::describe_exported`] para un documento.
pub fn describe_exported_document(document: &Document, saved: &Saved) -> String {
    let mut text = format!("exportado {}", file_label(&saved.path));
    let layers = document.layers().len();
    if layers > 1 {
        text.push_str(&format!(" · aplanado de {layers} capas"));
        match document.layers().iter().filter(|l| !l.visible()).count() {
            0 => {}
            1 => text.push_str(" (1 oculta no entra)"),
            n => text.push_str(&format!(" ({n} ocultas no entran)")),
        }
    }
    text
}

fn merge_error_text(error: &MergeError) -> String {
    error.to_string()
}
