//! Pixoko: ventana mínima que muestra una imagen con zoom entero y desplazamiento
//! (specs/hito-2-ventana.md), con lápiz y deshacer/rehacer (specs/hito-2-lapiz-deshacer.md)
//! y las herramientas línea y relleno (specs/hito-2-linea-relleno-app.md). Abre PNG por
//! argumento o arrastrando, y los guarda con `Ctrl`+`S` (specs/hito-2-abrir-guardar.md).

mod confirm;
mod demo;
mod files;
mod session;
mod title;

use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui;
use px_core::Document;
use px_ui::canvas::Canvas;
use px_ui::color::{pixel_for_slot, slot_label};
use px_ui::notice::{NOTICE_SECS, Notice};
use px_ui::shortcuts::{Action, FileAction, action_for_key, file_action_for_key};
use px_ui::texture::pixel_rgba;
use px_ui::tool::{ToolAction, ToolState, tool_action_for_key};

use crate::confirm::{Confirm, Confirmation, Request, close_warning, open_warning};
use crate::files::{file_label, parse_args, pick_dropped};
use crate::session::{Session, adjust_slot, describe_saved};
use crate::title::{TitleInfo, title_with_file, window_title};

struct App {
    session: Session,
    canvas: Canvas,
    title: String,
    /// Ranura de color activa (1 a 9). El `Pixel` se calcula al empezar cada gesto.
    slot: u8,
    /// Herramienta activa y gesto en curso; el documento no cambia hasta soltar.
    tools: ToolState,
    /// Último error de una herramienta, `undo` o `redo`; caduca a los `NOTICE_SECS`.
    notice: Option<Notice>,
    /// Confirmación por repetición al abrir o cerrar con cambios sin guardar.
    confirm: Confirm,
    /// Fichero del argumento de la línea de comandos, pendiente de abrir en el primer fotograma.
    cli_path: Option<PathBuf>,
}

impl App {
    fn new(document: Document, cli_path: Option<PathBuf>) -> App {
        let mut canvas = Canvas::new();
        if let Some(image) = document.image(document.main_image()) {
            canvas.set_image(image);
        }
        App {
            session: Session::sample(document),
            canvas,
            title: String::new(),
            slot: 1,
            tools: ToolState::new(),
            notice: None,
            confirm: Confirm::new(),
            cli_path,
        }
    }

    /// Un aviso en el título durante `NOTICE_SECS`; sustituye al anterior.
    fn notify(&mut self, ctx: &egui::Context, text: String) {
        let now = ctx.input(|i| i.time);
        self.notice = Some(Notice::new(text, now));
        ctx.request_repaint_after(Duration::from_secs_f64(NOTICE_SECS));
    }

    /// Abre `path` (del argumento o soltado). Con cambios sin guardar pide confirmación por
    /// repetición; con un trazo en curso se ignora. `ignored`: ficheros soltados de más.
    fn open_file(&mut self, ctx: &egui::Context, path: &Path, ignored: usize, from_cli: bool) {
        if self.tools.busy() {
            self.notify(
                ctx,
                "se ignora el fichero: hay un trazo en curso".to_string(),
            );
            return;
        }
        let mut discard = false;
        if self.session.modified() {
            let now = ctx.input(|i| i.time);
            match self.confirm.request(Request::Open(path.to_path_buf()), now) {
                Confirmation::Armed => {
                    self.notify(ctx, open_warning(&file_label(path)));
                    return;
                }
                Confirmation::Confirmed => discard = true,
            }
        }
        let max_side = ctx.input(|i| i.max_texture_side);
        match self.session.open(path, max_side, discard) {
            Ok(mut text) => {
                self.confirm.clear();
                if let Some(image) = self
                    .session
                    .document
                    .image(self.session.document.main_image())
                {
                    self.canvas.set_image(image);
                    self.slot = adjust_slot(image, self.slot);
                }
                if ignored > 0 {
                    text.push_str(&format!(" · se ignoran {ignored} más"));
                }
                self.notify(ctx, text);
            }
            Err(error) => {
                if from_cli {
                    eprintln!("px-app: {error}");
                }
                self.notify(ctx, error.to_string());
            }
        }
    }

    /// `Ctrl`+`S` (`as_copy` falso) o `Ctrl`+`Mayús`+`S`. Sin ruta, escribe en el directorio
    /// actual.
    fn save_file(&mut self, ctx: &egui::Context, as_copy: bool) {
        let result = std::env::current_dir()
            .map_err(|error| crate::files::FileError::Write(error.to_string()))
            .and_then(|dir| {
                if as_copy {
                    self.session.save_as(&dir)
                } else {
                    self.session.save(&dir)
                }
            });
        match result {
            Ok(saved) => {
                self.confirm.clear();
                self.notify(ctx, describe_saved(&saved));
            }
            Err(error) => self.notify(ctx, error.to_string()),
        }
    }

    /// Cierre de la ventana con cambios sin guardar: el primero se cancela con aviso y el
    /// segundo, en `CONFIRM_SECS`, cierra.
    fn handle_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || !self.session.modified() {
            return;
        }
        let now = ctx.input(|i| i.time);
        if self.confirm.request(Request::Close, now) == Confirmation::Armed {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.notify(ctx, close_warning());
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
            if let Some(action) = file_action_for_key(*key, *modifiers) {
                if !self.tools.busy() {
                    self.save_file(ctx, action == FileAction::SaveAs);
                }
                continue;
            }
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
                Some(Action::Undo) if self.session.document.can_undo() => {
                    let result = self.session.document.undo().map(|()| true);
                    self.finish(ctx, result);
                }
                Some(Action::Redo) if self.session.document.can_redo() => {
                    let result = self.session.document.redo().map(|()| true);
                    self.finish(ctx, result);
                }
                Some(Action::SelectColor(slot)) => {
                    let image = self
                        .session
                        .document
                        .image(self.session.document.main_image());
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
                if let Some(image) = self
                    .session
                    .document
                    .image(self.session.document.main_image())
                {
                    self.canvas.refresh(ctx, image);
                }
            }
            Ok(false) => {}
            Err(error) => self.notify(ctx, error.to_string()),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_close(&ctx);
        if let Some(path) = self.cli_path.take() {
            self.open_file(&ctx, &path, 0, true);
        }
        let dropped: Vec<Option<PathBuf>> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|file| Some(file.path().to_path_buf()))
                .collect()
        });
        if let Some((path, ignored)) = pick_dropped(&dropped) {
            self.open_file(&ctx, &path, ignored, false);
        }

        let id = self.session.document.main_image();
        for input in self.canvas.interact_ordered(ui) {
            let result = self
                .tools
                .canvas_input(&mut self.session.document, id, self.slot, input);
            self.finish(&ctx, result);
        }
        let events = ui.input(|i| i.events.clone());
        self.handle_keys(&ctx, &events);

        let image = self.session.document.image(id);
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
                can_undo: self.session.document.can_undo(),
                can_redo: self.session.document.can_redo(),
                error: info.error.map(|error| error.to_string()).or(notice),
            },
        );
        let label = self.session.path().map(file_label);
        let title = title_with_file(label.as_deref(), self.session.modified(), &title);
        if title != self.title {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }
}

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let cli_path = match parse_args(&args) {
        Ok(path) => path,
        Err(usage) => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    };
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
        Box::new(|_cc| Ok(Box::new(App::new(document, cli_path)))),
    )
}
