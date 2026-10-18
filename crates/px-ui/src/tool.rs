//! Herramientas (lápiz, línea, relleno, cuentagotas y borrador) como lógica pura
//! (specs/hito-2-linea-relleno-app.md y specs/hito-2-cuentagotas-borrador.md).
//!
//! Sin tipos de egui en la API de [`ToolState`]: quien dibuja traduce los eventos de egui a
//! [`CanvasEvent`] / [`CanvasInput`] y las teclas a [`ToolAction`].

use egui::{Key, Modifiers};
use px_core::{Command, Document, DocumentError, Image, ImageId, Pixel, PixelEdit};
use px_raster::{Line, draw_line, flood_fill};

use crate::canvas::{CanvasEvent, CanvasInput};
use crate::color::{ColorChoice, SlotColors, choice_label_with, pixel_for_choice};
use crate::pick::{Dropper, EraseError, Picked, eraser_label, eraser_pixel, is_erasable};
use crate::stroke::Stroke;

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

/// `P`, `L`, `F`, `I` y `E` eligen herramienta y `Escape` cancela, solo sin ningún modificador (así
/// `Ctrl`+`P`, `Ctrl`+`L`, `Ctrl`+`F`, `Ctrl`+`I` y `Ctrl`+`E` no hacen nada). Es aparte de
/// [`crate::shortcuts::action_for_key`], que da `None` para estas teclas.
pub fn tool_action_for_key(key: Key, modifiers: Modifiers) -> Option<ToolAction> {
    if !modifiers.is_none() {
        return None;
    }
    match key {
        Key::P => Some(ToolAction::Select(Tool::Pencil)),
        Key::L => Some(ToolAction::Select(Tool::Line)),
        Key::F => Some(ToolAction::Select(Tool::Fill)),
        Key::I => Some(ToolAction::Select(Tool::Dropper)),
        Key::E => Some(ToolAction::Select(Tool::Eraser)),
        Key::Escape => Some(ToolAction::Cancel),
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
        Tool::Eraser => Some(eraser_label(image).to_string()),
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
    gesture: Gesture,
    events: Vec<ToolEvent>,
}

impl ToolState {
    /// Lápiz y sin gesto.
    pub fn new() -> ToolState {
        ToolState::default()
    }

    pub fn tool(&self) -> Tool {
        self.tool
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
        let pixel = doc
            .image(image)
            .and_then(|target| pixel_for_choice(target, choice.into()));
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
                        let picked = doc
                            .image(image)
                            .and_then(|target| dropper.visit(target, pos));
                        self.events.extend(picked.map(ToolEvent::Picked));
                    }
                    Gesture::Line { to, .. } => *to = pos,
                    _ => {}
                }
                Ok(false)
            }
            CanvasEvent::StrokeEnd => match std::mem::take(&mut self.gesture) {
                Gesture::Pencil { stroke, .. } => stroke.commit(doc, image),
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
                    doc.apply(Command::SetPixels { image, pixels })
                }
                Gesture::Line {
                    from, to, pixel, ..
                } => draw_line(doc, image, from, to, pixel),
                Gesture::Idle | Gesture::Waiting | Gesture::Dropper(_) => Ok(false),
            },
        }
    }

    /// Como [`ToolState::canvas_event`], para la entrada ordenada de
    /// `Canvas::interact_ordered`: `Escape` equivale a `Cancel` (y da `Ok(false)`).
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
        let Some(target) = doc.image(image) else {
            return Ok(false);
        };
        match self.tool {
            Tool::Dropper => {
                let mut dropper = Dropper::new();
                self.events
                    .extend(dropper.visit(target, pos).map(ToolEvent::Picked));
                self.gesture = Gesture::Dropper(dropper);
                return Ok(false);
            }
            Tool::Eraser => {
                match eraser_pixel(target) {
                    Ok(pixel) => {
                        let mut stroke = Stroke::new(target.width(), target.height());
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
        let (width, height) = (target.width(), target.height());
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
            Tool::Fill => flood_fill(doc, image, pos.0, pos.1, pixel),
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
