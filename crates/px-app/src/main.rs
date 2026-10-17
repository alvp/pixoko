//! Pixoko: ventana mínima que muestra una imagen con zoom entero y desplazamiento
//! (specs/hito-2-ventana.md), con lápiz y deshacer/rehacer (specs/hito-2-lapiz-deshacer.md)
//! y las herramientas línea y relleno (specs/hito-2-linea-relleno-app.md). Todavía no abre
//! ficheros.

mod demo;
mod title;

use std::time::Duration;

use eframe::egui;
use px_core::Document;
use px_ui::canvas::Canvas;
use px_ui::color::{pixel_for_slot, slot_label};
use px_ui::notice::{NOTICE_SECS, Notice};
use px_ui::shortcuts::{Action, action_for_key};
use px_ui::texture::pixel_rgba;
use px_ui::tool::{ToolAction, ToolState, tool_action_for_key};

use crate::title::{TitleInfo, window_title};

struct App {
    document: Document,
    canvas: Canvas,
    title: String,
    /// Ranura de color activa (1 a 9). El `Pixel` se calcula al empezar cada gesto.
    slot: u8,
    /// Herramienta activa y gesto en curso; el documento no cambia hasta soltar.
    tools: ToolState,
    /// Último error de una herramienta, `undo` o `redo`; caduca a los `NOTICE_SECS`.
    notice: Option<Notice>,
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
            tools: ToolState::new(),
            notice: None,
        }
    }

    /// Atajos de teclado. Con un gesto en curso se ignoran (`Escape` ya llega en orden por
    /// `Canvas::interact_ordered`, y la selección de herramienta la ignora `ToolState`).
    fn handle_keys(&mut self, ctx: &egui::Context, events: &[egui::Event]) {
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
            if let Some(action) = tool_action_for_key(*key, *modifiers) {
                if let ToolAction::Select(_) = action {
                    self.tools.apply_action(action);
                }
                continue;
            }
            if self.tools.busy() {
                continue;
            }
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
    /// título durante `NOTICE_SECS`.
    fn finish<E: std::fmt::Display>(&mut self, ctx: &egui::Context, result: Result<bool, E>) {
        match result {
            Ok(true) => {
                if let Some(image) = self.document.image(self.document.main_image()) {
                    self.canvas.refresh(ctx, image);
                }
            }
            Ok(false) => {}
            Err(error) => {
                let now = ctx.input(|i| i.time);
                self.notice = Some(Notice::new(error.to_string(), now));
                ctx.request_repaint_after(Duration::from_secs_f64(NOTICE_SECS));
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let id = self.document.main_image();
        for input in self.canvas.interact_ordered(ui) {
            let result = self
                .tools
                .canvas_input(&mut self.document, id, self.slot, input);
            self.finish(&ctx, result);
        }
        let events = ui.input(|i| i.events.clone());
        self.handle_keys(&ctx, &events);

        let image = self.document.image(id);
        let overlay = self.tools.preview().filter_map(|edit| {
            let color = pixel_rgba(image?, edit.pixel)?;
            Some((edit.x, edit.y, color))
        });
        let info = self.canvas.paint(ui, overlay);

        let now = ui.input(|i| i.time);
        let notice = self
            .notice
            .as_ref()
            .and_then(|notice| notice.visible(now))
            .map(str::to_string);
        if notice.is_none() {
            self.notice = None;
        }
        let title = window_title(
            self.tools.tool(),
            &TitleInfo {
                size: image.map(|image| (image.width(), image.height())),
                zoom: info.zoom,
                hover: info.hover,
                color: image.and_then(|image| slot_label(image, self.slot)),
                can_undo: self.document.can_undo(),
                can_redo: self.document.can_redo(),
                error: info.error.map(|error| error.to_string()).or(notice),
            },
        );
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
