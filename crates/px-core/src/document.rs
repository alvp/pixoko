//! Documento y comandos con deshacer.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::mem::size_of;

use crate::convert::plan_cels;
use crate::palette_edit::{self, PaletteStep};
use crate::{
    Allowed, Conversion, ConvertError, Image, ImageError, Mode, Palette, PaletteEditError, Pixel,
    Rgba, convert_image,
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

impl LayerId {
    pub fn from_raw(raw: u32) -> LayerId {
        LayerId(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

/// Modo de mezcla de una capa con lo que hay debajo (D07). Los ocho modos separables de la
/// composición de `px-raster` (`compose`); las fórmulas están en `specs/hito-3-mezcla.md`.
///
/// Sin discriminantes explícitos y sin conversión a número: nada serializa el valor de Rust (los
/// códigos de un fichero se mapean con una tabla propia del formato). `#[non_exhaustive]`: el
/// hito 6 añade más modos.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Add,
    Difference,
}

impl BlendMode {
    /// Los ocho modos, en el orden de la declaración.
    pub const ALL: [BlendMode; 8] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::Add,
        BlendMode::Difference,
    ];
}

/// Una capa: sus propiedades y su celda, que es la imagen `image` (`None` = celda vacía, sin
/// píxeles reservados; vale transparente). `cel` es el `ImageId` de la celda y existe desde que
/// se crea la capa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    id: LayerId,
    name: String,
    visible: bool,
    opacity: u8,
    blend: BlendMode,
    locked: bool,
    cel: ImageId,
    image: Option<Image>,
}

impl Layer {
    pub fn id(&self) -> LayerId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Opacidad de 0 (transparente) a 255 (opaca); escala el alfa de la capa al componerla (fórmulas
    /// en `specs/hito-3-mezcla.md`). En indexado siempre es 255.
    pub fn opacity(&self) -> u8 {
        self.opacity
    }

    /// Modo de mezcla con lo que hay debajo. Las capas nuevas son `Normal`; en indexado siempre
    /// lo es.
    pub fn blend(&self) -> BlendMode {
        self.blend
    }

    /// Bloqueada: las herramientas de dibujo no la pintan (lo aplica la interfaz; el núcleo no
    /// impide escribir en una celda bloqueada, y los cambios de paleta y de modo la afectan).
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// El `ImageId` de la celda de la capa; su imagen se pide con `Document::image`.
    pub fn cel(&self) -> ImageId {
        self.cel
    }

    /// Bytes de píxeles de la celda si tiene imagen, y 0 si está vacía.
    fn content_bytes(&self) -> usize {
        self.image.as_ref().map_or(0, Image::content_bytes)
    }
}

/// Una capa para `Document::from_layers`: sus propiedades y la imagen de su celda (`None` =
/// celda vacía). No lleva ids: los reparte el documento.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerSpec {
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
    pub blend: BlendMode,
    pub locked: bool,
    pub image: Option<Image>,
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
    /// está en ese modo no hace nada. Con varias capas se usa `ConvertAll`. Solo como comando suelto: dentro de un `Batch` es
    /// `NotBatchable`.
    ConvertMode {
        image: ImageId,
        target: Mode,
        allowed: Allowed,
    },
    /// Añade una capa visible, de opacidad 255 y con la celda vacía en la posición `at`
    /// (`0..=capas`; 0 es el fondo y `capas`, encima de todas). Solo como comando suelto.
    AddLayer { name: String, at: usize },
    /// Quita la capa con su celda. No se puede quitar la única. Solo como comando suelto.
    RemoveLayer { layer: LayerId },
    /// Mueve la capa a la posición `to` (`0..capas`, la posición final). Solo como comando
    /// suelto.
    MoveLayer { layer: LayerId, to: usize },
    /// Cambia el nombre de la capa (hasta `Document::MAX_LAYER_NAME_BYTES` bytes). Solo como
    /// comando suelto.
    RenameLayer { layer: LayerId, name: String },
    /// Cambia la opacidad de la capa (0 a 255; en indexado solo vale 255). Solo como comando
    /// suelto.
    SetLayerOpacity { layer: LayerId, opacity: u8 },
    /// Muestra u oculta la capa. Solo como comando suelto.
    SetLayerVisible { layer: LayerId, visible: bool },
    /// Cambia el modo de mezcla de la capa (en indexado solo vale `Normal`). Solo como comando
    /// suelto.
    SetLayerBlend { layer: LayerId, blend: BlendMode },
    /// Bloquea o desbloquea la capa. Solo como comando suelto.
    SetLayerLocked { layer: LayerId, locked: bool },
    /// Copia la capa justo encima de ella (posición + 1) con las mismas propiedades, el bloqueo
    /// incluido, y el nombre `name`. La celda con imagen se copia; la vacía sigue vacía. Solo como
    /// comando suelto.
    DuplicateLayer { layer: LayerId, name: String },
    /// Une capas en una sola (specs/hito-3-combinar.md): quita las capas de `remove` (se mueven al
    /// paso, sin copiar), pone `image` como imagen de la celda de `target` (`None`: celda vacía) y
    /// deja a `target` con opacidad 255, mezcla `Normal` y desbloqueada; su id, el de su celda,
    /// su nombre y su visibilidad no cambian. La imagen la calcula quien llama (la composición,
    /// en `px-raster`) sobre el estado `based_on` (`state_id`): si el documento ya cambió, es
    /// `StaleImage`. Un solo paso. Solo como comando suelto.
    MergeLayers {
        target: LayerId,
        remove: Vec<LayerId>,
        image: Option<Image>,
        based_on: u64,
    },
    /// Convierte el modo y la paleta del lienzo y **todas** las celdas con imagen en un solo
    /// paso, con una paleta común si el destino es indexado (`convert_cels`). Las celdas vacías
    /// siguen vacías. Con una sola capa equivale a `ConvertMode`; con varias, `ConvertMode`
    /// sigue dando `MultiLayerConvert`. Solo como comando suelto.
    ConvertAll { target: Mode, allowed: Allowed },
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

/// Lo que costaría una combinación (`Document::plan_merge`), calculado sin tocar nada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MergePlan {
    /// Bytes de píxeles de las celdas de `target` y de `remove`: lo que irá al paso de historia.
    pub stored_bytes: usize,
    /// Ninguna de esas celdas tiene imagen.
    pub all_empty: bool,
    /// `materialized_bytes()` más una celda: el pico mientras viven las viejas y la nueva.
    pub peak_bytes: usize,
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
    /// En un paso de `ConvertAll`, las imágenes de todas las celdas que no están en el documento
    /// y la plantilla del lienzo del otro lado; `undo` y `redo` las intercambian.
    converted: Option<Converted>,
    /// En un paso de edición de paleta, el paso (que es todo su contenido).
    palette: Option<PaletteStep>,
    /// En un paso de comando de capa, el paso (que es todo su contenido).
    layer: Option<LayerStep>,
    /// Celdas que estaban vacías y este paso materializó: deshacer las vacía y rehacer las
    /// reserva otra vez.
    materialized: Vec<ImageId>,
    /// Identificador de estado del otro lado del paso: en la pila de deshacer, el de antes del
    /// paso; en la de rehacer, el de después. `undo` y `redo` lo intercambian con el actual.
    state: u64,
}

/// Paso de un comando de capa.
#[derive(Clone, Debug)]
enum LayerStep {
    /// Una capa entra en la posición `at` (`added`) o sale de ella; `stored` la guarda mientras
    /// está fuera del documento.
    Presence {
        at: usize,
        added: bool,
        stored: Option<Layer>,
    },
    /// La capa pasó de la posición `from` a la `to`.
    Move { from: usize, to: usize },
    /// El nombre del otro lado: aplicar y deshacer lo intercambian con el de la capa.
    Name { layer: LayerId, other: String },
    Opacity {
        layer: LayerId,
        before: u8,
        after: u8,
    },
    Visible {
        layer: LayerId,
        before: bool,
        after: bool,
    },
    Blend {
        layer: LayerId,
        before: BlendMode,
        after: BlendMode,
    },
    Locked {
        layer: LayerId,
        before: bool,
        after: bool,
    },
    /// `MergeLayers`: `positions` son las posiciones (ascendentes, de antes de combinar) de las
    /// capas quitadas; `removed` las guarda mientras están fuera del documento (vacío si están
    /// dentro). `image` y `props` (opacidad, mezcla y bloqueo) son los del otro lado de `target`.
    Merge {
        target: LayerId,
        positions: Vec<usize>,
        removed: Vec<Layer>,
        image: Option<Image>,
        props: (u8, BlendMode, bool),
    },
}

impl LayerStep {
    /// Cuántas capas mete en el documento deshacerlo (`undoing`) o rehacerlo.
    fn inserts(&self, undoing: bool) -> usize {
        match self {
            LayerStep::Presence { added, .. } => usize::from(*added != undoing),
            LayerStep::Merge { positions, .. } if undoing => positions.len(),
            _ => 0,
        }
    }

    fn cost(&self) -> usize {
        match self {
            LayerStep::Presence { stored, .. } => stored.as_ref().map_or(0, |layer| {
                layer.name.capacity().saturating_add(layer.content_bytes())
            }),
            LayerStep::Name { other, .. } => other.capacity(),
            LayerStep::Merge {
                positions,
                removed,
                image,
                ..
            } => {
                let layers = removed.iter().fold(0usize, |total, layer| {
                    total
                        .saturating_add(layer.name.capacity())
                        .saturating_add(layer.content_bytes())
                });
                layers
                    .saturating_add(image.as_ref().map_or(0, Image::content_bytes))
                    .saturating_add(positions.capacity().saturating_mul(size_of::<usize>()))
            }
            _ => 0,
        }
        .saturating_add(size_of::<LayerStep>())
    }
}

/// Lo que intercambia un paso de `ConvertAll`: la imagen de cada celda y la plantilla del lienzo.
#[derive(Clone, Debug)]
struct Converted {
    cels: Vec<(ImageId, Image)>,
    template: Image,
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
        let converted = self.converted.as_ref().map_or(0, |converted| {
            converted.cels.iter().fold(0usize, |total, (_, image)| {
                total
                    .saturating_add(size_of::<Image>())
                    .saturating_add(image.content_bytes())
            })
        });
        let palette = self.palette.as_ref().map_or(0, PaletteStep::cost);
        let layer = self.layer.as_ref().map_or(0, LayerStep::cost);
        let materialized = self
            .materialized
            .capacity()
            .saturating_mul(size_of::<ImageId>());
        size_of::<Vec<Change>>()
            .saturating_add(self.changes.capacity().saturating_mul(size_of::<Change>()))
            .saturating_add(self.fills.capacity().saturating_mul(size_of::<FillPart>()))
            .saturating_add(runs)
            .saturating_add(swapped)
            .saturating_add(converted)
            .saturating_add(palette)
            .saturating_add(layer)
            .saturating_add(materialized)
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
        if self.materialized.capacity() != self.materialized.len() {
            let mut exact = Vec::new();
            if exact.try_reserve_exact(self.materialized.len()).is_ok() {
                exact.append(&mut self.materialized);
                self.materialized = exact;
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
    /// Siguientes `LayerId` e `ImageId` de celda por repartir; solo crecen (no se reutilizan).
    next_layer: u32,
    next_image: u32,
    /// Tope de `materialized_bytes()`.
    cells_limit: usize,
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

    /// Número máximo de capas.
    pub const MAX_LAYERS: usize = 256;

    /// Longitud máxima del nombre de una capa, en bytes UTF-8.
    pub const MAX_LAYER_NAME_BYTES: usize = 255;

    /// Tope por defecto de `materialized_bytes()` (2 GiB).
    pub const DEFAULT_CELLS_LIMIT: usize = 2 << 30;

    /// Umbral del aviso de presupuesto (1 GiB): la interfaz avisa si
    /// `worst_case_bytes() + cel_bytes()` lo supera antes de añadir una capa. El núcleo no lo
    /// aplica.
    pub const MEMORY_WARN_BYTES: usize = 1 << 30;

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
                blend: BlendMode::Normal,
                locked: false,
                cel: ImageId(0),
                image: Some(image),
            }],
            undo: VecDeque::new(),
            redo: Vec::new(),
            undo_bytes: 0,
            limit: Document::DEFAULT_HISTORY_LIMIT,
            current_id: 0,
            next_id: 1,
            next_layer: 1,
            next_image: 1,
            cells_limit: Document::DEFAULT_CELLS_LIMIT,
        }
    }

    /// Documento con las capas dadas (de abajo arriba), sin historia: ids `LayerId(i)` e
    /// `ImageId(i)` por posición, `state_id` 0 (specs/hito-3-aseprite.md, `C-02`). Es el camino de
    /// entrada de los lectores de formatos con capas y **valida todas las invariantes** antes de
    /// construir nada: de 1 a `MAX_LAYERS` capas, nombres de hasta `MAX_LAYER_NAME_BYTES` bytes,
    /// toda celda con imagen del ancho, alto, modo y paleta del lienzo, indexado con opacidad 255 y
    /// mezcla `Normal`, celdas vacías solo si el modo tiene valor transparente, y celdas con
    /// píxeles dentro de `DEFAULT_CELLS_LIMIT`.
    ///
    /// `palette` es `Some` (y no vacía) solo en indexado. Un lienzo que no puede existir (ancho o
    /// alto cero, o demasiado grande) y una paleta que no corresponde al modo dan
    /// `InvalidCanvas`.
    pub fn from_layers(
        width: u32,
        height: u32,
        mode: Mode,
        palette: Option<Palette>,
        layers: Vec<LayerSpec>,
    ) -> Result<Document, DocumentError> {
        if layers.is_empty() {
            return Err(DocumentError::NoLayers);
        }
        if layers.len() > Document::MAX_LAYERS {
            return Err(DocumentError::TooManyLayers {
                limit: Document::MAX_LAYERS,
            });
        }
        let cel_bytes =
            Image::byte_len(width, height, mode).map_err(|_| DocumentError::InvalidCanvas)?;
        let template = match (mode, &palette) {
            (Mode::Indexed, Some(palette)) if !palette.is_empty() => {
                Image::filled_indexed(1, 1, palette.clone(), 0)
            }
            (Mode::Indexed, _) | (_, Some(_)) => return Err(DocumentError::InvalidCanvas),
            (mode, None) => Image::new(1, 1, mode),
        }
        .map_err(|_| DocumentError::InvalidCanvas)?;
        let transparent = match mode {
            Mode::Rgba | Mode::GrayAlpha => true,
            Mode::Indexed => palette
                .as_ref()
                .is_some_and(|palette| palette.transparent_index().is_some()),
            Mode::Gray => false,
        };
        let mut built = Vec::new();
        built
            .try_reserve_exact(layers.len())
            .map_err(|_| DocumentError::OutOfMemory)?;
        let mut materialized = 0usize;
        for (i, spec) in layers.into_iter().enumerate() {
            if spec.name.len() > Document::MAX_LAYER_NAME_BYTES {
                return Err(DocumentError::LayerName {
                    len: spec.name.len(),
                    limit: Document::MAX_LAYER_NAME_BYTES,
                });
            }
            if mode == Mode::Indexed {
                if spec.opacity != 255 {
                    return Err(DocumentError::IndexedOpacity);
                }
                if spec.blend != BlendMode::Normal {
                    return Err(DocumentError::IndexedBlend);
                }
            }
            match &spec.image {
                Some(image) => {
                    if image.width() != width
                        || image.height() != height
                        || image.mode() != mode
                        || image.palette() != palette.as_ref()
                    {
                        return Err(DocumentError::CelMismatch { layer: i });
                    }
                    materialized = materialized.saturating_add(cel_bytes);
                    if materialized > Document::DEFAULT_CELLS_LIMIT {
                        return Err(DocumentError::CelsTooLarge {
                            needed: materialized,
                            limit: Document::DEFAULT_CELLS_LIMIT,
                        });
                    }
                }
                None if !transparent => return Err(DocumentError::NoTransparency { mode }),
                None => {}
            }
            let id = u32::try_from(i).map_err(|_| DocumentError::TooManyLayers {
                limit: Document::MAX_LAYERS,
            })?;
            built.push(Layer {
                id: LayerId(id),
                name: spec.name,
                visible: spec.visible,
                opacity: spec.opacity,
                blend: spec.blend,
                locked: spec.locked,
                cel: ImageId(id),
                image: spec.image,
            });
        }
        let count = built.len() as u32;
        Ok(Document {
            canvas: Canvas {
                width,
                height,
                template,
            },
            layers: built,
            undo: VecDeque::new(),
            redo: Vec::new(),
            undo_bytes: 0,
            limit: Document::DEFAULT_HISTORY_LIMIT,
            current_id: 0,
            next_id: 1,
            next_layer: count,
            next_image: count,
            cells_limit: Document::DEFAULT_CELLS_LIMIT,
        })
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

    /// El id de la celda de la capa con la que se creó el documento (siempre `ImageId(0)`).
    ///
    /// **Obsoleta**: es un atajo para pruebas y documentos de una capa. El código de la
    /// aplicación usa la celda de la capa activa. Con varias capas, y si se quitó la capa 0,
    /// `image(main_image())` es `None`.
    ///
    /// TODO(limpieza): retirarla en la unidad de limpiezas (`specs/mapa-funciones.md`, sección 11).
    pub fn main_image(&self) -> ImageId {
        ImageId(0)
    }

    /// Las capas, de abajo a arriba: la posición 0 es el fondo y la última, la de encima.
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id == id)
    }

    /// Una imagen de 1 × 1 con el modo y la paleta del lienzo: de ahí salen el modo y la paleta
    /// cuando la celda que se mira está vacía. No es ninguna celda (`image(id)` nunca la
    /// devuelve) y sus píxeles no significan nada.
    pub fn canvas_image(&self) -> &Image {
        &self.canvas.template
    }

    /// Ancho del lienzo, y de toda celda.
    pub fn width(&self) -> u32 {
        self.canvas.width
    }

    /// Alto del lienzo, y de toda celda.
    pub fn height(&self) -> u32 {
        self.canvas.height
    }

    /// Modo del lienzo, y de toda celda.
    pub fn mode(&self) -> Mode {
        self.canvas.template.mode()
    }

    /// Paleta del lienzo (`Some` solo en indexado), idéntica a la de toda celda con imagen.
    pub fn palette(&self) -> Option<&Palette> {
        self.canvas.template.palette()
    }

    /// Bytes de píxeles de una celda (sin la paleta).
    pub fn cel_bytes(&self) -> usize {
        Image::byte_len(self.canvas.width, self.canvas.height, self.mode()).unwrap_or(usize::MAX)
    }

    /// Bytes de píxeles de las celdas con imagen (sin las paletas). No cuenta la historia.
    pub fn materialized_bytes(&self) -> usize {
        self.layers.iter().fold(0usize, |total, layer| {
            total.saturating_add(layer.image.as_ref().map_or(0, |image| image.bytes().len()))
        })
    }

    /// Lo que ocuparía el documento con todas las celdas materializadas.
    pub fn worst_case_bytes(&self) -> usize {
        self.layers.len().saturating_mul(self.cel_bytes())
    }

    pub fn cells_limit(&self) -> usize {
        self.cells_limit
    }

    /// Cambia el tope de celdas materializadas. No descarta nada: solo impide materializar más.
    pub fn set_cells_limit(&mut self, bytes: usize) {
        self.cells_limit = bytes;
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
        match command {
            Command::AddLayer { name, at } => return self.apply_add_layer(name, at),
            Command::RemoveLayer { layer } => return self.apply_remove_layer(layer),
            Command::MoveLayer { layer, to } => return self.apply_move_layer(layer, to),
            Command::RenameLayer { layer, name } => return self.apply_rename_layer(layer, name),
            Command::SetLayerOpacity { layer, opacity } => {
                return self.apply_layer_opacity(layer, opacity);
            }
            Command::SetLayerVisible { layer, visible } => {
                return self.apply_layer_visible(layer, visible);
            }
            Command::SetLayerBlend { layer, blend } => {
                return self.apply_layer_blend(layer, blend);
            }
            Command::SetLayerLocked { layer, locked } => {
                return self.apply_layer_locked(layer, locked);
            }
            Command::DuplicateLayer { layer, name } => {
                return self.apply_duplicate_layer(layer, name);
            }
            Command::MergeLayers {
                target,
                remove,
                image,
                based_on,
            } => return self.apply_merge(target, &remove, image, based_on),
            Command::ConvertAll { target, allowed } => {
                return self.apply_convert_all(target, allowed);
            }
            _ => {}
        }
        // Lo que tiene cota se reserva antes de modificar nada; los trozos de cada relleno,
        // justo antes de escribirlo (ver `execute_fill`).
        let (writes, fills) = count_leaves(&command)?;
        // Las celdas vacías que el comando nombra se materializan antes de ejecutarlo. El límite
        // y la transparencia se comprueban primero, sin reservar nada.
        let targets = self.empty_targets(&command)?;
        self.check_materialize(targets.len())?;
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
        let fresh = self.allocate_cels(targets.len())?;
        self.install_cels(&targets, fresh);

        if let Err(error) = self.execute(&command, &mut step) {
            self.revert(&step);
            self.release_cels(&targets);
            return Err(error);
        }
        if step.changes.is_empty() && step.fills.is_empty() {
            self.release_cels(&targets);
            return Ok(false);
        }
        step.materialized = targets;
        step.fit();
        self.commit(step);
        Ok(true)
    }

    /// `true` si una celda vacía puede escribirse: el modo tiene un valor transparente.
    fn has_transparency(&self) -> bool {
        match self.canvas.template.mode() {
            Mode::Rgba | Mode::GrayAlpha => true,
            Mode::Indexed => self
                .canvas
                .template
                .palette()
                .is_some_and(|palette| palette.transparent_index().is_some()),
            Mode::Gray => false,
        }
    }

    fn no_transparency(&self) -> DocumentError {
        DocumentError::NoTransparency { mode: self.mode() }
    }

    /// Las celdas vacías que nombran los `SetPixels` y `FillSpans` del comando, sin repetir. Un
    /// id que no es de ninguna celda se ignora aquí: falla después, con `UnknownImage`.
    fn empty_targets(&self, command: &Command) -> Result<Vec<ImageId>, DocumentError> {
        let mut targets: Vec<ImageId> = Vec::new();
        let leaves = match command {
            Command::Batch(commands) => commands.as_slice(),
            leaf => std::slice::from_ref(leaf),
        };
        for leaf in leaves {
            let (Command::SetPixels { image, .. } | Command::FillSpans { image, .. }) = leaf else {
                continue;
            };
            let empty = self
                .layers
                .iter()
                .any(|layer| layer.cel == *image && layer.image.is_none());
            if empty && !targets.contains(image) {
                targets
                    .try_reserve(1)
                    .map_err(|_| DocumentError::OutOfMemory)?;
                targets.push(*image);
            }
        }
        Ok(targets)
    }

    /// Antes de materializar `count` celdas: que haya valor transparente y que quepan bajo el
    /// límite. Es solo aritmética: no reserva nada y no toca nada.
    fn check_materialize(&self, count: usize) -> Result<(), DocumentError> {
        if count == 0 {
            return Ok(());
        }
        if !self.has_transparency() {
            return Err(self.no_transparency());
        }
        let needed = self
            .materialized_bytes()
            .saturating_add(self.cel_bytes().saturating_mul(count));
        if needed > self.cells_limit {
            return Err(DocumentError::CelsTooLarge {
                needed,
                limit: self.cells_limit,
            });
        }
        Ok(())
    }

    /// Una celda transparente del tamaño y el modo del lienzo.
    fn blank_cel(&self) -> Result<Image, DocumentError> {
        let template = &self.canvas.template;
        let (width, height) = (self.canvas.width, self.canvas.height);
        let image = match (template.mode(), template.palette()) {
            (Mode::Indexed, Some(palette)) => {
                let index = palette
                    .transparent_index()
                    .ok_or_else(|| self.no_transparency())?;
                Image::filled_indexed(width, height, palette.clone(), index)
            }
            (Mode::Rgba | Mode::GrayAlpha, _) => Image::new(width, height, template.mode()),
            _ => return Err(self.no_transparency()),
        };
        image.map_err(|_| DocumentError::OutOfMemory)
    }

    /// Reserva `count` celdas transparentes; `OutOfMemory` si no caben.
    fn allocate_cels(&self, count: usize) -> Result<Vec<Image>, DocumentError> {
        let mut cels = Vec::new();
        cels.try_reserve_exact(count)
            .map_err(|_| DocumentError::OutOfMemory)?;
        for _ in 0..count {
            cels.push(self.blank_cel()?);
        }
        Ok(cels)
    }

    /// Pone cada imagen en la celda `ids[i]`. No reserva ni falla.
    fn install_cels(&mut self, ids: &[ImageId], cels: Vec<Image>) {
        for (id, image) in ids.iter().zip(cels) {
            if let Some(layer) = self.layers.iter_mut().find(|layer| layer.cel == *id) {
                layer.image = Some(image);
            }
        }
    }

    /// Vacía las celdas `ids` (libera su memoria).
    fn release_cels(&mut self, ids: &[ImageId]) {
        for id in ids {
            if let Some(layer) = self.layers.iter_mut().find(|layer| layer.cel == *id) {
                layer.image = None;
            }
        }
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
        if self.layers.len() > 1 {
            return Err(DocumentError::MultiLayerConvert {
                layers: self.layers.len(),
            });
        }
        if target == Mode::Indexed && self.layers.iter().any(|layer| layer.opacity != 255) {
            return Err(DocumentError::IndexedOpacity);
        }
        if target == Mode::Indexed
            && self
                .layers
                .iter()
                .any(|layer| layer.blend != BlendMode::Normal)
        {
            return Err(DocumentError::IndexedBlend);
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

    /// Posiciones de `target` y de `remove` (esta, ascendente), validando capas y conjunto.
    fn merge_positions(
        &self,
        target: LayerId,
        remove: &[LayerId],
    ) -> Result<(usize, Vec<usize>), DocumentError> {
        let target_at = self.layer_position(target)?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(remove.len())
            .map_err(|_| DocumentError::OutOfMemory)?;
        for id in remove {
            positions.push(self.layer_position(*id)?);
        }
        positions.sort_unstable();
        if positions.is_empty()
            || positions.contains(&target_at)
            || positions.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(DocumentError::InvalidMerge);
        }
        Ok((target_at, positions))
    }

    /// Lo que costaría combinar `remove` en `target`, sin reservar ni tocar nada. Los mismos
    /// errores de capa que `MergeLayers`: `UnknownLayer` (primero `target`, luego `remove` en
    /// orden) e `InvalidMerge`.
    pub fn plan_merge(
        &self,
        target: LayerId,
        remove: &[LayerId],
    ) -> Result<MergePlan, DocumentError> {
        let (target_at, positions) = self.merge_positions(target, remove)?;
        let cell_bytes = |at: usize| {
            self.layers
                .get(at)
                .and_then(|layer| layer.image.as_ref())
                .map(|image| image.bytes().len())
        };
        let mut stored = 0usize;
        let mut all_empty = true;
        for at in std::iter::once(target_at).chain(positions.iter().copied()) {
            if let Some(bytes) = cell_bytes(at) {
                stored = stored.saturating_add(bytes);
                all_empty = false;
            }
        }
        Ok(MergePlan {
            stored_bytes: stored,
            all_empty,
            peak_bytes: self.materialized_bytes().saturating_add(self.cel_bytes()),
        })
    }

    /// `MergeLayers`. Orden: capa desconocida, conjunto inválido, imagen vieja, imagen de otro
    /// lienzo, tope de celdas (solo el resultado, y solo si crece) y memoria; nada se modifica
    /// antes de que todas pasen.
    fn apply_merge(
        &mut self,
        target: LayerId,
        remove: &[LayerId],
        image: Option<Image>,
        based_on: u64,
    ) -> Result<bool, DocumentError> {
        let plan = self.plan_merge(target, remove)?;
        let (target_at, positions) = self.merge_positions(target, remove)?;
        if based_on != self.current_id {
            return Err(DocumentError::StaleImage {
                based_on,
                current: self.current_id,
            });
        }
        if let Some(image) = &image {
            let fits = image.width() == self.canvas.width
                && image.height() == self.canvas.height
                && image.mode() == self.canvas.template.mode()
                && image.palette() == self.canvas.template.palette();
            if !fits {
                return Err(DocumentError::MergeImageMismatch);
            }
        }
        let new_bytes = image.as_ref().map_or(0, |image| image.bytes().len());
        let before = self.materialized_bytes();
        let needed = before
            .saturating_sub(plan.stored_bytes)
            .saturating_add(new_bytes);
        if needed > self.cells_limit && needed > before {
            return Err(DocumentError::CelsTooLarge {
                needed,
                limit: self.cells_limit,
            });
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        let mut removed = Vec::new();
        removed
            .try_reserve_exact(positions.len())
            .map_err(|_| DocumentError::OutOfMemory)?;
        // Hasta aquí nada ha cambiado; desde aquí no se reserva ni se falla.
        for at in positions.iter().rev() {
            removed.push(self.layers.remove(*at));
        }
        removed.reverse();
        let target_now = self
            .layers
            .iter()
            .position(|layer| layer.id == target)
            .unwrap_or(target_at);
        let Some(layer) = self.layers.get_mut(target_now) else {
            return Ok(false);
        };
        let old_image = std::mem::replace(&mut layer.image, image);
        let props = (
            std::mem::replace(&mut layer.opacity, 255),
            std::mem::replace(&mut layer.blend, BlendMode::Normal),
            std::mem::replace(&mut layer.locked, false),
        );
        self.commit(Step {
            layer: Some(LayerStep::Merge {
                target,
                positions,
                removed,
                image: old_image,
                props,
            }),
            ..Step::default()
        });
        Ok(true)
    }

    /// `ConvertAll`: todo lo que puede fallar o reservar ocurre antes de tocar el documento;
    /// después solo se intercambian imágenes.
    fn apply_convert_all(&mut self, target: Mode, allowed: Allowed) -> Result<bool, DocumentError> {
        let source = self.mode();
        if source == target {
            return Ok(false);
        }
        if target == Mode::Gray && self.layers.len() > 1 {
            return Err(DocumentError::NoTransparency { mode: Mode::Gray });
        }
        if target == Mode::Indexed {
            if self.layers.iter().any(|layer| layer.opacity != 255) {
                return Err(DocumentError::IndexedOpacity);
            }
            if self
                .layers
                .iter()
                .any(|layer| layer.blend != BlendMode::Normal)
            {
                return Err(DocumentError::IndexedBlend);
            }
        }
        let mut cels: Vec<Option<&Image>> = Vec::new();
        cels.try_reserve_exact(self.layers.len())
            .map_err(|_| DocumentError::OutOfMemory)?;
        cels.extend(self.layers.iter().map(|layer| layer.image.as_ref()));
        let plan = plan_cels(
            &cels,
            (self.canvas.width, self.canvas.height),
            source,
            target,
            allowed,
        )
        .map_err(DocumentError::Convert)?;
        let needed = plan.total_bytes();
        if needed > self.cells_limit {
            return Err(DocumentError::CelsTooLarge {
                needed,
                limit: self.cells_limit,
            });
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        let palette = plan.palette().map_err(DocumentError::Convert)?;
        let converted = plan.build().map_err(DocumentError::Convert)?;
        let mut template = match palette {
            Some(palette) => Image::filled_indexed(1, 1, palette, 0),
            None => Image::new(1, 1, target),
        }
        .map_err(|_| DocumentError::OutOfMemory)?;
        let mut swaps = Vec::new();
        swaps
            .try_reserve_exact(converted.images.len())
            .map_err(|_| DocumentError::OutOfMemory)?;
        // Hasta aquí nada ha cambiado; desde aquí solo se intercambia.
        for (layer, new) in self.layers.iter_mut().zip(converted.images) {
            if let (Some(slot), Some(mut new)) = (layer.image.as_mut(), new) {
                std::mem::swap(slot, &mut new);
                swaps.push((layer.cel, new));
            }
        }
        std::mem::swap(&mut self.canvas.template, &mut template);
        self.commit(Step {
            converted: Some(Converted {
                cels: swaps,
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

    /// `AddLayer`. Orden de comprobación: nombre, transparencia, número de capas y posición.
    fn apply_add_layer(&mut self, name: String, at: usize) -> Result<bool, DocumentError> {
        if name.len() > Document::MAX_LAYER_NAME_BYTES {
            return Err(DocumentError::LayerName {
                len: name.len(),
                limit: Document::MAX_LAYER_NAME_BYTES,
            });
        }
        if !self.has_transparency() {
            return Err(self.no_transparency());
        }
        let too_many = DocumentError::TooManyLayers {
            limit: Document::MAX_LAYERS,
        };
        if self.layers.len() >= Document::MAX_LAYERS {
            return Err(too_many);
        }
        if at > self.layers.len() {
            return Err(DocumentError::LayerIndex {
                index: at,
                max: self.layers.len(),
            });
        }
        // El id `u32::MAX` de las celdas queda reservado (la plantilla del lienzo).
        let (id, cel) = match (
            self.next_layer.checked_add(1),
            self.next_image.checked_add(1),
        ) {
            (Some(_), Some(next)) if next < u32::MAX => (self.next_layer, self.next_image),
            _ => return Err(too_many),
        };
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        self.layers
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        self.next_layer = id.saturating_add(1);
        self.next_image = cel.saturating_add(1);
        self.layers.insert(
            at,
            Layer {
                id: LayerId(id),
                name,
                visible: true,
                opacity: 255,
                blend: BlendMode::Normal,
                locked: false,
                cel: ImageId(cel),
                image: None,
            },
        );
        self.commit(Step {
            layer: Some(LayerStep::Presence {
                at,
                added: true,
                stored: None,
            }),
            ..Step::default()
        });
        Ok(true)
    }

    /// `RemoveLayer`: la capa, con su celda, pasa al paso (se mueve, no se copia).
    fn apply_remove_layer(&mut self, id: LayerId) -> Result<bool, DocumentError> {
        let at = self
            .layers
            .iter()
            .position(|layer| layer.id == id)
            .ok_or(DocumentError::UnknownLayer(id))?;
        if self.layers.len() == 1 {
            return Err(DocumentError::LastLayer);
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        let stored = self.layers.remove(at);
        self.commit(Step {
            layer: Some(LayerStep::Presence {
                at,
                added: false,
                stored: Some(stored),
            }),
            ..Step::default()
        });
        Ok(true)
    }

    fn layer_position(&self, id: LayerId) -> Result<usize, DocumentError> {
        self.layers
            .iter()
            .position(|layer| layer.id == id)
            .ok_or(DocumentError::UnknownLayer(id))
    }

    /// `MoveLayer`: `to` es la posición final.
    fn apply_move_layer(&mut self, id: LayerId, to: usize) -> Result<bool, DocumentError> {
        let from = self.layer_position(id)?;
        if to >= self.layers.len() {
            return Err(DocumentError::LayerIndex {
                index: to,
                max: self.layers.len().saturating_sub(1),
            });
        }
        if from == to {
            return Ok(false);
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        self.move_layer(from, to);
        self.commit(Step {
            layer: Some(LayerStep::Move { from, to }),
            ..Step::default()
        });
        Ok(true)
    }

    /// Lleva la capa de `from` a `to`; las de en medio se desplazan un lugar. No reserva.
    fn move_layer(&mut self, from: usize, to: usize) {
        if from < to {
            if let Some(range) = self.layers.get_mut(from..=to) {
                range.rotate_left(1);
            }
        } else if let Some(range) = self.layers.get_mut(to..=from) {
            range.rotate_right(1);
        }
    }

    /// `RenameLayer`: el nombre viejo pasa al paso, sin copiar ni reservar.
    fn apply_rename_layer(&mut self, id: LayerId, name: String) -> Result<bool, DocumentError> {
        let at = self.layer_position(id)?;
        if name.len() > Document::MAX_LAYER_NAME_BYTES {
            return Err(DocumentError::LayerName {
                len: name.len(),
                limit: Document::MAX_LAYER_NAME_BYTES,
            });
        }
        if self.layers.get(at).is_none_or(|layer| layer.name == name) {
            return Ok(false);
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        let Some(layer) = self.layers.get_mut(at) else {
            return Ok(false);
        };
        let other = std::mem::replace(&mut layer.name, name);
        self.commit(Step {
            layer: Some(LayerStep::Name { layer: id, other }),
            ..Step::default()
        });
        Ok(true)
    }

    /// `SetLayerOpacity`: en indexado solo vale 255.
    fn apply_layer_opacity(&mut self, id: LayerId, opacity: u8) -> Result<bool, DocumentError> {
        let at = self.layer_position(id)?;
        if self.mode() == Mode::Indexed && opacity != 255 {
            return Err(DocumentError::IndexedOpacity);
        }
        let Some(before) = self.layers.get(at).map(|layer| layer.opacity) else {
            return Ok(false);
        };
        if before == opacity {
            return Ok(false);
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        if let Some(layer) = self.layers.get_mut(at) {
            layer.opacity = opacity;
        }
        self.commit(Step {
            layer: Some(LayerStep::Opacity {
                layer: id,
                before,
                after: opacity,
            }),
            ..Step::default()
        });
        Ok(true)
    }

    /// `SetLayerBlend`: en indexado solo vale `Normal`. Orden de comprobación: capa desconocida,
    /// modo en indexado y, por último, «sin cambios».
    fn apply_layer_blend(&mut self, id: LayerId, blend: BlendMode) -> Result<bool, DocumentError> {
        let at = self.layer_position(id)?;
        if self.mode() == Mode::Indexed && blend != BlendMode::Normal {
            return Err(DocumentError::IndexedBlend);
        }
        let Some(before) = self.layers.get(at).map(|layer| layer.blend) else {
            return Ok(false);
        };
        if before == blend {
            return Ok(false);
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        if let Some(layer) = self.layers.get_mut(at) {
            layer.blend = blend;
        }
        self.commit(Step {
            layer: Some(LayerStep::Blend {
                layer: id,
                before,
                after: blend,
            }),
            ..Step::default()
        });
        Ok(true)
    }

    /// `SetLayerLocked`.
    fn apply_layer_locked(&mut self, id: LayerId, locked: bool) -> Result<bool, DocumentError> {
        let at = self.layer_position(id)?;
        let Some(before) = self.layers.get(at).map(|layer| layer.locked) else {
            return Ok(false);
        };
        if before == locked {
            return Ok(false);
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        if let Some(layer) = self.layers.get_mut(at) {
            layer.locked = locked;
        }
        self.commit(Step {
            layer: Some(LayerStep::Locked {
                layer: id,
                before,
                after: locked,
            }),
            ..Step::default()
        });
        Ok(true)
    }

    /// `DuplicateLayer`: copia exacta (propiedades y celda) justo encima de la original. Orden de
    /// comprobación: capa desconocida, nombre, número de capas, límite de celdas (solo si la celda
    /// tiene imagen) y memoria; ningún error reserva ni modifica nada.
    fn apply_duplicate_layer(&mut self, id: LayerId, name: String) -> Result<bool, DocumentError> {
        let at = self.layer_position(id)?;
        if name.len() > Document::MAX_LAYER_NAME_BYTES {
            return Err(DocumentError::LayerName {
                len: name.len(),
                limit: Document::MAX_LAYER_NAME_BYTES,
            });
        }
        let too_many = DocumentError::TooManyLayers {
            limit: Document::MAX_LAYERS,
        };
        if self.layers.len() >= Document::MAX_LAYERS {
            return Err(too_many);
        }
        let has_image = self
            .layers
            .get(at)
            .ok_or(DocumentError::UnknownLayer(id))?
            .image
            .is_some();
        if has_image {
            let needed = self.materialized_bytes().saturating_add(self.cel_bytes());
            if needed > self.cells_limit {
                return Err(DocumentError::CelsTooLarge {
                    needed,
                    limit: self.cells_limit,
                });
            }
        }
        // El id `u32::MAX` de las celdas queda reservado (la plantilla del lienzo).
        let (new_id, cel) = match (
            self.next_layer.checked_add(1),
            self.next_image.checked_add(1),
        ) {
            (Some(_), Some(next)) if next < u32::MAX => (self.next_layer, self.next_image),
            _ => return Err(too_many),
        };
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        self.layers
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        let source = self.layers.get(at).ok_or(DocumentError::UnknownLayer(id))?;
        let image = match &source.image {
            Some(image) => Some(image.try_clone().map_err(|_| DocumentError::OutOfMemory)?),
            None => None,
        };
        let copy = Layer {
            id: LayerId(new_id),
            name,
            visible: source.visible,
            opacity: source.opacity,
            blend: source.blend,
            locked: source.locked,
            cel: ImageId(cel),
            image,
        };
        self.next_layer = new_id.saturating_add(1);
        self.next_image = cel.saturating_add(1);
        self.layers.insert(at + 1, copy);
        self.commit(Step {
            layer: Some(LayerStep::Presence {
                at: at + 1,
                added: true,
                stored: None,
            }),
            ..Step::default()
        });
        Ok(true)
    }

    /// `SetLayerVisible`.
    fn apply_layer_visible(&mut self, id: LayerId, visible: bool) -> Result<bool, DocumentError> {
        let at = self.layer_position(id)?;
        let Some(before) = self.layers.get(at).map(|layer| layer.visible) else {
            return Ok(false);
        };
        if before == visible {
            return Ok(false);
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        if let Some(layer) = self.layers.get_mut(at) {
            layer.visible = visible;
        }
        self.commit(Step {
            layer: Some(LayerStep::Visible {
                layer: id,
                before,
                after: visible,
            }),
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

    /// Antes de sacar un paso de capa de su pila: si deshacerlo (`undoing`) o rehacerlo mete una
    /// capa en el documento, hace sitio. Si falla, nada ha cambiado y se puede reintentar.
    fn reserve_for_layer_step(&mut self, undoing: bool) -> Result<(), DocumentError> {
        let step = if undoing {
            self.undo.back()
        } else {
            self.redo.last()
        };
        let inserts = step
            .and_then(|step| step.layer.as_ref())
            .map_or(0, |layer| layer.inserts(undoing));
        if inserts > 0 {
            self.layers
                .try_reserve(inserts)
                .map_err(|_| DocumentError::OutOfMemory)?;
        }
        Ok(())
    }

    /// Deshace (`undoing`) o rehace un paso de capa. No reserva ni falla.
    fn run_layer_step(&mut self, step: &mut LayerStep, undoing: bool) {
        match step {
            LayerStep::Presence { at, added, stored } => {
                if *added != undoing {
                    if let Some(layer) = stored.take() {
                        self.layers.insert((*at).min(self.layers.len()), layer);
                    }
                } else if *at < self.layers.len() {
                    *stored = Some(self.layers.remove(*at));
                }
            }
            LayerStep::Move { from, to } => {
                if undoing {
                    self.move_layer(*to, *from);
                } else {
                    self.move_layer(*from, *to);
                }
            }
            LayerStep::Name { layer, other } => {
                if let Some(found) = self.layers.iter_mut().find(|found| found.id == *layer) {
                    std::mem::swap(&mut found.name, other);
                }
            }
            LayerStep::Opacity {
                layer,
                before,
                after,
            } => {
                if let Some(found) = self.layers.iter_mut().find(|found| found.id == *layer) {
                    found.opacity = if undoing { *before } else { *after };
                }
            }
            LayerStep::Visible {
                layer,
                before,
                after,
            } => {
                if let Some(found) = self.layers.iter_mut().find(|found| found.id == *layer) {
                    found.visible = if undoing { *before } else { *after };
                }
            }
            LayerStep::Blend {
                layer,
                before,
                after,
            } => {
                if let Some(found) = self.layers.iter_mut().find(|found| found.id == *layer) {
                    found.blend = if undoing { *before } else { *after };
                }
            }
            LayerStep::Locked {
                layer,
                before,
                after,
            } => {
                if let Some(found) = self.layers.iter_mut().find(|found| found.id == *layer) {
                    found.locked = if undoing { *before } else { *after };
                }
            }
            LayerStep::Merge {
                target,
                positions,
                removed,
                image,
                props,
            } => {
                if undoing {
                    // Ascendente: cada posición es la de antes de combinar.
                    for (at, layer) in positions.iter().zip(removed.drain(..)) {
                        self.layers.insert((*at).min(self.layers.len()), layer);
                    }
                } else {
                    for at in positions.iter().rev() {
                        if *at < self.layers.len() {
                            removed.push(self.layers.remove(*at));
                        }
                    }
                    removed.reverse();
                }
                if let Some(found) = self.layers.iter_mut().find(|found| found.id == *target) {
                    std::mem::swap(&mut found.image, image);
                    std::mem::swap(&mut found.opacity, &mut props.0);
                    std::mem::swap(&mut found.blend, &mut props.1);
                    std::mem::swap(&mut found.locked, &mut props.2);
                }
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

    /// Intercambia las imágenes de las celdas y la plantilla del lienzo con las que guarda el paso
    /// de `ConvertAll`. No reserva ni falla.
    fn swap_converted(&mut self, step: &mut Step) {
        if let Some(converted) = step.converted.as_mut() {
            for (id, image) in &mut converted.cels {
                if let Some(slot) = self.image_mut(*id) {
                    std::mem::swap(slot, image);
                }
            }
            std::mem::swap(&mut self.canvas.template, &mut converted.template);
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
        self.reserve_for_layer_step(true)?;
        let Some(mut step) = self.undo.pop_back() else {
            return Err(DocumentError::NothingToUndo);
        };
        self.undo_bytes = self.undo_bytes.saturating_sub(step.cost());
        if step.swapped.is_some() {
            self.swap_image(&mut step);
        } else if step.converted.is_some() {
            self.swap_converted(&mut step);
        } else if let Some(palette) = step.palette.as_ref() {
            self.run_palette_step(palette, true);
        } else if let Some(layer) = step.layer.as_mut() {
            self.run_layer_step(layer, true);
        } else {
            self.revert(&step);
            self.release_cels(&step.materialized);
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
        self.reserve_for_layer_step(false)?;
        // Si el paso materializó celdas, hay que reservarlas de nuevo antes de tocar nada.
        let fresh = match self.redo.last() {
            Some(step) if !step.materialized.is_empty() => {
                self.check_materialize(step.materialized.len())?;
                self.allocate_cels(step.materialized.len())?
            }
            _ => Vec::new(),
        };
        let Some(mut step) = self.redo.pop() else {
            return Err(DocumentError::NothingToRedo);
        };
        if step.swapped.is_some() {
            self.swap_image(&mut step);
        } else if step.converted.is_some() {
            self.swap_converted(&mut step);
        } else if let Some(palette) = step.palette.as_ref() {
            self.run_palette_step(palette, false);
        } else if let Some(layer) = step.layer.as_mut() {
            self.run_layer_step(layer, false);
        } else {
            self.install_cels(&step.materialized, fresh);
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
            Command::Batch(_) => Err(DocumentError::NestedBatch),
            _ => Err(DocumentError::NotBatchable),
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
    /// `ConvertMode`, un comando de paleta o un comando de capa dentro de un `Batch`.
    NotBatchable,
    /// `MergeLayers` con la lista `remove` vacía, con repetidas o con la propia capa `target`.
    InvalidMerge,
    /// La imagen de `MergeLayers` se calculó sobre el estado `based_on` y el documento ya está en
    /// el `current`.
    StaleImage {
        based_on: u64,
        current: u64,
    },
    /// La imagen de `MergeLayers` no tiene el ancho, alto, modo o paleta del lienzo.
    MergeImageMismatch,
    /// El documento no tiene esa capa.
    UnknownLayer(LayerId),
    /// Posición de capa fuera de rango; `max` es la mayor válida.
    LayerIndex {
        index: usize,
        max: usize,
    },
    /// `RemoveLayer` de la única capa.
    LastLayer,
    /// Ya hay `Document::MAX_LAYERS` capas (o se agotaron los ids).
    TooManyLayers {
        limit: usize,
    },
    /// Nombre de capa de más de `Document::MAX_LAYER_NAME_BYTES` bytes.
    LayerName {
        len: usize,
        limit: usize,
    },
    /// El modo (o el indexado sin índice transparente) no tiene valor transparente: no se puede
    /// añadir una capa ni escribir en una celda vacía.
    NoTransparency {
        mode: Mode,
    },
    /// Opacidad distinta de 255 en indexado.
    IndexedOpacity,
    /// Modo de mezcla distinto de `Normal` en indexado.
    IndexedBlend,
    /// `ConvertMode` con más de una capa (con varias capas se usa `ConvertAll`).
    MultiLayerConvert {
        layers: usize,
    },
    /// Materializar la celda superaría `Document::cells_limit`; `needed` es lo que habría
    /// materializado.
    CelsTooLarge {
        needed: usize,
        limit: usize,
    },
    /// `Document::from_layers` sin ninguna capa.
    NoLayers,
    /// `Document::from_layers`: el lienzo no puede existir (ancho o alto cero, más de
    /// `Image::MAX_BYTES`, o una paleta que no corresponde al modo).
    InvalidCanvas,
    /// `Document::from_layers`: la imagen de la capa `layer` no tiene el ancho, alto, modo o paleta
    /// del lienzo.
    CelMismatch {
        layer: usize,
    },
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
            DocumentError::InvalidMerge => write!(
                f,
                "las capas que se combinan no valen: hay que dar al menos una, sin repetir y distinta de la que se queda"
            ),
            DocumentError::StaleImage { based_on, current } => write!(
                f,
                "la imagen combinada se calculó sobre el estado {based_on} y el documento ya está en el {current}"
            ),
            DocumentError::MergeImageMismatch => write!(
                f,
                "la imagen combinada no tiene el tamaño, el modo o la paleta del lienzo"
            ),
            DocumentError::NotBatchable => write!(
                f,
                "la conversión de modo y la edición de la paleta no pueden ir dentro de un Batch"
            ),
            DocumentError::UnknownLayer(id) => {
                write!(f, "el documento no tiene la capa {}", id.raw())
            }
            DocumentError::LayerIndex { index, max } => {
                write!(
                    f,
                    "la posición {index} de capa no existe (la mayor es {max})"
                )
            }
            DocumentError::LastLayer => write!(f, "no se puede quitar la única capa"),
            DocumentError::TooManyLayers { limit } => {
                write!(f, "no caben más capas (el máximo es {limit})")
            }
            DocumentError::LayerName { len, limit } => write!(
                f,
                "el nombre de la capa ocupa {len} bytes y el máximo es {limit}"
            ),
            DocumentError::NoTransparency { mode } => write!(
                f,
                "el modo {mode:?} no tiene un valor transparente para las capas"
            ),
            DocumentError::IndexedOpacity => {
                write!(f, "en indexado la opacidad de las capas es siempre 255")
            }
            DocumentError::IndexedBlend => {
                write!(
                    f,
                    "en indexado el modo de mezcla de las capas es siempre Normal"
                )
            }
            DocumentError::MultiLayerConvert { layers } => write!(
                f,
                "no se puede convertir el modo con {layers} capas (convierte todas a la vez)"
            ),
            DocumentError::CelsTooLarge { needed, limit } => write!(
                f,
                "las celdas ocuparían {needed} bytes y el límite es {limit}"
            ),
            DocumentError::NoLayers => write!(f, "un documento necesita al menos una capa"),
            DocumentError::InvalidCanvas => write!(
                f,
                "el lienzo no puede existir: tamaño imposible o paleta que no corresponde al modo"
            ),
            DocumentError::CelMismatch { layer } => write!(
                f,
                "la celda de la capa {layer} no encaja: ancho, alto, modo o paleta distintos"
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
