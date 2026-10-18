//! Pixoko: ventana mínima que muestra una imagen con zoom entero y desplazamiento
//! (specs/hito-2-ventana.md), con lápiz y deshacer/rehacer (specs/hito-2-lapiz-deshacer.md)
//! y las herramientas línea y relleno (specs/hito-2-linea-relleno-app.md). Abre PNG por
//! argumento o arrastrando, y los guarda con `Ctrl`+`S` (specs/hito-2-abrir-guardar.md).

mod confirm;
#[cfg(test)]
mod damero_guardado;
mod demo;
mod dialogs;
mod dispatch;
mod files;
mod keys;
mod licenses;
mod session;
mod title;
mod window_min;

use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui;
use px_core::Document;
use px_ui::actions::{AppAction, Locks, Origin, Platform, UiState, action_for_shortcut};
use px_ui::canvas::{Canvas, CanvasInfo};
use px_ui::checker::{ENV_MESH, mesh_fallback_enabled};
use px_ui::color_edit::{DOCUMENT_CHANGED_NOTICE, EditTarget};
use px_ui::licenses_view::show_licenses;
use px_ui::menu::show_menu_with_background;
use px_ui::notice::{NOTICE_SECS, Notice};
use px_ui::palette_ops::{PaletteOp, REMOVAL_CANCELLED_NOTICE};
use px_ui::shortcuts::{FileAction, gate_keys};
use px_ui::texture::pixel_rgba;
use px_ui::tool::{ToolEvent, ToolState, tool_color_label};
use px_ui::toolbar::{show_toolbar, zoom_label};

use crate::confirm::{
    Confirm, Confirmation, Request, close_allowed, close_warning, open_dialog_warning, open_warning,
};
use crate::dialogs::{
    DialogKind, Dialogs, Finished, IGNORED_FILE_WHILE_DIALOG, Outcome, RfdPicker, fallback_request,
    open_gate, unavailable_notice,
};
use crate::dispatch::{Dispatched, Targets, dispatch};
use crate::files::{file_label, parse_args, pick_dropped};
use crate::licenses::wants_licenses;
use crate::session::{Session, describe_saved, describe_saved_dialog};
use crate::title::{TitleInfo, status_line, title_with_file, window_title};
use crate::window_min::{ENV_NO_MIN, min_enabled, min_inner_size};

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
    /// Diálogos de archivo (specs/hito-2-dialogos-archivo.md).
    dialogs: Dialogs,
    /// Fichero del argumento de la línea de comandos, pendiente de abrir en el primer fotograma.
    cli_path: Option<PathBuf>,
    /// El tamaño mínimo (alto y ancho) de la ventana está activo (`PIXOKO_SIN_MINIMO=1` lo desactiva).
    min_enabled: bool,
    /// Último mínimo enviado a la ventana, para enviar solo cuando cambia.
    min_sent: Option<[f32; 2]>,
    /// Sistema para el que se compiló: decide los atajos que se muestran y los que valen.
    platform: Platform,
    /// El panel de licencias está abierto (sustituye al lienzo y es modal).
    licenses_open: bool,
    /// Primera línea visible del panel de licencias.
    licenses_first: usize,
}

impl App {
    fn new(document: Document, cli_path: Option<PathBuf>) -> App {
        let mut canvas = Canvas::new();
        // Las teclas de zoom las trata la capa de acciones, con sus propios bloqueos.
        canvas.set_view_keys(false);
        // `PIXOKO_DAMERO_MALLA=1`: el damero con una malla de casillas en vez de la textura repetida.
        canvas.set_checker_mesh(mesh_fallback_enabled(
            std::env::var(ENV_MESH).ok().as_deref(),
        ));
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
            dialogs: Dialogs::new(
                Box::new(RfdPicker::new()),
                cfg!(target_os = "linux"),
                std::env::current_dir().ok(),
            ),
            cli_path,
            min_enabled: min_enabled(std::env::var(ENV_NO_MIN).ok().as_deref()),
            min_sent: None,
            platform: Platform::current(),
            licenses_open: false,
            licenses_first: 0,
        }
    }

    /// Escala del texto y tema para el `ppp` actual (se repite al cambiar de monitor).
    fn apply_theme(&mut self, ctx: &egui::Context) {
        let ppp = ctx.pixels_per_point();
        px_ui::text::apply_scale(ctx);
        if ppp != self.themed_ppp {
            ctx.set_visuals_of(egui::Theme::Dark, px_ui::theme::visuals(ppp));
            self.themed_ppp = ppp;
            // El mínimo depende de la escala: se envía al empezar y al cambiar de monitor.
            let min = min_inner_size(ppp, self.min_enabled);
            if let Some(size) = min
                && min != self.min_sent
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(size.into()));
                self.min_sent = min;
            }
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
        self.tools.busy() || self.session.busy() || self.dialogs.pending()
    }

    /// Abre `path` (del argumento, soltado o elegido en el diálogo). Con cambios sin guardar pide
    /// confirmación por repetición; con un gesto o un diálogo en curso se ignora. `ignored`:
    /// ficheros soltados de más. `confirmed`: el descarte ya se confirmó (Ctrl+O con cambios).
    fn open_file(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        ignored: usize,
        from_cli: bool,
        confirmed: bool,
    ) {
        if self.busy() {
            let text = if self.dialogs.pending() {
                IGNORED_FILE_WHILE_DIALOG
            } else {
                "se ignora el fichero: hay un trazo en curso"
            };
            self.notify(ctx, text.to_string());
            return;
        }
        let mut discard = confirmed;
        if self.session.modified() && !confirmed {
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
                self.dialogs.remember_dir(path);
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
                if self.session.take_discarded_text() {
                    text = format!("{DOCUMENT_CHANGED_NOTICE} · {text}");
                }
                if self.session.take_removal_cancelled() {
                    text = format!("{REMOVAL_CANCELLED_NOTICE} · {text}");
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
                self.dialogs.remember_dir(&saved.path);
                self.notify(ctx, describe_saved(&saved));
            }
            Err(error) => self.notify(ctx, error.to_string()),
        }
    }

    /// Atajo de fichero: `Ctrl`+`O` abre el diálogo (con cambios, tras la confirmación por
    /// repetición); `Ctrl`+`S` con ruta guarda y sin ella abre el diálogo de guardar;
    /// `Ctrl`+`Mayús`+`S` abre el de guardar como.
    fn file_action(&mut self, ctx: &egui::Context, frame: &eframe::Frame, action: FileAction) {
        match action {
            FileAction::Open => {
                let now = ctx.input(|i| i.time);
                match open_gate(self.session.modified(), &mut self.confirm, now) {
                    None => self.notify(ctx, open_dialog_warning()),
                    Some(discard) => self.begin_dialog(ctx, frame, DialogKind::Open, discard),
                }
            }
            FileAction::Save if self.session.path().is_some() => self.save_file(ctx, false),
            FileAction::Save => self.save_via_dialog(ctx, frame, DialogKind::Save),
            FileAction::SaveAs => self.save_via_dialog(ctx, frame, DialogKind::SaveAs),
        }
    }

    /// Guardar con diálogo. Si un diálogo anterior falló y su aviso sigue armado, esta segunda
    /// pulsación guarda sin diálogo (`sin-titulo.png` o la copia, sin pisar nada).
    fn save_via_dialog(&mut self, ctx: &egui::Context, frame: &eframe::Frame, kind: DialogKind) {
        let as_copy = kind == DialogKind::SaveAs;
        let now = ctx.input(|i| i.time);
        if self
            .confirm
            .confirm_armed(&Request::SaveFallback { as_copy }, now)
        {
            self.save_file(ctx, as_copy);
            return;
        }
        self.begin_dialog(ctx, frame, kind, false);
    }

    fn begin_dialog(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        kind: DialogKind,
        discard: bool,
    ) {
        let now = ctx.input(|i| i.time);
        let path = self.session.path().map(Path::to_path_buf);
        let converted = self.session.converted();
        if self
            .dialogs
            .begin(kind, discard, path.as_deref(), converted, now, Some(frame))
        {
            self.confirm.clear();
            // El futuro empieza a ejecutarse al consultarlo: un fotograma ya.
            ctx.request_repaint();
        }
    }

    /// Resultado de un diálogo: abre o guarda la ruta elegida, o avisa de que no hay diálogo.
    fn finish_dialog(&mut self, ctx: &egui::Context, finished: Finished) {
        match finished.outcome {
            Outcome::Cancelled => self.confirm.clear(),
            Outcome::Unavailable => {
                self.notify(ctx, unavailable_notice(finished.kind));
                if let Some(request) = fallback_request(finished.kind) {
                    let now = ctx.input(|i| i.time);
                    self.confirm.request(request, now);
                }
            }
            Outcome::Chosen(path) => match finished.kind {
                DialogKind::Open => self.open_file(ctx, &path, 0, false, finished.discard),
                DialogKind::Save | DialogKind::SaveAs => match self.session.save_to(&path) {
                    Ok(saved) => {
                        self.confirm.clear();
                        self.dialogs.remember_dir(&saved.path);
                        self.notify(ctx, describe_saved_dialog(&saved));
                    }
                    Err(error) => self.notify(ctx, error.to_string()),
                },
            },
        }
    }

    /// Cierre de la ventana con cambios sin guardar: el primero se cancela con aviso y el
    /// segundo, en `CONFIRM_SECS`, cierra. Un diálogo abierto se abandona.
    fn handle_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        self.dialogs.abandon();
        let now = ctx.input(|i| i.time);
        if !close_allowed(self.session.modified(), &mut self.confirm, now) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.notify(ctx, close_warning());
        }
    }

    /// Bloqueos de este fotograma (specs/hito-2-menu-herramientas.md, «Piezas»).
    fn locks(&self, ctx: &egui::Context) -> Locks {
        Locks {
            gesture: self.tools.busy() || self.session.busy(),
            dialog: self.dialogs.pending(),
            text_focus: ctx.text_edit_focused(),
            removal: self.session.ops.removal().is_some(),
            modal: self.licenses_open,
        }
    }

    /// Lo que el menú y la columna necesitan para habilitarse.
    fn ui_state(&self, ctx: &egui::Context) -> UiState {
        UiState {
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            zoom: self.canvas.zoom(),
            tool: self.tools.tool(),
            locks: self.locks(ctx),
        }
    }

    /// Traduce las teclas del fotograma a acciones. Un atajo de fichero o `Salir` mantenido
    /// pulsado no repite: confirmaría solo. `Escape` no es una acción (lo ordena el lienzo).
    fn key_actions(&self, events: &[egui::Event], out: &mut Vec<(AppAction, Origin)>) {
        for event in events {
            let egui::Event::Key {
                key,
                pressed: true,
                repeat,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            if let Some(action) = action_for_shortcut(*key, *modifiers, self.platform)
                && !(*repeat && action.ignores_repeat())
            {
                out.push((action, Origin::Key));
            }
        }
    }

    /// Despacha una acción venga de donde venga (menú, columna o atajo) y ejecuta lo que necesita
    /// la ventana.
    fn apply_action(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        action: AppAction,
        origin: Origin,
    ) {
        let locks = self.locks(ctx);
        let result = dispatch(
            action,
            origin,
            &mut Targets {
                session: &mut self.session,
                tools: &mut self.tools,
                locks,
            },
        );
        match result {
            Dispatched::Blocked => {}
            Dispatched::Done { refresh, notice } => {
                self.finish::<std::convert::Infallible>(ctx, Ok(refresh));
                if let Some(text) = notice {
                    self.notify(ctx, text);
                }
            }
            Dispatched::File(file) => self.file_action(ctx, frame, file),
            // La regla de cierre por repetición la aplica `handle_close` en el siguiente fotograma.
            Dispatched::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Dispatched::View(op) => {
                self.canvas.request_view(op);
                ctx.request_repaint();
            }
            Dispatched::Licenses(open) => {
                self.licenses_open = open;
                self.licenses_first = 0;
            }
            Dispatched::Background(background) => {
                self.canvas.set_background(background);
                ctx.request_repaint();
            }
        }
    }

    /// Aplica una edición de la paleta pedida por el panel y avisa del resultado.
    fn apply_palette_op(&mut self, ctx: &egui::Context, op: PaletteOp, unused: bool) {
        match self.session.apply_palette_op(op, unused) {
            Ok(outcome) => {
                self.finish::<std::convert::Infallible>(ctx, Ok(outcome.changed));
                if let Some(text) = outcome.notice {
                    self.notify(ctx, text);
                }
            }
            Err(error) => self.notify(ctx, error.to_string()),
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
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.apply_theme(&ctx);
        // Las acciones del fotograma, en este orden: menú, columna y atajos.
        let mut actions: Vec<(AppAction, Origin)> = Vec::new();
        let state = self.ui_state(&ctx);
        if let Some(action) =
            show_menu_with_background(ui, &state, self.platform, self.canvas.background())
        {
            actions.push((action, Origin::Menu));
        }
        px_ui::status::show(ui, &self.status.0, self.status.1);
        if let Some(action) = show_toolbar(ui, &state, &zoom_label(state.zoom)) {
            actions.push((action, Origin::Button));
        }
        self.session.normalize();
        if self.session.take_removal_cancelled() {
            self.notify(&ctx, REMOVAL_CANCELLED_NOTICE.to_string());
        }
        self.session.sync_editor();
        let enabled = !self.busy();
        let main = self.session.document.main_image();
        let mut panel = None;
        if let Some(image) = self.session.document.image(main) {
            panel = Some(px_ui::palette_panel::show_editing_ops(
                ui,
                image,
                self.session.choice,
                &mut self.session.scroll,
                &mut self.session.view,
                enabled,
                self.session.slots.get(image.mode()),
                &mut self.session.editor,
                &mut self.session.ops,
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
            if let Some(op) = panel.op {
                self.apply_palette_op(&ctx, op, panel.op_unused);
            }
        }
        let now = ctx.input(|i| i.time);
        if let Some(finished) = self.dialogs.poll(now) {
            self.finish_dialog(&ctx, finished);
        }
        if let Some(after) = self.dialogs.repaint_after() {
            ctx.request_repaint_after(after);
        }
        self.handle_close(&ctx);
        if let Some(path) = self.cli_path.take() {
            self.open_file(&ctx, &path, 0, true, false);
        }
        let dropped: Vec<Option<PathBuf>> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|file| Some(file.path().to_path_buf()))
                .collect()
        });
        if let Some((path, ignored)) = pick_dropped(&dropped) {
            self.open_file(&ctx, &path, ignored, false, false);
        }

        let id = self.session.document.main_image();
        // Con el panel de licencias abierto el lienzo no interactúa.
        let canvas_inputs = if self.licenses_open {
            Vec::new()
        } else {
            self.canvas.interact_ordered(ui)
        };
        for input in canvas_inputs {
            let pixel = self.session.active_pixel();
            let result = self
                .tools
                .canvas_input_with(&mut self.session.document, id, pixel, input);
            self.finish(&ctx, result);
            for event in self.tools.take_events() {
                match event {
                    ToolEvent::Picked(picked) => {
                        if let Some(text) = self.session.apply_pick(picked).notice {
                            self.notify(&ctx, text);
                        }
                    }
                    ToolEvent::Refused(error) => self.notify(&ctx, error.to_string()),
                }
            }
        }
        // Con un campo de texto enfocado el teclado es suyo: los atajos no ven nada.
        let events = gate_keys(&ui.input(|i| i.events.clone()), ctx.text_edit_focused());
        self.key_actions(&events, &mut actions);
        for (action, origin) in actions {
            self.apply_action(&ctx, frame, action, origin);
        }

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
        let mut close_licenses = false;
        let canvas_info = if self.licenses_open {
            let output = show_licenses(ui, licenses::THIRD_PARTY, &mut self.licenses_first);
            close_licenses = output.close;
            CanvasInfo {
                zoom: self.canvas.zoom().unwrap_or_default(),
                hover: None,
                image_rect: None,
                error: None,
            }
        } else {
            let overlay = self.tools.preview().filter_map(|edit| {
                let color = pixel_rgba(image?, edit.pixel)?;
                Some((edit.x, edit.y, color))
            });
            self.canvas.paint(ui, overlay)
        };

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
                tool_color_label(
                    self.tools.tool(),
                    image,
                    self.session.choice,
                    self.session.slots.get(image.mode()),
                )
            }),
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            error: canvas_error.or(notice),
        };
        let title = window_title(self.tools.tool(), &info);
        let status = (status_line(self.tools.tool(), &info), info.error.is_some());
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
        if close_licenses {
            self.apply_action(&ctx, frame, AppAction::CloseLicenses, Origin::Button);
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
    // Mínimo inicial para `ppp` 1,0: el real se envía en el primer fotograma (`apply_theme`).
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Pixoko")
        .with_inner_size([960.0, 720.0]);
    let min_enabled = min_enabled(std::env::var(ENV_NO_MIN).ok().as_deref());
    if let Some(size) = min_inner_size(1.0, min_enabled) {
        viewport = viewport.with_min_inner_size(size);
    }
    let options = eframe::NativeOptions {
        viewport,
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
