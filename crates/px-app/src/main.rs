//! Pixoko: ventana mínima que muestra una imagen con zoom entero y desplazamiento
//! (specs/hito-2-ventana.md), con lápiz y deshacer/rehacer (specs/hito-2-lapiz-deshacer.md)
//! y las herramientas línea y relleno (specs/hito-2-linea-relleno-app.md). Abre PNG por
//! argumento o arrastrando, y los guarda con `Ctrl`+`S` (specs/hito-2-abrir-guardar.md).

mod confirm;
mod demo;
mod files;
mod licenses;
mod session;
mod status_error;
mod title;

use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui;
use px_core::Document;
use px_ui::canvas::Canvas;
use px_ui::color::{choice_label_with, key_choice};
use px_ui::color_edit::EditTarget;
use px_ui::notice::{NOTICE_SECS, Notice};
use px_ui::shortcuts::{Action, FileAction, action_for_key, file_action_for_key, gate_keys};
use px_ui::texture::pixel_rgba;
use px_ui::tool::{ToolAction, ToolState, tool_action_for_key};

use crate::confirm::{Confirm, Confirmation, Request, close_warning, open_warning};
use crate::files::{file_label, parse_args, pick_dropped};
use crate::licenses::wants_licenses;
use crate::session::{Session, describe_saved};
use crate::status_error::status_error;
use crate::title::{TitleInfo, status_line, title_with_file, window_title};

struct App {
    session: Session,
    canvas: Canvas,
    title: String,
    /// Texto de la barra de estado del fotograma anterior y si es un aviso: la barra se pinta
    /// antes del lienzo, que usa el espacio que sobra, y su texto sale del estado del lienzo.
    status: (String, bool),
    /// `ppp` con el que se aplicaron el tema y la escala del texto.
    themed_ppp: f32,
    /// Herramienta activa y gesto en curso; el documento no cambia hasta soltar.
    tools: ToolState,
    /// Último error de una herramienta, `undo` o `redo`; caduca a los `NOTICE_SECS`.
    notice: Option<Notice>,
    /// Confirmación por repetición al abrir o cerrar con cambios sin guardar.
    confirm: Confirm,
    /// Fichero del argumento de la línea de comandos, pendiente de abrir en el primer fotograma.
    cli_path: Option<PathBuf>,
    /// El panel es demasiado bajo para el editor de color (la barra de estado lo avisa).
    editor_hidden: bool,
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
            status: (String::new(), false),
            themed_ppp: 0.0,
            tools: ToolState::new(),
            notice: None,
            confirm: Confirm::new(),
            cli_path,
            editor_hidden: false,
        }
    }

    /// Escala del texto y tema para el `ppp` actual (se repite al cambiar de monitor).
    fn apply_theme(&mut self, ctx: &egui::Context) {
        let ppp = ctx.pixels_per_point();
        px_ui::text::apply_scale(ctx);
        if ppp != self.themed_ppp {
            ctx.set_visuals_of(egui::Theme::Dark, px_ui::theme::visuals(ppp));
            self.themed_ppp = ppp;
        }
    }

    /// Un aviso en el título durante `NOTICE_SECS`; sustituye al anterior.
    fn notify(&mut self, ctx: &egui::Context, text: String) {
        let now = ctx.input(|i| i.time);
        self.notice = Some(Notice::new(text, now));
        ctx.request_repaint_after(Duration::from_secs_f64(NOTICE_SECS));
    }

    /// Hay un gesto en curso: un trazo del lápiz, la línea, o el arrastre de un deslizador del
    /// editor de color. Mientras dura se ignoran atajos, elecciones y ficheros.
    fn busy(&self) -> bool {
        self.tools.busy() || self.session.editor.dragging()
    }

    /// Abre `path` (del argumento o soltado). Con cambios sin guardar pide confirmación por
    /// repetición; con un trazo en curso se ignora. `ignored`: ficheros soltados de más.
    fn open_file(&mut self, ctx: &egui::Context, path: &Path, ignored: usize, from_cli: bool) {
        if self.busy() {
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
                if !self.busy() {
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
            if self.busy() {
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
                    if let Some(choice) = image.and_then(|image| key_choice(image, slot)) {
                        self.session.choice = choice;
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
        self.apply_theme(&ctx);
        px_ui::status::show(ui, &self.status.0, self.status.1);
        self.session.sync_editor();
        let enabled = !self.busy();
        let main = self.session.document.main_image();
        let mut panel = None;
        if let Some(image) = self.session.document.image(main) {
            panel = Some(px_ui::palette_panel::show_editing(
                ui,
                image,
                self.session.choice,
                &mut self.session.scroll,
                enabled,
                self.session.slots.get(image.mode()),
                &mut self.session.editor,
            ));
        }
        if let Some(panel) = panel {
            if let Some(choice) = panel.choice {
                self.session.choice = choice;
            }
            if let Some(commit) = panel.commit {
                let result = self.session.commit_edit(commit);
                self.finish(&ctx, result);
            }
            if let Some(text) = panel.notice {
                self.notify(&ctx, text);
            }
            self.editor_hidden = panel.editor_hidden;
        }
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
            let pixel = self.session.active_pixel();
            let result = self
                .tools
                .canvas_input_with(&mut self.session.document, id, pixel, input);
            self.finish(&ctx, result);
        }
        // Con un campo de texto enfocado el teclado es suyo: los atajos no ven nada.
        let events = gate_keys(&ui.input(|i| i.events.clone()), ctx.text_edit_focused());
        self.handle_keys(&ctx, &events);

        // Vista previa del color que se está editando: la textura la lleva el lienzo y vuelve al
        // documento al soltar o cancelar.
        if let Some(image) = self.session.document.image(id) {
            match self.session.editor.preview() {
                Some((EditTarget::Entry(index), color)) => {
                    self.canvas.preview_entry(&ctx, image, index, color);
                }
                _ => self.canvas.end_preview(&ctx, image),
            }
        }

        let image = self.session.document.image(id);
        let overlay = self.tools.preview().filter_map(|edit| {
            let color = pixel_rgba(image?, edit.pixel)?;
            Some((edit.x, edit.y, color))
        });
        let canvas_info = self.canvas.paint(ui, overlay);

        let now = ui.input(|i| i.time);
        let notice = self
            .notice
            .as_ref()
            .and_then(|notice| notice.visible(now))
            .map(str::to_string);
        if notice.is_none() {
            self.notice = None;
        }
        let canvas_error = canvas_info.error.map(|error| error.to_string());
        let info = TitleInfo {
            size: image.map(|image| (image.width(), image.height())),
            zoom: canvas_info.zoom,
            hover: canvas_info.hover,
            color: image.and_then(|image| {
                choice_label_with(
                    image,
                    self.session.choice,
                    self.session.slots.get(image.mode()),
                )
            }),
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            error: canvas_error.clone().or(notice.clone()),
        };
        let title = window_title(self.tools.tool(), &info);
        // La barra, además, avisa de que el editor está oculto por falta de altura.
        let status_info = TitleInfo {
            error: status_error(canvas_error, notice, self.editor_hidden),
            ..info
        };
        let status = (
            status_line(self.tools.tool(), &status_info),
            status_info.error.is_some(),
        );
        if status != self.status {
            self.status = status;
            ctx.request_repaint();
        }
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
    if wants_licenses(&args) {
        print!("{}", licenses::THIRD_PARTY);
        return Ok(());
    }
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
        Box::new(|cc| {
            px_ui::text::install_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
            if cfg!(debug_assertions) {
                // Un glifo ausente en texto fijo es un fallo; el texto externo ya pasa por
                // `replace_missing`. En release se dibuja el cuadrado en vez de caerse.
                cc.egui_ctx
                    .set_missing_glyph_policy(egui::MissingGlyphPolicy::Panic);
            }
            Ok(Box::new(App::new(document, cli_path)))
        }),
    )
}
