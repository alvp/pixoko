//! Pixoko: ventana mínima que muestra una imagen con zoom entero y desplazamiento
//! (specs/hito-2-ventana.md), con lápiz y deshacer/rehacer (specs/hito-2-lapiz-deshacer.md).
//! Todavía no abre ficheros.

mod demo;
mod title;

use eframe::egui;
use px_core::{Document, Pixel};
use px_ui::canvas::{Canvas, CanvasEvent};
use px_ui::color::{pixel_for_slot, slot_label};
use px_ui::shortcuts::{Action, action_for_key};
use px_ui::stroke::Stroke;
use px_ui::texture::pixel_rgba;

use crate::title::{TitleInfo, title_text};

struct App {
    document: Document,
    canvas: Canvas,
    title: String,
    /// Ranura de color activa (1 a 9). El `Pixel` se calcula al empezar cada trazo.
    slot: u8,
    /// Trazo en curso con su píxel; el documento no cambia hasta soltar.
    pen: Option<(Stroke, Pixel)>,
    /// Último error de `apply`, `undo` o `redo`, hasta la siguiente acción correcta.
    error: Option<String>,
}

impl App {
    fn new(document: Document) -> App {
        let mut canvas = Canvas::new();
        if let Some(image) = document.image(document.main_image()) {
            canvas.set_image(image);
        }
        App {
            document,
            canvas,
            title: String::new(),
            slot: 1,
            pen: None,
            error: None,
        }
    }

    /// Aplica los eventos de trazo del fotograma: el trazo se acumula y al terminar es un solo
    /// paso de deshacer.
    fn handle_strokes(&mut self, ctx: &egui::Context, events: Vec<CanvasEvent>) {
        for event in events {
            match event {
                CanvasEvent::StrokeBegin(pos) => {
                    let image = self.document.image(self.document.main_image());
                    self.pen = image.and_then(|image| {
                        let pixel = pixel_for_slot(image, self.slot)?;
                        let mut stroke = Stroke::new(image.width(), image.height());
                        stroke.move_to(pos, pixel);
                        Some((stroke, pixel))
                    });
                }
                CanvasEvent::StrokeMove(pos) => {
                    if let Some((stroke, pixel)) = &mut self.pen {
                        stroke.move_to(pos, *pixel);
                    }
                }
                CanvasEvent::StrokeEnd => {
                    if let Some((stroke, _)) = self.pen.take() {
                        let id = self.document.main_image();
                        let result = stroke.commit(&mut self.document, id);
                        self.finish(ctx, result);
                    }
                }
            }
        }
    }

    /// Atajos de teclado. Con un trazo en curso se ignoran.
    fn handle_keys(&mut self, ctx: &egui::Context, events: &[egui::Event]) {
        if self.pen.is_some() {
            return;
        }
        for event in events {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            match action_for_key(*key, *modifiers) {
                Some(Action::Undo) if self.document.can_undo() => {
                    let result = self.document.undo().map(|()| true);
                    self.finish(ctx, result);
                }
                Some(Action::Redo) if self.document.can_redo() => {
                    let result = self.document.redo().map(|()| true);
                    self.finish(ctx, result);
                }
                Some(Action::SelectColor(slot)) => {
                    let image = self.document.image(self.document.main_image());
                    if image.is_some_and(|image| pixel_for_slot(image, slot).is_some()) {
                        self.slot = slot;
                    }
                }
                _ => {}
            }
        }
    }

    /// `Ok(true)`: algo cambió y la textura se regenera entera (las regiones son de la unidad
    /// 14). `Ok(false)`: nada que hacer. `Err`: el documento quedó intacto y el error va al
    /// título.
    fn finish<E: std::fmt::Display>(&mut self, ctx: &egui::Context, result: Result<bool, E>) {
        match result {
            Ok(changed) => {
                self.error = None;
                if changed && let Some(image) = self.document.image(self.document.main_image()) {
                    self.canvas.refresh(ctx, image);
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let strokes = self.canvas.interact(ui);
        self.handle_strokes(&ctx, strokes);
        let events = ui.input(|i| i.events.clone());
        self.handle_keys(&ctx, &events);

        let image = self.document.image(self.document.main_image());
        let overlay = self
            .pen
            .iter()
            .flat_map(|(stroke, _)| stroke.pixels())
            .filter_map(|edit| {
                let color = pixel_rgba(image?, edit.pixel)?;
                Some((edit.x, edit.y, color))
            });
        let info = self.canvas.paint(ui, overlay);

        let title = title_text(&TitleInfo {
            size: image.map(|image| (image.width(), image.height())),
            zoom: info.zoom,
            hover: info.hover,
            color: image.and_then(|image| slot_label(image, self.slot)),
            can_undo: self.document.can_undo(),
            can_redo: self.document.can_redo(),
            error: info
                .error
                .map(|error| error.to_string())
                .or_else(|| self.error.clone()),
        });
        if title != self.title {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pixoko")
            .with_inner_size([960.0, 720.0]),
        ..eframe::NativeOptions::default()
    };
    let document = Document::new(demo::demo_image());
    eframe::run_native(
        "Pixoko",
        options,
        Box::new(|_cc| Ok(Box::new(App::new(document)))),
    )
}
