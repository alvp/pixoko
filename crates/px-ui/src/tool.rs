//! Herramientas (lápiz, línea y relleno) como lógica pura (specs/hito-2-linea-relleno-app.md).
//!
//! Sin tipos de egui en la API de [`ToolState`]: quien dibuja traduce los eventos de egui a
//! [`CanvasEvent`] / [`CanvasInput`] y las teclas a [`ToolAction`].

use egui::{Key, Modifiers};
use px_core::{Document, DocumentError, ImageId, Pixel, PixelEdit};
use px_raster::{Line, draw_line, flood_fill};

use crate::canvas::{CanvasEvent, CanvasInput};
use crate::color::pixel_for_slot;
use crate::stroke::Stroke;

/// Herramienta activa.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Pencil,
    Line,
    Fill,
}

impl Tool {
    /// Nombre para el título: `lápiz`, `línea` o `relleno`.
    pub fn label(self) -> &'static str {
        match self {
            Tool::Pencil => "lápiz",
            Tool::Line => "línea",
            Tool::Fill => "relleno",
        }
    }
}

/// Lo que pide una tecla de herramienta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolAction {
    Select(Tool),
    Cancel,
}

/// `P`, `L` y `F` eligen herramienta y `Escape` cancela, solo sin ningún modificador (así
/// `Ctrl`+`P`, `Ctrl`+`L` y `Ctrl`+`F` no hacen nada). Es aparte de
/// [`crate::shortcuts::action_for_key`], que da `None` para estas teclas.
pub fn tool_action_for_key(key: Key, modifiers: Modifiers) -> Option<ToolAction> {
    if !modifiers.is_none() {
        return None;
    }
    match key {
        Key::P => Some(ToolAction::Select(Tool::Pencil)),
        Key::L => Some(ToolAction::Select(Tool::Line)),
        Key::F => Some(ToolAction::Select(Tool::Fill)),
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
        slot: u8,
        event: CanvasEvent,
    ) -> Result<bool, DocumentError> {
        match event {
            CanvasEvent::StrokeBegin(pos) if !self.busy() => self.begin(doc, image, slot, pos),
            CanvasEvent::StrokeBegin(_) => Ok(false),
            CanvasEvent::StrokeMove(pos) => {
                match &mut self.gesture {
                    Gesture::Pencil { stroke, pixel } => stroke.move_to(pos, *pixel),
                    Gesture::Line { to, .. } => *to = pos,
                    _ => {}
                }
                Ok(false)
            }
            CanvasEvent::StrokeEnd => match std::mem::take(&mut self.gesture) {
                Gesture::Pencil { stroke, .. } => stroke.commit(doc, image),
                Gesture::Line {
                    from, to, pixel, ..
                } => draw_line(doc, image, from, to, pixel),
                Gesture::Idle | Gesture::Waiting => Ok(false),
            },
        }
    }

    /// Como [`ToolState::canvas_event`], para la entrada ordenada de
    /// `Canvas::interact_ordered`: `Escape` equivale a `Cancel` (y da `Ok(false)`).
    pub fn canvas_input(
        &mut self,
        doc: &mut Document,
        image: ImageId,
        slot: u8,
        input: CanvasInput,
    ) -> Result<bool, DocumentError> {
        match input {
            CanvasInput::Stroke(event) => self.canvas_event(doc, image, slot, event),
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
        slot: u8,
        pos: (i32, i32),
    ) -> Result<bool, DocumentError> {
        self.gesture = Gesture::Waiting;
        let Some(target) = doc.image(image) else {
            return Ok(false);
        };
        let Some(pixel) = pixel_for_slot(target, slot) else {
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
        }
    }

    /// Píxeles pendientes para la superposición: el trazo del lápiz o la línea recortada al
    /// lienzo (la misma que aplicará `draw_line`). Vacío en reposo y con el relleno.
    pub fn preview(&self) -> impl Iterator<Item = PixelEdit> + '_ {
        let pixels: Box<dyn Iterator<Item = PixelEdit> + '_> = match &self.gesture {
            Gesture::Pencil { stroke, .. } => Box::new(stroke.pixels().iter().copied()),
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
