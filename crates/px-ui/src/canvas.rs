//! Lienzo de egui: pinta una `Image` con zoom entero y desplazamiento
//! (specs/hito-2-ventana.md, «Lienzo»).
//!
//! La geometría vive en [`crate::view`] (sin tipos de egui); aquí solo se traducen las entradas
//! de egui a operaciones de [`View`] y el resultado a un rectángulo y una textura.

use std::error::Error;
use std::fmt;

use egui::{
    Color32, ColorImage, Event, Key, PointerButton, Rect, Sense, TextureHandle, TextureOptions, Ui,
    pos2, vec2,
};
use px_core::Image;

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
    }

    /// Textura de la imagen, una vez subida.
    pub fn texture_id(&self) -> Option<egui::TextureId> {
        self.texture.as_ref().map(TextureHandle::id)
    }

    /// Dibuja el lienzo en todo el espacio libre de `ui` y procesa sus entradas.
    ///
    /// Rueda o pellizco: un paso de zoom por evento, anclado al cursor. Arrastre con el botón
    /// central, o con espacio y el izquierdo: desplazamiento. `Home`: ajuste. `+` y `-`: zoom
    /// en el centro.
    pub fn show(&mut self, ui: &mut Ui) -> CanvasInfo {
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

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, BACKGROUND);

        let (Some(texture), Some(size)) = (&self.texture, self.size) else {
            return CanvasInfo {
                zoom: Zoom::default(),
                hover: None,
                image_rect: None,
                error: self.error.clone(),
            };
        };

        let mut view = self.view.unwrap_or_else(|| View::fit(viewport, size));
        let center = (
            i64::from(viewport.width) / 2,
            i64::from(viewport.height) / 2,
        );
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
                    _ => {}
                },
                _ => {}
            }
        }
        // Pellizco y Ctrl + rueda: egui los resume en `zoom_delta`.
        let pinch = ui.input(|i| i.zoom_delta());
        if (pinch - 1.0).abs() > 1e-3
            && let Some(anchor) = pointer
        {
            zoom_step(&mut view, pinch > 1.0, anchor);
        }
        let panning = response.dragged_by(PointerButton::Middle)
            || (response.dragged_by(PointerButton::Primary)
                && ui.input(|i| i.key_down(Key::Space)));
        if panning {
            let delta = response.drag_delta();
            view.pan(physical(delta.x), physical(delta.y), viewport, size);
        }
        view.clamp(viewport, size);
        self.view = Some(view);

        let placed = view.image_rect(size);
        let image_rect = Rect::from_min_size(
            rect.min + vec2(placed.x as f32 / ppp, placed.y as f32 / ppp),
            vec2(placed.width as f32 / ppp, placed.height as f32 / ppp),
        );
        painter.image(
            texture.id(),
            image_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );

        CanvasInfo {
            zoom: view.zoom(),
            hover: pointer.and_then(|p| view.screen_to_image(p, size)),
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
        let pixels = ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            &to_rgba8(&image),
        );
        // Sin suavizado al ampliar ni al reducir, y sin mipmaps.
        self.texture = Some(
            ui.ctx()
                .load_texture("px-image", pixels, TextureOptions::NEAREST),
        );
    }
}

fn zoom_step(view: &mut View, zoom_in: bool, anchor: (i64, i64)) {
    if zoom_in {
        view.zoom_in(anchor);
    } else {
        view.zoom_out(anchor);
    }
}
