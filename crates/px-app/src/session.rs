//! Sesión de edición: el documento, su fichero y si hay cambios sin guardar
//! (specs/hito-2-abrir-guardar.md). Sin egui: se prueba sin ventana.

use std::path::{Path, PathBuf};

use px_core::{Document, Image};
use px_formats::write_png;
use px_ui::color::pixel_for_slot;

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

pub struct Session {
    pub document: Document,
    path: Option<PathBuf>,
    /// `state_id` del documento al abrir o guardar por última vez.
    saved_id: u64,
    /// Conversiones del fichero abierto, si las hubo: el primer `save` hace una copia.
    conversion: Option<String>,
}

impl Session {
    pub fn sample(document: Document) -> Session {
        Session {
            saved_id: document.state_id(),
            document,
            path: None,
            conversion: None,
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

/// La ranura de color si vale para la imagen nueva (su modo y su paleta); si no, la 1.
pub fn adjust_slot(image: &Image, slot: u8) -> u8 {
    if pixel_for_slot(image, slot).is_some() {
        slot
    } else {
        1
    }
}

#[cfg(test)]
mod tests;
