//! Herramientas (lápiz, línea, relleno, cuentagotas y borrador) como lógica pura
//! (specs/hito-2-linea-relleno-app.md y specs/hito-2-cuentagotas-borrador.md).
//!
//! Sin tipos de egui en la API de [`ToolState`]: quien dibuja traduce los eventos de egui a
//! [`CanvasEvent`] / [`CanvasInput`] y las teclas a [`ToolAction`].

use egui::{Key, Modifiers};
use px_core::{
    Command, Document, DocumentError, Image, ImageId, Mode, Pixel, PixelEdit, Rgba, Span,
};
use px_raster::{Line, fill_command, line_command};

use crate::actions::{AppAction, Platform};
use crate::canvas::{CanvasEvent, CanvasInput};
use crate::color::{ColorChoice, SlotColors, choice_label_with, pixel_for_choice};
use crate::dirty::{Change, dirty_of};
use crate::key_events::press_from_key;
use crate::keymap::default_keymap;
use crate::pick::{
    Dropper, EraseError, Picked, eraser_label, eraser_pixel, is_erasable, picked_at,
};
use crate::source::visible_picked;
use crate::stroke::Stroke;
use crate::view::Size;

/// `image` es la celda de alguna capa (vacía o no).
fn is_cell(doc: &Document, image: ImageId) -> bool {
    doc.layers().iter().any(|layer| layer.cel() == image)
}

/// La imagen de la que salen el modo y la paleta de la celda `image`: la propia o, si está vacía,
/// la del lienzo; `None` si `image` no es de ninguna celda.
fn reference(doc: &Document, image: ImageId) -> Option<&Image> {
    doc.image(image)
        .or_else(|| is_cell(doc, image).then(|| doc.canvas_image()))
}

/// El valor de una celda vacía en el modo del lienzo: el transparente (`None` en los modos que no
/// lo tienen).
fn empty_value(canvas: &Image) -> Option<Pixel> {
    match canvas.mode() {
        Mode::Rgba => Some(Pixel::Rgba(Rgba::TRANSPARENT)),
        Mode::GrayAlpha => Some(Pixel::GrayAlpha { gray: 0, alpha: 0 }),
        Mode::Indexed => canvas
            .palette()
            .and_then(|palette| palette.transparent_index())
            .map(Pixel::Indexed),
        Mode::Gray => None,
    }
}

/// `line_command` también sobre una celda vacía (que `px-raster` no ve): `SetPixels` con lo que
/// cae dentro del lienzo, sin materializar nada antes de aplicarlo.
fn line_in(
    doc: &Document,
    image: ImageId,
    a: (i32, i32),
    b: (i32, i32),
    pixel: Pixel,
) -> Result<Option<Command>, DocumentError> {
    if doc.image(image).is_some() || !is_cell(doc, image) {
        return line_command(doc, image, a, b, pixel);
    }
    let line = Line::new(a, b).clip_to(doc.width(), doc.height());
    if line.is_empty() {
        return Ok(None);
    }
    let len = usize::try_from(line.len()).map_err(|_| DocumentError::OutOfMemory)?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(len)
        .map_err(|_| DocumentError::OutOfMemory)?;
    for (x, y) in line {
        // Tras recortar, `x` e `y` están dentro del lienzo.
        if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) {
            pixels.push(PixelEdit { x, y, pixel });
        }
    }
    Ok(Some(Command::SetPixels { image, pixels }))
}

/// `fill_command` también sobre una celda vacía: todo el lienzo es una región del valor
/// transparente, así que el relleno es un `FillSpans` de un tramo por fila (sin reservar la celda
/// antes de aplicarlo). El color igual al transparente no cambia nada.
fn fill_in(
    doc: &Document,
    image: ImageId,
    x: i32,
    y: i32,
    pixel: Pixel,
) -> Result<Option<Command>, DocumentError> {
    if doc.image(image).is_some() || !is_cell(doc, image) {
        return fill_command(doc, image, x, y, pixel);
    }
    let canvas = doc.canvas_image();
    canvas
        .check_pixel(pixel)
        .map_err(|source| DocumentError::Pixel { pixel: 0, source })?;
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return Ok(None);
    };
    if x >= doc.width() || y >= doc.height() {
        return Ok(None);
    }
    let seed = empty_value(canvas).ok_or(DocumentError::NoTransparency { mode: doc.mode() })?;
    if seed == pixel {
        return Ok(None);
    }
    let height = doc.height();
    let mut spans = Vec::new();
    spans
        .try_reserve_exact(height as usize)
        .map_err(|_| DocumentError::OutOfMemory)?;
    spans.extend((0..height).map(|y| Span {
        y,
        x_start: 0,
        x_end: doc.width(),
    }));
    Ok(Some(Command::FillSpans {
        image,
        spans,
        pixel,
    }))
}

/// Qué lee el cuentagotas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropperSource {
    /// Lo que se ve: la composición de las capas visibles.
    #[default]
    Visible,
    /// La celda de la capa activa (vacía: el valor transparente).
    Layer,
}

/// Lo que elige el cuentagotas en `pos` según `source`; `None` fuera del lienzo.
fn pick_at(
    doc: &Document,
    image: ImageId,
    source: DropperSource,
    x: u32,
    y: u32,
) -> Option<Picked> {
    match source {
        DropperSource::Visible => visible_picked(doc, x, y),
        DropperSource::Layer => match doc.image(image) {
            Some(cell) => picked_at(cell, x, y),
            None => {
                let value = empty_value(doc.canvas_image())?;
                (x < doc.width() && y < doc.height()).then_some(())?;
                Some(match value {
                    Pixel::Indexed(index) => Picked::Entry(index),
                    Pixel::Rgba(color) => Picked::Color(color),
                    Pixel::GrayAlpha { gray, alpha } => {
                        Picked::Color(Rgba::new(gray, gray, gray, alpha))
                    }
                    Pixel::Gray(gray) => Picked::Color(Rgba::opaque(gray, gray, gray)),
                })
            }
        },
    }
}

/// Herramienta activa.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Pencil,
    Line,
    Fill,
    Dropper,
    Eraser,
}

impl Tool {
    /// Nombre para el título: `lápiz`, `línea`, `relleno`, `cuentagotas` o `borrador`.
    pub fn label(self) -> &'static str {
        match self {
            Tool::Pencil => "lápiz",
            Tool::Line => "línea",
            Tool::Fill => "relleno",
            Tool::Dropper => "cuentagotas",
            Tool::Eraser => "borrador",
        }
    }
}

/// Lo que pide una tecla de herramienta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolAction {
    Select(Tool),
    Cancel,
}

/// Vista de la tabla de teclas por defecto: `d`, `v`, `f`, `,` y `k` eligen herramienta y `Escape`
/// cancela, solo sin ningún modificador (así `Ctrl`+`d` y las demás no hacen nada; `K` con `Mayús`
/// es Borrar página, no el Borrador). Es aparte de [`crate::shortcuts::action_for_key`], que da `None` para estas teclas.
pub fn tool_action_for_key(key: Key, modifiers: Modifiers) -> Option<ToolAction> {
    if !modifiers.is_none() {
        return None;
    }
    if key == Key::Escape {
        return Some(ToolAction::Cancel);
    }
    match default_keymap().action_for(&press_from_key(key, modifiers), Platform::Linux)? {
        AppAction::SelectTool(tool) => Some(ToolAction::Select(tool)),
        _ => None,
    }
}

/// Texto del título para herramienta y color: `línea · color 3 #ff0000`; sin color, solo la
/// herramienta.
pub fn status_text(tool: Tool, color: Option<&str>) -> String {
    match color {
        Some(color) => format!("{} · {color}", tool.label()),
        None => tool.label().to_string(),
    }
}

/// Texto que acompaña a la herramienta en el título y la barra de estado: el color activo (con el
/// cuentagotas, el valor que acaba de elegir) o, con el borrador, lo que escribe
/// (`transparente`) o por qué no puede.
pub fn tool_color_label(
    tool: Tool,
    image: &Image,
    choice: ColorChoice,
    slots: &SlotColors,
) -> Option<String> {
    match tool {
        Tool::Eraser => Some(eraser_label(image)),
        Tool::Pencil | Tool::Line | Tool::Fill | Tool::Dropper => {
            choice_label_with(image, choice, slots)
        }
    }
}

/// Lo que un gesto pide a `px-app` además de cambiar el documento.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolEvent {
    /// El cuentagotas eligió un color.
    Picked(Picked),
    /// El borrador no puede con la imagen (aviso al pulsar).
    Refused(EraseError),
}

/// Gesto en curso, desde `Begin` hasta `End`.
#[derive(Debug, Default)]
enum Gesture {
    #[default]
    Idle,
    /// Pulsado sin nada pendiente: ranura sin color, relleno ya hecho o línea cancelada. Solo
    /// espera el `End`.
    Waiting,
    Pencil {
        stroke: Stroke,
        pixel: Pixel,
    },
    /// Como el lápiz, con el valor de borrado de la imagen.
    Erase {
        stroke: Stroke,
        pixel: Pixel,
    },
    Dropper(Dropper),
    Line {
        from: (i32, i32),
        to: (i32, i32),
        pixel: Pixel,
        width: u32,
        height: u32,
    },
}

/// Herramienta activa y gesto en curso.
#[derive(Debug, Default)]
pub struct ToolState {
    tool: Tool,
    dropper_source: DropperSource,
    gesture: Gesture,
    events: Vec<ToolEvent>,
    changes: Vec<Change>,
}

impl ToolState {
    /// Lápiz y sin gesto.
    pub fn new() -> ToolState {
        ToolState::default()
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Qué lee el cuentagotas (por defecto, lo visible). Sin gesto ni opción hasta el panel de
    /// capas (3.4).
    pub fn dropper_source(&self) -> DropperSource {
        self.dropper_source
    }

    pub fn set_dropper_source(&mut self, source: DropperSource) {
        self.dropper_source = source;
    }

    /// Cierto entre `Begin` y `End`, ambos incluidos, haya o no gesto real.
    pub fn busy(&self) -> bool {
        !matches!(self.gesture, Gesture::Idle)
    }

    /// Vacía y devuelve lo que los gestos piden a `px-app` (elecciones del cuentagotas y avisos del
    /// borrador), en orden.
    pub fn take_events(&mut self) -> Vec<ToolEvent> {
        std::mem::take(&mut self.events)
    }

    /// Vacía y devuelve los cambios aplicados al documento desde la última vez, en orden: el
    /// `state_id` que dejó cada `apply` y los bloques que tocó (para el registro de la sesión y la
    /// región sucia del lienzo).
    pub fn take_changes(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.changes)
    }

    /// Aplica `command` y, si cambió algo, anota su región.
    fn apply(&mut self, doc: &mut Document, command: Command) -> Result<bool, DocumentError> {
        let image = match &command {
            Command::SetPixels { image, .. } | Command::FillSpans { image, .. } => Some(*image),
            _ => None,
        };
        let size = image
            .filter(|&id| is_cell(doc, id))
            .map(|_| Size::new(doc.width(), doc.height()));
        let blocks = size.map(|size| dirty_of(&command, size));
        let changed = doc.apply(command)?;
        if changed && let Some(blocks) = blocks {
            self.changes.push(Change {
                state: doc.state_id(),
                blocks,
            });
        }
        Ok(changed)
    }

    /// `Select` cambia de herramienta solo en reposo. `Cancel` descarta una línea pendiente
    /// (el gesto sigue `busy` hasta su `End`); con lápiz, relleno o en reposo no hace nada.
    pub fn apply_action(&mut self, action: ToolAction) {
        match action {
            ToolAction::Select(tool) if !self.busy() => self.tool = tool,
            ToolAction::Cancel if matches!(self.gesture, Gesture::Line { .. }) => {
                self.gesture = Gesture::Waiting;
            }
            _ => {}
        }
    }

    /// `Ok(true)`: el documento cambió (hay que regenerar la textura). `Ok(false)`: nada que
    /// hacer. Un `Err` deja el documento intacto y el gesto terminado o en espera de su `End`.
    pub fn canvas_event(
        &mut self,
        doc: &mut Document,
        image: ImageId,
        choice: impl Into<ColorChoice>,
        event: CanvasEvent,
    ) -> Result<bool, DocumentError> {
        let pixel =
            reference(doc, image).and_then(|target| pixel_for_choice(target, choice.into()));
        self.canvas_event_with(doc, image, pixel, event)
    }

    /// Como [`ToolState::canvas_event`], con el `Pixel` ya calculado (`None`: no hay color
    /// para esa elección, y un trazo que empieza no hace nada).
    pub fn canvas_event_with(
        &mut self,
        doc: &mut Document,
        image: ImageId,
        pixel: Option<Pixel>,
        event: CanvasEvent,
    ) -> Result<bool, DocumentError> {
        match event {
            CanvasEvent::StrokeBegin(pos) if !self.busy() => self.begin(doc, image, pixel, pos),
            CanvasEvent::StrokeBegin(_) => Ok(false),
            CanvasEvent::StrokeMove(pos) => {
                match &mut self.gesture {
                    Gesture::Pencil { stroke, pixel } | Gesture::Erase { stroke, pixel } => {
                        stroke.move_to(pos, *pixel);
                    }
                    Gesture::Dropper(dropper) => {
                        let source = self.dropper_source;
                        let picked =
                            dropper.visit_with((doc.width(), doc.height()), pos, |x, y| {
                                pick_at(doc, image, source, x, y)
                            });
                        self.events.extend(picked.map(ToolEvent::Picked));
                    }
                    Gesture::Line { to, .. } => *to = pos,
                    _ => {}
                }
                Ok(false)
            }
            CanvasEvent::StrokeEnd => match std::mem::take(&mut self.gesture) {
                Gesture::Pencil { stroke, .. } => {
                    if stroke.is_empty() {
                        return Ok(false);
                    }
                    let pixels = stroke.into_pixels();
                    self.apply(doc, Command::SetPixels { image, pixels })
                }
                Gesture::Erase { stroke, .. } => {
                    let Some(target) = doc.image(image) else {
                        return Ok(false);
                    };
                    let pixels: Vec<PixelEdit> = stroke
                        .pixels()
                        .iter()
                        .copied()
                        .filter(|edit| is_erasable(target, edit.x, edit.y))
                        .collect();
                    if pixels.is_empty() {
                        return Ok(false);
                    }
                    self.apply(doc, Command::SetPixels { image, pixels })
                }
                Gesture::Line {
                    from, to, pixel, ..
                } => match line_in(doc, image, from, to, pixel)? {
                    Some(command) => self.apply(doc, command),
                    None => Ok(false),
                },
                Gesture::Idle | Gesture::Waiting | Gesture::Dropper(_) => Ok(false),
            },
        }
    }

    /// Como [`ToolState::canvas_event`], para la entrada ordenada de
    /// `Canvas::interact_roles`: `Escape` equivale a `Cancel` (y da `Ok(false)`).
    pub fn canvas_input(
        &mut self,
        doc: &mut Document,
        image: ImageId,
        choice: impl Into<ColorChoice>,
        input: CanvasInput,
    ) -> Result<bool, DocumentError> {
        match input {
            CanvasInput::Stroke(event) => self.canvas_event(doc, image, choice, event),
            CanvasInput::Escape => {
                self.apply_action(ToolAction::Cancel);
                Ok(false)
            }
        }
    }

    /// Como [`ToolState::canvas_input`], con el `Pixel` ya calculado.
    pub fn canvas_input_with(
        &mut self,
        doc: &mut Document,
        image: ImageId,
        pixel: Option<Pixel>,
        input: CanvasInput,
    ) -> Result<bool, DocumentError> {
        match input {
            CanvasInput::Stroke(event) => self.canvas_event_with(doc, image, pixel, event),
            CanvasInput::Escape => {
                self.apply_action(ToolAction::Cancel);
                Ok(false)
            }
        }
    }

    fn begin(
        &mut self,
        doc: &mut Document,
        image: ImageId,
        pixel: Option<Pixel>,
        pos: (i32, i32),
    ) -> Result<bool, DocumentError> {
        self.gesture = Gesture::Waiting;
        let Some(target) = reference(doc, image) else {
            return Ok(false);
        };
        let (width, height) = (doc.width(), doc.height());
        match self.tool {
            Tool::Dropper => {
                let mut dropper = Dropper::new();
                let source = self.dropper_source;
                let picked = dropper.visit_with((width, height), pos, |x, y| {
                    pick_at(doc, image, source, x, y)
                });
                self.events.extend(picked.map(ToolEvent::Picked));
                self.gesture = Gesture::Dropper(dropper);
                return Ok(false);
            }
            Tool::Eraser => {
                match eraser_pixel(target) {
                    Ok(pixel) => {
                        let mut stroke = Stroke::new(width, height);
                        stroke.move_to(pos, pixel);
                        self.gesture = Gesture::Erase { stroke, pixel };
                    }
                    Err(error) => self.events.push(ToolEvent::Refused(error)),
                }
                return Ok(false);
            }
            Tool::Pencil | Tool::Line | Tool::Fill => {}
        }
        let Some(pixel) = pixel else {
            return Ok(false);
        };
        match self.tool {
            Tool::Pencil => {
                let mut stroke = Stroke::new(width, height);
                stroke.move_to(pos, pixel);
                self.gesture = Gesture::Pencil { stroke, pixel };
                Ok(false)
            }
            Tool::Line => {
                self.gesture = Gesture::Line {
                    from: pos,
                    to: pos,
                    pixel,
                    width,
                    height,
                };
                Ok(false)
            }
            Tool::Fill => match fill_in(doc, image, pos.0, pos.1, pixel)? {
                Some(command) => self.apply(doc, command),
                None => Ok(false),
            },
            Tool::Dropper | Tool::Eraser => Ok(false),
        }
    }

    /// Píxeles pendientes para la superposición: el trazo del lápiz o la línea recortada al
    /// lienzo (la misma que aplicará `draw_line`). Vacío en reposo y con el relleno.
    pub fn preview(&self) -> impl Iterator<Item = PixelEdit> + '_ {
        let pixels: Box<dyn Iterator<Item = PixelEdit> + '_> = match &self.gesture {
            Gesture::Pencil { stroke, .. } | Gesture::Erase { stroke, .. } => {
                Box::new(stroke.pixels().iter().copied())
            }
            Gesture::Line {
                from,
                to,
                pixel,
                width,
                height,
            } => {
                let pixel = *pixel;
                Box::new(Line::new(*from, *to).clip_to(*width, *height).filter_map(
                    move |(x, y)| {
                        // Tras recortar, `x` e `y` están dentro del lienzo.
                        Some(PixelEdit {
                            x: u32::try_from(x).ok()?,
                            y: u32::try_from(y).ok()?,
                            pixel,
                        })
                    },
                ))
            }
            _ => Box::new(std::iter::empty()),
        };
        pixels
    }
}
