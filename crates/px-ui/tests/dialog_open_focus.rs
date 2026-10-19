//! Foco inicial de un diálogo modal **tal como lo abre el usuario** (specs/hito-2-dialogos-modales.md,
//! decisión 7 y «Prueba manual» 1): con un clic de ratón sobre una entrada del menú desplegable.
//!
//! Fallo observado a mano en la unidad 18: abriendo `Ayuda → Licencias` con el ratón, tras abrirse
//! no hay ningún widget con foco (`AvPág`, `RePág`, `Inicio` y `Fin` no mueven el texto e `Intro`
//! cierra el diálogo); el primer `Tab` lleva el foco a la zona de texto. Las pruebas sin ventana de
//! la fila 11 pasaban porque abren el diálogo de forma sintética, en el primer fotograma y sin clic.
//!
//! El banco de este fichero reproduce el orden de fotograma de `px-app` (`App::ui`): primero el menú
//! (`show_menu`), luego las acciones del fotograma (`ShowLicenses` abre el `Modal` con los `Locks`
//! del fotograma) y, el último, `show_dialog`. Sin ventana ni capturas: `Context::run_ui` con
//! `RawInput` sintético. Tras **cada** fotograma se anota qué `Id` tiene el foco en la memoria de
//! egui (`ctx.memory(|m| m.focused())`).
//!
//! - Con el clic en el menú: el foco inicial tiene que estar en la zona de texto (`Zone("texto")`)
//!   desde el fotograma siguiente al de apertura y mantenerse en los demás.
//! - Con el foco ahí: `AvPág`, `Fin`, `RePág` e `Inicio` desplazan la zona e `Intro` no cierra.
//! - La variante sin menú (el diálogo abierto antes del primer fotograma, como con
//!   `PIXOKO_DIALOGO_PRUEBA=1`) se comprueba igual, con Licencias y con un formulario con campos.

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

use common::{PPPS, boton, contexto, fotograma, mover, n_de, pintado, tecla};
use dialog_forms::all_kinds;

/// La ventana mínima real de la escala (`468n × 333n` px) o 960 × 720 pt.
#[derive(Clone, Copy, Debug)]
enum Ventana {
    Minima,
    Grande,
}

const VENTANAS: [Ventana; 2] = [Ventana::Minima, Ventana::Grande];

fn pantalla(ppp: f32, ventana: Ventana) -> Vec2 {
    match ventana {
        Ventana::Minima => {
            let n = n_de(ppp) as f32;
            vec2(468.0 * n / ppp, 333.0 * n / ppp)
        }
        Ventana::Grande => vec2(960.0, 720.0),
    }
}

/// Un texto largo (200 líneas) para que la zona de Licencias tenga a dónde desplazarse.
fn texto_largo() -> String {
    (1..=200)
        .map(|n| format!("linea {n}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn id_de_la_zona() -> Id {
    Id::unique(("pixoko-dialog-zone", LICENSES_FIELD))
}

/// Un fotograma de la aplicación: menú, apertura por acción y diálogo.
struct Aplicacion {
    ctx: Context,
    ppp: f32,
    ventana: Ventana,
    modal: Modal,
    focus: Option<FocusTarget>,
    /// Diálogos aceptados o cancelados (cerrados) desde el principio.
    cierres: usize,
    /// Qué `Id` tenía el foco al final de cada fotograma, y si el diálogo estaba abierto.
    traza: Vec<(bool, Option<Id>)>,
    /// El fotograma (índice en `traza`) en que `show_dialog` pintó el diálogo por primera vez.
    abierto_en: Option<usize>,
    /// Lo pintado en el último fotograma, para buscar entradas del menú.
    ultimo: common::Pintado,
    /// Si está, un botón «Abrir formulario» (en una capa bajo el velo) abre este formulario con un
    /// clic: la misma apertura que el menú, para un diálogo con campos.
    formulario: Option<fn() -> FormSpec>,
}

impl Aplicacion {
    fn nueva(ppp: f32, ventana: Ventana) -> Aplicacion {
        let ctx = contexto(ppp);
        ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
        Aplicacion {
            ctx,
            ppp,
            ventana,
            modal: Modal::Closed,
            focus: None,
            cierres: 0,
            traza: Vec::new(),
            abierto_en: None,
            ultimo: common::Pintado::default(),
            formulario: None,
        }
    }

    /// Con un botón que abre `formulario` con un clic.
    fn con_boton_de_formulario(ppp: f32, ventana: Ventana, formulario: fn() -> FormSpec) -> Self {
        let mut app = Aplicacion::nueva(ppp, ventana);
        app.formulario = Some(formulario);
        app
    }

    /// Cambio de escala y de tamaño de la ventana con el diálogo abierto.
    fn cambia(&mut self, ppp: f32, ventana: Ventana) {
        self.ppp = ppp;
        self.ventana = ventana;
        self.ctx.set_pixels_per_point(ppp);
        self.ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
    }

    /// El diálogo ya abierto antes del primer fotograma (como `PIXOKO_DIALOGO_PRUEBA=1`).
    fn con_dialogo(ppp: f32, ventana: Ventana, kind: DialogKind, spec: FormSpec) -> Aplicacion {
        let mut app = Aplicacion::nueva(ppp, ventana);
        assert!(app.modal.open(kind, spec, Locks::default()));
        app
    }

    fn dialogo(&self) -> Option<&Dialog> {
        match &self.modal {
            Modal::Open(d) => Some(d),
            Modal::Closed => None,
        }
    }

    fn fotograma(&mut self, events: Vec<egui::Event>) {
        let size = pantalla(self.ppp, self.ventana);
        let mut cierra = false;
        let mut pinto_el_dialogo = false;
        let formas = {
            let Aplicacion {
                ctx,
                modal,
                focus,
                formulario,
                ..
            } = &mut *self;
            fotograma(ctx, size, events, |ui| {
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
                        if modal.open(DialogKind::Licenses, licenses_form(&texto_largo()), locks) {
                            *focus = None;
                        }
                    }
                }
                if let Some(formulario) = formulario {
                    let boton = Area::new(Id::unique("abrir_formulario"))
                        .order(Order::Middle)
                        .fixed_pos(pos2(2.0, 150.0))
                        .show(&c, |ui| ui.button("Abrir formulario"))
                        .inner;
                    if boton.clicked() {
                        let locks = Locks {
                            text_focus: c.text_edit_focused(),
                            modal: modal.is_open(),
                            ..Locks::default()
                        };
                        if modal.open(DialogKind::Licenses, formulario(), locks) {
                            *focus = None;
                        }
                    }
                }
                // El diálogo, el último, como `App::show_modal`.
                if let Modal::Open(dialog) = modal {
                    pinto_el_dialogo = true;
                    let reaction = show_dialog(&c, dialog, focus);
                    if matches!(reaction, Reaction::Accepted(_) | Reaction::Cancelled) {
                        cierra = true;
                    }
                }
            })
        };
        if cierra {
            self.modal.abandon();
            self.focus = None;
            self.cierres += 1;
        }
        if pinto_el_dialogo && self.abierto_en.is_none() {
            self.abierto_en = Some(self.traza.len());
        }
        self.traza
            .push((self.modal.is_open(), self.ctx.memory(|m| m.focused())));
        self.ultimo = pintado(&formas);
    }

    fn reposo(&mut self) {
        for _ in 0..3 {
            self.fotograma(vec![]);
        }
    }

    /// Mueve, pulsa y suelta en `pos`: tres fotogramas, como los entrega winit.
    fn clic(&mut self, pos: Pos2) {
        self.fotograma(vec![mover(pos)]);
        self.fotograma(vec![boton(pos, true)]);
        self.fotograma(vec![boton(pos, false)]);
    }

    fn pulsa(&mut self, key: Key) {
        self.fotograma(vec![tecla(key, Modifiers::NONE)]);
        self.reposo();
    }

    /// El centro del texto pintado `buscado` en el último fotograma.
    fn centro_de(&self, buscado: &str) -> Pos2 {
        self.ultimo
            .texto_con(buscado)
            .unwrap_or_else(|| {
                panic!(
                    "no se pinta {buscado:?}; hay {:?}",
                    self.ultimo.todos_los_textos()
                )
            })
            .rect()
            .center()
    }

    /// `Ayuda → Licencias` con el ratón, como lo hace el usuario, y unos fotogramas de reposo.
    fn abre_licencias_con_el_raton(&mut self) {
        self.reposo();
        let ayuda = self.centro_de("Ayuda");
        self.clic(ayuda);
        self.reposo();
        let licencias = self.centro_de("Licencias");
        self.clic(licencias);
        self.reposo();
    }

    /// El `Id` que tenía el foco tras cada fotograma posterior al de apertura.
    fn foco_tras_la_apertura(&self) -> Vec<(usize, Option<Id>)> {
        let desde = self.abierto_en.map_or(self.traza.len(), |i| i + 1);
        self.traza
            .iter()
            .enumerate()
            .skip(desde)
            .map(|(i, (_, id))| (i, *id))
            .collect()
    }

    fn primera_linea(&self) -> usize {
        self.dialogo()
            .expect("el diálogo está abierto")
            .first_line(LICENSES_FIELD)
    }
}

fn describe(app: &Aplicacion) -> String {
    format!(
        "abierto_en {:?}; foco tras cada fotograma (abierto, id) {:?}; zona {:?}; focus {:?}",
        app.abierto_en,
        app.traza,
        id_de_la_zona(),
        app.focus
    )
}

// ---------------------------------------------------------------------------------------------
// Con el clic en el menú
// ---------------------------------------------------------------------------------------------

#[test]
fn licencias_abiertas_con_un_clic_en_el_menu_dejan_el_foco_en_la_zona_de_texto() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut app = Aplicacion::nueva(ppp, ventana);
            app.abre_licencias_con_el_raton();
            assert!(app.modal.is_open(), "ppp {ppp} {ventana:?}: no se abrió");
            assert!(
                app.abierto_en.is_some(),
                "ppp {ppp} {ventana:?}: nunca se pintó"
            );
            let foco = app.foco_tras_la_apertura();
            assert!(
                foco.len() >= 3,
                "ppp {ppp} {ventana:?}: faltan fotogramas tras abrir"
            );
            for (fotograma, id) in foco {
                assert_eq!(
                    id,
                    Some(id_de_la_zona()),
                    "ppp {ppp} {ventana:?}: el foco de egui en el fotograma {fotograma} \
                     (el diálogo se abrió en {:?}) no es la zona de texto; {}",
                    app.abierto_en,
                    describe(&app)
                );
            }
            assert_eq!(
                app.focus,
                Some(FocusTarget::Zone(LICENSES_FIELD)),
                "ppp {ppp} {ventana:?}: el foco que lleva show_dialog"
            );
        }
    }
}

#[test]
fn con_el_dialogo_abierto_por_el_menu_avpag_fin_repag_e_inicio_desplazan_la_zona() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut app = Aplicacion::nueva(ppp, ventana);
            app.abre_licencias_con_el_raton();
            assert_eq!(app.primera_linea(), 0, "ppp {ppp} {ventana:?}: al abrir");

            app.pulsa(Key::PageDown);
            let tras_avpag = app.primera_linea();
            assert!(
                tras_avpag > 0,
                "ppp {ppp} {ventana:?}: AvPág no desplaza; {}",
                describe(&app)
            );

            app.pulsa(Key::End);
            let tras_fin = app.primera_linea();
            assert!(
                tras_fin > tras_avpag,
                "ppp {ppp} {ventana:?}: Fin ({tras_fin}) no llega más lejos que AvPág ({tras_avpag})"
            );

            app.pulsa(Key::PageUp);
            let tras_repag = app.primera_linea();
            assert!(
                tras_repag < tras_fin,
                "ppp {ppp} {ventana:?}: RePág no sube"
            );

            app.pulsa(Key::Home);
            assert_eq!(
                app.primera_linea(),
                0,
                "ppp {ppp} {ventana:?}: Inicio no vuelve al principio"
            );
            assert_eq!(app.cierres, 0);
        }
    }
}

#[test]
fn con_el_dialogo_abierto_por_el_menu_intro_en_la_zona_no_lo_cierra() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut app = Aplicacion::nueva(ppp, ventana);
            app.abre_licencias_con_el_raton();
            app.pulsa(Key::Enter);
            assert!(
                app.modal.is_open() && app.cierres == 0,
                "ppp {ppp} {ventana:?}: Intro en la zona de texto cerró el diálogo; {}",
                describe(&app)
            );
        }
    }
}

#[test]
fn con_el_dialogo_abierto_por_el_menu_tab_va_a_cerrar_y_escape_cierra() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut app = Aplicacion::nueva(ppp, ventana);
            app.abre_licencias_con_el_raton();
            app.pulsa(Key::Tab);
            assert_eq!(
                app.focus,
                Some(FocusTarget::Accept),
                "ppp {ppp} {ventana:?}: tras el primer Tab el foco va a Cerrar (la zona era el inicial)"
            );
            app.pulsa(Key::Escape);
            assert_eq!(app.cierres, 1, "ppp {ppp} {ventana:?}: Escape no cierra");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Sin menú: el diálogo abierto antes del primer fotograma (PIXOKO_DIALOGO_PRUEBA=1)
// ---------------------------------------------------------------------------------------------

#[test]
fn variante_sin_menu_licencias_en_el_primer_fotograma_tiene_el_foco_en_la_zona() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut app = Aplicacion::con_dialogo(
                ppp,
                ventana,
                DialogKind::Licenses,
                licenses_form(&texto_largo()),
            );
            app.reposo();
            assert_eq!(app.abierto_en, Some(0));
            for (fotograma, id) in app.foco_tras_la_apertura() {
                assert_eq!(
                    id,
                    Some(id_de_la_zona()),
                    "ppp {ppp} {ventana:?}: fotograma {fotograma}; {}",
                    describe(&app)
                );
            }
            app.pulsa(Key::PageDown);
            assert!(app.primera_linea() > 0, "ppp {ppp} {ventana:?}: AvPág");
            app.pulsa(Key::Enter);
            assert_eq!(app.cierres, 0, "ppp {ppp} {ventana:?}: Intro cierra");
        }
    }
}

#[test]
fn variante_sin_menu_un_formulario_con_campos_tiene_el_foco_en_el_primer_campo() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let dialog = Dialog::new(DialogKind::Licenses, all_kinds()).expect("spec válida");
            let esperado = initial_focus(&dialog);
            let FocusTarget::Field(campo) = esperado else {
                panic!("el foco inicial del formulario no es un campo: {esperado:?}");
            };
            let mut app = Aplicacion::con_dialogo(ppp, ventana, DialogKind::Licenses, all_kinds());
            app.reposo();
            assert_eq!(app.focus, Some(esperado), "ppp {ppp} {ventana:?}");
            for (fotograma, id) in app.foco_tras_la_apertura() {
                assert_eq!(
                    id,
                    Some(Id::unique(("pixoko-dialog", campo))),
                    "ppp {ppp} {ventana:?}: fotograma {fotograma}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Con el diálogo abierto por un clic: redimensionar, cambiar de escala y recorrer con Tab
// ---------------------------------------------------------------------------------------------

fn abre_formulario_con_un_clic(app: &mut Aplicacion) {
    app.reposo();
    let boton = app.centro_de("Abrir formulario");
    app.clic(boton);
    app.reposo();
}

fn formulario_de_prueba() -> FormSpec {
    all_kinds()
}

/// Un ciclo entero de `Tab` más una vuelta extra desde el foco actual: cada paso es el siguiente de
/// `focus_order`.
fn tab_recorre_el_orden(app: &mut Aplicacion, contexto: &str) {
    let orden = focus_order(app.dialogo().expect("abierto"));
    let mut actual = app.focus.expect("hay un foco para empezar");
    for paso in 0..=orden.len() {
        let posicion = orden
            .iter()
            .position(|t| *t == actual)
            .unwrap_or_else(|| panic!("{contexto}: {actual:?} no está en {orden:?}"));
        let esperado = orden[(posicion + 1) % orden.len()];
        app.pulsa(Key::Tab);
        assert_eq!(
            app.focus,
            Some(esperado),
            "{contexto}: paso {paso} de Tab desde {actual:?}; orden {orden:?}"
        );
        actual = esperado;
    }
}

#[test]
fn un_formulario_abierto_con_un_clic_tiene_el_foco_inicial_y_tab_recorre_el_orden_de_la_spec() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut app = Aplicacion::con_boton_de_formulario(ppp, ventana, formulario_de_prueba);
            abre_formulario_con_un_clic(&mut app);
            let dialog = app.dialogo().expect("se abrió");
            let esperado = initial_focus(dialog);
            let contexto = format!("ppp {ppp} {ventana:?}");
            assert_eq!(app.focus, Some(esperado), "{contexto}: foco inicial");
            assert!(
                app.ctx.memory(|m| m.focused()).is_some(),
                "{contexto}: egui no tiene ningún widget con foco"
            );
            tab_recorre_el_orden(&mut app, &contexto);
        }
    }
}

#[test]
fn el_foco_inicial_se_mantiene_tras_redimensionar_y_cambiar_de_escala_con_el_menu() {
    for ppp in PPPS {
        let mut app = Aplicacion::nueva(ppp, Ventana::Minima);
        app.abre_licencias_con_el_raton();
        let zona = Some(FocusTarget::Zone(LICENSES_FIELD));
        assert_eq!(app.focus, zona, "ppp {ppp}: tras abrir");
        let id = app.ctx.memory(|m| m.focused());
        assert_eq!(id, Some(id_de_la_zona()), "ppp {ppp}: tras abrir");
        for otra in PPPS {
            for ventana in [Ventana::Grande, Ventana::Minima] {
                app.cambia(otra, ventana);
                app.reposo();
                assert_eq!(
                    app.focus, zona,
                    "ppp {ppp} -> {otra} {ventana:?}: el foco que lleva show_dialog"
                );
                assert_eq!(
                    app.ctx.memory(|m| m.focused()),
                    id,
                    "ppp {ppp} -> {otra} {ventana:?}: el foco de egui"
                );
            }
        }
        // El orden de Tab sigue siendo el de la spec tras tanto cambio.
        tab_recorre_el_orden(&mut app, &format!("ppp {ppp}, tras los cambios"));
        // Y el diálogo sigue abierto y sin cerrar.
        assert!(app.modal.is_open() && app.cierres == 0);
    }
}

#[test]
fn el_foco_inicial_de_un_formulario_se_mantiene_tras_redimensionar_y_cambiar_de_escala() {
    for ppp in PPPS {
        let mut app =
            Aplicacion::con_boton_de_formulario(ppp, Ventana::Minima, formulario_de_prueba);
        abre_formulario_con_un_clic(&mut app);
        let inicial = Some(initial_focus(app.dialogo().expect("abierto")));
        assert_eq!(app.focus, inicial, "ppp {ppp}: tras abrir");
        let id = app.ctx.memory(|m| m.focused());
        assert!(id.is_some(), "ppp {ppp}: egui sin foco tras abrir");
        for otra in PPPS {
            for ventana in [Ventana::Grande, Ventana::Minima] {
                app.cambia(otra, ventana);
                app.reposo();
                assert_eq!(app.focus, inicial, "ppp {ppp} -> {otra} {ventana:?}");
                assert_eq!(
                    app.ctx.memory(|m| m.focused()),
                    id,
                    "ppp {ppp} -> {otra} {ventana:?}: el foco de egui"
                );
            }
        }
        tab_recorre_el_orden(&mut app, &format!("ppp {ppp}, tras los cambios"));
    }
}
