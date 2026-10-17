//! Documento y comandos con deshacer.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::mem::size_of;

use crate::{Image, ImageError, Pixel};

/// Identificador de una imagen dentro de un documento. Solo `Document` reparte ids válidos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImageId(u32);

impl ImageId {
    pub fn from_raw(raw: u32) -> ImageId {
        ImageId(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

/// Escritura de un píxel en una coordenada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelEdit {
    pub x: u32,
    pub y: u32,
    pub pixel: Pixel,
}

/// Una edición del documento, como dato.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Command {
    /// Escribe los píxeles en orden; si una coordenada se repite, gana la última.
    SetPixels {
        image: ImageId,
        pixels: Vec<PixelEdit>,
    },
    /// Ejecuta los comandos en orden como un solo paso de deshacer. No se anida.
    Batch(Vec<Command>),
}

/// Un píxel que cambió de valor.
#[derive(Clone, Copy, Debug)]
struct Change {
    image: ImageId,
    x: u32,
    y: u32,
    before: Pixel,
    after: Pixel,
}

/// Paso de la historia: los cambios de un comando, en orden de aplicación.
#[derive(Clone, Debug)]
struct Step {
    changes: Vec<Change>,
}

impl Step {
    /// Memoria que ocupa el paso, contando la capacidad reservada.
    fn cost(&self) -> usize {
        size_of::<Step>()
            .saturating_add(self.changes.capacity().saturating_mul(size_of::<Change>()))
    }

    /// Ajusta la capacidad al número de cambios; si no se puede reservar, no hace nada.
    fn fit(&mut self) {
        if self.changes.capacity() == self.changes.len() {
            return;
        }
        let mut exact = Vec::new();
        if exact.try_reserve_exact(self.changes.len()).is_ok() {
            exact.extend_from_slice(&self.changes);
            self.changes = exact;
        }
    }
}

/// Documento: sus imágenes y la historia de deshacer.
///
/// La igualdad compara el contenido (las imágenes por id), no la historia.
#[derive(Clone, Debug)]
pub struct Document {
    images: Vec<(ImageId, Image)>,
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    /// Suma de `Step::cost` de la pila de deshacer.
    undo_bytes: usize,
    limit: usize,
}

impl PartialEq for Document {
    fn eq(&self, other: &Document) -> bool {
        self.images == other.images
    }
}

impl Eq for Document {}

impl Document {
    /// Límite de memoria de la historia por defecto (64 MiB).
    pub const DEFAULT_HISTORY_LIMIT: usize = 64 << 20;

    pub fn new(image: Image) -> Document {
        Document {
            images: vec![(ImageId(0), image)],
            undo: VecDeque::new(),
            redo: Vec::new(),
            undo_bytes: 0,
            limit: Document::DEFAULT_HISTORY_LIMIT,
        }
    }

    /// El id de la imagen con la que se creó el documento.
    pub fn main_image(&self) -> ImageId {
        ImageId(0)
    }

    pub fn image(&self, id: ImageId) -> Option<&Image> {
        self.images
            .iter()
            .find(|(image_id, _)| *image_id == id)
            .map(|(_, image)| image)
    }

    fn image_mut(&mut self, id: ImageId) -> Option<&mut Image> {
        self.images
            .iter_mut()
            .find(|(image_id, _)| *image_id == id)
            .map(|(_, image)| image)
    }

    /// Ejecuta un comando. `Ok(true)` si algún píxel cambió de valor (y entonces hay un paso
    /// nuevo y no se puede rehacer); `Ok(false)` si no cambió ninguno. Con error, el documento
    /// y la historia quedan como estaban.
    pub fn apply(&mut self, command: Command) -> Result<bool, DocumentError> {
        // Todo lo que pide memoria se reserva antes de modificar nada.
        let mut changes = Vec::new();
        changes
            .try_reserve_exact(count_writes(&command)?)
            .map_err(|_| DocumentError::OutOfMemory)?;
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;

        if let Err(error) = self.execute(&command, &mut changes) {
            self.revert(&changes);
            return Err(error);
        }
        if changes.is_empty() {
            return Ok(false);
        }
        let mut step = Step { changes };
        step.fit();
        self.redo.clear();
        self.undo_bytes = self.undo_bytes.saturating_add(step.cost());
        self.undo.push_back(step);
        self.trim();
        Ok(true)
    }

    pub fn undo(&mut self) -> Result<(), DocumentError> {
        if self.undo.is_empty() {
            return Err(DocumentError::NothingToUndo);
        }
        self.redo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        let Some(step) = self.undo.pop_back() else {
            return Err(DocumentError::NothingToUndo);
        };
        self.undo_bytes = self.undo_bytes.saturating_sub(step.cost());
        self.revert(&step.changes);
        self.redo.push(step);
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), DocumentError> {
        if self.redo.is_empty() {
            return Err(DocumentError::NothingToRedo);
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        let Some(step) = self.redo.pop() else {
            return Err(DocumentError::NothingToRedo);
        };
        for change in &step.changes {
            self.write(change.image, change.x, change.y, change.after);
        }
        self.undo_bytes = self.undo_bytes.saturating_add(step.cost());
        self.undo.push_back(step);
        self.trim();
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Memoria de los pasos de deshacer, en bytes.
    pub fn history_bytes(&self) -> usize {
        self.undo_bytes
    }

    pub fn history_limit(&self) -> usize {
        self.limit
    }

    /// Cambia el límite y descarta ya los pasos más antiguos que no quepan.
    pub fn set_history_limit(&mut self, bytes: usize) {
        self.limit = bytes;
        self.trim();
    }

    /// Descarta los pasos más antiguos mientras se supere el límite, salvo el más reciente.
    fn trim(&mut self) {
        while self.undo_bytes > self.limit && self.undo.len() > 1 {
            let Some(oldest) = self.undo.pop_front() else {
                break;
            };
            self.undo_bytes = self.undo_bytes.saturating_sub(oldest.cost());
        }
    }

    fn execute(
        &mut self,
        command: &Command,
        changes: &mut Vec<Change>,
    ) -> Result<(), DocumentError> {
        let Command::Batch(commands) = command else {
            return self.execute_leaf(command, changes);
        };
        if commands.is_empty() {
            return Err(DocumentError::EmptyCommand);
        }
        for (index, command) in commands.iter().enumerate() {
            self.execute_leaf(command, changes)
                .map_err(|error| DocumentError::InBatch {
                    index,
                    source: Box::new(error),
                })?;
        }
        Ok(())
    }

    /// Ejecuta un comando que no es un `Batch`, anotando las escrituras que cambian un valor.
    /// No reserva memoria: `changes` tiene capacidad para todas las escrituras (ver `apply`).
    fn execute_leaf(
        &mut self,
        command: &Command,
        changes: &mut Vec<Change>,
    ) -> Result<(), DocumentError> {
        let Command::SetPixels { image: id, pixels } = command else {
            return Err(DocumentError::NestedBatch);
        };
        if pixels.is_empty() {
            return Err(DocumentError::EmptyCommand);
        }
        let image = self
            .image_mut(*id)
            .ok_or(DocumentError::UnknownImage(*id))?;
        for (pixel, edit) in pixels.iter().enumerate() {
            let at_pixel = |source: ImageError| DocumentError::Pixel { pixel, source };
            let before = image.get_pixel(edit.x, edit.y).map_err(at_pixel)?;
            image
                .set_pixel(edit.x, edit.y, edit.pixel)
                .map_err(at_pixel)?;
            if before != edit.pixel {
                changes.push(Change {
                    image: *id,
                    x: edit.x,
                    y: edit.y,
                    before,
                    after: edit.pixel,
                });
            }
        }
        Ok(())
    }

    /// Restaura los valores «antes» en orden inverso.
    fn revert(&mut self, changes: &[Change]) {
        for change in changes.iter().rev() {
            self.write(change.image, change.x, change.y, change.before);
        }
    }

    /// Escribe un valor que salió de la propia imagen: no puede fallar, y si fallara no hay
    /// nada razonable que hacer, así que se ignora el resultado.
    fn write(&mut self, id: ImageId, x: u32, y: u32, pixel: Pixel) {
        if let Some(image) = self.image_mut(id) {
            let _ = image.set_pixel(x, y, pixel);
        }
    }
}

/// Número máximo de escrituras de un comando, para reservar la lista de cambios.
fn count_writes(command: &Command) -> Result<usize, DocumentError> {
    match command {
        Command::SetPixels { pixels, .. } => Ok(pixels.len()),
        Command::Batch(commands) => commands.iter().try_fold(0usize, |total, command| {
            let writes = match command {
                Command::SetPixels { pixels, .. } => pixels.len(),
                _ => 0,
            };
            total.checked_add(writes).ok_or(DocumentError::OutOfMemory)
        }),
    }
}

/// Error al aplicar un comando o al deshacer y rehacer.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DocumentError {
    /// El documento no tiene esa imagen.
    UnknownImage(ImageId),
    /// Falló la escritura del píxel número `pixel` de la lista.
    Pixel {
        pixel: usize,
        source: ImageError,
    },
    /// `SetPixels` sin píxeles o `Batch` sin comandos.
    EmptyCommand,
    /// Un `Batch` dentro de otro `Batch`.
    NestedBatch,
    /// El comando número `index` de un `Batch` falló con `source`.
    InBatch {
        index: usize,
        source: Box<DocumentError>,
    },
    NothingToUndo,
    NothingToRedo,
    /// No se pudo reservar memoria para la historia.
    OutOfMemory,
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::UnknownImage(id) => {
                write!(f, "el documento no tiene la imagen {}", id.raw())
            }
            DocumentError::Pixel { pixel, source } => {
                write!(
                    f,
                    "no se pudo escribir el píxel {pixel} de la lista: {source}"
                )
            }
            DocumentError::EmptyCommand => write!(f, "el comando no contiene ninguna edición"),
            DocumentError::NestedBatch => write!(f, "un Batch no puede contener otro Batch"),
            DocumentError::InBatch { index, source } => {
                write!(f, "el comando {index} del Batch falló: {source}")
            }
            DocumentError::NothingToUndo => write!(f, "no hay nada que deshacer"),
            DocumentError::NothingToRedo => write!(f, "no hay nada que rehacer"),
            DocumentError::OutOfMemory => {
                write!(f, "no hay memoria para guardar el paso en la historia")
            }
        }
    }
}

impl Error for DocumentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DocumentError::Pixel { source, .. } => Some(source),
            DocumentError::InBatch { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
