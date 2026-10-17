//! Pixoko: ventana mínima que muestra una imagen con zoom entero y desplazamiento
//! (specs/hito-2-ventana.md). Todavía no abre ficheros ni edita.

mod demo;

use eframe::egui;
use px_core::Document;
use px_ui::canvas::{Canvas, CanvasInfo};

struct App {
    document: Document,
    canvas: Canvas,
    title: String,
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
        }
    }

    /// Texto del título de la ventana: tamaño, zoom y píxel bajo el cursor, o el error.
    /// (Sin fuente en egui todavía, el estado va en el título.)
    fn title_for(&self, info: &CanvasInfo) -> String {
        if let Some(error) = &info.error {
            return format!("Pixoko: {error}");
        }
        let size = self
            .document
            .image(self.document.main_image())
            .map(|image| format!("{}×{}", image.width(), image.height()))
            .unwrap_or_default();
        match info.hover {
            Some((x, y)) => format!("Pixoko: {size} · {} · ({x}, {y})", info.zoom),
            None => format!("Pixoko: {size} · {}", info.zoom),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let info = self.canvas.show(ui);
        let title = self.title_for(&info);
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
