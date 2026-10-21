//! Pixoko: ventana mínima que muestra una imagen con zoom entero y desplazamiento
//! (specs/hito-2-ventana.md), con lápiz y deshacer/rehacer (specs/hito-2-lapiz-deshacer.md)
//! y las herramientas línea y relleno (specs/hito-2-linea-relleno-app.md). Abre PNG por
//! argumento o arrastrando, y los guarda con `Ctrl`+`S` (specs/hito-2-abrir-guardar.md).

mod background_color;
mod config;
mod confirm;
mod convert_mode;
#[cfg(test)]
mod damero_guardado;
mod demo;
mod dialogs;
mod dispatch;
mod dropper_once;
mod files;
mod icons;
mod keys;
mod licenses;
mod new_image;
#[cfg(test)]
mod puerta;
mod session;
mod shortcuts_flow;
mod title;
mod window_min;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::background_color::{background_from_values, open_background_color};
use eframe::egui;
use px_core::{Document, DocumentError, Rgba};
use px_ui::actions::{AppAction, Locks, Origin, Platform, UiState};
use px_ui::canvas::Canvas;
use px_ui::checker::{ENV_MESH, mesh_fallback_enabled};
use px_ui::color::choice_label_with;
use px_ui::color_edit::{DOCUMENT_CHANGED_NOTICE, EditTarget};
use px_ui::dialog::{DialogKind as ModalKind, Modal, Reaction};
use px_ui::dialog_keys::FocusTarget;
use px_ui::dialog_view::show_dialog;
use px_ui::gestures::GestureMap;
use px_ui::help::{HelpTarget, help_target_at};
use px_ui::key_events::{help_pressed, presses};
use px_ui::keymap::Keymap;
use px_ui::keymap_file::{Loaded, load_notice};
use px_ui::licenses_view::licenses_form;
use px_ui::menu::show_menu_in;
use px_ui::notice::{NOTICE_SECS, Notice};
use px_ui::options::{OptionsState, options_row_count};
use px_ui::palette_ops::{PaletteOp, REMOVAL_CANCELLED_NOTICE};
use px_ui::palette_panel::{Indicator, Roles, show_roles};
use px_ui::pointer_events::window_actions;
use px_ui::shortcuts::{FileAction, gate_keys};
use px_ui::texture::pixel_rgba;
use px_ui::tiles::{SystemTileAlloc, TileAlloc};
use px_ui::tool::{Tool, ToolAction, ToolEvent, ToolState, tool_color_label};
use px_ui::toolbar::{ToolboxView, show_toolbox, zoom_label};

use crate::config::{SaveError, ShortcutStore, shortcuts_path};
use crate::confirm::{
    Confirm, Confirmation, Request, close_warning, new_warning, open_dialog_warning, open_warning,
};
use crate::convert_mode::{
    Analysis, CONVERTING_NOTICE, ConvertModeError, ConvertSync, History, PendingConvert, Plan,
    Start, SystemAnalyzer, convert_memory_notice, describe_converted, open_convert_mode,
    plan_from_values, reserve_for_convert, start_convert, sync_convert,
};
use crate::demo::{ENV_DEMO_DIALOG, demo_dialog_enabled, demo_form, demo_notice};
use crate::dialogs::{
    DialogKind, Dialogs, Finished, Outcome, RfdPicker, fallback_request, ignored_file_notice,
    new_gate, open_gate, unavailable_notice,
};
use crate::dispatch::{Dispatched, Targets, dispatch};
use crate::dropper_once::DropperOnce;
use crate::files::{file_label, parse_args, pick_dropped};
use crate::keys::select_tool;
use crate::licenses::wants_licenses;
use crate::new_image::{
    FormSync, NewImageError, SystemAlloc, build_new_image, describe_new, memory_notice,
    open_new_image, spec_from_values, sync_form,
};
use crate::session::{Session, describe_saved, describe_saved_dialog};
use crate::shortcuts_flow::{Flow, FlowOutcome, Strip};
use crate::title::{app_title_info, colors_label, status_line_fit, title_with_file, window_title};
use crate::window_min::{ENV_NO_MIN, min_enabled, min_inner_size};

/// Una conversión que espera su fotograma tras `Aceptar` (specs/hito-2-convertir-modo.md, D5).
struct PendingConversion {
    pending: PendingConvert,
    /// El informe vigente al aceptar, para el aviso de 5 s.
    analysis: Option<Analysis>,
    /// Fotogramas que se dejan pasar antes de contar: el aviso se pinta un fotograma después de
    /// ponerlo (la barra de estado va un fotograma por detrás).
    skip: u8,
}

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
    /// El cuentagotas del indicador de color es de un solo uso: vuelve a la herramienta anterior.
    dropper_once: DropperOnce,
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
    /// Las teclas: el menú, la columna y `key_actions` leen esta tabla (el hito 8 la hará
    /// configurable; hoy es el esquema por defecto).
    keymap: Keymap,
    /// Los gestos del ratón y del trackpad: el lienzo y `pointer_actions` leen esta tabla (el hito 8
    /// la hará configurable; hoy es el esquema por defecto).
    gestures: GestureMap,
    /// Foco y desplazamiento del recuadro de opciones de la caja (I.3).
    options: OptionsState,
    /// Las texturas de los iconos propios (I.4); vacío hasta que `main` las carga.
    icons: icons::Icons,
    /// El diálogo modal abierto, si lo hay (Licencias o el de prueba).
    modal: Modal,
    /// Ayuda, lista de atajos y editor: sustitución con vuelta y captura de teclas (I.5).
    flow: Flow,
    /// El fichero de atajos.
    shortcuts: ShortcutStore,
    /// Ya se avisó de que no hay carpeta de configuración.
    config_warned: bool,
    /// El aviso de la lectura del fichero de atajos, pendiente del primer fotograma.
    startup_notice: Option<String>,
    /// El elemento enfocado del diálogo modal.
    modal_focus: Option<FocusTarget>,
    /// Abrir el diálogo de prueba en el primer fotograma (`PIXOKO_DIALOGO_PRUEBA=1`).
    demo_pending: bool,
    /// Con el diálogo de nueva imagen abierto: su traslado de fondo y si el descarte de los
    /// cambios sin guardar ya se confirmó (specs/hito-2-nueva-imagen.md, D6 y D8).
    new_image: Option<(FormSync, bool)>,
    /// Con el diálogo de convertir modo abierto: su informe y su espera.
    convert: Option<ConvertSync>,
    /// Una conversión de una imagen grande que espera su fotograma: la entrada está bloqueada.
    pending_convert: Option<PendingConversion>,
    /// Reserva de los búferes de las baldosas (cargar y regenerar); las pruebas la sustituyen por
    /// una que falla.
    tile_alloc: Box<dyn TileAlloc>,
}

impl App {
    fn new(document: Document, cli_path: Option<PathBuf>) -> App {
        let mut canvas = Canvas::new();
        let gestures = GestureMap::default_scheme();
        canvas.set_gestures(gestures.clone());
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
            dropper_once: DropperOnce::new(),
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
            keymap: Keymap::default_scheme(),
            gestures,
            options: OptionsState::default(),
            icons: icons::Icons::empty(),
            modal: Modal::Closed,
            flow: Flow::new(Vec::new()),
            shortcuts: ShortcutStore::new(None),
            config_warned: false,
            startup_notice: None,
            modal_focus: None,
            demo_pending: demo_dialog_enabled(std::env::var(ENV_DEMO_DIALOG).ok().as_deref()),
            new_image: None,
            convert: None,
            pending_convert: None,
            tile_alloc: Box::new(SystemTileAlloc),
        }
    }

    /// Instala los atajos leídos del fichero: la tabla viva, la marca de conflicto y el aviso de 5 s
    /// del primer fotograma.
    fn install_shortcuts(&mut self, store: ShortcutStore, loaded: Loaded) {
        self.startup_notice = load_notice(&loaded);
        self.keymap = loaded.keymap;
        self.flow = Flow::new(loaded.conflicted);
        self.shortcuts = store;
    }

    /// Guarda la tabla viva. Si falla, la tabla cambia igual (esta sesión) y un aviso lo dice; sin
    /// carpeta de configuración se avisa una sola vez.
    fn save_shortcuts(&mut self, ctx: &egui::Context, reset_all: bool) {
        let default = Keymap::default_scheme();
        match self.shortcuts.save(&self.keymap, &default, reset_all) {
            Ok(()) => {}
            Err(SaveError::NoDirectory) if self.config_warned => {}
            Err(error) => {
                if matches!(error, SaveError::NoDirectory) {
                    self.config_warned = true;
                }
                self.notify(ctx, error.to_string());
            }
        }
    }

    /// De qué pide ayuda `F1`: el botón de la caja bajo el puntero o la herramienta activa.
    fn help_target(&self, ctx: &egui::Context) -> HelpTarget {
        let ppp = ctx.pixels_per_point();
        let screen = ctx.content_rect();
        let window = (
            (screen.width() * ppp).round() as i64,
            (screen.height() * ppp).round() as i64,
        );
        match ctx.input(|i| i.pointer.latest_pos()) {
            Some(pos) => help_target_at(
                ((pos.x * ppp).round() as i64, (pos.y * ppp).round() as i64),
                window,
                px_ui::text::text_scale(ppp),
            ),
            None => HelpTarget::Active,
        }
    }

    /// La captura de un atajo (editor abierto): le da los eventos de teclado del fotograma y los
    /// retira de los que ven las acciones, el menú y egui (specs/interfaz-ayuda.md, decisión 6).
    fn handle_capture(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        let pointer_pressed = ctx.input(|i| i.pointer.any_pressed());
        let strip = self.flow.keys(&events, pointer_pressed);
        let drop = |event: &egui::Event| match strip {
            Strip::None => false,
            Strip::All => matches!(
                event,
                egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::Ime(_)
            ),
            Strip::Escape => matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::Escape,
                    ..
                }
            ),
        };
        if strip != Strip::None {
            ctx.input_mut(|i| {
                i.events.retain(|event| !drop(event));
                i.raw.events.retain(|event| !drop(event));
            });
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
        self.tools.busy()
            || self.session.busy()
            || self.dialogs.pending()
            || self.modal.is_open()
            || self.pending_convert.is_some()
    }

    /// El cuentagotas del indicador de color es de un solo uso: al soltar su primer gesto vuelve la
    /// herramienta anterior.
    fn tick_dropper_once(&mut self, ctx: &egui::Context) {
        let Some(previous) = self
            .dropper_once
            .update(self.tools.tool(), self.tools.busy())
        else {
            return;
        };
        let image = self
            .session
            .document
            .image(self.session.document.main_image());
        if let Some(text) = select_tool(&mut self.tools, ToolAction::Select(previous), false, image)
        {
            self.notify(ctx, text);
        }
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
            let text = ignored_file_notice(
                self.dialogs.pending(),
                self.modal.is_open() || self.pending_convert.is_some(),
            );
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
        // El lienzo se carga **antes** de reemplazar el documento: si no hay memoria para las
        // baldosas, la sesión y el lienzo quedan como estaban.
        let loaded = match self.session.load(path, Canvas::MAX_SIDE, discard) {
            Ok(loaded) => loaded,
            Err(error) => {
                if from_cli {
                    eprintln!("px-app: {error}");
                }
                self.notify(ctx, error.to_string());
                return;
            }
        };
        if let Err(error) = self
            .canvas
            .load_with(ctx, &loaded.image, &mut *self.tile_alloc)
        {
            if from_cli {
                eprintln!("px-app: {error}");
            }
            self.notify(ctx, error.to_string());
            return;
        }
        let mut text = loaded.text.clone();
        self.session.install(loaded);
        self.confirm.clear();
        self.dialogs.remember_dir(path);
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
        self.pending_convert = None;
        let now = ctx.input(|i| i.time);
        // Un diálogo modal se abandona sin aplicar nada, como uno de archivo, también con una
        // captura de atajo armada; después, la regla de cierre por repetición.
        let modified = self.session.modified();
        if !self
            .flow
            .close_requested(&mut self.modal, modified, &mut self.confirm, now)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.notify(ctx, close_warning());
        }
    }

    /// Bloqueos de este fotograma (specs/hito-2-menu-herramientas.md, «Piezas»).
    fn locks(&self, ctx: &egui::Context) -> Locks {
        Locks {
            gesture: self.tools.busy() || self.session.busy(),
            stroke: self.tools.busy() && !self.session.busy(),
            dialog: self.dialogs.pending(),
            text_focus: ctx.text_edit_focused(),
            removal: self.session.ops.removal().is_some(),
            modal: self.modal.is_open() || self.pending_convert.is_some(),
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
        for press in presses(events) {
            if let Some(action) = self.keymap.action_for(&press, self.platform)
                && !(press.repeat && action.ignores_repeat())
            {
                out.push((action, Origin::Key));
            }
        }
    }

    /// Traduce los botones laterales del fotograma (los gestos de ventana de la tabla) a acciones,
    /// con `Origin::Button`: los bloqueos son los de un botón (gesto, diálogo, modal y campo de
    /// texto enfocado; `dispatch` los aplica). Los eventos van sin filtrar por el foco del campo.
    fn pointer_actions(&self, events: &[egui::Event], out: &mut Vec<(AppAction, Origin)>) {
        out.extend(
            window_actions(events, &self.gestures, self.platform)
                .into_iter()
                .map(|action| (action, Origin::Button)),
        );
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
            Dispatched::NewImage => self.begin_new_image(ctx),
            Dispatched::ConvertMode => self.begin_convert_mode(ctx),
            Dispatched::Licenses(true) => {
                let locks = self.locks(ctx);
                if self.modal.open(
                    ModalKind::Licenses,
                    licenses_form(licenses::THIRD_PARTY),
                    locks,
                ) {
                    self.modal_focus = None;
                }
            }
            // Solo cierra el diálogo de Licencias, nunca otro.
            Dispatched::Licenses(false) => {
                self.modal.close_kind(ModalKind::Licenses);
            }
            Dispatched::BackgroundColor => {
                let locks = self.locks(ctx);
                if open_background_color(&mut self.modal, self.canvas.background(), locks) {
                    self.modal_focus = None;
                }
            }
            Dispatched::Background(background) => {
                self.canvas.set_background(background);
                ctx.request_repaint();
            }
            Dispatched::Help(target) => {
                let locks = self.locks(ctx);
                let active = self.tools.tool();
                if self.flow.open_help(
                    &mut self.modal,
                    target,
                    active,
                    &self.keymap,
                    self.platform,
                    locks,
                ) {
                    self.modal_focus = None;
                }
            }
            Dispatched::Shortcuts => {
                let locks = self.locks(ctx);
                if self
                    .flow
                    .open_list(&mut self.modal, &self.keymap, self.platform, locks)
                {
                    self.modal_focus = None;
                }
            }
        }
    }

    /// `Archivo → Nueva imagen…`: con cambios sin guardar, pide confirmación por repetición; luego
    /// abre el diálogo (el lado máximo es `Canvas::MAX_SIDE`).
    fn begin_new_image(&mut self, ctx: &egui::Context) {
        let modified = self.session.modified();
        let now = ctx.input(|i| i.time);
        if !new_gate(modified, &mut self.confirm, now) {
            self.notify(ctx, new_warning());
            return;
        }
        let locks = self.locks(ctx);
        if open_new_image(&mut self.modal, Canvas::MAX_SIDE, locks) {
            self.modal_focus = None;
            self.confirm.clear();
            self.new_image = Some((FormSync::default(), modified));
        }
    }

    /// El diálogo de nueva imagen se aceptó: construye la imagen, carga el lienzo desde ella y solo
    /// entonces reemplaza el documento; si algo falla, avisa y todo queda como estaba.
    fn finish_new_image(
        &mut self,
        ctx: &egui::Context,
        values: &px_ui::dialog::Values,
        discard: bool,
    ) {
        let Some(spec) = spec_from_values(values) else {
            return;
        };
        if self.session.modified() && !discard {
            self.notify(ctx, crate::files::FileError::Unsaved.to_string());
            return;
        }
        let image = match build_new_image(&spec, &mut SystemAlloc) {
            Ok(image) => image,
            Err(NewImageError::Size(reason)) => {
                self.notify(ctx, reason);
                return;
            }
            Err(NewImageError::Memory { bytes }) => {
                self.notify(ctx, memory_notice(spec.width, spec.height, bytes));
                return;
            }
        };
        if let Err(error) = self.canvas.load_with(ctx, &image, &mut *self.tile_alloc) {
            self.notify(ctx, error.to_string());
            return;
        }
        if let Err(error) = self.session.new_image(image, discard) {
            self.notify(ctx, error.to_string());
            return;
        }
        self.confirm.clear();
        let mut text = describe_new(&spec);
        if self.session.take_discarded_text() {
            text = format!("{DOCUMENT_CHANGED_NOTICE} · {text}");
        }
        if self.session.take_removal_cancelled() {
            text = format!("{REMOVAL_CANCELLED_NOTICE} · {text}");
        }
        self.notify(ctx, text);
    }

    /// `Imagen → Convertir modo…`: abre el diálogo con el modo y el color activo de ahora.
    fn begin_convert_mode(&mut self, ctx: &egui::Context) {
        let main = self.session.document.main_image();
        let Some(image) = self.session.document.image(main) else {
            return;
        };
        let from = image.mode();
        let active = self
            .session
            .active_pixel()
            .and_then(|pixel| pixel_rgba(image, pixel));
        let state_id = self.session.document.state_id();
        let locks = self.locks(ctx);
        if open_convert_mode(&mut self.modal, from, locks) {
            self.modal_focus = None;
            self.convert = Some(ConvertSync::new(from, state_id, active));
        }
    }

    /// El diálogo de convertir se aceptó: con una imagen grande avisa «Convirtiendo…» y convierte
    /// unos fotogramas después; si no, convierte ya.
    fn finish_convert(&mut self, ctx: &egui::Context, values: &px_ui::dialog::Values) {
        let Some(sync) = self.convert.take() else {
            return;
        };
        let main = self.session.document.main_image();
        let Some(image) = self.session.document.image(main) else {
            return;
        };
        let from = image.mode();
        let pixels = u64::from(image.width()) * u64::from(image.height());
        let Some(plan) = plan_from_values(values, from, sync.active()) else {
            return;
        };
        let analysis = sync.analysis().cloned();
        match start_convert(plan, pixels) {
            Start::Now(plan) => self.run_convert(ctx, plan, analysis.as_ref()),
            Start::Later(pending) => {
                self.pending_convert = Some(PendingConversion {
                    pending,
                    analysis,
                    skip: 1,
                });
                self.notify(ctx, CONVERTING_NOTICE.to_string());
                ctx.request_repaint();
            }
        }
    }

    /// Cuenta un fotograma de una conversión en espera y la ejecuta en el que toca.
    fn advance_convert(&mut self, ctx: &egui::Context) {
        let Some(waiting) = &mut self.pending_convert else {
            return;
        };
        if waiting.skip > 0 {
            waiting.skip -= 1;
            ctx.request_repaint();
            return;
        }
        match waiting.pending.tick() {
            Some(plan) => {
                debug_assert_eq!(waiting.pending.plan(), plan);
                let analysis = waiting.analysis.take();
                self.pending_convert = None;
                self.run_convert(ctx, plan, analysis.as_ref());
            }
            None => ctx.request_repaint(),
        }
    }

    /// Reserva la memoria que hará falta y convierte como un paso de la historia; con un fallo,
    /// aviso de 5 s y todo queda como estaba.
    fn run_convert(&mut self, ctx: &egui::Context, plan: Plan, analysis: Option<&Analysis>) {
        let main = self.session.document.main_image();
        let Some(image) = self.session.document.image(main) else {
            return;
        };
        let (width, height) = (image.width(), image.height());
        if let Err(ConvertModeError::Memory { bytes }) =
            reserve_for_convert(image, plan.target, &mut SystemAlloc)
        {
            self.notify(
                ctx,
                convert_memory_notice(width, height, plan.target, bytes),
            );
            return;
        }
        match self.session.convert_mode(plan.target, plan.allowed) {
            Ok(changed) => {
                if changed && !self.refresh_now(ctx) {
                    // No hay memoria para regenerar las baldosas: el documento manda, así que se
                    // deshace la conversión (un intercambio que no reserva) y queda rehacer.
                    let _ = self.session.undo();
                    let blocks = self.session.take_dirty();
                    self.canvas.invalidate(&blocks);
                    self.notify(
                        ctx,
                        convert_memory_notice(
                            width,
                            height,
                            plan.target,
                            u64::from(width) * u64::from(height) * 4,
                        ),
                    );
                    return;
                }
                if changed {
                    self.notify(ctx, describe_converted(plan.target, analysis));
                }
            }
            Err(DocumentError::OutOfMemory | DocumentError::Convert(_)) => {
                self.notify(
                    ctx,
                    convert_memory_notice(
                        width,
                        height,
                        plan.target,
                        u64::from(width) * u64::from(height) * 4,
                    ),
                );
            }
            Err(error) => self.notify(ctx, error.to_string()),
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

    /// `Ok(true)`: algo cambió y el lienzo marca como sucios los bloques que anotó la sesión (los
    /// sube `sync` en este mismo fotograma, solo en las baldosas visibles). `Ok(false)`: nada que
    /// hacer. `Err`: el documento quedó intacto y el error va al título durante `NOTICE_SECS`.
    fn finish<E: std::fmt::Display>(&mut self, ctx: &egui::Context, result: Result<bool, E>) {
        match result {
            Ok(true) => {
                let blocks = self.session.take_dirty();
                self.canvas.invalidate(&blocks);
                ctx.request_repaint();
            }
            Ok(false) => {}
            Err(error) => self.notify(ctx, error.to_string()),
        }
    }

    /// Marca lo anotado y regenera ya las baldosas visibles; `false` si no hubo memoria (las
    /// baldosas siguen sucias).
    fn refresh_now(&mut self, ctx: &egui::Context) -> bool {
        let blocks = self.session.take_dirty();
        self.canvas.invalidate(&blocks);
        let Some(image) = self
            .session
            .document
            .image(self.session.document.main_image())
        else {
            return true;
        };
        self.canvas
            .sync_with(ctx, image, &mut *self.tile_alloc)
            .is_ok()
    }
}

impl App {
    /// Pinta el diálogo modal, si lo hay, y aplica su respuesta. Va el último del fotograma: los
    /// bloqueos ya se leyeron al principio y el cierre se aplica al final.
    fn show_modal(&mut self, ctx: &egui::Context) {
        if self.demo_pending {
            self.demo_pending = false;
            let locks = self.locks(ctx);
            self.modal.open(ModalKind::Demo, demo_form(), locks);
            self.modal_focus = None;
        }
        self.flow.sync(&mut self.modal, self.platform);
        let Modal::Open(dialog) = &mut self.modal else {
            // Cerrado por `Quit` o por la ventana: el estado de nueva imagen se va con él.
            self.new_image = None;
            self.convert = None;
            self.flow.abandon(&mut self.modal);
            return;
        };
        let kind = dialog.kind();
        let reaction = show_dialog(ctx, dialog, &mut self.modal_focus);
        if Flow::handles(kind) {
            if !matches!(reaction, Reaction::None | Reaction::Rejected(_)) {
                let editing = kind == ModalKind::ShortcutEditor
                    && matches!(reaction, Reaction::Pressed { .. });
                let active = self.tools.tool();
                let outcome = self.flow.react(
                    &mut self.modal,
                    reaction,
                    &mut self.keymap,
                    self.platform,
                    active,
                );
                if !editing {
                    self.modal_focus = None;
                }
                match outcome {
                    FlowOutcome::None => {}
                    FlowOutcome::KeymapChanged(_) => self.save_shortcuts(ctx, false),
                    FlowOutcome::ResetAll => self.save_shortcuts(ctx, true),
                }
                ctx.request_repaint();
            }
            return;
        }
        if kind == ModalKind::NewImage
            && let Some((sync, _)) = &mut self.new_image
            && sync_form(dialog, sync)
        {
            ctx.request_repaint();
        }
        if kind == ModalKind::ConvertMode
            && let Some(sync) = &mut self.convert
            && let Some(image) = self
                .session
                .document
                .image(self.session.document.main_image())
        {
            let history = History {
                limit: self.session.document.history_limit(),
                used: self.session.document.history_bytes(),
            };
            let state_id = self.session.document.state_id();
            if sync_convert(dialog, sync, image, state_id, history, &mut SystemAnalyzer) {
                ctx.request_repaint();
            }
        }
        if !matches!(reaction, Reaction::Accepted(_) | Reaction::Cancelled) {
            return;
        }
        // `Cerrar` y `Escape` de Licencias solo cierran; el de prueba avisa y no toca nada más.
        self.modal.abandon();
        self.modal_focus = None;
        if kind == ModalKind::Demo {
            self.notify(ctx, demo_notice(&reaction));
        }
        if kind == ModalKind::ConvertMode {
            if let Reaction::Accepted(values) = &reaction {
                self.finish_convert(ctx, values);
            } else {
                self.convert = None;
            }
        }
        if kind == ModalKind::BackgroundColor
            && let Reaction::Accepted(values) = &reaction
            && let Some(background) = background_from_values(values)
        {
            // Solo al aceptar; sin pasar por `dispatch` (con el modal abierto lo bloquearía).
            self.canvas.set_background(background);
            ctx.request_repaint();
        }
        if kind == ModalKind::NewImage {
            let discard = self.new_image.take().is_some_and(|(_, discard)| discard);
            if let Reaction::Accepted(values) = &reaction {
                self.finish_new_image(ctx, values, discard);
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.apply_theme(&ctx);
        if let Some(text) = self.startup_notice.take() {
            self.notify(&ctx, text);
        }
        // Con la captura de un atajo armada, el teclado es de la captura.
        self.handle_capture(&ctx);
        self.advance_convert(&ctx);
        // Las acciones del fotograma, en este orden: menú, columna y atajos.
        let mut actions: Vec<(AppAction, Origin)> = Vec::new();
        let state = self.ui_state(&ctx);
        if let Some(action) = show_menu_in(
            ui,
            &state,
            self.platform,
            self.canvas.background(),
            &self.keymap,
        ) {
            actions.push((action, Origin::Menu));
        }
        px_ui::status::show(ui, &self.status.0, self.status.1);
        // La caja va primero a la derecha y el panel de paletas se apila a su izquierda.
        let level = zoom_label(state.zoom);
        let toolbox = show_toolbox(
            ui,
            &ToolboxView {
                state: &state,
                zoom_label: &level,
                options: &self.options,
                icons: &self.icons.set,
            },
            &self.keymap,
            self.platform,
        );
        if toolbox.pressed_outside_options {
            self.options.press_outside();
        }
        self.options
            .scroll_by(toolbox.options_scroll_rows, options_row_count(state.tool));
        if let Some(action) = toolbox.action {
            actions.push((action, Origin::Button));
        }
        self.session.normalize();
        if self.session.take_removal_cancelled() {
            self.notify(&ctx, REMOVAL_CANCELLED_NOTICE.to_string());
        }
        self.session.sync_editor();
        // El panel sigue vivo con un trazo en curso (el color se puede cambiar: afecta al trazo
        // siguiente); no con el deslizador del editor, un arrastre de muestra ni un diálogo.
        let enabled = !(self.session.busy()
            || self.dialogs.pending()
            || self.modal.is_open()
            || self.pending_convert.is_some());
        let main = self.session.document.main_image();
        let mut panel = None;
        self.session.view.icons = self.icons.set;
        if let Some(image) = self.session.document.image(main) {
            let roles = Roles {
                foreground: self.session.choice,
                background: self.session.background,
                editing: self.session.editing,
            };
            panel = Some(show_roles(
                ui,
                image,
                roles,
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
            if let Some(choice) = panel.background {
                self.session.background = choice;
            }
            if let Some(role) = panel.editing {
                self.session.editing = role;
            }
            match panel.indicator {
                Some(Indicator::Dropper) => {
                    let before = self.tools.tool();
                    self.apply_action(
                        &ctx,
                        frame,
                        AppAction::SelectTool(Tool::Dropper),
                        Origin::Button,
                    );
                    if before != Tool::Dropper && self.tools.tool() == Tool::Dropper {
                        self.dropper_once.arm(before);
                    }
                }
                Some(Indicator::FocusEditor(role)) => self.session.editing = role,
                None => {}
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
        // Con un diálogo modal abierto el lienzo no interactúa.
        let canvas_inputs = if self.modal.is_open() {
            Vec::new()
        } else {
            self.canvas.interact_roles(ui)
        };
        for (input, role) in canvas_inputs {
            let pixel = self.session.pixel_of(role);
            let result = self
                .tools
                .canvas_input_with(&mut self.session.document, id, pixel, input);
            self.session.absorb(self.tools.take_changes());
            self.finish(&ctx, result);
            for event in self.tools.take_events() {
                match event {
                    ToolEvent::Picked(picked) => {
                        if let Some(text) = self.session.apply_pick_for(picked, role).notice {
                            self.notify(&ctx, text);
                        }
                    }
                    ToolEvent::Refused(error) => self.notify(&ctx, error.to_string()),
                }
            }
            self.tick_dropper_once(&ctx);
        }
        self.tick_dropper_once(&ctx);
        // Con un campo de texto enfocado el teclado es suyo: los atajos no ven nada.
        let events = gate_keys(&ui.input(|i| i.events.clone()), ctx.text_edit_focused());
        self.key_actions(&events, &mut actions);
        if help_pressed(&events) {
            actions.push((AppAction::ShowHelp(self.help_target(&ctx)), Origin::Key));
        }
        let raw_events = ui.input(|i| i.events.clone());
        self.pointer_actions(&raw_events, &mut actions);
        for (action, origin) in actions {
            self.apply_action(&ctx, frame, action, origin);
        }
        // Foco del recuadro de opciones: otra herramienta por cualquier vía lo quita; el clic
        // derecho permitido en una herramienta lo da (después), salvo con un diálogo abierto.
        self.options.sync_tool(self.tools.tool());
        if toolbox.focus_options {
            self.options.focus();
        }
        if self.dialogs.pending() || self.modal.is_open() {
            self.options.blur();
        }

        // Vista previa del color que se está editando y superposición del trazo: ambas viven en
        // las texturas del lienzo y vuelven al documento al soltar o cancelar. Después, `sync`
        // sube lo que cambió en el documento (solo las baldosas visibles).
        let mut sync_error = None;
        if let Some(image) = self.session.document.image(id) {
            match self.session.editor.preview() {
                Some((EditTarget::Entry(index), color)) => {
                    self.canvas.preview_entry(&ctx, image, index, color);
                }
                _ => self.canvas.end_preview(&ctx, image),
            }
            let pending: Vec<(u32, u32, Rgba)> = self
                .tools
                .preview()
                .filter_map(|edit| Some((edit.x, edit.y, pixel_rgba(image, edit.pixel)?)))
                .collect();
            self.canvas.set_overlay(&ctx, image, &pending);
            if let Err(error) = self.canvas.sync_with(&ctx, image, &mut *self.tile_alloc) {
                sync_error = Some(error.to_string());
            }
        }
        if let Some(text) = sync_error {
            self.notify(&ctx, text);
        }
        let image = self.session.document.image(id);
        let canvas_info = self.canvas.paint(ui);

        let now = ui.input(|i| i.time);
        let notice = self
            .notice
            .as_ref()
            .and_then(|notice| notice.visible(now))
            .map(str::to_string);
        if notice.is_none() {
            self.notice = None;
        }
        let canvas_error = canvas_info.error.as_ref().map(|error| error.to_string());
        let tool = self.tools.tool();
        let colors = image.and_then(|image| {
            let slots = self.session.slots.get(image.mode());
            colors_label(
                tool,
                tool_color_label(tool, image, self.session.choice, slots),
                choice_label_with(image, self.session.background, slots),
            )
        });
        let info = app_title_info(
            tool,
            image,
            &canvas_info,
            colors,
            self.session.document.can_undo(),
            self.session.document.can_redo(),
            canvas_error.or(notice),
        );
        let title = window_title(&info);
        let columns =
            px_ui::status::bar_columns(ctx.content_rect().width(), ctx.pixels_per_point());
        let status = (status_line_fit(&info, columns), info.error.is_some());
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
        self.show_modal(&ctx);
    }
}

/// Arranca la aplicación con los argumentos de la línea de comandos (sin el nombre del programa):
/// `--licencias` imprime y devuelve `Ok`; unos argumentos inválidos salen del proceso con el código 2.
pub fn run(args: Vec<OsString>) -> eframe::Result {
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
            let mut app = App::new(document, cli_path);
            // Los atajos del usuario: se leen sin tocar el fichero; lo que falle, se avisa.
            let mut store = ShortcutStore::new(shortcuts_path(Platform::current(), &|name| {
                std::env::var(name).ok()
            }));
            let loaded = store.load(&Keymap::default_scheme());
            app.install_shortcuts(store, loaded);
            app.icons = icons::load(&cc.egui_ctx).map_err(|e| e.to_string())?;
            Ok(Box::new(app))
        }),
    )
}
