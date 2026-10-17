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

/// Los píxeles `x_start..x_end` (fin excluido) de la fila `y`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub y: u32,
    pub x_start: u32,
    pub x_end: u32,
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
    /// Pone `pixel` en todos los píxeles de los tramos. Los tramos van estrictamente ordenados
    /// por `(y, x_start)`, sin solapes ni vacíos; los pegados valen.
    FillSpans {
        image: ImageId,
        spans: Vec<Span>,
        pixel: Pixel,
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

/// Trozo de un relleno: `len` píxeles contiguos de la fila `y` desde `x`, todos con el mismo
/// valor anterior.
#[derive(Clone, Copy, Debug)]
struct Run {
    y: u32,
    x: u32,
    len: u32,
    before: Pixel,
}

/// Parte de un paso que viene de un `FillSpans`: los trozos que cambiaron y el valor que se
/// escribió (el mismo en todos). Va tras los primeros `at` cambios sueltos del paso.
#[derive(Clone, Debug)]
struct FillPart {
    at: usize,
    image: ImageId,
    after: Pixel,
    runs: Vec<Run>,
}

/// Paso de la historia: una secuencia ordenada de partes, que son cambios sueltos (`changes`,
/// en orden de aplicación) y rellenos (`fills`, cada uno con su posición `at` entre los
/// cambios, no decreciente). La secuencia es `changes[..at₀]`, `fills[0]`, `changes[at₀..at₁]`,
/// `fills[1]`, …, y el resto de `changes`.
#[derive(Clone, Debug, Default)]
struct Step {
    changes: Vec<Change>,
    fills: Vec<FillPart>,
    /// Identificador de estado del otro lado del paso: en la pila de deshacer, el de antes del
    /// paso; en la de rehacer, el de después. `undo` y `redo` lo intercambian con el actual.
    state: u64,
}

impl Step {
    /// Memoria que ocupa el paso, contando la capacidad reservada. Un paso sin rellenos cuesta
    /// lo mismo que cuando `Step` solo tenía el `Vec<Change>`.
    fn cost(&self) -> usize {
        let runs = self.fills.iter().fold(0usize, |total, fill| {
            total.saturating_add(fill.runs.capacity().saturating_mul(size_of::<Run>()))
        });
        size_of::<Vec<Change>>()
            .saturating_add(self.changes.capacity().saturating_mul(size_of::<Change>()))
            .saturating_add(self.fills.capacity().saturating_mul(size_of::<FillPart>()))
            .saturating_add(runs)
    }

    /// Ajusta las capacidades al número de elementos; si no se puede reservar, no hace nada.
    fn fit(&mut self) {
        if self.changes.capacity() != self.changes.len() {
            let mut exact = Vec::new();
            if exact.try_reserve_exact(self.changes.len()).is_ok() {
                exact.extend_from_slice(&self.changes);
                self.changes = exact;
            }
        }
        if self.fills.capacity() != self.fills.len() {
            let mut exact = Vec::new();
            if exact.try_reserve_exact(self.fills.len()).is_ok() {
                exact.append(&mut self.fills);
                self.fills = exact;
            }
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
    /// Identificador del estado actual (`state_id`).
    current_id: u64,
    /// Siguiente identificador por estrenar; solo crece.
    next_id: u64,
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
            current_id: 0,
            next_id: 1,
        }
    }

    /// Identificador opaco del estado actual del documento (specs/hito-2-abrir-guardar.md).
    ///
    /// `0` en el estado inicial. Cada paso aplicado (`apply` que devuelve `Ok(true)`) lleva a un
    /// identificador nuevo y nunca reutilizado; `undo` y `redo` recorren esos identificadores.
    /// Un `apply` sin cambios o con error, y un `undo` o `redo` que falla, no lo alteran, y
    /// descartar pasos viejos por el límite de la historia tampoco. Dos estados con el mismo
    /// identificador tienen el mismo contenido; la inversa no se garantiza.
    pub fn state_id(&self) -> u64 {
        self.current_id
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
        // Lo que tiene cota se reserva antes de modificar nada; los trozos de cada relleno,
        // justo antes de escribirlo (ver `execute_fill`).
        let (writes, fills) = count_leaves(&command)?;
        let mut step = Step::default();
        step.changes
            .try_reserve_exact(writes)
            .map_err(|_| DocumentError::OutOfMemory)?;
        step.fills
            .try_reserve_exact(fills)
            .map_err(|_| DocumentError::OutOfMemory)?;
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;

        if let Err(error) = self.execute(&command, &mut step) {
            self.revert(&step);
            return Err(error);
        }
        if step.changes.is_empty() && step.fills.is_empty() {
            return Ok(false);
        }
        step.fit();
        step.state = self.current_id;
        self.current_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
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
        let Some(mut step) = self.undo.pop_back() else {
            return Err(DocumentError::NothingToUndo);
        };
        self.undo_bytes = self.undo_bytes.saturating_sub(step.cost());
        self.revert(&step);
        std::mem::swap(&mut step.state, &mut self.current_id);
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
        let Some(mut step) = self.redo.pop() else {
            return Err(DocumentError::NothingToRedo);
        };
        self.replay(&step);
        std::mem::swap(&mut step.state, &mut self.current_id);
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

    fn execute(&mut self, command: &Command, step: &mut Step) -> Result<(), DocumentError> {
        let Command::Batch(commands) = command else {
            return self.execute_leaf(command, step);
        };
        if commands.is_empty() {
            return Err(DocumentError::EmptyCommand);
        }
        for (index, command) in commands.iter().enumerate() {
            self.execute_leaf(command, step)
                .map_err(|error| DocumentError::InBatch {
                    index,
                    source: Box::new(error),
                })?;
        }
        Ok(())
    }

    /// Ejecuta un comando que no es un `Batch`, anotando en `step` lo que cambia. Los cambios
    /// sueltos caben en la capacidad reservada en `apply`.
    fn execute_leaf(&mut self, command: &Command, step: &mut Step) -> Result<(), DocumentError> {
        match command {
            Command::SetPixels { image, pixels } => self.execute_set(*image, pixels, step),
            Command::FillSpans {
                image,
                spans,
                pixel,
            } => self.execute_fill(*image, spans, *pixel, step),
            _ => Err(DocumentError::NestedBatch),
        }
    }

    fn execute_set(
        &mut self,
        id: ImageId,
        pixels: &[PixelEdit],
        step: &mut Step,
    ) -> Result<(), DocumentError> {
        if pixels.is_empty() {
            return Err(DocumentError::EmptyCommand);
        }
        let image = self.image_mut(id).ok_or(DocumentError::UnknownImage(id))?;
        for (pixel, edit) in pixels.iter().enumerate() {
            let at_pixel = |source: ImageError| DocumentError::Pixel { pixel, source };
            let before = image.get_pixel(edit.x, edit.y).map_err(at_pixel)?;
            image
                .set_pixel(edit.x, edit.y, edit.pixel)
                .map_err(at_pixel)?;
            if before != edit.pixel {
                step.changes.push(Change {
                    image: id,
                    x: edit.x,
                    y: edit.y,
                    before,
                    after: edit.pixel,
                });
            }
        }
        Ok(())
    }

    /// Relleno de tramos. Primero una pasada de solo lectura que valida y cuenta los trozos;
    /// después reserva exacta; después una segunda pasada de lectura que los anota; y solo
    /// entonces escribe, sin poder fallar.
    fn execute_fill(
        &mut self,
        id: ImageId,
        spans: &[Span],
        pixel: Pixel,
        step: &mut Step,
    ) -> Result<(), DocumentError> {
        if spans.is_empty() {
            return Err(DocumentError::EmptyCommand);
        }
        let image = self.image(id).ok_or(DocumentError::UnknownImage(id))?;
        image
            .check_pixel(pixel)
            .map_err(|source| DocumentError::Pixel { pixel: 0, source })?;

        let mut count = 0usize;
        scan_runs(image, spans, pixel, |_| count = count.saturating_add(1))?;
        if count == 0 {
            return Ok(());
        }
        let mut runs = Vec::new();
        runs.try_reserve_exact(count)
            .map_err(|_| DocumentError::OutOfMemory)?;
        scan_runs(image, spans, pixel, |run| {
            if runs.len() < count {
                runs.push(run);
            }
        })?;

        if let Some(image) = self.image_mut(id) {
            for run in &runs {
                write_run(image, run, pixel);
            }
        }
        step.fills.push(FillPart {
            at: step.changes.len(),
            image: id,
            after: pixel,
            runs,
        });
        Ok(())
    }

    /// Deshace un paso: recorre la secuencia al revés escribiendo los valores «antes».
    fn revert(&mut self, step: &Step) {
        let mut end = step.changes.len();
        for fill in step.fills.iter().rev() {
            for change in step.changes.get(fill.at..end).unwrap_or(&[]).iter().rev() {
                self.write(change.image, change.x, change.y, change.before);
            }
            end = fill.at;
            if let Some(image) = self.image_mut(fill.image) {
                for run in fill.runs.iter().rev() {
                    write_run(image, run, run.before);
                }
            }
        }
        for change in step.changes.get(..end).unwrap_or(&[]).iter().rev() {
            self.write(change.image, change.x, change.y, change.before);
        }
    }

    /// Rehace un paso: recorre la secuencia hacia delante escribiendo los valores «después».
    fn replay(&mut self, step: &Step) {
        let mut start = 0;
        for fill in &step.fills {
            for change in step.changes.get(start..fill.at).unwrap_or(&[]) {
                self.write(change.image, change.x, change.y, change.after);
            }
            start = fill.at;
            if let Some(image) = self.image_mut(fill.image) {
                for run in &fill.runs {
                    write_run(image, run, fill.after);
                }
            }
        }
        for change in step.changes.get(start..).unwrap_or(&[]) {
            self.write(change.image, change.x, change.y, change.after);
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

/// Escribe `pixel` en los `run.len` píxeles del trozo. Los trozos salen de la propia imagen,
/// así que no puede fallar; si fallara, se ignora como en `Document::write`.
fn write_run(image: &mut Image, run: &Run, pixel: Pixel) {
    for dx in 0..run.len {
        let _ = image.set_pixel(run.x.saturating_add(dx), run.y, pixel);
    }
}

/// Recorre los tramos sin modificar nada: los valida y llama a `emit` con cada trozo, en
/// orden. Un trozo es una serie máxima de píxeles de un tramo con el mismo valor anterior,
/// todos distintos de `pixel`. Orden de errores: por tramo, `BadSpans` antes que `Span`.
fn scan_runs(
    image: &Image,
    spans: &[Span],
    pixel: Pixel,
    mut emit: impl FnMut(Run),
) -> Result<(), DocumentError> {
    let (width, height) = (image.width(), image.height());
    let mut previous: Option<&Span> = None;
    for (index, span) in spans.iter().enumerate() {
        let after_previous = previous
            .is_none_or(|prev| span.y > prev.y || (span.y == prev.y && span.x_start >= prev.x_end));
        if span.x_start >= span.x_end || !after_previous {
            return Err(DocumentError::BadSpans { span: index });
        }
        previous = Some(span);
        if span.y >= height || span.x_end > width {
            let x = if span.y >= height {
                span.x_start
            } else {
                span.x_start.max(width)
            };
            return Err(DocumentError::Span {
                span: index,
                source: ImageError::OutOfBounds {
                    x,
                    y: span.y,
                    width,
                    height,
                },
            });
        }
        let mut open: Option<Run> = None;
        for x in span.x_start..span.x_end {
            let old = image
                .get_pixel(x, span.y)
                .map_err(|source| DocumentError::Span {
                    span: index,
                    source,
                })?;
            match &mut open {
                Some(run) if old != pixel && run.before == old => run.len += 1,
                _ => {
                    if let Some(run) = open.take() {
                        emit(run);
                    }
                    if old != pixel {
                        open = Some(Run {
                            y: span.y,
                            x,
                            len: 1,
                            before: old,
                        });
                    }
                }
            }
        }
        if let Some(run) = open {
            emit(run);
        }
    }
    Ok(())
}

/// Cotas del comando para reservar antes de modificar nada: escrituras de los `SetPixels` y
/// número de `FillSpans`.
fn count_leaves(command: &Command) -> Result<(usize, usize), DocumentError> {
    let leaf = |command: &Command| match command {
        Command::SetPixels { pixels, .. } => (pixels.len(), 0),
        Command::FillSpans { .. } => (0, 1),
        _ => (0, 0),
    };
    let Command::Batch(commands) = command else {
        return Ok(leaf(command));
    };
    commands
        .iter()
        .try_fold((0usize, 0usize), |(writes, fills), command| {
            let (w, f) = leaf(command);
            Ok((
                writes.checked_add(w).ok_or(DocumentError::OutOfMemory)?,
                fills.checked_add(f).ok_or(DocumentError::OutOfMemory)?,
            ))
        })
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
    /// El tramo número `span` de un `FillSpans` se sale del lienzo; `source` es `OutOfBounds`
    /// con el primer píxel fuera.
    Span {
        span: usize,
        source: ImageError,
    },
    /// El tramo número `span` de un `FillSpans` está vacío o no va estrictamente después del
    /// anterior (desordenado o solapado).
    BadSpans {
        span: usize,
    },
    /// `SetPixels` sin píxeles, `FillSpans` sin tramos o `Batch` sin comandos.
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
            DocumentError::Span { span, source } => {
                write!(f, "el tramo {span} se sale del lienzo: {source}")
            }
            DocumentError::BadSpans { span } => write!(
                f,
                "el tramo {span} está vacío o no va después del tramo anterior"
            ),
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
            DocumentError::Pixel { source, .. } | DocumentError::Span { source, .. } => {
                Some(source)
            }
            DocumentError::InBatch { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
