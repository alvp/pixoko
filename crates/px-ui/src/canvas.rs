//! Lienzo de egui: pinta una `Image` con zoom entero y desplazamiento
//! (specs/hito-2-ventana.md, «Lienzo»).
//!
//! La geometría vive en [`crate::view`] (sin tipos de egui); aquí solo se traducen las entradas
//! de egui a operaciones de [`View`] y el resultado a un rectángulo y una textura.

use std::error::Error;
use std::fmt;

use egui::{
    Color32, ColorImage, Context, Event, Key, PointerButton, Rect, Sense, TextureHandle,
    TextureOptions, Ui, pos2, vec2,
};
use px_core::{Image, Rgba};

use crate::texture::to_rgba8;
use crate::view::{Size, View, Zoom};

/// Fondo liso detrás de la imagen (el tablero de la transparencia queda para más adelante).
const BACKGROUND: Color32 = Color32::from_gray(40);

/// Por qué no se puede mostrar la imagen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanvasError {
    /// La imagen no cabe en una textura de la GPU (dividirla en teselas es de la unidad 14).
    TooLarge { width: u32, height: u32, max: usize },
}

impl fmt::Display for CanvasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanvasError::TooLarge { width, height, max } => write!(
                f,
                "la imagen de {width}×{height} supera el lado máximo de textura ({max})"
            ),
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
    /// Dibujar (botón izquierdo).
    Draw,
}

/// Lo que `interact` calcula para que `paint` lo dibuje.
#[derive(Clone, Copy)]
struct Frame {
    rect: Rect,
    viewport: Size,
    ppp: f32,
    hover: Option<(u32, u32)>,
}

/// Estado del lienzo: la imagen, su textura y la vista.
#[derive(Default)]
pub struct Canvas {
    /// Imagen aún sin subir: la subida espera al primer fotograma, que es cuando egui sabe el
    /// lado máximo de textura de la GPU.
    pending: Option<Image>,
    texture: Option<TextureHandle>,
    size: Option<Size>,
    view: Option<View>,
    error: Option<CanvasError>,
    gesture: Gesture,
    frame: Option<Frame>,
}

impl Canvas {
    pub fn new() -> Canvas {
        Canvas::default()
    }

    /// Cambia la imagen mostrada. La textura se sube entera en el siguiente `show` y la vista
    /// vuelve al ajuste.
    pub fn set_image(&mut self, image: &Image) {
        self.size = Some(Size::new(image.width(), image.height()));
        self.pending = Some(image.clone());
        self.texture = None;
        self.view = None;
        self.error = None;
        self.gesture = Gesture::None;
    }

    /// Regenera la textura con el contenido de `image` **conservando la vista** (tras un trazo,
    /// un deshacer o un rehacer). Si la imagen tiene otro tamaño o aún no hay textura, se
    /// comporta como [`Canvas::set_image`]. La textura se sube entera: las regiones son de la
    /// unidad 14.
    pub fn refresh(&mut self, ctx: &Context, image: &Image) {
        let same_size = self.size == Some(Size::new(image.width(), image.height()));
        match &mut self.texture {
            Some(texture) if same_size => {
                texture.set(color_image(image), TextureOptions::NEAREST);
                ctx.request_repaint();
            }
            _ => self.set_image(image),
        }
    }

    /// Textura de la imagen, una vez subida.
    pub fn texture_id(&self) -> Option<egui::TextureId> {
        self.texture.as_ref().map(TextureHandle::id)
    }

    /// Dibuja el lienzo en todo el espacio libre de `ui` y procesa sus entradas: [`interact`]
    /// y [`paint`] seguidos, sin superposición y sin los eventos de trazo.
    ///
    /// Rueda o pellizco: un paso de zoom por evento, anclado al cursor. Arrastre con el botón
    /// central, o con espacio y el izquierdo: desplazamiento. `Home`: ajuste. `+` y `-`: zoom
    /// en el centro.
    ///
    /// [`interact`]: Canvas::interact
    /// [`paint`]: Canvas::paint
    pub fn show(&mut self, ui: &mut Ui) -> CanvasInfo {
        self.interact(ui);
        self.paint(ui, std::iter::empty())
    }

    /// Primera mitad de [`Canvas::show`]: sube la textura si falta, reserva el espacio, procesa
    /// las entradas (zoom, desplazamiento y el gesto del lápiz) y devuelve los eventos de trazo
    /// del fotograma, en orden. Debe ir antes de [`Canvas::paint`] en el mismo fotograma.
    ///
    /// El gesto se decide al pulsar: botón central, o izquierdo con espacio abajo, desplaza;
    /// izquierdo sin espacio dibuja (solo si la pulsación cae dentro del lienzo). Un segundo
    /// botón durante el gesto se ignora. El trazo sigue aunque el puntero salga del lienzo o de
    /// la ventana y termina al soltar, al perder el foco o si egui ya no ve el botón pulsado.
    pub fn interact(&mut self, ui: &mut Ui) -> Vec<CanvasEvent> {
        self.interact_ordered(ui)
            .into_iter()
            .filter_map(|input| match input {
                CanvasInput::Stroke(event) => Some(event),
                CanvasInput::Escape => None,
            })
            .collect()
    }

    /// Como [`Canvas::interact`], pero además emite `CanvasInput::Escape` (tecla `Escape` sin
    /// modificadores) **en su sitio** entre los eventos de trazo del fotograma, para que quien
    /// dibuja respete el orden de los eventos de egui. `interact` es esto sin los `Escape`.
    pub fn interact_ordered(&mut self, ui: &mut Ui) -> Vec<CanvasInput> {
        self.upload_if_pending(ui);

        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        let physical = |v: f32| (v * ppp).round() as i64;
        let viewport = Size::new(
            u32::try_from(physical(rect.width())).unwrap_or(0),
            u32::try_from(physical(rect.height())).unwrap_or(0),
        );
        let pointer = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| rect.contains(*p))
            .map(|p| (physical(p.x - rect.min.x), physical(p.y - rect.min.y)));

        let mut frame = Frame {
            rect,
            viewport,
            ppp,
            hover: None,
        };
        let (Some(_), Some(size)) = (&self.texture, self.size) else {
            self.frame = Some(frame);
            return Vec::new();
        };

        let mut view = self.view.unwrap_or_else(|| View::fit(viewport, size));
        let center = (
            i64::from(viewport.width) / 2,
            i64::from(viewport.height) / 2,
        );
        let to_image = |view: &View, p: egui::Pos2| {
            view.screen_to_image_i32(
                (physical(p.x - rect.min.x), physical(p.y - rect.min.y)),
                size,
            )
        };
        let space_down = ui.input(|i| i.key_down(Key::Space));
        let mut strokes = Vec::new();
        let events = ui.input(|i| i.events.clone());
        for event in &events {
            match event {
                Event::MouseWheel {
                    delta, modifiers, ..
                } if !modifiers.command && delta.y != 0.0 => {
                    if let Some(anchor) = pointer {
                        zoom_step(&mut view, delta.y > 0.0, anchor);
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if !modifiers.command => match key {
                    Key::Home => view = View::fit(viewport, size),
                    Key::Plus | Key::Equals => zoom_step(&mut view, true, center),
                    Key::Minus => zoom_step(&mut view, false, center),
                    Key::Escape if modifiers.is_none() => strokes.push(CanvasInput::Escape),
                    _ => {}
                },
                Event::PointerButton {
                    pos,
                    button,
                    pressed: true,
                    ..
                } if self.gesture == Gesture::None && rect.contains(*pos) => {
                    self.gesture = match button {
                        PointerButton::Middle => Gesture::Pan(PointerButton::Middle),
                        PointerButton::Primary if space_down => {
                            Gesture::Pan(PointerButton::Primary)
                        }
                        PointerButton::Primary => Gesture::Draw,
                        _ => Gesture::None,
                    };
                    if self.gesture == Gesture::Draw {
                        strokes.push(CanvasInput::Stroke(CanvasEvent::StrokeBegin(to_image(
                            &view, *pos,
                        ))));
                    }
                }
                Event::PointerButton {
                    pos,
                    button,
                    pressed: false,
                    ..
                } => match self.gesture {
                    Gesture::Draw if *button == PointerButton::Primary => {
                        strokes.push(CanvasInput::Stroke(CanvasEvent::StrokeMove(to_image(
                            &view, *pos,
                        ))));
                        strokes.push(CanvasInput::Stroke(CanvasEvent::StrokeEnd));
                        self.gesture = Gesture::None;
                    }
                    Gesture::Pan(owner) if *button == owner => self.gesture = Gesture::None,
                    _ => {}
                },
                Event::PointerMoved(pos) if self.gesture == Gesture::Draw => {
                    strokes.push(CanvasInput::Stroke(CanvasEvent::StrokeMove(to_image(
                        &view, *pos,
                    ))));
                }
                Event::WindowFocused(false) => {
                    if self.gesture == Gesture::Draw {
                        strokes.push(CanvasInput::Stroke(CanvasEvent::StrokeEnd));
                    }
                    self.gesture = Gesture::None;
                }
                _ => {}
            }
        }
        // Red de seguridad: un soltar que no llegó (por ejemplo, fuera de la ventana).
        match self.gesture {
            Gesture::Draw if !ui.input(|i| i.pointer.primary_down()) => {
                strokes.push(CanvasInput::Stroke(CanvasEvent::StrokeEnd));
                self.gesture = Gesture::None;
            }
            Gesture::Pan(button) if !ui.input(|i| i.pointer.button_down(button)) => {
                self.gesture = Gesture::None;
            }
            _ => {}
        }
        // Pellizco y Ctrl + rueda: egui los resume en `zoom_delta`.
        let pinch = ui.input(|i| i.zoom_delta());
        if (pinch - 1.0).abs() > 1e-3
            && let Some(anchor) = pointer
        {
            zoom_step(&mut view, pinch > 1.0, anchor);
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
    /// Si no hubo `interact` antes en el fotograma, no pinta nada.
    pub fn paint(
        &mut self,
        ui: &mut Ui,
        overlay: impl IntoIterator<Item = (u32, u32, Rgba)>,
    ) -> CanvasInfo {
        let Some(frame) = self.frame else {
            return CanvasInfo {
                zoom: Zoom::default(),
                hover: None,
                image_rect: None,
                error: self.error.clone(),
            };
        };
        let painter = ui.painter_at(frame.rect);
        painter.rect_filled(frame.rect, 0.0, BACKGROUND);

        let (Some(texture), Some(size), Some(view)) = (&self.texture, self.size, self.view) else {
            return CanvasInfo {
                zoom: Zoom::default(),
                hover: None,
                image_rect: None,
                error: self.error.clone(),
            };
        };
        let ppp = frame.ppp;
        let to_points = |p: (i64, i64)| frame.rect.min + vec2(p.0 as f32 / ppp, p.1 as f32 / ppp);

        let placed = view.image_rect(size);
        let image_rect = Rect::from_min_size(
            to_points((placed.x, placed.y)),
            vec2(placed.width as f32 / ppp, placed.height as f32 / ppp),
        );
        painter.image(
            texture.id(),
            image_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );

        let (view_w, view_h) = (
            i64::from(frame.viewport.width),
            i64::from(frame.viewport.height),
        );
        for (x, y, color) in overlay {
            let min = view.image_to_screen(x, y);
            let max = view.image_to_screen(x.saturating_add(1), y.saturating_add(1));
            if max.0 <= 0 || max.1 <= 0 || min.0 >= view_w || min.1 >= view_h {
                continue;
            }
            let block = Rect::from_min_max(to_points(min), to_points(max));
            if color.a < 255 {
                painter.rect_filled(block, 0.0, BACKGROUND);
            }
            if color.a > 0 {
                let color = Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
                painter.rect_filled(block, 0.0, color);
            }
        }

        CanvasInfo {
            zoom: view.zoom(),
            hover: frame.hover,
            image_rect: Some(image_rect),
            error: None,
        }
    }

    fn upload_if_pending(&mut self, ui: &Ui) {
        let Some(image) = self.pending.take() else {
            return;
        };
        let max = ui.input(|i| i.max_texture_side);
        let (width, height) = (image.width(), image.height());
        // `load_texture` no admite imágenes mayores que el lado máximo: se comprueba antes.
        if width as usize > max || height as usize > max {
            self.error = Some(CanvasError::TooLarge { width, height, max });
            return;
        }
        // Sin suavizado al ampliar ni al reducir, y sin mipmaps.
        self.texture = Some(ui.ctx().load_texture(
            "px-image",
            color_image(&image),
            TextureOptions::NEAREST,
        ));
    }
}

fn color_image(image: &Image) -> ColorImage {
    ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        &to_rgba8(image),
    )
}

fn zoom_step(view: &mut View, zoom_in: bool, anchor: (i64, i64)) {
    if zoom_in {
        view.zoom_in(anchor);
    } else {
        view.zoom_out(anchor);
    }
}
