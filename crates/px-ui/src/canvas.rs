//! Lienzo de egui: pinta una `Image` con zoom (de 1/64 a ×64) y desplazamiento
//! (specs/hito-2-ventana.md, «Lienzo»).
//!
//! La geometría vive en [`crate::view`] (sin tipos de egui); aquí solo se traducen las entradas
//! de egui a operaciones de [`View`] y el resultado a un rectángulo y una textura.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use egui::epaint::Mesh;
use egui::{
    Color32, ColorImage, Context, Event, Key, PointerButton, Rect, Sense, Shape, TextureHandle,
    TextureOptions, Ui, pos2, vec2,
};
use px_core::{Image, Rgba};

use crate::actions::Platform;
use crate::background::{CanvasBackground, Fill, background_fill};
use crate::checker::{self, Shade};
use crate::color::Role;
use crate::dirty::BlockSet;
use crate::gestures::{GestureAction, GestureMap, Input, Navigation, PAN_LINE_POINTS, Pointing};
use crate::pointer_events::{button_of, wheel_of};
use crate::source::PixelSource;
use crate::text::text_scale;
use crate::tiles::{BLOCK, SystemTileAlloc, TileAlloc, TileRect, Tiling, block_rect, tile_side};
use crate::view::{ScreenRect, Size, View, ViewOp, Zoom};

/// Fondo liso del lienzo fuera de la imagen, y el fondo «Gris» elegible bajo la transparencia
/// (el de antes del damero). Dentro de la imagen, un píxel transparente o borrado se ve sobre el
/// fondo elegido, por defecto el damero (specs/hito-2-damero-lienzo.md).
pub const BACKGROUND: Color32 = Color32::from_gray(40);

/// Por qué no se puede mostrar la imagen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanvasError {
    /// Un lado de la imagen supera [`Canvas::MAX_SIDE`].
    TooLarge { width: u32, height: u32, max: usize },
    /// No hay memoria para construir las baldosas.
    OutOfMemory,
}

impl fmt::Display for CanvasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanvasError::TooLarge { width, height, max } => write!(
                f,
                "la imagen de {width}×{height} supera el lado máximo mostrable ({max})"
            ),
            CanvasError::OutOfMemory => write!(f, "sin memoria para mostrar la imagen"),
        }
    }
}

impl Error for CanvasError {}

/// Lo que el gesto del lápiz pide a quien dibuja, en coordenadas de imagen con signo (pueden
/// caer fuera de la imagen) y en el orden en que ocurrieron (specs/hito-2-lapiz-deshacer.md,
/// sección 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasEvent {
    /// Botón izquierdo pulsado en el lienzo, sin espacio.
    StrokeBegin((i32, i32)),
    /// El puntero se movió (uno por cada `PointerMoved`), o se soltó el botón en esa posición.
    StrokeMove((i32, i32)),
    /// Fin del trazo: botón soltado, ventana sin foco o botón que egui ya no ve pulsado.
    StrokeEnd,
}

/// Entrada ordenada del lienzo: un evento de trazo o `Escape` (sin modificadores), en el orden
/// en que egui entregó los eventos del fotograma (specs/hito-2-linea-relleno-app.md, sección 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasInput {
    Stroke(CanvasEvent),
    Escape,
}

/// Lo que el lienzo dejó en pantalla en el último fotograma.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasInfo {
    /// Zoom actual (el de ajuste si todavía no hay imagen).
    pub zoom: Zoom,
    /// Píxel de la imagen bajo el cursor, si lo hay.
    pub hover: Option<(u32, u32)>,
    /// Rectángulo de la imagen en puntos de egui; `None` si no se pinta ninguna.
    pub image_rect: Option<Rect>,
    /// Motivo por el que la imagen no se pinta, si es el caso.
    pub error: Option<CanvasError>,
}

/// Qué hace el arrastre en curso. Se decide al pulsar y no cambia hasta soltar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Gesture {
    #[default]
    None,
    /// Desplazar, con el botón que lo empezó.
    Pan(PointerButton),
    /// Dibujar con el color del rol que la tabla de gestos da a `button`, el botón que lo empezó
    /// (por defecto, izquierdo: primer plano; derecho: fondo).
    Draw { role: Role, button: PointerButton },
}

/// Lo que `interact_roles` calcula para que `paint` lo dibuje.
#[derive(Clone, Copy)]
struct Frame {
    rect: Rect,
    viewport: Size,
    ppp: f32,
    hover: Option<(u32, u32)>,
}

/// Una baldosa: su textura y lo que falta por subir.
struct Tile {
    texture: TextureHandle,
    rect: TileRect,
    /// Bloques (de la rejilla de la imagen) que cortan la baldosa y aún no están al día.
    dirty: BlockSet,
    /// La textura lleva la vista previa del valor vigente.
    previewed: bool,
    /// La textura lleva (o llevó) una vista previa: al terminarla hay que regenerarla.
    tainted: bool,
}

/// Copias de trabajo de la superposición del trazo: una por bloque de 64×64 tocado, creada desde
/// el documento con los píxeles pendientes escritos.
#[derive(Default)]
struct Overlay {
    /// El trazo pendiente ya escrito (para escribir solo lo nuevo).
    pending: Vec<(u32, u32, Rgba)>,
    /// Bloque `(bx, by)` → sus píxeles (`ancho · alto` del bloque recortado a la imagen).
    blocks: HashMap<(u32, u32), Vec<Color32>>,
}

/// Estado del lienzo: la imagen en baldosas, la superposición y la vista.
#[derive(Default)]
pub struct Canvas {
    /// Imagen aún sin cargar (`set_image`): se construye en el primer fotograma.
    pending: Option<Image>,
    /// Lado de baldosa inyectado (pruebas).
    tile_side_override: Option<usize>,
    tiles: Vec<Tile>,
    tiling: Option<Tiling>,
    size: Option<Size>,
    view: Option<View>,
    error: Option<CanvasError>,
    gesture: Gesture,
    frame: Option<Frame>,
    overlay: Overlay,
    /// Última vista previa subida: entrada y color resuelto.
    previewing: Option<(u8, Rgba)>,
    preview_uploads: u32,
    /// Operaciones de zoom pedidas desde fuera (menú, columna, atajos), que se aplican en el
    /// siguiente `interact_roles`, cuando se conoce el tamaño del lienzo.
    requested: Vec<ViewOp>,
    /// Las teclas `+`, `=`, `-` y `Home` ya no las trata el lienzo (las trata la capa de acciones).
    view_keys_off: bool,
    /// Qué se ve bajo la transparencia de la imagen: preferencia de la sesión, no del documento.
    background: CanvasBackground,
    /// Qué hace cada botón, la rueda, los dos dedos y el pellizco (specs/interfaz-raton.md).
    gestures: GestureMap,
    /// Acumuladores de pasos de zoom y restos de píxel del desplazamiento.
    nav: Navigation,
    /// Respaldo: el damero como una malla de casillas en vez de la textura repetida.
    checker_mesh: bool,
    /// Textura de 2×2 del damero, creada la primera vez que se pinta con ella.
    checker: Option<TextureHandle>,
}

impl Canvas {
    pub fn new() -> Canvas {
        Canvas::default()
    }

    /// Lado máximo de imagen que se muestra: con el tope de 2²⁶ píxeles de apertura evita imágenes
    /// de 2²⁶×1 con miles de baldosas (specs/hito-2-zoom-baldosas.md, decisión 9).
    pub const MAX_SIDE: usize = 16384;

    /// Un lienzo con el lado de baldosa inyectado (pruebas con imágenes pequeñas); se acota al
    /// lado máximo de textura del contexto.
    pub fn with_tile_side(side: usize) -> Canvas {
        Canvas {
            tile_side_override: Some(side.max(1)),
            ..Canvas::default()
        }
    }

    /// Sustituye la tabla de gestos (hito 8; hoy solo la usan las pruebas).
    pub fn set_gestures(&mut self, gestures: GestureMap) {
        self.gestures = gestures;
    }

    /// Cambia la imagen mostrada (compatibilidad: clona la imagen). Las baldosas se construyen en
    /// el siguiente `interact_roles` y la vista vuelve al ajuste. La app usa [`Canvas::load`].
    pub fn set_image(&mut self, image: &Image) {
        self.set_owned_image(image.clone());
    }

    /// Como [`Canvas::set_image`], pero con una imagen ya construida (no se clona).
    pub fn set_owned_image(&mut self, image: Image) {
        self.clear_state();
        self.size = Some(Size::new(image.width(), image.height()));
        self.pending = Some(image);
    }

    /// Olvida la imagen actual (baldosas, vista, gesto, superposición y vista previa).
    fn clear_state(&mut self) {
        self.tiles.clear();
        self.tiling = None;
        self.size = None;
        self.view = None;
        self.error = None;
        self.gesture = Gesture::None;
        self.previewing = None;
        self.overlay = Overlay::default();
        self.requested.clear();
        self.pending = None;
    }

    /// Carga `image` como la imagen del lienzo: crea **todas** las baldosas desde las filas y
    /// solo instala si salen todas. Con un error (`TooLarge`, `OutOfMemory`) el lienzo anterior
    /// queda intacto y no queda ninguna textura nueva viva. La vista vuelve al ajuste en el
    /// siguiente `interact_roles`.
    pub fn load(
        &mut self,
        ctx: &Context,
        image: &(impl PixelSource + ?Sized),
    ) -> Result<(), CanvasError> {
        self.load_with(ctx, image, &mut SystemTileAlloc)
    }

    /// Como [`Canvas::load`], con el asignador de los búferes de baldosa a la vista.
    pub fn load_with(
        &mut self,
        ctx: &Context,
        image: &(impl PixelSource + ?Sized),
        alloc: &mut dyn TileAlloc,
    ) -> Result<(), CanvasError> {
        let (width, height) = (image.width(), image.height());
        if width as usize > Canvas::MAX_SIDE || height as usize > Canvas::MAX_SIDE {
            return Err(CanvasError::TooLarge {
                width,
                height,
                max: Canvas::MAX_SIDE,
            });
        }
        let size = Size::new(width, height);
        let max_side = ctx.input(|i| i.max_texture_side);
        let side = self
            .tile_side_override
            .map_or_else(|| tile_side(max_side), |side| side.min(max_side).max(1));
        let tiling = Tiling::new(size, side);
        // Primero todos los búferes: si alguno falla no se ha creado ninguna textura.
        let mut images = Vec::new();
        images
            .try_reserve_exact(tiling.len())
            .map_err(|_| CanvasError::OutOfMemory)?;
        for index in 0..tiling.len() {
            let rect = tiling.rect(index);
            let pixels = image.region(rect, None, alloc)?;
            images.push(ColorImage::new(
                [rect.width as usize, rect.height as usize],
                pixels,
            ));
        }
        let mut tiles = Vec::with_capacity(images.len());
        for (index, color_image) in images.into_iter().enumerate() {
            tiles.push(Tile {
                texture: ctx.load_texture(
                    format!("px-image-{index}"),
                    color_image,
                    TextureOptions::NEAREST,
                ),
                rect: tiling.rect(index),
                dirty: BlockSet::new(width, height),
                previewed: false,
                tainted: false,
            });
        }
        self.clear_state();
        self.size = Some(size);
        self.tiling = Some(tiling);
        self.tiles = tiles;
        ctx.request_repaint();
        Ok(())
    }

    /// Pide una operación de zoom: se aplica en el siguiente [`Canvas::interact_roles`], también a mitad
    /// de un trazo (el trazo sigue y sus siguientes puntos usan la vista nueva).
    pub fn request_view(&mut self, op: ViewOp) {
        self.requested.push(op);
    }

    /// Elige qué se ve bajo la transparencia. No toca el documento ni la imagen subida; abrir otra
    /// imagen no lo restablece.
    pub fn set_background(&mut self, background: CanvasBackground) {
        self.background = background;
    }

    /// El fondo elegido bajo la transparencia.
    pub fn background(&self) -> CanvasBackground {
        self.background
    }

    /// Activa el respaldo por malla del damero (`PIXOKO_CHECKER_MESH=1`).
    pub fn set_checker_mesh(&mut self, enabled: bool) {
        self.checker_mesh = enabled;
    }

    pub fn checker_mesh(&self) -> bool {
        self.checker_mesh
    }

    /// Textura de 2×2 del damero, si ya se creó (no se crea con un fondo sólido ni con el
    /// respaldo por malla).
    pub fn checker_texture_id(&self) -> Option<egui::TextureId> {
        self.checker.as_ref().map(TextureHandle::id)
    }

    /// Zoom actual, o `None` si todavía no hay imagen mostrada.
    pub fn zoom(&self) -> Option<Zoom> {
        self.view.map(|view| view.zoom())
    }

    /// Con `false`, el lienzo ignora las teclas `+`, `=`, `-` y `Home` (la app las traduce a
    /// acciones y las despacha con sus propios bloqueos). Por defecto, `true`.
    pub fn set_view_keys(&mut self, enabled: bool) {
        self.view_keys_off = !enabled;
    }

    /// Regenera las baldosas con el contenido de `image` **conservando la vista** (compatibilidad:
    /// `invalidate_all` + `sync`). Si la imagen tiene otro tamaño o aún no hay baldosas, se
    /// comporta como [`Canvas::set_image`].
    pub fn refresh(&mut self, ctx: &Context, image: &Image) {
        if !self.matches(image) {
            self.set_image(image);
            return;
        }
        self.invalidate_all();
        // Si falla la reserva las baldosas siguen sucias y se reintenta en el siguiente `sync`.
        let _ = self.sync(ctx, image);
    }

    /// `image` es la imagen que tienen las baldosas (mismo tamaño).
    fn matches(&self, image: &(impl PixelSource + ?Sized)) -> bool {
        !self.tiles.is_empty() && self.size == Some(Size::new(image.width(), image.height()))
    }

    /// Marca como sucios los bloques de `blocks` (lo que cambió en el documento). No sube nada: lo
    /// hace [`Canvas::sync`]. Si hay una superposición viva se descarta antes: sus copias de
    /// trabajo ya no valen y sus bloques se regeneran desde el documento.
    pub fn invalidate(&mut self, blocks: &BlockSet) {
        let Some(size) = self.size else {
            return;
        };
        if blocks.size() != size {
            self.invalidate_all();
            return;
        }
        self.release_overlay();
        for tile in &mut self.tiles {
            let mut part = blocks.clone();
            part.clip(tile.rect);
            tile.dirty.union(&part);
        }
    }

    /// Marca toda la imagen como sucia.
    pub fn invalidate_all(&mut self) {
        let Some(size) = self.size else {
            return;
        };
        self.release_overlay();
        for tile in &mut self.tiles {
            tile.dirty = full_of(size, tile.rect);
        }
    }

    /// Sube desde el documento los bloques sucios de las baldosas **visibles** (las demás los
    /// conservan hasta que se vean). Si todos los bloques de una baldosa están sucios, una sola
    /// subida de la baldosa entera; si no, una por serie de bloques contiguos (`BlockSet::runs`).
    pub fn sync(
        &mut self,
        ctx: &Context,
        image: &(impl PixelSource + ?Sized),
    ) -> Result<(), CanvasError> {
        self.sync_with(ctx, image, &mut SystemTileAlloc)
    }

    /// Como [`Canvas::sync`], con el asignador de los búferes a la vista. Con un error las
    /// baldosas sin terminar siguen sucias.
    pub fn sync_with(
        &mut self,
        ctx: &Context,
        image: &(impl PixelSource + ?Sized),
        alloc: &mut dyn TileAlloc,
    ) -> Result<(), CanvasError> {
        if !self.matches(image) {
            return Ok(());
        }
        let size = self.size.unwrap_or(Size::new(0, 0));
        for index in 0..self.tiles.len() {
            if self.tiles[index].dirty.is_empty() || !self.tile_visible(index) {
                continue;
            }
            let rect = self.tiles[index].rect;
            let runs = if self.tiles[index].dirty.len() == blocks_in(rect) {
                vec![rect]
            } else {
                self.tiles[index].dirty.runs(rect)
            };
            for run in runs {
                let pixels = image.region(run, None, alloc)?;
                let patch = ColorImage::new([run.width as usize, run.height as usize], pixels);
                let tile = &mut self.tiles[index];
                if run == rect {
                    tile.texture.set(patch, TextureOptions::NEAREST);
                } else {
                    tile.texture.set_partial(
                        [(run.x - rect.x) as usize, (run.y - rect.y) as usize],
                        patch,
                        TextureOptions::NEAREST,
                    );
                }
            }
            let tile = &mut self.tiles[index];
            tile.dirty = BlockSet::new(size.width, size.height);
            tile.previewed = false;
            ctx.request_repaint();
        }
        Ok(())
    }

    /// La baldosa `index` corta el lienzo del último `interact_roles` (todas, si aún no hubo uno).
    fn tile_visible(&self, index: usize) -> bool {
        match (&self.tiling, self.view, self.frame) {
            (Some(tiling), Some(view), Some(frame)) => tiling.visible(index, &view, frame.viewport),
            _ => true,
        }
    }

    /// Bloques sucios pendientes de subir, sumados de todas las baldosas.
    pub fn pending_blocks(&self) -> usize {
        self.tiles.iter().map(|tile| tile.dirty.len()).sum()
    }

    /// Bloques de 64×64 con copia de trabajo de la superposición.
    pub fn overlay_blocks(&self) -> usize {
        self.overlay.blocks.len()
    }

    /// Número de baldosas.
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Rectángulo de la baldosa `index` en la imagen.
    pub fn tile_rect(&self, index: usize) -> TileRect {
        self.tiles[index].rect
    }

    /// Lado de baldosa de la imagen cargada (0 sin imagen).
    pub fn tile_side(&self) -> usize {
        self.tiling.map_or(0, |tiling| tiling.side())
    }

    /// El trazo pendiente `pixels` (el **entero**, con el color que tendrá cada píxel en la
    /// imagen; el borrador pasa alfa 0) como parte de la textura, a `image` (el documento, que no
    /// cambia hasta soltar). Se escriben solo los píxeles nuevos si lo anterior es su prefijo
    /// (lápiz y borrador); si no (la goma elástica de la línea), los bloques de lo anterior
    /// vuelven al documento y se escriben los nuevos. Vacío es [`Canvas::clear_overlay`].
    pub fn set_overlay(
        &mut self,
        ctx: &Context,
        image: &(impl PixelSource + ?Sized),
        pixels: &[(u32, u32, Rgba)],
    ) {
        if pixels.is_empty() {
            self.clear_overlay();
            return;
        }
        let Some(size) = self.size.filter(|_| self.matches(image)) else {
            return;
        };
        let known = self.overlay.pending.len();
        let prefix = pixels.len() >= known && pixels[..known] == self.overlay.pending[..];
        let start = if prefix {
            known
        } else {
            let keep: HashSet<(u32, u32)> = pixels
                .iter()
                .filter(|(x, y, _)| *x < size.width && *y < size.height)
                .map(|(x, y, _)| (x / BLOCK, y / BLOCK))
                .collect();
            let old: Vec<(u32, u32)> = self.overlay.blocks.keys().copied().collect();
            self.overlay.blocks.clear();
            self.overlay.pending.clear();
            for (bx, by) in old.into_iter().filter(|block| !keep.contains(block)) {
                let rect = block_rect(bx, by, size);
                self.put(
                    rect,
                    image
                        .region(rect, None, &mut SystemTileAlloc)
                        .unwrap_or_default(),
                );
            }
            0
        };
        let mut touched: HashSet<(u32, u32)> = HashSet::new();
        for &(x, y, color) in &pixels[start..] {
            if x >= size.width || y >= size.height {
                continue;
            }
            let block = (x / BLOCK, y / BLOCK);
            let rect = block_rect(block.0, block.1, size);
            let buffer = self.overlay.blocks.entry(block).or_insert_with(|| {
                image
                    .region(rect, None, &mut SystemTileAlloc)
                    .unwrap_or_default()
            });
            buffer[(y - rect.y) as usize * rect.width as usize + (x - rect.x) as usize] =
                Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
            touched.insert(block);
        }
        self.overlay.pending.extend_from_slice(&pixels[start..]);
        for (bx, by) in touched {
            let rect = block_rect(bx, by, size);
            if let Some(buffer) = self.overlay.blocks.get(&(bx, by)) {
                self.put(rect, buffer.clone());
            }
        }
        ctx.request_repaint();
    }

    /// Descarta la superposición: las copias de trabajo se liberan y sus bloques quedan sucios
    /// (se regeneran desde el documento en el siguiente [`Canvas::sync`], que ya incluye el
    /// trazo si se aplicó).
    pub fn clear_overlay(&mut self) {
        self.release_overlay();
    }

    fn release_overlay(&mut self) {
        let Some(size) = self.size else {
            return;
        };
        let blocks: Vec<(u32, u32)> = self.overlay.blocks.keys().copied().collect();
        self.overlay = Overlay::default();
        for (bx, by) in blocks {
            let rect = block_rect(bx, by, size);
            for tile in &mut self.tiles {
                if tile.rect.intersect(&rect).is_some() {
                    tile.dirty.insert(bx, by);
                }
            }
        }
    }

    /// Sube `pixels` (el contenido de `region`, por filas) a cada baldosa que corta la región, y
    /// da por al día los bloques de la región en esas baldosas.
    fn put(&mut self, region: TileRect, pixels: Vec<Color32>) {
        for tile in &mut self.tiles {
            let Some(part) = tile.rect.intersect(&region) else {
                continue;
            };
            let mut patch = Vec::with_capacity(part.width as usize * part.height as usize);
            for y in part.y..part.y + part.height {
                let row = (y - region.y) as usize * region.width as usize;
                let from = row + (part.x - region.x) as usize;
                patch.extend_from_slice(&pixels[from..from + part.width as usize]);
            }
            tile.texture.set_partial(
                [
                    (part.x - tile.rect.x) as usize,
                    (part.y - tile.rect.y) as usize,
                ],
                ColorImage::new([part.width as usize, part.height as usize], patch),
                TextureOptions::NEAREST,
            );
            let (bx, by) = (region.x / BLOCK, region.y / BLOCK);
            tile.dirty.remove(bx, by);
        }
    }

    /// Vista previa de la edición de la entrada `index` de la paleta: reconstruye con esa entrada
    /// valiendo `color` las baldosas **visibles** (las demás, al verse) y sube cada una, solo si el
    /// color resuelto cambió respecto a lo último subido o la baldosa aún no lleva la vista previa
    /// (specs/hito-2-editar-color.md, decisión 7). Sin baldosas o con otro tamaño no hace nada.
    /// [`Canvas::end_preview`], [`Canvas::refresh`] y [`Canvas::set_image`] la deshacen.
    pub fn preview_entry(
        &mut self,
        ctx: &Context,
        image: &(impl PixelSource + ?Sized),
        index: u8,
        color: Rgba,
    ) {
        if !self.matches(image) {
            return;
        }
        let size = self.size.unwrap_or(Size::new(0, 0));
        let resolved = if image.transparent_index() == Some(index) {
            Rgba { a: 0, ..color }
        } else {
            color
        };
        let changed = self.previewing != Some((index, resolved));
        if changed {
            for tile in &mut self.tiles {
                tile.previewed = false;
            }
        }
        for i in 0..self.tiles.len() {
            if self.tiles[i].previewed || !self.tile_visible(i) {
                continue;
            }
            let rect = self.tiles[i].rect;
            let Ok(pixels) = image.region(rect, Some((index, color)), &mut SystemTileAlloc) else {
                continue;
            };
            let tile = &mut self.tiles[i];
            tile.texture.set(
                ColorImage::new([rect.width as usize, rect.height as usize], pixels),
                TextureOptions::NEAREST,
            );
            tile.previewed = true;
            tile.tainted = true;
            tile.dirty = BlockSet::new(size.width, size.height);
            ctx.request_repaint();
        }
        if changed {
            self.previewing = Some((index, resolved));
            self.preview_uploads += 1;
        }
    }

    /// Devuelve las baldosas tocadas por la vista previa al contenido de `image` (las visibles
    /// ya; las demás, al verse). Sin vista previa, no hace nada.
    pub fn end_preview(&mut self, ctx: &Context, image: &(impl PixelSource + ?Sized)) {
        if self.previewing.is_none() && self.tiles.iter().all(|tile| !tile.tainted) {
            return;
        }
        self.previewing = None;
        let Some(size) = self.size else {
            return;
        };
        for tile in &mut self.tiles {
            if tile.tainted {
                tile.dirty = full_of(size, tile.rect);
                tile.tainted = false;
                tile.previewed = false;
            }
        }
        let _ = self.sync(ctx, image);
    }

    /// Subidas de textura hechas por `preview_entry` desde que se creó el lienzo (una por cambio de
    /// valor).
    pub fn preview_uploads(&self) -> u32 {
        self.preview_uploads
    }

    /// Textura de la primera baldosa, una vez cargada.
    pub fn texture_id(&self) -> Option<egui::TextureId> {
        self.tiles.first().map(|tile| tile.texture.id())
    }

    /// Las texturas de todas las baldosas, por filas.
    pub fn texture_ids(&self) -> Vec<egui::TextureId> {
        self.tiles.iter().map(|tile| tile.texture.id()).collect()
    }

    /// Dibuja el lienzo en todo el espacio libre de `ui` y procesa sus entradas: [`interact_roles`]
    /// y [`paint`] seguidos, sin los eventos de trazo.
    ///
    /// Rueda o pellizco: un paso de zoom por evento, anclado al cursor. Arrastre con el botón
    /// central, o con espacio y el izquierdo: desplazamiento. `Home`: ajuste. `+` y `-`: zoom
    /// en el centro.
    ///
    /// [`interact_roles`]: Canvas::interact_roles
    /// [`paint`]: Canvas::paint
    pub fn show(&mut self, ui: &mut Ui) -> CanvasInfo {
        self.interact_roles(ui);
        self.paint(ui)
    }

    /// Primera mitad de [`Canvas::show`]: sube la textura si falta, reserva el espacio, procesa
    /// las entradas (zoom, desplazamiento y el gesto de dibujo) y devuelve los eventos de trazo
    /// del fotograma, en orden, con `CanvasInput::Escape` (tecla `Escape` sin modificadores) **en
    /// su sitio** entre ellos, para que quien dibuja respete el orden de los eventos de egui.
    /// Debe ir antes de [`Canvas::paint`] en el mismo fotograma.
    ///
    /// El gesto se decide al pulsar: botón central, o izquierdo con espacio abajo, desplaza;
    /// izquierdo sin espacio dibuja (solo si la pulsación cae dentro del lienzo). Un segundo
    /// botón durante el gesto se ignora. El trazo sigue aunque el puntero salga del lienzo o de
    /// la ventana y termina al soltar, al perder el foco o si egui ya no ve el botón pulsado.
    ///
    /// El botón derecho también dibuja: cada entrada lleva el rol del botón que **empezó** el
    /// gesto (izquierdo: `Foreground`; derecho: `Background`), igual para sus `Move` y su `End`. El
    /// central desplaza y el derecho no (specs/interfaz-dos-colores.md, decisión 2). `Escape`
    /// lleva el rol del gesto en curso o `Foreground`.
    pub fn interact_roles(&mut self, ui: &mut Ui) -> Vec<(CanvasInput, Role)> {
        self.interact_inner(ui)
    }

    fn interact_inner(&mut self, ui: &mut Ui) -> Vec<(CanvasInput, Role)> {
        self.upload_if_pending(ui);

        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        let physical = |v: f32| (v * ppp).round() as i64;
        let viewport = Size::new(
            u32::try_from(physical(rect.width())).unwrap_or(0),
            u32::try_from(physical(rect.height())).unwrap_or(0),
        );
        // El puntero cuenta como «en el lienzo» solo si ninguna otra capa de egui (un desplegable
        // del menú, una ventana) está encima: se pregunta a egui por el orden de capas, no solo a
        // la geometría (specs/hito-2-menu-herramientas.md, decisión 6).
        let layer = ui.layer_id();
        let ctx = ui.ctx().clone();
        let on_canvas =
            |p: egui::Pos2| rect.contains(p) && ctx.layer_id_at(p).is_none_or(|top| top == layer);
        let pointer = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| on_canvas(*p))
            .map(|p| (physical(p.x - rect.min.x), physical(p.y - rect.min.y)));

        let mut frame = Frame {
            rect,
            viewport,
            ppp,
            hover: None,
        };
        let (false, Some(size)) = (self.tiles.is_empty(), self.size) else {
            self.frame = Some(frame);
            return Vec::new();
        };

        let mut view = self.view.unwrap_or_else(|| View::fit(viewport, size));
        // `n` (centrar bajo el cursor) necesita el puntero: la última posición conocida, si cae
        // en el lienzo y ninguna otra capa está encima.
        let cursor = ui
            .input(|i| i.pointer.latest_pos())
            .filter(|p| on_canvas(*p))
            .map(|p| (physical(p.x - rect.min.x), physical(p.y - rect.min.y)));
        for op in std::mem::take(&mut self.requested) {
            match (op, cursor) {
                (ViewOp::CenterOnCursor, Some(point)) => view.center_on(point, viewport, size),
                (ViewOp::CenterOnCursor, None) => {}
                _ => view.apply_op(op, viewport, size),
            }
        }
        let center = (
            i64::from(viewport.width) / 2,
            i64::from(viewport.height) / 2,
        );
        let to_image = |view: &View, p: egui::Pos2| {
            view.screen_to_image_i32((physical(p.x - rect.min.x), physical(p.y - rect.min.y)))
        };
        let space_down = ui.input(|i| i.key_down(Key::Space));
        let platform = Platform::current();
        // Con un campo de texto enfocado el teclado es suyo (specs/hito-2-editar-color.md, 11).
        let typing = ui.ctx().text_edit_focused();
        let mut strokes = Vec::new();
        let events = ui.input(|i| i.events.clone());
        let (mut saw_wheel, mut saw_scroll) = (false, false);
        for event in &events {
            match event {
                Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } => {
                    let Some((input, dx, dy)) = wheel_of(*unit, *delta, *modifiers) else {
                        continue;
                    };
                    saw_wheel |= input == Input::Wheel;
                    saw_scroll |= input == Input::Scroll;
                    let Some(anchor) = pointer else { continue };
                    let pointing = Pointing {
                        input,
                        command: false,
                        shift: modifiers.shift,
                        space: space_down,
                    };
                    // Muescas (`Wheel`) o puntos (`Scroll`): las muescas valen `PAN_LINE_POINTS`.
                    let to_points = |amount: f32| match input {
                        Input::Wheel => amount * PAN_LINE_POINTS,
                        _ => amount,
                    };
                    let steps = |nav: &mut Navigation| match input {
                        Input::Wheel => nav.wheel.push(dy),
                        _ => nav.scroll_zoom.push(dy / PAN_LINE_POINTS),
                    };
                    match self.gestures.resolve(&pointing, true, platform) {
                        Some(GestureAction::ZoomStep) => {
                            zoom_steps(&mut view, steps(&mut self.nav), anchor);
                        }
                        Some(GestureAction::Pan) => {
                            let (px, py) = self.nav.carry.add(to_points(dx), to_points(dy), ppp);
                            view.pan(px, py, viewport, size);
                        }
                        Some(GestureAction::PanX) => {
                            let (px, _) = self.nav.carry.add(to_points(dx + dy), 0.0, ppp);
                            view.pan(px, 0, viewport, size);
                        }
                        _ => {}
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } if !modifiers.command && !typing => match key {
                    Key::Home if !self.view_keys_off => view = View::fit(viewport, size),
                    Key::Plus | Key::Equals if !self.view_keys_off => {
                        zoom_step(&mut view, true, center);
                    }
                    Key::Minus if !self.view_keys_off => zoom_step(&mut view, false, center),
                    Key::Escape if modifiers.is_none() => {
                        let role = match self.gesture {
                            Gesture::Draw { role, .. } => role,
                            _ => Role::Foreground,
                        };
                        strokes.push((CanvasInput::Escape, role));
                    }
                    // `Espacio` con un trazo en curso lo cancela, como `Escape`; sin gesto es la
                    // tecla de la mano. Su repetición no cancela, y un desplazamiento no se cancela.
                    Key::Space if !repeat && modifiers.is_none() => {
                        if let Gesture::Draw { role, .. } = self.gesture {
                            strokes.push((CanvasInput::Escape, role));
                        }
                    }
                    _ => {}
                },
                Event::PointerButton {
                    pos,
                    button,
                    pressed: true,
                    modifiers,
                } if self.gesture == Gesture::None && on_canvas(*pos) => {
                    let pointing = Pointing {
                        input: Input::Press(button_of(*button)),
                        command: modifiers.command,
                        shift: modifiers.shift,
                        space: space_down,
                    };
                    self.gesture = match self.gestures.resolve(&pointing, true, platform) {
                        Some(GestureAction::Draw(role)) => Gesture::Draw {
                            role,
                            button: *button,
                        },
                        Some(GestureAction::Pan) => Gesture::Pan(*button),
                        _ => Gesture::None,
                    };
                    if let Gesture::Draw { role, .. } = self.gesture {
                        strokes.push((
                            CanvasInput::Stroke(CanvasEvent::StrokeBegin(to_image(&view, *pos))),
                            role,
                        ));
                    }
                }
                Event::PointerButton {
                    pos,
                    button,
                    pressed: false,
                    ..
                } => match self.gesture {
                    Gesture::Draw {
                        role,
                        button: owner,
                    } if *button == owner => {
                        strokes.push((
                            CanvasInput::Stroke(CanvasEvent::StrokeMove(to_image(&view, *pos))),
                            role,
                        ));
                        strokes.push((CanvasInput::Stroke(CanvasEvent::StrokeEnd), role));
                        self.gesture = Gesture::None;
                    }
                    Gesture::Pan(owner) if *button == owner => self.gesture = Gesture::None,
                    _ => {}
                },
                Event::PointerMoved(pos) if matches!(self.gesture, Gesture::Draw { .. }) => {
                    if let Gesture::Draw { role, .. } = self.gesture {
                        strokes.push((
                            CanvasInput::Stroke(CanvasEvent::StrokeMove(to_image(&view, *pos))),
                            role,
                        ));
                    }
                }
                Event::WindowFocused(false) => {
                    if let Gesture::Draw { role, .. } = self.gesture {
                        strokes.push((CanvasInput::Stroke(CanvasEvent::StrokeEnd), role));
                    }
                    self.gesture = Gesture::None;
                }
                _ => {}
            }
        }
        // Un fotograma sin rueda o sin dedos acaba el gesto: se descarta lo que no dio un paso.
        if !saw_wheel {
            self.nav.wheel.reset();
        }
        if !saw_scroll {
            self.nav.scroll_zoom.reset();
            self.nav.carry.reset();
        }
        // Red de seguridad: un soltar que no llegó (por ejemplo, fuera de la ventana).
        match self.gesture {
            Gesture::Draw { role, button } if !ui.input(|i| i.pointer.button_down(button)) => {
                strokes.push((CanvasInput::Stroke(CanvasEvent::StrokeEnd), role));
                self.gesture = Gesture::None;
            }
            Gesture::Pan(button) if !ui.input(|i| i.pointer.button_down(button)) => {
                self.gesture = Gesture::None;
            }
            _ => {}
        }
        // Pellizco y Ctrl + rueda: egui los resume en `zoom_delta`, un paso cada vez que el
        // logaritmo acumulado cruza `PINCH_STEP_LN` (antes, un paso por fotograma).
        let pinch = ui.input(|i| i.zoom_delta());
        if (pinch - 1.0).abs() > 1e-3 {
            let (command, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
            let pointing = Pointing {
                input: Input::Pinch,
                command,
                shift,
                space: space_down,
            };
            if let (Some(anchor), Some(GestureAction::ZoomStep)) =
                (pointer, self.gestures.resolve(&pointing, true, platform))
            {
                zoom_steps(&mut view, self.nav.pinch.push(pinch.ln()), anchor);
            }
        } else {
            self.nav.pinch.reset();
        }
        if let Gesture::Pan(button) = self.gesture
            && response.dragged_by(button)
        {
            let delta = response.drag_delta();
            view.pan(physical(delta.x), physical(delta.y), viewport, size);
        }
        view.clamp(viewport, size);
        self.view = Some(view);

        frame.hover = pointer.and_then(|p| view.screen_to_image(p, size));
        self.frame = Some(frame);
        strokes
    }

    /// Segunda mitad de [`Canvas::show`]: pinta el fondo, la imagen y, encima, los píxeles de
    /// `overlay` (`(x, y, color)`: lo que se escribirá, con el color que tendrá en la imagen).
    /// Cada uno es un rectángulo de `zoom × zoom` píxeles físicos; con alfa menor que 255 va
    /// sobre el fondo y no sobre la imagen, porque el resultado reemplaza el píxel.
    ///
    /// Si no hubo `interact_roles` antes en el fotograma, no pinta nada.
    pub fn paint(&mut self, ui: &mut Ui) -> CanvasInfo {
        let Some(frame) = self.frame else {
            return CanvasInfo {
                zoom: Zoom::default(),
                hover: None,
                image_rect: None,
                error: self.error.clone(),
            };
        };
        // Sin suavizado de bordes. Con él, egui teselaba un rectángulo de 1×1 píxel físico con todos
        // sus vértices en el mismo punto (triángulos degenerados) y no pintaba nada: a ×1 la
        // superposición del trazo no se veía. Toda la interfaz está alineada a la rejilla de
        // píxeles físicos (múltiplos de `n`), así que el suavizado no aporta nada y sin él cada
        // rectángulo cubre exactamente sus píxeles.
        let ctx = ui.ctx();
        if ctx.tessellation_options(|options| options.feathering) {
            ctx.tessellation_options_mut(|options| options.feathering = false);
        }
        let fill = background_fill(self.background);
        // La textura del damero se crea una vez, la primera vez que hay imagen que pintar con ella.
        if fill == Fill::Checker
            && !self.checker_mesh
            && !self.tiles.is_empty()
            && self.checker.is_none()
        {
            self.checker = Some(ui.ctx().load_texture(
                "px-checker",
                checker_image(),
                TextureOptions::NEAREST_REPEAT,
            ));
        }
        let painter = ui.painter_at(frame.rect);
        painter.rect_filled(frame.rect, 0.0, BACKGROUND);

        let (Some(tiling), Some(size), Some(view)) = (self.tiling, self.size, self.view) else {
            return CanvasInfo {
                zoom: Zoom::default(),
                hover: None,
                image_rect: None,
                error: self.error.clone(),
            };
        };
        let ppp = frame.ppp;
        let to_points = |p: (i64, i64)| frame.rect.min + vec2(p.0 as f32 / ppp, p.1 as f32 / ppp);
        let rect_points = |r: ScreenRect| {
            Rect::from_min_max(
                to_points((r.x, r.y)),
                to_points((r.x + r.width, r.y + r.height)),
            )
        };

        let placed = view.image_rect(size);
        let image_rect = Rect::from_min_size(
            to_points((placed.x, placed.y)),
            vec2(placed.width as f32 / ppp, placed.height as f32 / ppp),
        );

        // Fondo bajo la transparencia: la parte de la imagen que cae en el lienzo, con los mismos
        // enteros de píxel físico que la malla de la imagen. El damero está anclado a su esquina.
        let cell = checker::cell_px(text_scale(ppp));
        let origin = (placed.x, placed.y);
        let checker_quad = |area: ScreenRect, painter: &egui::Painter| {
            if let (Some(handle), Some(quad)) = (&self.checker, checker::quad(area, origin, cell)) {
                let uv =
                    Rect::from_min_max(pos2(quad.uv[0], quad.uv[1]), pos2(quad.uv[2], quad.uv[3]));
                painter.image(handle.id(), rect_points(quad.rect), uv, Color32::WHITE);
            }
        };
        if let Some(area) = checker::clip(placed, frame.viewport) {
            match fill {
                Fill::Solid(color) => {
                    painter.rect_filled(rect_points(area), 0.0, color32(color));
                }
                Fill::Checker if self.checker_mesh => {
                    painter.rect_filled(rect_points(area), 0.0, shade_color(Shade::A));
                    let mut mesh = Mesh::default();
                    for dark in checker::cells(area, origin, cell).filter(|c| c.shade == Shade::B) {
                        mesh.add_colored_rect(rect_points(dark.rect), shade_color(Shade::B));
                    }
                    if !mesh.is_empty() {
                        painter.add(Shape::mesh(mesh));
                    }
                }
                Fill::Checker => checker_quad(area, &painter),
            }
        }
        // Las baldosas visibles, cada una con su cuadrilátero entero y su uv. La superposición del
        // trazo ya va en las texturas (sobre el damero o el fondo elegido, que está debajo).
        for (index, tile) in self.tiles.iter().enumerate() {
            if !tiling.visible(index, &view, frame.viewport) {
                continue;
            }
            let [u0, v0, u1, v1] = tiling.uv(index, &view);
            painter.image(
                tile.texture.id(),
                rect_points(tiling.screen_rect(index, &view)),
                Rect::from_min_max(pos2(u0, v0), pos2(u1, v1)),
                Color32::WHITE,
            );
        }

        CanvasInfo {
            zoom: view.zoom(),
            hover: frame.hover,
            image_rect: Some(image_rect),
            error: None,
        }
    }

    /// Construye las baldosas de una imagen puesta con `set_image`, en el primer fotograma.
    fn upload_if_pending(&mut self, ui: &Ui) {
        let Some(image) = self.pending.take() else {
            return;
        };
        // Las operaciones de zoom pedidas entre `set_image` y este fotograma valen para la imagen
        // nueva: `load_with` las descartaría como las de la imagen anterior.
        let requested = std::mem::take(&mut self.requested);
        if let Err(error) = self.load_with(ui.ctx(), &image, &mut SystemTileAlloc) {
            self.clear_state();
            self.size = Some(Size::new(image.width(), image.height()));
            self.error = Some(error);
        }
        self.requested = requested;
    }
}

/// Textura de 2×2 del damero: `texels()` por filas.
fn checker_image() -> ColorImage {
    ColorImage::new([2, 2], checker::texels().map(color32).to_vec())
}

fn color32(color: Rgba) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

fn shade_color(shade: Shade) -> Color32 {
    color32(shade.color())
}

/// Todos los bloques de la imagen que cortan `rect`.
fn full_of(size: Size, rect: TileRect) -> BlockSet {
    let mut set = BlockSet::full(size.width, size.height);
    set.clip(rect);
    set
}

/// Bloques de la rejilla de la imagen que cortan `rect`.
fn blocks_in(rect: TileRect) -> usize {
    let block = u64::from(BLOCK);
    let cols = rect.right().div_ceil(block) - u64::from(rect.x) / block;
    let rows = rect.bottom().div_ceil(block) - u64::from(rect.y) / block;
    (cols * rows) as usize
}

/// `steps` pasos de zoom (con signo: positivo acerca) anclados en `anchor`.
fn zoom_steps(view: &mut View, steps: i32, anchor: (i64, i64)) {
    for _ in 0..steps.unsigned_abs() {
        zoom_step(view, steps > 0, anchor);
    }
}

fn zoom_step(view: &mut View, zoom_in: bool, anchor: (i64, i64)) {
    if zoom_in {
        view.zoom_in(anchor);
    } else {
        view.zoom_out(anchor);
    }
}
