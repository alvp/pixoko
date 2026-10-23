//! Pixoko: ventana mínima que muestra una imagen con zoom entero y desplazamiento
//! (specs/hito-2-ventana.md), con lápiz y deshacer/rehacer (specs/hito-2-lapiz-deshacer.md)
//! y las herramientas línea y relleno (specs/hito-2-linea-relleno-app.md). Abre PNG por
//! argumento o arrastrando, y los guarda con `Ctrl`+`S` (specs/hito-2-abrir-guardar.md).

mod background_color;
#[cfg(test)]
mod checker_saved;
mod config;
mod confirm;
mod convert_mode;
mod demo;
mod dialogs;
mod dispatch;
mod dropper_once;
mod files;
#[cfg(test)]
mod gate;
mod icons;
mod keys;
mod licenses;
mod merge_flow;
mod new_image;
mod save_wait;
mod session;
mod shortcuts_flow;
mod title;
mod window_min;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::background_color::{background_from_values, open_background_color};
use eframe::egui;
use px_core::{Document, DocumentError, Mode};
use px_ui::actions::{AppAction, Locks, Origin, Platform, UiState};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInput};
use px_ui::checker::{ENV_MESH, mesh_fallback_enabled};
use px_ui::chrome::COLUMN_DEFAULT_COLS;
use px_ui::color::choice_label_with;
use px_ui::color_edit::{DOCUMENT_CHANGED_NOTICE, EditTarget};
use px_ui::column::{ColumnView, show_column};
use px_ui::dialog::{DialogKind as ModalKind, Modal, Reaction};
use px_ui::dialog_keys::FocusTarget;
use px_ui::dialog_view::show_dialog;
use px_ui::gestures::GestureMap;
use px_ui::help::{
    HelpTarget, help_target_at_cols, help_target_in_indicator_cols, help_target_in_options,
    help_target_in_panel,
};
use px_ui::key_events::{help_pressed, presses};
use px_ui::keymap::Keymap;
use px_ui::keymap_file::{Loaded, load_notice};
use px_ui::layers_panel::PanelState;
use px_ui::layers_panel_view::{PanelEvent, PanelView, show_layer_panel};
use px_ui::licenses_view::licenses_form;
use px_ui::menu::show_menu_full;
use px_ui::notice::{NOTICE_SECS, Notice};
use px_ui::options::{OptionsWindow, options_row_count};
use px_ui::options_view::{OptionsEvent, OptionsView, show_options_window};
use px_ui::palette_ops::{PaletteOp, REMOVAL_CANCELLED_NOTICE};
use px_ui::palette_panel::{Indicator, Roles};
use px_ui::pointer_events::window_actions;
use px_ui::shortcuts::{FileAction, gate_keys};
use px_ui::source::{DocSource, OverlayCache, flatten};
use px_ui::texture::pixel_rgba;
use px_ui::tiles::{SystemTileAlloc, TileAlloc};
use px_ui::tool::{Tool, ToolAction, ToolEvent, ToolState, tool_color_label};
use px_ui::toolbar::{ToolboxOutput, ToolboxView, zoom_label};

use crate::config::{SaveError, ShortcutStore, shortcuts_path};
use crate::confirm::{
    Confirm, Confirmation, Request, close_warning, new_warning, open_dialog_warning, open_warning,
};
use crate::convert_mode::{
    Analysis, CONVERTING_NOTICE, ConvertModeError, ConvertSync, History, PendingConvert, Plan,
    Start, SystemAnalyzer, SystemDocAnalyzer, convert_memory_notice, convert_pixels,
    describe_converted, flatten_offer, open_convert_mode_layers, plan_from_values,
    reserve_for_convert, start_convert, sync_convert, sync_convert_all,
};
use crate::demo::{ENV_DEMO_DIALOG, demo_dialog_enabled, demo_form, demo_notice};
use crate::dialogs::{
    DialogKind, Dialogs, Finished, Outcome, RfdPicker, SaveRoute, dialog_title, export_name,
    fallback_request, ignored_file_notice, is_aseprite_path, new_gate, open_gate,
    proposed_save_name, unavailable_notice,
};
use crate::dispatch::{Dispatched, LayerPanelOp, Targets, dispatch};
use crate::dropper_once::DropperOnce;
use crate::files::{file_label, parse_args, pick_dropped};
use crate::keys::select_tool;
use crate::licenses::wants_licenses;
use crate::merge_flow::{
    MergeKind, PendingMerge, StartMerge, flatten_gate, flatten_warning, merge_pixels, start_merge,
    wait_notice,
};
use crate::new_image::{
    FormSync, NewImageError, SystemAlloc, build_new_image, describe_new, memory_notice,
    open_new_image, spec_from_values, sync_form,
};
use crate::save_wait::{
    EXPORTING_NOTICE, PendingSave, SAVING_NOTICE, StartSave, save_pixels, start_save,
};
use crate::session::{
    LayerReport, Session, describe_saved, describe_saved_dialog, dropper_source_for, saved_notice,
};
use crate::shortcuts_flow::{Flow, FlowOutcome, Strip};
use crate::title::{
    app_title_info, colors_label, status_for, status_line_fit, title_text, title_with_file,
};
use crate::window_min::{ENV_NO_MIN, min_enabled, min_inner_size};

/// Una conversión que espera su fotograma tras `Accept` (specs/hito-2-convertir-modo.md, D5).
struct PendingConversion {
    pending: PendingConvert,
    /// El informe vigente al aceptar, para el aviso de 5 s.
    analysis: Option<Analysis>,
    /// Fotogramas que se dejan pasar antes de contar: el aviso se pinta un fotograma después de
    /// ponerlo (la barra de estado va un fotograma por detrás).
    skip: u8,
}

/// Un guardado que espera su fotograma (specs/hito-3-capas-vista.md, condición 4).
enum SaveJob {
    /// `Ctrl+S` (o `Ctrl+Mayús+S`, con `as_copy`) sin diálogo.
    File { as_copy: bool },
    /// La ruta elegida en el diálogo.
    Chosen(PathBuf),
    /// `Export PNG aplanado…` en la ruta elegida (no cambia el estado del documento).
    Export(PathBuf),
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
    /// What the palette panel asked the status line to show in the last frame.
    panel_hint: Option<String>,
    /// Confirmación por repetición al abrir o cerrar con cambios sin guardar.
    confirm: Confirm,
    /// Diálogos de archivo (specs/hito-2-dialogos-archivo.md).
    dialogs: Dialogs,
    /// Fichero del argumento de la línea de comandos, pendiente de abrir en el primer fotograma.
    cli_path: Option<PathBuf>,
    /// El tamaño mínimo (alto y ancho) de la ventana está activo (`PIXOKO_NO_MIN_SIZE=1` lo desactiva).
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
    /// La ventana flotante de opciones de la herramienta activa (C2): abierta o cerrada, su posición
    /// (una sola, de todas las herramientas) y el desplazamiento de cada una; no es del documento.
    options: OptionsWindow,
    /// Columnas de muestras que eligió el usuario arrastrando el borde de la columna (C2); se
    /// recuerda en la sesión (el hito 8 lo guardará).
    column_cols: i64,
    /// Columnas con que se pintó la columna en el último fotograma (la ventana la recorta y el
    /// editor la ensancha): de ahí salen la ayuda `F1` y la posición de la ventana de opciones.
    shown_cols: i64,
    /// Las texturas de los iconos propios (I.4); vacío hasta que `main` las carga.
    icons: icons::Icons,
    /// El panel de capas flotante (3.4): abierto o cerrado, posición y lista; no es del documento.
    layers: PanelState,
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
    /// Abrir el diálogo de prueba en el primer fotograma (`PIXOKO_TEST_DIALOG=1`).
    demo_pending: bool,
    /// Con el diálogo de nueva imagen abierto: su traslado de fondo y si el descarte de los
    /// cambios sin guardar ya se confirmó (specs/hito-2-nueva-imagen.md, D6 y D8).
    new_image: Option<(FormSync, bool)>,
    /// Con el diálogo de convertir modo abierto: su informe y su espera.
    convert: Option<ConvertSync>,
    /// Una conversión de una imagen grande que espera su fotograma: la entrada está bloqueada.
    pending_convert: Option<PendingConversion>,
    /// Un guardado que compone mucho y espera su fotograma: la entrada está bloqueada.
    pending_save: Option<(SaveJob, PendingSave)>,
    /// Combinar o aplanar un documento grande que espera su fotograma: la entrada está bloqueada.
    pending_merge: Option<PendingMerge>,
    /// Los colores del trazo pendiente (solo se calculan los píxeles nuevos de cada fotograma).
    overlay: OverlayCache,
    /// Reserva de los búferes de las baldosas (cargar y regenerar); las pruebas la sustituyen por
    /// una que falla.
    tile_alloc: Box<dyn TileAlloc>,
}

/// Lo que cierra una pulsación de `Esc` que la paleta dejó libre (specs/ui-single-column.md,
/// decisión 9): un eslabón por pulsación y la ventana de opciones antes que el editor. Los eslabones
/// de antes (campo de texto, gesto o deslizador, arrastre de muestra, quita pendiente) los reclama la
/// propia paleta y entonces `free` es falso. El panel de capas no se cierra con `Esc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EscapeStep {
    Nothing,
    CloseOptions,
    CloseEditor,
}

/// El eslabón de la cadena de `Esc` que toca cerrar, dado si la paleta lo dejó libre y qué está
/// abierto. Con nada abierto `Esc` sigue siendo del lienzo.
fn escape_step(free: bool, options_open: bool, editor_open: bool) -> EscapeStep {
    if !free {
        EscapeStep::Nothing
    } else if options_open {
        EscapeStep::CloseOptions
    } else if editor_open {
        EscapeStep::CloseEditor
    } else {
        EscapeStep::Nothing
    }
}

/// Lo que hace la salida de la caja con la ventana de opciones: un clic derecho permitido en una
/// herramienta (`toggle_options`, con `SelectTool` de ella) la abre o la cierra según la herramienta
/// que estaba activa antes del clic (`active_before`). Elegir la herramienta es de `dispatch`.
fn apply_toolbox_toggle(window: &mut OptionsWindow, toolbox: &ToolboxOutput, active_before: Tool) {
    if toolbox.toggle_options
        && let Some(AppAction::SelectTool(clicked)) = toolbox.action
    {
        window.right_click(active_before, clicked);
    }
}

impl App {
    fn new(document: Document, cli_path: Option<PathBuf>) -> App {
        let mut canvas = Canvas::new();
        let gestures = GestureMap::default_scheme();
        canvas.set_gestures(gestures.clone());
        // Las teclas de zoom las trata la capa de acciones, con sus propios bloqueos.
        canvas.set_view_keys(false);
        // `PIXOKO_CHECKER_MESH=1`: el damero con una malla de casillas en vez de la textura repetida.
        canvas.set_checker_mesh(mesh_fallback_enabled(
            std::env::var(ENV_MESH).ok().as_deref(),
        ));
        let session = Session::sample(document);
        if let Ok(image) = flatten(&session.document) {
            canvas.set_image(&image);
        }
        App {
            session,
            canvas,
            title: String::new(),
            status: (String::new(), false),
            themed_ppp: 0.0,
            tools: ToolState::new(),
            dropper_once: DropperOnce::new(),
            notice: None,
            panel_hint: None,
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
            options: OptionsWindow::default(),
            column_cols: COLUMN_DEFAULT_COLS,
            shown_cols: COLUMN_DEFAULT_COLS,
            icons: icons::Icons::empty(),
            layers: PanelState::default(),
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
            pending_save: None,
            pending_merge: None,
            overlay: OverlayCache::default(),
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
        let n = px_ui::text::text_scale(ppp);
        match ctx.input(|i| i.pointer.latest_pos()) {
            Some(pos) => {
                let point = ((pos.x * ppp).round() as i64, (pos.y * ppp).round() as i64);
                // El panel de capas flota encima de todo: la ayuda es la del control que haya debajo.
                if self.layers.open
                    && let Some(target) =
                        help_target_in_panel(point, self.layers.current_rect(window, n), n)
                {
                    return target;
                }
                // La ventana de opciones flota sobre la columna y el lienzo: la ayuda es la de la
                // herramienta activa.
                if self.options.open
                    && let Some(target) = help_target_in_options(
                        point,
                        self.options.current_rect(
                            options_row_count(self.tools.tool()),
                            window,
                            n,
                            self.column_cols,
                        ),
                    )
                {
                    return target;
                }
                if let Some(target) =
                    help_target_in_indicator_cols(point, window, n, self.shown_cols)
                {
                    return target;
                }
                help_target_at_cols(point, window, n, self.shown_cols)
            }
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
            || self.waiting()
    }

    /// Una conversión, un guardado o una combinación esperan su fotograma: la entrada está
    /// bloqueada.
    fn waiting(&self) -> bool {
        self.pending_convert.is_some()
            || self.pending_save.is_some()
            || self.pending_merge.is_some()
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
        let image = Some(self.session.document.canvas_image());
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
                self.modal.is_open() || self.waiting(),
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
        if let Err(error) =
            self.canvas
                .load_with(ctx, &DocSource(&loaded.document), &mut *self.tile_alloc)
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
        self.request_save(ctx, SaveJob::File { as_copy });
    }

    /// Guarda ya, o con muchos píxeles por componer dice «Guardando…» y guarda dos fotogramas
    /// después, con la entrada bloqueada mientras tanto.
    fn request_save(&mut self, ctx: &egui::Context, job: SaveJob) {
        let aseprite = match &job {
            SaveJob::Chosen(path) => is_aseprite_path(path),
            SaveJob::File { .. } => self.session.path().is_some_and(is_aseprite_path),
            SaveJob::Export(_) => false,
        };
        match start_save(save_pixels(&self.session.document, aseprite)) {
            StartSave::Now => self.run_save(ctx, job),
            StartSave::Later(wait) => {
                let notice = match job {
                    SaveJob::Export(_) => EXPORTING_NOTICE,
                    SaveJob::File { .. } | SaveJob::Chosen(_) => SAVING_NOTICE,
                };
                self.pending_save = Some((job, wait));
                self.notify(ctx, notice.to_string());
                ctx.request_repaint();
            }
        }
    }

    /// Cuenta un fotograma de un guardado en espera y lo ejecuta en el que toca.
    fn advance_save(&mut self, ctx: &egui::Context) {
        let Some((_, wait)) = &mut self.pending_save else {
            return;
        };
        if wait.tick() {
            if let Some((job, _)) = self.pending_save.take() {
                self.run_save(ctx, job);
            }
        } else {
            ctx.request_repaint();
        }
    }

    fn run_save(&mut self, ctx: &egui::Context, job: SaveJob) {
        match job {
            SaveJob::File { as_copy } => self.save_file_now(ctx, as_copy),
            SaveJob::Chosen(path) => match self.session.save_to(&path) {
                Ok(saved) => {
                    self.confirm.clear();
                    self.dialogs.remember_dir(&saved.path);
                    let text = describe_saved_dialog(&saved);
                    self.notify(ctx, saved_notice(text, &saved, &self.session.document));
                }
                Err(error) => self.notify(ctx, error.to_string()),
            },
            SaveJob::Export(path) => match self.session.export_flat(&path) {
                Ok(saved) => {
                    self.dialogs.remember_dir(&saved.path);
                    let text = self.session.describe_exported(&saved);
                    self.notify(ctx, text);
                }
                Err(error) => self.notify(ctx, error.to_string()),
            },
        }
    }

    fn save_file_now(&mut self, ctx: &egui::Context, as_copy: bool) {
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
                let text = describe_saved(&saved);
                self.notify(ctx, saved_notice(text, &saved, &self.session.document));
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
            FileAction::Save if self.session.save_route() == SaveRoute::Direct => {
                self.save_file(ctx, false);
            }
            FileAction::Save => self.save_via_dialog(ctx, frame, DialogKind::Save),
            FileAction::SaveAs => self.save_via_dialog(ctx, frame, DialogKind::SaveAs),
        }
    }

    /// Guardar con diálogo. Si un diálogo anterior falló y su aviso sigue armado, esta segunda
    /// pulsación guarda sin diálogo (`without-title.png` o la copia, sin pisar nada).
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

    /// `Archivo → Export PNG aplanado…`: el diálogo de guardar con el nombre
    /// `<name_2>-aplanado.png` (o `without-title-aplanado.png`) junto al fichero del documento.
    fn begin_export_dialog(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let proposed = export_name(self.session.path());
        let now = ctx.input(|i| i.time);
        self.dialogs
            .set_title(dialog_title(DialogKind::Export, 0, None));
        if self.dialogs.begin(
            DialogKind::Export,
            false,
            Some(&proposed),
            false,
            now,
            Some(frame),
        ) {
            self.confirm.clear();
            ctx.request_repaint();
        }
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
        let layers = self.session.document.layers().len();
        self.dialogs
            .set_title(dialog_title(kind, layers, path.as_deref()));
        // Con 2 o más capas, el diálogo de guardar propone `.aseprite` (specs/hito-3-aseprite.md).
        let proposed = if kind != DialogKind::Open && layers >= 2 {
            let name = proposed_save_name(path.as_deref(), layers);
            Some(match &path {
                Some(path) if !is_aseprite_path(path) => path.with_file_name(name),
                Some(path) => path.clone(),
                None => PathBuf::from(name),
            })
        } else {
            path.clone()
        };
        if self.dialogs.begin(
            kind,
            discard,
            proposed.as_deref(),
            converted,
            now,
            Some(frame),
        ) {
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
                DialogKind::Save | DialogKind::SaveAs => {
                    self.request_save(ctx, SaveJob::Chosen(path));
                }
                // Escribe el aplanado y no tiene la segunda pulsación de guardar.
                DialogKind::Export => self.request_save(ctx, SaveJob::Export(path)),
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
        self.pending_save = None;
        self.pending_merge = None;
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
            modal: self.modal.is_open() || self.waiting(),
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
            Dispatched::ConvertMode => self.begin_convert_mode(ctx, None),
            Dispatched::LayerPanel(op) => match op {
                LayerPanelOp::Toggle => {
                    self.layers.toggle();
                    ctx.request_repaint();
                }
                LayerPanelOp::FocusName => {
                    self.layers.open = true;
                    self.layers.focus_name = true;
                    ctx.request_repaint();
                }
                LayerPanelOp::ShowProperties => {
                    self.layers.open = true;
                    ctx.request_repaint();
                }
            },
            Dispatched::OfferGrayAlpha { notice, .. } => {
                // Repetir la orden en 5 s abre `Convert mode_2…` con «Gris con alfa» elegido.
                let now = ctx.input(|i| i.time);
                match self.confirm.request(Request::LayersToGrayAlpha, now) {
                    Confirmation::Confirmed => self.begin_convert_mode(ctx, Some(Mode::GrayAlpha)),
                    Confirmation::Armed => self.notify(ctx, notice),
                }
            }
            Dispatched::Merge(kind) => self.begin_merge(ctx, kind),
            Dispatched::ExportFlat => self.begin_export_dialog(ctx, frame),
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

    /// Combinar abajo, combinar visibles o aplanar: los rechazos de D4, la confirmación de Aplanar
    /// con ocultas con píxeles y, con mucho que componer, «Combinando…» / «Aplanando…» dos
    /// fotogramas antes de empezar.
    fn begin_merge(&mut self, ctx: &egui::Context, kind: MergeKind) {
        if let Some(reason) = self.session.merge_refusal(kind) {
            self.notify(ctx, reason.to_string());
            return;
        }
        if kind == MergeKind::Flatten {
            let hidden = self.session.flatten_hidden_with_pixels();
            let now = ctx.input(|i| i.time);
            if !flatten_gate(hidden, &mut self.confirm, now) {
                self.notify(ctx, flatten_warning(hidden));
                return;
            }
        }
        let pixels = merge_pixels(&self.session.document, kind, self.session.active_layer());
        match start_merge(kind, pixels) {
            StartMerge::Now => self.run_merge(ctx, kind),
            StartMerge::Later(wait) => {
                self.pending_merge = Some(wait);
                self.notify(ctx, wait_notice(kind).to_string());
                ctx.request_repaint();
            }
        }
    }

    /// Cuenta un fotograma de una combinación en espera y la ejecuta en el que toca.
    fn advance_merge(&mut self, ctx: &egui::Context) {
        let Some(wait) = &mut self.pending_merge else {
            return;
        };
        let kind = wait.kind();
        if wait.tick() {
            self.pending_merge = None;
            self.run_merge(ctx, kind);
        } else {
            ctx.request_repaint();
        }
    }

    fn run_merge(&mut self, ctx: &egui::Context, kind: MergeKind) {
        let report = self.session.merge(kind);
        self.finish::<std::convert::Infallible>(ctx, Ok(report.changed));
        if let Some(text) = report.notice {
            self.notify(ctx, text);
        }
    }

    /// `Archivo → New image_2…`: con cambios sin guardar, pide confirmación por repetición; luego
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

    /// Convertir el modo toca todas las capas (`ConvertAll`): hay varias, o la celda activa está
    /// vacía y `ConvertMode` no tendría imagen que convertir.
    fn converts_all(&self) -> bool {
        self.session.document.layers().len() > 1
            || self
                .session
                .document
                .image(self.session.active_cel())
                .is_none()
    }

    /// `TestImage → Convert mode_2…`: abre el diálogo con el modo y el color activo de ahora.
    fn begin_convert_mode(&mut self, ctx: &egui::Context, target: Option<Mode>) {
        // El modo y la paleta son los del lienzo (con varias capas, o con la celda activa vacía, no
        // hay otra imagen de la que leerlos).
        let image = self.session.document.canvas_image();
        let from = image.mode();
        let layers = self.session.document.layers().len();
        let active = self
            .session
            .active_pixel()
            .and_then(|pixel| pixel_rgba(image, pixel));
        let state_id = self.session.document.state_id();
        let locks = self.locks(ctx);
        let opened = open_convert_mode_layers(&mut self.modal, from, layers, target, locks);
        if opened {
            self.modal_focus = None;
            self.convert = Some(ConvertSync::new(from, state_id, active));
        }
    }

    /// Lo que pide un control del panel de capas: la misma regla de bloqueo que las acciones de
    /// documento (un trazo, un diálogo o un campo de texto en curso lo impiden; el campo del propio
    /// panel confirma al perder el foco, ya sin texto enfocado).
    fn apply_panel_event(&mut self, ctx: &egui::Context, frame: &eframe::Frame, event: PanelEvent) {
        use px_ui::actions::is_blocked;
        use px_ui::layers_panel::PanelButton;
        match event {
            PanelEvent::Close => {
                self.layers.open = false;
                ctx.request_repaint();
                return;
            }
            PanelEvent::Notice(text) => {
                self.notify(ctx, text);
                return;
            }
            PanelEvent::Button(button) => {
                let action = match button {
                    PanelButton::New => AppAction::NewLayer,
                    PanelButton::Duplicate => AppAction::DuplicateLayer,
                    PanelButton::Remove => AppAction::RemoveLayer,
                    PanelButton::Up => AppAction::LayerUp,
                    PanelButton::Down => AppAction::LayerDown,
                };
                self.apply_action(ctx, frame, action, Origin::Button);
                return;
            }
            _ => {}
        }
        // `Locks` sin el campo de texto: el del nombre perdió el foco justo ahora.
        let mut locks = self.locks(ctx);
        locks.text_focus = false;
        if is_blocked(AppAction::RenameLayer, locks, Origin::Button) {
            return;
        }
        let report = match event {
            PanelEvent::Select(id) => {
                let _ = self.session.set_active_layer(id);
                ctx.request_repaint();
                LayerReport::default()
            }
            PanelEvent::StartRename(_) => {
                self.layers.focus_name = true;
                LayerReport::default()
            }
            PanelEvent::ToggleVisible(id) => {
                let visible = self.session.document.layer(id).is_some_and(|l| l.visible());
                self.session.set_layer_visible(id, !visible)
            }
            PanelEvent::ToggleLocked(id) => {
                let locked = self.session.document.layer(id).is_some_and(|l| l.locked());
                self.session.set_layer_locked(id, !locked)
            }
            PanelEvent::Rename(id, name) => self.session.rename_layer(id, name),
            PanelEvent::Opacity(id, value) => self.session.set_layer_opacity(id, value),
            PanelEvent::Blend(id, blend) => self.session.set_layer_blend(id, blend),
            PanelEvent::Close | PanelEvent::Notice(_) | PanelEvent::Button(_) => {
                LayerReport::default()
            }
        };
        self.finish::<std::convert::Infallible>(ctx, Ok(report.changed));
        if let Some(text) = report.notice {
            self.notify(ctx, text);
        }
    }

    /// El diálogo de convertir se aceptó: con una imagen grande avisa «Convirtiendo…» y convierte
    /// unos fotogramas después; si no, convierte ya.
    fn finish_convert(&mut self, ctx: &egui::Context, values: &px_ui::dialog::Values) {
        let Some(sync) = self.convert.take() else {
            return;
        };
        let doc = &self.session.document;
        let from = doc.mode();
        let pixels = if self.converts_all() {
            convert_pixels(doc)
        } else {
            u64::from(doc.width()) * u64::from(doc.height())
        };
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
        let all = self.converts_all();
        let (width, height) = (
            self.session.document.width(),
            self.session.document.height(),
        );
        if !all {
            let Some(image) = self.session.document.image(self.session.active_cel()) else {
                return;
            };
            if let Err(ConvertModeError::Memory { bytes }) =
                reserve_for_convert(image, plan.target, &mut SystemAlloc)
            {
                self.notify(
                    ctx,
                    convert_memory_notice(width, height, plan.target, bytes),
                );
                return;
            }
        }
        let result = if all {
            self.session.convert_all(plan.target, plan.allowed)
        } else {
            self.session.convert_mode(plan.target, plan.allowed)
        };
        match result {
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
        let source = DocSource(&self.session.document);
        self.canvas
            .sync_with(ctx, &source, &mut *self.tile_alloc)
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
        // El botón `Aplanar` del motivo «con varias capas: aplana antes»: cierra el diálogo y lanza
        // la acción (con su confirmación si hay ocultas con píxeles).
        if kind == ModalKind::ConvertMode
            && let Reaction::Pressed { field, .. } = &reaction
            && flatten_offer(field).is_some()
        {
            self.modal.abandon();
            self.modal_focus = None;
            self.convert = None;
            self.begin_merge(ctx, MergeKind::Flatten);
            return;
        }
        if kind == ModalKind::ConvertMode && self.convert.is_some() {
            let doc = &self.session.document;
            let history = History {
                limit: doc.history_limit(),
                used: doc.history_bytes(),
            };
            let all = doc.layers().len() > 1 || doc.image(self.session.active_cel()).is_none();
            let changed = match &mut self.convert {
                Some(sync) if all => {
                    sync_convert_all(dialog, sync, doc, history, &mut SystemDocAnalyzer)
                }
                Some(sync) => doc.image(self.session.active_cel()).is_some_and(|image| {
                    sync_convert(
                        dialog,
                        sync,
                        image,
                        doc.state_id(),
                        history,
                        &mut SystemAnalyzer,
                    )
                }),
                None => false,
            };
            if changed {
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
        self.advance_save(&ctx);
        self.advance_merge(&ctx);
        // Las acciones del fotograma, en este orden: menú, columna y atajos.
        let mut actions: Vec<(AppAction, Origin)> = Vec::new();
        let state = self.ui_state(&ctx);
        if let Some(action) = show_menu_full(
            ui,
            &state,
            self.platform,
            self.canvas.background(),
            &self.keymap,
            &self.session.layer_nav(),
        ) {
            actions.push((action, Origin::Menu));
        }
        px_ui::status::show(ui, &self.status.0, self.status.1);
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
            || self.waiting());
        self.session.view.icons = self.icons.set;
        self.session.view.editor_open = self.session.editor_open;
        // La columna única (C2): la caja arriba y la paleta debajo, en el borde derecho.
        let level = zoom_label(state.zoom);
        let column = {
            // La celda activa si tiene imagen (la quita de una entrada pregunta si la usa); si
            // está vacía, el lienzo da el modo y la paleta.
            let image = self
                .session
                .document
                .image(self.session.active_cel())
                .unwrap_or_else(|| self.session.document.canvas_image());
            let roles = Roles {
                foreground: self.session.choice,
                background: self.session.background,
                editing: self.session.editing,
            };
            show_column(
                ui,
                &ColumnView {
                    toolbox: ToolboxView {
                        state: &state,
                        zoom_label: &level,
                        icons: &self.icons.set,
                    },
                    image,
                    roles,
                    slots: self.session.slots.get(image.mode()),
                    enabled,
                    cols: self.column_cols,
                },
                &self.keymap,
                self.platform,
                &mut self.session.scroll,
                &mut self.session.view,
                &mut self.session.editor,
                &mut self.session.ops,
            )
        };
        self.shown_cols = column.cols;
        if let Some(cols) = column.resized {
            self.column_cols = cols;
            ctx.request_repaint();
        }
        if let Some(action) = column.toolbox.action {
            actions.push((action, Origin::Button));
        }
        apply_toolbox_toggle(&mut self.options, &column.toolbox, state.tool);
        let panel = Some(column.panel);
        if let Some(panel) = panel {
            if let Some(choice) = panel.choice {
                self.session.choice = choice;
            }
            if let Some(choice) = panel.background {
                self.session.background = choice;
            }
            // La cadena de `Esc`: la ventana de opciones antes que el editor.
            match escape_step(panel.escape_free, self.options.open, panel.close_editor) {
                EscapeStep::CloseOptions => self.options.close(),
                EscapeStep::CloseEditor => self.session.close_editor(),
                EscapeStep::Nothing => {}
            }
            self.panel_hint = px_ui::palette_panel::panel_info(&ctx).status_hint;
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
                Some(Indicator::FocusEditor(role)) => self.session.indicator_right_click(role),
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
                // Con varias capas, «nadie la usa» mira todas las celdas, no solo la activa: si
                // otra la usa, se pide el destino como con una sola capa.
                let used_elsewhere = match op {
                    PaletteOp::Remove { index, .. } if panel.op_unused => {
                        self.session.removal_needs_destination(index)
                    }
                    _ => false,
                };
                if used_elsewhere && let PaletteOp::Remove { index, .. } = op {
                    self.session.begin_removal_for(index);
                } else {
                    self.apply_palette_op(&ctx, op, panel.op_unused);
                }
            }
        }
        // La ventana de opciones de la herramienta activa (C2): flota como el panel de capas, no es
        // modal y con un diálogo, una conversión o un guardado en espera no responde a nada.
        let options_events = {
            let screen = ctx.content_rect();
            let ppp = ctx.pixels_per_point();
            let window_px = (
                (screen.width() * ppp).round() as i64,
                (screen.height() * ppp).round() as i64,
            );
            let tool = self.tools.tool();
            let responsive = !(self.dialogs.pending() || self.modal.is_open() || self.waiting());
            show_options_window(
                &ctx,
                &OptionsView {
                    tool,
                    rows: options_row_count(tool),
                    keymap: &self.keymap,
                    platform: self.platform,
                    enabled: responsive,
                },
                &mut self.options,
                window_px,
                self.column_cols,
            )
        };
        for event in options_events {
            match event {
                OptionsEvent::Close => self.options.close(),
            }
        }
        // El panel de capas (3.4): flota sobre el lienzo y se muestra antes que él para que egui lo
        // vea como la capa de encima. Con un diálogo modal, una conversión o un guardado en espera
        // no responde a nada.
        let panel_events = {
            let screen = ctx.content_rect();
            let ppp = ctx.pixels_per_point();
            let window_px = (
                (screen.width() * ppp).round() as i64,
                (screen.height() * ppp).round() as i64,
            );
            let responsive = !(self.dialogs.pending() || self.modal.is_open() || self.waiting());
            show_layer_panel(
                &ctx,
                &PanelView {
                    doc: &self.session.document,
                    active: self.session.active_layer(),
                    enabled: responsive,
                    icons: &self.icons.set,
                },
                &mut self.layers,
                window_px,
            )
        };
        for event in panel_events {
            self.apply_panel_event(&ctx, frame, event);
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

        let id = self.session.active_cel();
        // Con un diálogo modal abierto el lienzo no interactúa.
        let canvas_inputs = if self.modal.is_open() {
            Vec::new()
        } else {
            self.canvas.interact_roles(ui)
        };
        for (input, role) in canvas_inputs {
            // La capa activa oculta no se pinta: un aviso y ningún gesto (el cuentagotas sí).
            if let CanvasInput::Stroke(CanvasEvent::StrokeBegin(_)) = input
                && !self.tools.busy()
                && let Some(reason) = self.session.paint_refusal(self.tools.tool())
            {
                self.notify(&ctx, reason.to_string());
                continue;
            }
            // El cuentagotas lee lo que se ve; con `Mayús` al pulsar, solo la capa activa.
            if let CanvasInput::Stroke(CanvasEvent::StrokeBegin(_)) = input
                && !self.tools.busy()
            {
                let shift = ctx.input(|i| i.modifiers.shift);
                self.tools.set_dropper_source(dropper_source_for(shift));
            }
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
        // Presupuesto (D04): un aviso, una vez, al cruzar 1 GiB de celdas con píxeles.
        if let Some(text) = self.session.memory_crossing_notice() {
            self.notify(&ctx, text);
        }
        // Vista previa del color que se está editando y superposición del trazo: ambas viven en
        // las texturas del lienzo y vuelven al documento al soltar o cancelar. Después, `sync`
        // sube lo que cambió en el documento (solo las baldosas visibles).
        let mut sync_error = None;
        {
            let source = DocSource(&self.session.document);
            match self.session.editor.preview() {
                Some((EditTarget::Entry(index), color)) => {
                    self.canvas.preview_entry(&ctx, &source, index, color);
                }
                _ => self.canvas.end_preview(&ctx, &source),
            }
            // Los colores del trazo pendiente son lo que se verá con las capas, no el color crudo.
            let pending = self
                .overlay
                .colors(&self.session.document, id, self.tools.preview());
            self.canvas.set_overlay(&ctx, &source, pending);
            if let Err(error) = self.canvas.sync_with(&ctx, &source, &mut *self.tile_alloc) {
                sync_error = Some(error.to_string());
            }
        }
        if let Some(text) = sync_error {
            self.notify(&ctx, text);
        }
        let image = Some(self.session.document.canvas_image());
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
        let mut info = app_title_info(
            tool,
            image,
            &canvas_info,
            colors,
            self.session.document.can_undo(),
            self.session.document.can_redo(),
            canvas_error.or(notice),
        );
        // El tamaño y las capas son del documento: `image` solo da el modo y la paleta.
        info.size = Some((
            self.session.document.width(),
            self.session.document.height(),
        ));
        info.layers = self.session.layers_info();
        let title = title_text(&info);
        let columns =
            px_ui::status::bar_columns(ctx.content_rect().width(), ctx.pixels_per_point());
        // The status line: a notice or error first; then what the panel asks to show (a swatch
        // drag, a pending removal or the swatch under the pointer); then the usual line.
        let status = status_for(
            status_line_fit(&info, columns),
            info.error.is_some(),
            self.panel_hint.as_deref(),
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
