//! Documento y comandos con deshacer.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::mem::size_of;

use crate::palette_edit::{self, PaletteStep};
use crate::{
    Allowed, Conversion, ConvertError, Image, ImageError, Mode, PaletteEditError, Pixel, Rgba,
    convert_image,
};

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

/// Identificador de una capa dentro de un documento. Solo `Document` reparte ids válidos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerId(u32);

/// Una capa: sus propiedades y su celda, que es la imagen `image` (`None` = celda vacía, sin
/// píxeles reservados; vale transparente). `cel` es el `ImageId` de la celda y existe desde que
/// se crea la capa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    id: LayerId,
    name: String,
    visible: bool,
    opacity: u8,
    cel: ImageId,
    image: Option<Image>,
}

/// Id reservado de la plantilla del lienzo; ninguna celda lo recibe nunca.
const TEMPLATE: ImageId = ImageId(u32::MAX);

/// El lienzo del documento: tamaño y una plantilla de 1 × 1 con el modo y la paleta de todas
/// las celdas. Con las celdas vacías puede no haber ninguna imagen de la que leerlos.
#[derive(Clone, Debug)]
struct Canvas {
    width: u32,
    height: u32,
    template: Image,
}

/// Plantilla de 1 × 1 con el modo y la paleta de `image`.
fn make_template(image: &Image) -> Result<Image, ImageError> {
    match image.palette() {
        Some(palette) => Image::filled_indexed(1, 1, palette.clone(), 0),
        None => Image::new(1, 1, image.mode()),
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
    /// Convierte la imagen a otro modo (`convert_image`) como un solo paso de deshacer. Si ya
    /// está en ese modo no hace nada. Solo como comando suelto: dentro de un `Batch` es
    /// `NotBatchable`.
    ConvertMode {
        image: ImageId,
        target: Mode,
        allowed: Allowed,
    },
    /// Cambia el color guardado de la entrada `index` de la paleta; los píxeles no se tocan.
    SetPaletteColor {
        image: ImageId,
        index: u8,
        color: Rgba,
    },
    /// La entrada `from` pasa a estar en `to` y las de en medio se desplazan un lugar, como
    /// `Vec::remove` más `Vec::insert`. Píxeles e índice transparente se remapean: la imagen
    /// no cambia. Equivale a la `PermutePalette` correspondiente.
    MovePaletteEntry { image: ImageId, from: u8, to: u8 },
    /// Reordena la paleta: `order[nuevo] = antiguo`. Debe ser una permutación de las entradas.
    /// Píxeles e índice transparente se remapean: la imagen no cambia.
    PermutePalette { image: ImageId, order: Vec<u8> },
    /// Añade una entrada al final de la paleta.
    AddPaletteEntry { image: ImageId, color: Rgba },
    /// Quita la entrada `index`; los píxeles que la usan pasan a `into` (los dos, índices
    /// antes de quitar). Si `index` era el índice transparente, deja de haberlo.
    RemovePaletteEntry { image: ImageId, index: u8, into: u8 },
    /// Fija o quita el índice transparente.
    SetTransparentIndex { image: ImageId, index: Option<u8> },
    /// Rota `steps` posiciones (con signo, hacia índices mayores si es positivo) los colores
    /// de las entradas `start..start + len`; los píxeles conservan su índice y el índice
    /// transparente se queda en su ranura.
    RotateColors {
        image: ImageId,
        start: u8,
        len: u16,
        steps: i32,
    },
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
    /// En un paso de conversión de modo, la imagen que no está en el documento (la anterior
    /// mientras el paso está en la pila de deshacer, la convertida en la de rehacer) y la
    /// plantilla del lienzo que le corresponde. `undo` y `redo` las intercambian con las del
    /// documento.
    swapped: Option<Swapped>,
    /// En un paso de edición de paleta, el paso (que es todo su contenido).
    palette: Option<PaletteStep>,
    /// Identificador de estado del otro lado del paso: en la pila de deshacer, el de antes del
    /// paso; en la de rehacer, el de después. `undo` y `redo` lo intercambian con el actual.
    state: u64,
}

/// Lo que intercambia un paso de conversión de modo.
#[derive(Clone, Debug)]
struct Swapped {
    id: ImageId,
    image: Image,
    template: Image,
}

impl Step {
    /// Memoria que ocupa el paso, contando la capacidad reservada. Un paso sin rellenos cuesta
    /// lo mismo que cuando `Step` solo tenía el `Vec<Change>`.
    fn cost(&self) -> usize {
        let runs = self.fills.iter().fold(0usize, |total, fill| {
            total.saturating_add(fill.runs.capacity().saturating_mul(size_of::<Run>()))
        });
        // La plantilla (1 × 1 y una paleta) no se cuenta: es despreciable.
        let swapped = self.swapped.as_ref().map_or(0, |swapped| {
            size_of::<Image>().saturating_add(swapped.image.content_bytes())
        });
        let palette = self.palette.as_ref().map_or(0, PaletteStep::cost);
        size_of::<Vec<Change>>()
            .saturating_add(self.changes.capacity().saturating_mul(size_of::<Change>()))
            .saturating_add(self.fills.capacity().saturating_mul(size_of::<FillPart>()))
            .saturating_add(runs)
            .saturating_add(swapped)
            .saturating_add(palette)
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
    /// De abajo a arriba.
    layers: Vec<Layer>,
    canvas: Canvas,
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
        self.layers == other.layers
    }
}

impl Eq for Document {}

impl Document {
    /// Límite de memoria de la historia por defecto (64 MiB).
    pub const DEFAULT_HISTORY_LIMIT: usize = 64 << 20;

    /// Tope de tramos de un relleno por inundación y de trozos que un `FillSpans` guarda en su
    /// paso de historia (specs/hito-2-limites-relleno.md). Un paso así cuesta como mucho
    /// `20 · 3 000 000 = 57,2 MiB`, por debajo de `DEFAULT_HISTORY_LIMIT`. Pasarse es
    /// `FillTooFragmented`, antes de reservar nada.
    pub const MAX_FILL_SPANS: usize = 3_000_000;

    pub fn new(image: Image) -> Document {
        let template = make_template(&image).unwrap_or_else(|_| image.clone());
        Document {
            canvas: Canvas {
                width: image.width(),
                height: image.height(),
                template,
            },
            layers: vec![Layer {
                id: LayerId(0),
                name: String::from("Capa 1"),
                visible: true,
                opacity: 255,
                cel: ImageId(0),
                image: Some(image),
            }],
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

    /// La imagen de la celda `id`: `None` si no existe o si la celda está vacía.
    pub fn image(&self, id: ImageId) -> Option<&Image> {
        self.layers
            .iter()
            .find(|layer| layer.cel == id)
            .and_then(|layer| layer.image.as_ref())
    }

    /// La imagen de una celda con imagen (nunca la plantilla del lienzo).
    fn image_mut(&mut self, id: ImageId) -> Option<&mut Image> {
        self.layers
            .iter_mut()
            .find(|layer| layer.cel == id)
            .and_then(|layer| layer.image.as_mut())
    }

    /// `true` si `id` es la celda de alguna capa, vacía o no.
    fn has_cel(&self, id: ImageId) -> bool {
        self.layers.iter().any(|layer| layer.cel == id)
    }

    /// Ejecuta un comando. `Ok(true)` si algún píxel cambió de valor (y entonces hay un paso
    /// nuevo y no se puede rehacer); `Ok(false)` si no cambió ninguno. Con error, el documento
    /// y la historia quedan como estaban.
    pub fn apply(&mut self, command: Command) -> Result<bool, DocumentError> {
        if let Command::ConvertMode {
            image,
            target,
            allowed,
        } = command
        {
            return self.apply_convert(image, target, allowed);
        }
        if let Some(id) = palette_edit::command_image(&command) {
            return self.apply_palette(id, &command);
        }
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
        self.commit(step);
        Ok(true)
    }

    /// Anota un paso ya aplicado: nuevo identificador de estado, rehacer borrado, coste en la
    /// pila de deshacer y recorte por el límite. La pila de deshacer tiene el hueco reservado.
    fn commit(&mut self, mut step: Step) {
        step.state = self.current_id;
        self.current_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.redo.clear();
        self.undo_bytes = self.undo_bytes.saturating_add(step.cost());
        self.undo.push_back(step);
        self.trim();
    }

    /// `ConvertMode`: todo lo que puede fallar o reservar ocurre antes de tocar el documento;
    /// después solo se intercambian imágenes.
    fn apply_convert(
        &mut self,
        id: ImageId,
        target: Mode,
        allowed: Allowed,
    ) -> Result<bool, DocumentError> {
        let mode = self
            .image(id)
            .ok_or(DocumentError::UnknownImage(id))?
            .mode();
        if mode == target {
            return Ok(false);
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        let current = self.image(id).ok_or(DocumentError::UnknownImage(id))?;
        let Conversion { image: mut new, .. } =
            convert_image(current, target, allowed).map_err(DocumentError::Convert)?;
        let mut template = make_template(&new).map_err(|_| DocumentError::OutOfMemory)?;
        let slot = self.image_mut(id).ok_or(DocumentError::UnknownImage(id))?;
        std::mem::swap(slot, &mut new);
        std::mem::swap(&mut self.canvas.template, &mut template);
        self.commit(Step {
            swapped: Some(Swapped {
                id,
                image: new,
                template,
            }),
            ..Step::default()
        });
        Ok(true)
    }

    /// Comandos de paleta: se aplican al lienzo y a todas las celdas con imagen en un solo
    /// paso. Todo lo que puede fallar o reservar ocurre antes de tocar nada (spec, puntos 5 y 6);
    /// después solo se aplica el paso, que no reserva ni falla.
    fn apply_palette(&mut self, id: ImageId, command: &Command) -> Result<bool, DocumentError> {
        if !self.has_cel(id) {
            return Err(DocumentError::UnknownImage(id));
        }
        if !palette_edit::check(&self.canvas.template, command)
            .map_err(DocumentError::PaletteEdit)?
        {
            return Ok(false);
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        let step = {
            let mut images: Vec<(ImageId, &Image)> = Vec::new();
            images
                .try_reserve_exact(self.layers.len().saturating_add(1))
                .map_err(|_| DocumentError::OutOfMemory)?;
            images.push((TEMPLATE, &self.canvas.template));
            images.extend(
                self.layers
                    .iter()
                    .filter_map(|layer| layer.image.as_ref().map(|image| (layer.cel, image))),
            );
            palette_edit::build(&images, command)?
        };
        if step.grows_on_redo() {
            self.reserve_palette_slots()?;
        }
        self.run_palette_step(&step, false);
        self.commit(Step {
            palette: Some(step),
            ..Step::default()
        });
        Ok(true)
    }

    /// Hace sitio en la paleta del lienzo y en la de cada celda con imagen para una entrada más
    /// (`OutOfMemory` si no hay). Lo ya reservado se queda: no estorba.
    fn reserve_palette_slots(&mut self) -> Result<(), DocumentError> {
        let reserved = self.canvas.template.reserve_palette_slot()
            && self
                .layers
                .iter_mut()
                .filter_map(|layer| layer.image.as_mut())
                .all(Image::reserve_palette_slot);
        if reserved {
            Ok(())
        } else {
            Err(DocumentError::OutOfMemory)
        }
    }

    /// Antes de sacar un paso de paleta de su pila: si deshacerlo (`undoing`) o rehacerlo
    /// agranda la paleta, reserva ese hueco. Si falla, nada ha cambiado y se puede reintentar.
    fn reserve_for_palette_step(&mut self, undoing: bool) -> Result<(), DocumentError> {
        let step = if undoing {
            self.undo.back()
        } else {
            self.redo.last()
        };
        let grows = step
            .and_then(|step| step.palette.as_ref())
            .is_some_and(|palette| {
                if undoing {
                    palette.grows_on_undo()
                } else {
                    palette.grows_on_redo()
                }
            });
        if grows {
            self.reserve_palette_slots()
        } else {
            Ok(())
        }
    }

    /// Deshace (`undoing`) o rehace el paso de paleta en el lienzo y en cada celda con imagen.
    /// No reserva ni falla.
    fn run_palette_step(&mut self, step: &PaletteStep, undoing: bool) {
        let images = std::iter::once((TEMPLATE, &mut self.canvas.template)).chain(
            self.layers
                .iter_mut()
                .filter_map(|layer| layer.image.as_mut().map(|image| (layer.cel, image))),
        );
        for (id, image) in images {
            if undoing {
                step.undo(id, image);
            } else {
                step.redo(image);
            }
        }
    }

    /// Intercambia la imagen del documento y la plantilla del lienzo con las que guarda el
    /// paso. No reserva ni falla.
    fn swap_image(&mut self, step: &mut Step) {
        if let Some(swapped) = step.swapped.as_mut()
            && let Some(slot) = self.image_mut(swapped.id)
        {
            std::mem::swap(slot, &mut swapped.image);
            std::mem::swap(&mut self.canvas.template, &mut swapped.template);
        }
    }

    pub fn undo(&mut self) -> Result<(), DocumentError> {
        if self.undo.is_empty() {
            return Err(DocumentError::NothingToUndo);
        }
        self.redo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        self.reserve_for_palette_step(true)?;
        let Some(mut step) = self.undo.pop_back() else {
            return Err(DocumentError::NothingToUndo);
        };
        self.undo_bytes = self.undo_bytes.saturating_sub(step.cost());
        if step.swapped.is_some() {
            self.swap_image(&mut step);
        } else if let Some(palette) = step.palette.as_ref() {
            self.run_palette_step(palette, true);
        } else {
            self.revert(&step);
        }
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
        self.reserve_for_palette_step(false)?;
        let Some(mut step) = self.redo.pop() else {
            return Err(DocumentError::NothingToRedo);
        };
        if step.swapped.is_some() {
            self.swap_image(&mut step);
        } else if let Some(palette) = step.palette.as_ref() {
            self.run_palette_step(palette, false);
        } else {
            self.replay(&step);
        }
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
            Command::ConvertMode { .. } => Err(DocumentError::NotBatchable),
            command if palette_edit::command_image(command).is_some() => {
                Err(DocumentError::NotBatchable)
            }
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
        if count > Document::MAX_FILL_SPANS {
            return Err(DocumentError::FillTooFragmented {
                limit: Document::MAX_FILL_SPANS,
            });
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
        let mut cursor = Cursor::default();
        let mut end = step.changes.len();
        for fill in step.fills.iter().rev() {
            for change in step.changes.get(fill.at..end).unwrap_or(&[]).iter().rev() {
                self.write(&mut cursor, change.image, change.x, change.y, change.before);
            }
            end = fill.at;
            if let Some(image) = self.image_mut(fill.image) {
                for run in fill.runs.iter().rev() {
                    write_run(image, run, run.before);
                }
            }
        }
        for change in step.changes.get(..end).unwrap_or(&[]).iter().rev() {
            self.write(&mut cursor, change.image, change.x, change.y, change.before);
        }
    }

    /// Rehace un paso: recorre la secuencia hacia delante escribiendo los valores «después».
    fn replay(&mut self, step: &Step) {
        let mut cursor = Cursor::default();
        let mut start = 0;
        for fill in &step.fills {
            for change in step.changes.get(start..fill.at).unwrap_or(&[]) {
                self.write(&mut cursor, change.image, change.x, change.y, change.after);
            }
            start = fill.at;
            if let Some(image) = self.image_mut(fill.image) {
                for run in &fill.runs {
                    write_run(image, run, fill.after);
                }
            }
        }
        for change in step.changes.get(start..).unwrap_or(&[]) {
            self.write(&mut cursor, change.image, change.x, change.y, change.after);
        }
    }

    /// Escribe un valor que salió de la propia imagen: no puede fallar, y si fallara no hay
    /// nada razonable que hacer, así que se ignora el resultado. `cursor` recuerda la capa de
    /// la última escritura: buscarla una vez por racha, no por píxel, para que deshacer no
    /// cueste más con más capas.
    fn write(&mut self, cursor: &mut Cursor, id: ImageId, x: u32, y: u32, pixel: Pixel) {
        let hit = self
            .layers
            .get(cursor.0)
            .is_some_and(|layer| layer.cel == id);
        if !hit && let Some(position) = self.layers.iter().position(|layer| layer.cel == id) {
            cursor.0 = position;
        }
        if let Some(image) = self
            .layers
            .get_mut(cursor.0)
            .filter(|layer| layer.cel == id)
            .and_then(|layer| layer.image.as_mut())
        {
            let _ = image.set_pixel(x, y, pixel);
        }
    }
}

/// Posición de la última capa escrita al deshacer o rehacer.
#[derive(Default)]
struct Cursor(usize);

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
    /// `ConvertMode` o un comando de paleta dentro de un `Batch`.
    NotBatchable,
    /// Falló un comando de edición de la paleta.
    PaletteEdit(PaletteEditError),
    /// La conversión de modo de `ConvertMode` falló.
    Convert(ConvertError),
    NothingToUndo,
    NothingToRedo,
    /// No se pudo reservar memoria para la historia.
    OutOfMemory,
    /// Un relleno tiene más de `limit` (`Document::MAX_FILL_SPANS`) tramos, o un `FillSpans`
    /// dejaría más de `limit` trozos en su paso. Se rechaza antes de reservar nada.
    FillTooFragmented {
        limit: usize,
    },
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
            DocumentError::NotBatchable => write!(
                f,
                "la conversión de modo y la edición de la paleta no pueden ir dentro de un Batch"
            ),
            DocumentError::PaletteEdit(source) => {
                write!(f, "no se pudo editar la paleta: {source}")
            }
            DocumentError::Convert(source) => write!(f, "no se pudo convertir la imagen: {source}"),
            DocumentError::NothingToUndo => write!(f, "no hay nada que deshacer"),
            DocumentError::NothingToRedo => write!(f, "no hay nada que rehacer"),
            DocumentError::OutOfMemory => {
                write!(f, "no hay memoria para guardar el paso en la historia")
            }
            DocumentError::FillTooFragmented { .. } => {
                write!(f, "relleno cancelado: región demasiado fragmentada")
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
            DocumentError::Convert(source) => Some(source),
            DocumentError::PaletteEdit(source) => Some(source),
            DocumentError::InBatch { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
