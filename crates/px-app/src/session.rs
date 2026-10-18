//! Sesión de edición: el documento, su fichero y si hay cambios sin guardar
//! (specs/hito-2-abrir-guardar.md). Sin egui: se prueba sin ventana.

use std::path::{Path, PathBuf};

use px_core::{Command, Document, DocumentError, Pixel};
use px_formats::write_png;
use px_ui::color::{ColorChoice, SlotBank, clamp_choice, initial_choice, pixel_for_choice_with};
use px_ui::color_edit::{ColorEditor, Commit, EditTarget, StaleReason, stale_notice};
use px_ui::palette_grid::{PanelView, Scroll};
use px_ui::palette_ops::{
    Fate, NOTHING_TO_ROTATE_NOTICE, PaletteOp, PaletteOps, choice_after, entry_fate, summary,
};

use crate::files::{
    FileError, MAX_FILE_BYTES, copy_path, describe_open, describe_report, file_label, load_png,
    untitled_path, write_atomic,
};

/// Resultado de guardar: dónde se escribió y, si no se sobrescribió el original porque Pixoko
/// lo convierte al abrir, qué conversiones eran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub path: PathBuf,
    pub copy_reason: Option<String>,
}

/// Resultado de una edición de la paleta: si el documento cambió y el aviso de 5 s (resumen de lo
/// ocurrido, o por qué no pasó nada; con texto pendiente descartado, lo lleva antepuesto).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteOutcome {
    pub changed: bool,
    pub notice: Option<String>,
}

pub struct Session {
    pub document: Document,
    /// Color activo del panel de paletas. Abrir un fichero lo reinicia (specs/hito-2-panel-paletas.md).
    pub choice: ColorChoice,
    /// Desplazamiento del panel de paletas; abrir un fichero lo reinicia.
    pub scroll: Scroll,
    /// Estado del panel entre fotogramas (desplazamiento de todo el panel y «mostrar la elegida»);
    /// abrir un fichero lo reinicia (specs/hito-2-panel-ventana-baja.md).
    pub view: PanelView,
    /// Colores de las ranuras por modo; duran toda la sesión y no son del documento
    /// (specs/hito-2-editar-color.md).
    pub slots: SlotBank,
    /// Estado del editor de color; abrir un fichero lo reinicia.
    pub editor: ColorEditor,
    /// Arrastre de muestras, quita pendiente y rango de rotación (specs/hito-2-paleta-operaciones.md);
    /// abrir un fichero los reinicia.
    pub ops: PaletteOps,
    /// Una quita pendiente se canceló porque el documento cambió y aún no se avisó.
    removal_cancelled: bool,
    path: Option<PathBuf>,
    /// Un `open` descartó el texto pendiente del campo hexadecimal y aún no se avisó.
    discarded_text: bool,
    /// `state_id` del documento al abrir o guardar por última vez.
    saved_id: u64,
    /// Conversiones del fichero abierto, si las hubo: el primer `save` hace una copia.
    conversion: Option<String>,
}

impl Session {
    pub fn sample(document: Document) -> Session {
        let choice = document
            .image(document.main_image())
            .map_or(ColorChoice::Slot(1), initial_choice);
        Session {
            saved_id: document.state_id(),
            document,
            choice,
            scroll: Scroll::default(),
            view: PanelView::default(),
            slots: SlotBank::new(),
            editor: ColorEditor::default(),
            ops: PaletteOps::default(),
            removal_cancelled: false,
            path: None,
            discarded_text: false,
            conversion: None,
        }
    }

    /// Cambia la elección por teclado; si cambió, el panel muestra su fila en el fotograma
    /// siguiente.
    pub fn set_choice_revealing(&mut self, choice: ColorChoice) {
        if choice != self.choice {
            self.choice = choice;
            self.view.reveal = true;
            self.ops.clear_range();
        }
    }

    /// Deshace un paso y deja la selección válida para la paleta resultante.
    pub fn undo(&mut self) -> Result<(), DocumentError> {
        let before = self.choice;
        let result = self.document.undo();
        self.after_history_move(before);
        result
    }

    /// Rehace un paso y deja la selección válida para la paleta resultante.
    pub fn redo(&mut self) -> Result<(), DocumentError> {
        let before = self.choice;
        let result = self.document.redo();
        self.after_history_move(before);
        result
    }

    fn after_history_move(&mut self, before: ColorChoice) {
        // El rango no sobrevive a deshacer ni a rehacer.
        self.ops.clear_range();
        self.normalize();
        self.reveal_if_choice_changed(before);
    }

    /// Deja la selección válida para la imagen y pone al día el estado de las operaciones de
    /// paleta (rango, quita pendiente). Cada fotograma, antes del panel, y tras deshacer o
    /// rehacer.
    pub fn normalize(&mut self) {
        let Some(image) = self.document.image(self.document.main_image()) else {
            return;
        };
        self.choice = clamp_choice(image, self.choice);
        let len = image.palette().map_or(0, |palette| palette.len());
        if self.ops.sync(self.document.state_id(), len) {
            self.removal_cancelled = true;
        }
    }

    /// Una quita pendiente se canceló porque el documento cambió: se dice una sola vez.
    pub fn take_removal_cancelled(&mut self) -> bool {
        std::mem::take(&mut self.removal_cancelled)
    }

    /// Aplica una edición de la paleta como **un** paso: construye el comando, fija la
    /// selección, resuelve el texto pendiente del campo hexadecimal y cierra la quita pendiente.
    /// `unused`: ningún píxel usaba la entrada quitada (solo para el resumen).
    pub fn apply_palette_op(
        &mut self,
        op: PaletteOp,
        unused: bool,
    ) -> Result<PaletteOutcome, DocumentError> {
        let id = self.document.main_image();
        let active = self.active_entry().unwrap_or(0);
        let (old_len, was_transparent, command) = {
            let Some(image) = self.document.image(id) else {
                return Ok(PaletteOutcome {
                    changed: false,
                    notice: None,
                });
            };
            let palette = image.palette();
            let old_len = palette.map_or(0, |palette| palette.len());
            let was_transparent = match (op, palette) {
                (PaletteOp::Remove { index, .. }, Some(palette)) => {
                    palette.transparent_index() == Some(index)
                }
                _ => false,
            };
            let command = match op {
                PaletteOp::Add => {
                    let color = palette
                        .and_then(|palette| palette.resolved(active).ok())
                        .unwrap_or(px_core::Rgba::TRANSPARENT);
                    Command::AddPaletteEntry { image: id, color }
                }
                PaletteOp::Remove { index, into } => Command::RemovePaletteEntry {
                    image: id,
                    index,
                    into,
                },
                PaletteOp::Move { from, to } => Command::MovePaletteEntry {
                    image: id,
                    from,
                    to,
                },
                PaletteOp::Rotate { start, len, steps } => Command::RotateColors {
                    image: id,
                    start,
                    len,
                    steps,
                },
            };
            (old_len, was_transparent, command)
        };
        let own_removal = matches!(
            (op, self.ops.removal()),
            (PaletteOp::Remove { index, .. }, Some(removal)) if removal.index == index
        );
        if !self.document.apply(command)? {
            let notice = match op {
                PaletteOp::Rotate { len, .. } if len >= 2 => {
                    Some("nada que rotar: colores iguales")
                }
                PaletteOp::Rotate { .. } => Some(NOTHING_TO_ROTATE_NOTICE),
                _ => None,
            };
            return Ok(PaletteOutcome {
                changed: false,
                notice: notice.map(str::to_string),
            });
        }

        // El texto pendiente del campo, si la operación movió, quitó o cambió su entrada.
        let stale = match self.editor.pending_target() {
            Some(EditTarget::Entry(entry)) => match entry_fate(&op, entry) {
                Fate::Same => None,
                Fate::Moved => Some(StaleReason::Moved),
                Fate::Changed => Some(StaleReason::Changed),
                Fate::Gone => Some(StaleReason::Gone),
            },
            _ => None,
        };
        let stale = stale
            .filter(|_| self.editor.discard_pending())
            .map(stale_notice);

        // Selección y estado de las operaciones.
        if !matches!(op, PaletteOp::Rotate { .. }) {
            let after = choice_after(&op, active, old_len);
            self.set_choice_revealing(ColorChoice::Entry(after));
            self.ops.clear_range();
        }
        if self.ops.removal().is_some() {
            self.ops.cancel_removal();
            if !own_removal {
                self.removal_cancelled = true;
            }
        }
        self.normalize();
        let text = summary(&op, active, old_len, was_transparent, unused);
        Ok(PaletteOutcome {
            changed: true,
            notice: Some(match stale {
                Some(stale) => format!("{stale} · {text}"),
                None => text,
            }),
        })
    }

    fn active_entry(&self) -> Option<u8> {
        match self.choice {
            ColorChoice::Entry(index) => Some(index),
            ColorChoice::Slot(_) => None,
        }
    }

    /// Tras deshacer o rehacer: si la elección ya no es `before`, el panel muestra su fila.
    pub fn reveal_if_choice_changed(&mut self, before: ColorChoice) {
        if self.choice != before {
            self.view.reveal = true;
        }
    }

    /// Si un `open` descartó texto pendiente del campo, lo dice una sola vez.
    pub fn take_discarded_text(&mut self) -> bool {
        std::mem::take(&mut self.discarded_text)
    }

    /// `Pixel` del color activo con los colores de ranura de la sesión.
    pub fn active_pixel(&self) -> Option<Pixel> {
        let image = self.document.image(self.document.main_image())?;
        pixel_for_choice_with(image, self.choice, self.slots.get(image.mode()))
    }

    /// Pone al día el editor con el destino y el color guardado de la selección actual.
    pub fn sync_editor(&mut self) {
        let current = self
            .document
            .image(self.document.main_image())
            .and_then(|image| {
                let target = EditTarget::of(image, self.choice)?;
                let color = target.stored(image, self.slots.get(image.mode()))?;
                Some((target, color))
            });
        self.editor.sync(current);
    }

    /// Aplica un color del editor: `SetPaletteColor` en indexadas (`Ok(true)` si el documento
    /// cambió, `Ok(false)` si el color era el mismo) o escribe la ranura de ese modo
    /// (`Ok(false)`: no es del documento, ni deshace ni marca cambios).
    pub fn commit_edit(&mut self, commit: Commit) -> Result<bool, DocumentError> {
        let image = self.document.main_image();
        match commit.target {
            EditTarget::Entry(index) => self.document.apply(Command::SetPaletteColor {
                image,
                index,
                color: commit.color,
            }),
            EditTarget::Slot(slot) => {
                if let Some(mode) = self.document.image(image).map(|image| image.mode()) {
                    self.slots.get_mut(mode).set(slot, commit.color);
                }
                Ok(false)
            }
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Hay cambios sin guardar: sin falsos negativos (con cambios nunca da falso), pero puede
    /// haber falsos positivos.
    pub fn modified(&self) -> bool {
        self.document.state_id() != self.saved_id
    }

    /// Abre `path` y reemplaza el documento (historia vacía). Con cambios sin guardar y sin
    /// `discard`, `Err(Unsaved)` sin leer el fichero. Con cualquier error la sesión no cambia.
    /// Devuelve el aviso de apertura.
    pub fn open(
        &mut self,
        path: &Path,
        max_side: usize,
        discard: bool,
    ) -> Result<String, FileError> {
        if self.modified() && !discard {
            return Err(FileError::Unsaved);
        }
        let read = load_png(path, MAX_FILE_BYTES, max_side)?;
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let text = describe_open(&file_label(&absolute), &read.image, &read.report);
        self.conversion = (!read.report.is_empty()).then(|| describe_report(&read.report));
        self.choice = initial_choice(&read.image);
        self.scroll = Scroll::default();
        self.view = PanelView::default();
        self.discarded_text = self.editor.has_pending();
        self.removal_cancelled |= self.ops.removal().is_some();
        self.ops.reset();
        self.editor.reset();
        self.document = Document::new(read.image);
        self.saved_id = self.document.state_id();
        self.path = Some(absolute);
        Ok(text)
    }

    /// Guarda en la ruta del documento; sin ruta, en `untitled_path(new_dir)`. Si el fichero
    /// abierto se convierte al abrir, escribe una copia en vez de sobrescribirlo.
    pub fn save(&mut self, new_dir: &Path) -> Result<Saved, FileError> {
        let (dest, copy_reason) = match (&self.path, &self.conversion) {
            (Some(path), Some(reason)) => (copy_path(path), Some(reason.clone())),
            (Some(path), None) => (path.clone(), None),
            (None, _) => (untitled_path(new_dir), None),
        };
        self.write_to(dest, copy_reason)
    }

    /// Guarda una copia junto al original (`copy_path`), que pasa a ser el fichero del
    /// documento; sin ruta, en `untitled_path(new_dir)`. Nunca pisa un fichero existente.
    pub fn save_as(&mut self, new_dir: &Path) -> Result<Saved, FileError> {
        let dest = match &self.path {
            Some(path) => copy_path(path),
            None => untitled_path(new_dir),
        };
        self.write_to(dest, None)
    }

    fn write_to(&mut self, dest: PathBuf, copy_reason: Option<String>) -> Result<Saved, FileError> {
        let image = self
            .document
            .image(self.document.main_image())
            .ok_or_else(|| FileError::Write("el documento no tiene imagen".to_string()))?;
        let bytes = write_png(image).map_err(FileError::Png)?;
        write_atomic(&dest, &bytes)?;
        self.path = Some(dest.clone());
        self.saved_id = self.document.state_id();
        self.conversion = None;
        Ok(Saved {
            path: dest,
            copy_reason,
        })
    }
}

/// Aviso al guardar. Con copia por conversión, explica por qué no se sobrescribió el original.
pub fn describe_saved(saved: &Saved) -> String {
    let name = file_label(&saved.path);
    match &saved.copy_reason {
        None => format!("guardado {name}"),
        Some(reason) => format!(
            "guardado como {name}: el original se convierte al abrir ({reason}) y no se sobrescribe"
        ),
    }
}

#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod ops_tests;
#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod tests;
