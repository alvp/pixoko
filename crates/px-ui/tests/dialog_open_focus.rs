//! Foco inicial de un diálogo modal **tal como lo abre el usuario** (specs/hito-2-dialogos-modales.md,
//! decisión 7 y «Prueba manual» 1): con un clic de ratón sobre una entrada del menú desplegable.
//!
//! Fallo observado a mano en la unidad 18: abriendo `Ayuda → Licencias` con el ratón, tras abrirse
//! no hay ningún widget con foco (`AvPág`, `RePág`, `Start` y `Fin` no mueven el texto e `Intro`
//! cierra el diálogo); el primer `Tab` lleva el foco a la zona de texto. Las pruebas sin ventana de
//! la fila 11 pasaban porque abren el diálogo de forma sintética, en el primer fotograma y sin clic.
//!
//! El banco de este fichero reproduce el orden de fotograma de `px-app` (`App::ui`): primero el menú
//! (`show_menu`), luego las acciones del fotograma (`ShowLicenses` abre el `Modal` con los `Locks`
//! del fotograma) y, el último, `show_dialog`. Sin ventana ni capturas: `Context::run_ui` con
//! `RawInput` sintético. Tras **cada** fotograma se anota qué `Id` tiene el foco en la memoria de
//! egui (`ctx.memory(|m| m.focused())`).
//!
//! - Con el clic en el menú: el foco inicial tiene que estar en la zona de texto (`Zone("text_2")`)
//!   desde el fotograma siguiente al de apertura y mantenerse en los demás.
//! - Con el foco ahí: `AvPág`, `Fin`, `RePág` e `Start` desplazan la zona e `Intro` no cierra.
//! - La variante sin menú (el diálogo abierto antes del primer fotograma, como con
//!   `PIXOKO_TEST_DIALOG=1`) se comprueba igual, con Licencias y con un formulario con campos.

use egui::{Area, Context, Id, Key, Modifiers, Order, Pos2, Vec2, pos2, vec2};
use px_ui::actions::{AppAction, Locks, Platform, UiState};
use px_ui::dialog::{Dialog, DialogKind, FormSpec, Modal, Reaction};
use px_ui::dialog_keys::{FocusTarget, focus_order, initial_focus};
use px_ui::dialog_view::show_dialog;
use px_ui::licenses_view::{LICENSES_FIELD, licenses_form};
use px_ui::menu::show_menu;
use px_ui::text::apply_scale;
use px_ui::theme::visuals;
use px_ui::tool::Tool;

mod common;
#[path = "common/dialog_forms.rs"]
mod dialog_forms;

use common::{PPPS, button_2, context, frame, key_2, move_pointer, n_de, painted};
use dialog_forms::all_kinds;

/// La ventana mínima real de la escala (`468n × 333n` px) o 960 × 720 pt.
#[derive(Clone, Copy, Debug)]
enum Window {
    Minimum,
    Large,
}

const WINDOWS: [Window; 2] = [Window::Minimum, Window::Large];

fn screen(ppp: f32, window: Window) -> Vec2 {
    match window {
        Window::Minimum => {
            let n = n_de(ppp) as f32;
            vec2(468.0 * n / ppp, 333.0 * n / ppp)
        }
        Window::Large => vec2(960.0, 720.0),
    }
}

/// Un texto largo (200 líneas) para que la zona de Licencias tenga a dónde desplazarse.
fn text_long() -> String {
    (1..=200)
        .map(|n| format!("linea {n}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn id_of_the_area() -> Id {
    Id::unique(("pixoko-dialog-zone", LICENSES_FIELD))
}

/// Un fotograma de la aplicación: menú, apertura por acción y diálogo.
struct Application {
    ctx: Context,
    ppp: f32,
    window: Window,
    modal: Modal,
    focus: Option<FocusTarget>,
    /// Diálogos aceptados o cancelados (cerrados) desde el principio.
    closes_2: usize,
    /// Qué `Id` tenía el foco al final de cada fotograma, y si el diálogo estaba abierto.
    trace: Vec<(bool, Option<Id>)>,
    /// El fotograma (índice en `trace`) en que `show_dialog` pintó el diálogo por primera vez.
    open_in: Option<usize>,
    /// Lo pintado en el último fotograma, para buscar entradas del menú.
    last_2: common::Painted,
    /// Si está, un botón «Abrir formulario» (en una capa bajo el velo) abre este formulario con un
    /// clic: la misma apertura que el menú, para un diálogo con campos.
    form: Option<fn() -> FormSpec>,
}

impl Application {
    fn new(ppp: f32, window: Window) -> Application {
        let ctx = context(ppp);
        ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
        Application {
            ctx,
            ppp,
            window,
            modal: Modal::Closed,
            focus: None,
            closes_2: 0,
            trace: Vec::new(),
            open_in: None,
            last_2: common::Painted::default(),
            form: None,
        }
    }

    /// Con un botón que abre `form` con un clic.
    fn with_form_button(ppp: f32, window: Window, form: fn() -> FormSpec) -> Self {
        let mut app = Application::new(ppp, window);
        app.form = Some(form);
        app
    }

    /// Cambio de escala y de tamaño de la ventana con el diálogo abierto.
    fn changes(&mut self, ppp: f32, window: Window) {
        self.ppp = ppp;
        self.window = window;
        self.ctx.set_pixels_per_point(ppp);
        self.ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
    }

    /// El diálogo ya abierto antes del primer fotograma (como `PIXOKO_TEST_DIALOG=1`).
    fn with_dialog(ppp: f32, window: Window, kind: DialogKind, spec: FormSpec) -> Application {
        let mut app = Application::new(ppp, window);
        assert!(app.modal.open(kind, spec, Locks::default()));
        app
    }

    fn dialog_2(&self) -> Option<&Dialog> {
        match &self.modal {
            Modal::Open(d) => Some(d),
            Modal::Closed => None,
        }
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        let size = screen(self.ppp, self.window);
        let mut closes = false;
        let mut paint_the_dialog = false;
        let shapes_2 = {
            let Application {
                ctx,
                modal,
                focus,
                form,
                ..
            } = &mut *self;
            frame(ctx, size, events, |ui| {
                let c = ui.ctx().clone();
                apply_scale(&c);
                let state = UiState {
                    can_undo: false,
                    can_redo: false,
                    zoom: None,
                    tool: Tool::Pencil,
                    locks: Locks {
                        text_focus: c.text_edit_focused(),
                        modal: modal.is_open(),
                        ..Locks::default()
                    },
                };
                // Menú y acciones del fotograma, como `App::ui`.
                let mut actions = Vec::new();
                if let Some(action) = show_menu(ui, &state, Platform::Linux) {
                    actions.push(action);
                }
                for action in actions {
                    if action == AppAction::ShowLicenses {
                        let locks = Locks {
                            text_focus: c.text_edit_focused(),
                            modal: modal.is_open(),
                            ..Locks::default()
                        };
                        if modal.open(DialogKind::Licenses, licenses_form(&text_long()), locks) {
                            *focus = None;
                        }
                    }
                }
                if let Some(form) = form {
                    let button_2 = Area::new(Id::unique("abrir_formulario"))
                        .order(Order::Middle)
                        .fixed_pos(pos2(2.0, 150.0))
                        .show(&c, |ui| ui.button("Abrir formulario"))
                        .inner;
                    if button_2.clicked() {
                        let locks = Locks {
                            text_focus: c.text_edit_focused(),
                            modal: modal.is_open(),
                            ..Locks::default()
                        };
                        if modal.open(DialogKind::Licenses, form(), locks) {
                            *focus = None;
                        }
                    }
                }
                // El diálogo, el último, como `App::show_modal`.
                if let Modal::Open(dialog) = modal {
                    paint_the_dialog = true;
                    let reaction = show_dialog(&c, dialog, focus);
                    if matches!(reaction, Reaction::Accepted(_) | Reaction::Cancelled) {
                        closes = true;
                    }
                }
            })
        };
        if closes {
            self.modal.abandon();
            self.focus = None;
            self.closes_2 += 1;
        }
        if paint_the_dialog && self.open_in.is_none() {
            self.open_in = Some(self.trace.len());
        }
        self.trace
            .push((self.modal.is_open(), self.ctx.memory(|m| m.focused())));
        self.last_2 = painted(&shapes_2);
    }

    fn rest_2_2(&mut self) {
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    /// Mueve, pulsa y suelta en `pos`: tres fotogramas, como los entrega winit.
    fn click(&mut self, pos: Pos2) {
        self.frame(vec![move_pointer(pos)]);
        self.frame(vec![button_2(pos, true)]);
        self.frame(vec![button_2(pos, false)]);
    }

    fn presses_2(&mut self, key: Key) {
        self.frame(vec![key_2(key, Modifiers::NONE)]);
        self.rest_2_2();
    }

    /// El centro del texto pintado `searched` en el último fotograma.
    fn center_of(&self, searched: &str) -> Pos2 {
        self.last_2
            .text_with(searched)
            .unwrap_or_else(|| {
                panic!(
                    "no se pinta {searched:?}; hay {:?}",
                    self.last_2.all_the_texts()
                )
            })
            .rect()
            .center()
    }

    /// `Ayuda → Licencias` con el ratón, como lo hace el usuario, y unos fotogramas de reposo.
    fn opens_licenses_with_the_mouse(&mut self) {
        self.rest_2_2();
        let help = self.center_of("Ayuda");
        self.click(help);
        self.rest_2_2();
        let licenses = self.center_of("Licencias");
        self.click(licenses);
        self.rest_2_2();
    }

    /// El `Id` que tenía el foco tras cada fotograma posterior al de apertura.
    fn focus_after_opening(&self) -> Vec<(usize, Option<Id>)> {
        let from_2 = self.open_in.map_or(self.trace.len(), |i| i + 1);
        self.trace
            .iter()
            .enumerate()
            .skip(from_2)
            .map(|(i, (_, id))| (i, *id))
            .collect()
    }

    fn first_line(&self) -> usize {
        self.dialog_2()
            .expect("el diálogo está abierto")
            .first_line(LICENSES_FIELD)
    }
}

fn describe(app: &Application) -> String {
    format!(
        "abierto_en {:?}; foco tras cada fotograma (abierto, id) {:?}; zona {:?}; focus {:?}",
        app.open_in,
        app.trace,
        id_of_the_area(),
        app.focus
    )
}

// ---------------------------------------------------------------------------------------------
// Con el clic en el menú
// ---------------------------------------------------------------------------------------------

#[test]
fn licenses_opened_with_a_click_on_the_menu_leave_the_focus_in_the_text_area() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut app = Application::new(ppp, window);
            app.opens_licenses_with_the_mouse();
            assert!(app.modal.is_open(), "ppp {ppp} {window:?}: no se abrió");
            assert!(
                app.open_in.is_some(),
                "ppp {ppp} {window:?}: nunca se pintó"
            );
            let focus = app.focus_after_opening();
            assert!(
                focus.len() >= 3,
                "ppp {ppp} {window:?}: faltan fotogramas tras abrir"
            );
            for (frame, id) in focus {
                assert_eq!(
                    id,
                    Some(id_of_the_area()),
                    "ppp {ppp} {window:?}: el foco de egui en el fotograma {frame} \
                     (el diálogo se abrió en {:?}) no es la zona de texto; {}",
                    app.open_in,
                    describe(&app)
                );
            }
            assert_eq!(
                app.focus,
                Some(FocusTarget::Zone(LICENSES_FIELD)),
                "ppp {ppp} {window:?}: el foco que lleva show_dialog"
            );
        }
    }
}

#[test]
fn with_the_dialog_opened_from_the_menu_page_down_end_page_up_and_home_scroll_the_area() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut app = Application::new(ppp, window);
            app.opens_licenses_with_the_mouse();
            assert_eq!(app.first_line(), 0, "ppp {ppp} {window:?}: al abrir");

            app.presses_2(Key::PageDown);
            let after_page_down = app.first_line();
            assert!(
                after_page_down > 0,
                "ppp {ppp} {window:?}: AvPág no desplaza; {}",
                describe(&app)
            );

            app.presses_2(Key::End);
            let after_fin = app.first_line();
            assert!(
                after_fin > after_page_down,
                "ppp {ppp} {window:?}: Fin ({after_fin}) no llega más lejos que AvPág ({after_page_down})"
            );

            app.presses_2(Key::PageUp);
            let after_page_up = app.first_line();
            assert!(
                after_page_up < after_fin,
                "ppp {ppp} {window:?}: RePág no sube"
            );

            app.presses_2(Key::Home);
            assert_eq!(
                app.first_line(),
                0,
                "ppp {ppp} {window:?}: Inicio no vuelve al principio"
            );
            assert_eq!(app.closes_2, 0);
        }
    }
}

#[test]
fn with_the_dialog_opened_from_the_menu_enter_in_the_area_does_not_close_it() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut app = Application::new(ppp, window);
            app.opens_licenses_with_the_mouse();
            app.presses_2(Key::Enter);
            assert!(
                app.modal.is_open() && app.closes_2 == 0,
                "ppp {ppp} {window:?}: Intro en la zona de texto cerró el diálogo; {}",
                describe(&app)
            );
        }
    }
}

#[test]
fn with_the_dialog_opened_from_the_menu_tab_goes_to_close_and_escape_closes() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut app = Application::new(ppp, window);
            app.opens_licenses_with_the_mouse();
            app.presses_2(Key::Tab);
            assert_eq!(
                app.focus,
                Some(FocusTarget::Accept),
                "ppp {ppp} {window:?}: tras el primer Tab el foco va a Cerrar (la zona era el inicial)"
            );
            app.presses_2(Key::Escape);
            assert_eq!(app.closes_2, 1, "ppp {ppp} {window:?}: Escape no cierra");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Sin menú: el diálogo abierto antes del primer fotograma (PIXOKO_TEST_DIALOG=1)
// ---------------------------------------------------------------------------------------------

#[test]
fn variant_without_menu_licenses_in_the_first_frame_has_the_focus_on_the_area() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut app = Application::with_dialog(
                ppp,
                window,
                DialogKind::Licenses,
                licenses_form(&text_long()),
            );
            app.rest_2_2();
            assert_eq!(app.open_in, Some(0));
            for (frame, id) in app.focus_after_opening() {
                assert_eq!(
                    id,
                    Some(id_of_the_area()),
                    "ppp {ppp} {window:?}: fotograma {frame}; {}",
                    describe(&app)
                );
            }
            app.presses_2(Key::PageDown);
            assert!(app.first_line() > 0, "ppp {ppp} {window:?}: AvPág");
            app.presses_2(Key::Enter);
            assert_eq!(app.closes_2, 0, "ppp {ppp} {window:?}: Intro cierra");
        }
    }
}

#[test]
fn variant_without_menu_a_form_with_fields_has_the_focus_on_the_first_field() {
    for ppp in PPPS {
        for window in WINDOWS {
            let dialog = Dialog::new(DialogKind::Licenses, all_kinds()).expect("spec válida");
            let expected_2 = initial_focus(&dialog);
            let FocusTarget::Field(field_2) = expected_2 else {
                panic!("el foco inicial del formulario no es un campo: {expected_2:?}");
            };
            let mut app = Application::with_dialog(ppp, window, DialogKind::Licenses, all_kinds());
            app.rest_2_2();
            assert_eq!(app.focus, Some(expected_2), "ppp {ppp} {window:?}");
            for (frame, id) in app.focus_after_opening() {
                assert_eq!(
                    id,
                    Some(Id::unique(("pixoko-dialog", field_2))),
                    "ppp {ppp} {window:?}: fotograma {frame}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Con el diálogo abierto por un clic: redimensionar, cambiar de escala y recorrer con Tab
// ---------------------------------------------------------------------------------------------

fn opens_the_form_with_a_click(app: &mut Application) {
    app.rest_2_2();
    let button_2 = app.center_of("Abrir formulario");
    app.click(button_2);
    app.rest_2_2();
}

fn form_of_test() -> FormSpec {
    all_kinds()
}

/// Un ciclo entero de `Tab` más una vuelta extra desde el foco actual: cada paso es el siguiente de
/// `focus_order`.
fn tab_walks_the_order(app: &mut Application, context: &str) {
    let order = focus_order(app.dialog_2().expect("abierto"));
    let mut actual = app.focus.expect("hay un foco para empezar");
    for step in 0..=order.len() {
        let position = order
            .iter()
            .position(|t| *t == actual)
            .unwrap_or_else(|| panic!("{context}: {actual:?} no está en {order:?}"));
        let expected_2 = order[(position + 1) % order.len()];
        app.presses_2(Key::Tab);
        assert_eq!(
            app.focus,
            Some(expected_2),
            "{context}: paso {step} de Tab desde {actual:?}; orden {order:?}"
        );
        actual = expected_2;
    }
}

#[test]
fn a_form_opened_with_a_click_has_the_initial_focus_and_tab_walks_the_order_of_the_spec() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut app = Application::with_form_button(ppp, window, form_of_test);
            opens_the_form_with_a_click(&mut app);
            let dialog = app.dialog_2().expect("se abrió");
            let expected_2 = initial_focus(dialog);
            let context = format!("ppp {ppp} {window:?}");
            assert_eq!(app.focus, Some(expected_2), "{context}: foco inicial");
            assert!(
                app.ctx.memory(|m| m.focused()).is_some(),
                "{context}: egui no tiene ningún widget con foco"
            );
            tab_walks_the_order(&mut app, &context);
        }
    }
}

#[test]
fn the_initial_focus_is_kept_after_resizing_and_changing_scale_with_the_menu() {
    for ppp in PPPS {
        let mut app = Application::new(ppp, Window::Minimum);
        app.opens_licenses_with_the_mouse();
        let zone = Some(FocusTarget::Zone(LICENSES_FIELD));
        assert_eq!(app.focus, zone, "ppp {ppp}: tras abrir");
        let id = app.ctx.memory(|m| m.focused());
        assert_eq!(id, Some(id_of_the_area()), "ppp {ppp}: tras abrir");
        for other in PPPS {
            for window in [Window::Large, Window::Minimum] {
                app.changes(other, window);
                app.rest_2_2();
                assert_eq!(
                    app.focus, zone,
                    "ppp {ppp} -> {other} {window:?}: el foco que lleva show_dialog"
                );
                assert_eq!(
                    app.ctx.memory(|m| m.focused()),
                    id,
                    "ppp {ppp} -> {other} {window:?}: el foco de egui"
                );
            }
        }
        // El orden de Tab sigue siendo el de la spec tras tanto cambio.
        tab_walks_the_order(&mut app, &format!("ppp {ppp}, tras los cambios"));
        // Y el diálogo sigue abierto y sin cerrar.
        assert!(app.modal.is_open() && app.closes_2 == 0);
    }
}

#[test]
fn the_initial_focus_of_a_form_is_kept_after_resizing_and_changing_scale() {
    for ppp in PPPS {
        let mut app = Application::with_form_button(ppp, Window::Minimum, form_of_test);
        opens_the_form_with_a_click(&mut app);
        let initial = Some(initial_focus(app.dialog_2().expect("abierto")));
        assert_eq!(app.focus, initial, "ppp {ppp}: tras abrir");
        let id = app.ctx.memory(|m| m.focused());
        assert!(id.is_some(), "ppp {ppp}: egui sin foco tras abrir");
        for other in PPPS {
            for window in [Window::Large, Window::Minimum] {
                app.changes(other, window);
                app.rest_2_2();
                assert_eq!(app.focus, initial, "ppp {ppp} -> {other} {window:?}");
                assert_eq!(
                    app.ctx.memory(|m| m.focused()),
                    id,
                    "ppp {ppp} -> {other} {window:?}: el foco de egui"
                );
            }
        }
        tab_walks_the_order(&mut app, &format!("ppp {ppp}, tras los cambios"));
    }
}
