//! Tests del flujo de ayuda y atajos de la app (specs/interfaz-ayuda.md, unidad I.5): sustitución
//! con vuelta, `Esc` en cadena, lista de atajos, `Restablecer todo`, marca de conflicto y cierre de
//! la ventana con una captura armada. Filas 8 y 12 y condiciones 1 y 2 del visto bueno. Sin
//! ventana: se alimentan las `Reaction` de los diálogos como lo hace `show_modal`. En ROJO hasta la
//! lógica: `shortcuts_flow` no existe todavía.

use egui::{Event, Key, Modifiers};
use px_ui::actions::{AppAction, Locks, Platform};
use px_ui::dialog::{Dialog, DialogKind, Event as Evento, Modal, Reaction};
use px_ui::help::{HELP_PRIMARY, HELP_SECONDARY, HELP_ZONE, HelpTarget, SHORTCUTS_RESET};
use px_ui::keymap::{Chord, KeyId, Keymap};
use px_ui::shortcut_editor::{EDITOR_CONFLICT, EDITOR_PRIMARY, EDITOR_RESET, slots_of};
use px_ui::tool::Tool;

use super::{Flow, Outcome, Return, Strip};
use crate::confirm::Confirm;

const LINUX: Platform = Platform::Linux;

fn lapiz() -> AppAction {
    AppAction::SelectTool(Tool::Pencil)
}

fn relleno() -> AppAction {
    AppAction::SelectTool(Tool::Fill)
}

fn tecla(key: Key, texto: Option<&str>) -> Vec<Event> {
    let mut v = vec![Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }];
    if let Some(t) = texto {
        v.push(Event::Text(t.to_string()));
    }
    v
}

fn chord(c: char) -> Chord {
    Chord {
        key: KeyId::Char(c),
        command: false,
        shift: false,
    }
}

struct Mundo {
    modal: Modal,
    flow: Flow,
    keymap: Keymap,
}

impl Mundo {
    fn nuevo() -> Mundo {
        Mundo {
            modal: Modal::Closed,
            flow: Flow::new(Vec::new()),
            keymap: Keymap::default_scheme(),
        }
    }

    fn abre_ayuda(&mut self, target: HelpTarget) {
        assert!(self.flow.open_help(
            &mut self.modal,
            target,
            Tool::Pencil,
            &self.keymap,
            LINUX,
            Locks::default()
        ));
    }

    fn abre_lista(&mut self) {
        assert!(self
            .flow
            .open_list(&mut self.modal, &self.keymap, LINUX, Locks::default()));
    }

    fn dialogo(&self) -> &Dialog {
        match &self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("no hay diálogo abierto"),
        }
    }

    fn clase(&self) -> Option<DialogKind> {
        self.modal.kind()
    }

    /// Un evento al diálogo (como el pintado) y su reacción al flujo.
    fn evento(&mut self, evento: Evento) -> Outcome {
        let reaccion = match &mut self.modal {
            Modal::Open(d) => d.event(evento),
            Modal::Closed => Reaction::None,
        };
        self.reacciona(reaccion)
    }

    fn reacciona(&mut self, reaccion: Reaction) -> Outcome {
        self.flow
            .react(&mut self.modal, reaccion, &mut self.keymap, LINUX, Tool::Pencil)
    }

    fn pulsa(&mut self, field: &'static str, aside: bool) -> Outcome {
        self.evento(Evento::Press { field, aside })
    }

    /// `Aceptar` o `Cancelar` del diálogo abierto, como lo da `show_dialog`.
    fn acepta(&mut self) -> Outcome {
        self.evento(Evento::Accept)
    }

    fn cancela(&mut self) -> Outcome {
        self.evento(Evento::Cancel)
    }

    fn teclas(&mut self, eventos: &[Event]) -> Strip {
        self.flow.keys(eventos, false)
    }

    fn sincroniza(&mut self) {
        self.flow.sync(&mut self.modal, LINUX);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: sustitución con vuelta y tabla en vivo
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_8_ayuda_editor_aceptar_vuelve_a_la_ayuda_con_la_tabla_viva_y_su_linea() {
    let mut m = Mundo::nuevo();
    m.abre_ayuda(HelpTarget::Tool(Tool::Pencil));
    assert_eq!(m.clase(), Some(DialogKind::Help));
    if let Modal::Open(d) = &mut m.modal {
        d.set_first_line(HELP_ZONE, 2);
    }
    // `Principal` abre el editor: la ayuda se cierra (un solo modal) y la casilla queda armada.
    assert_eq!(m.pulsa(HELP_PRIMARY, false), Outcome::None);
    assert_eq!(m.clase(), Some(DialogKind::ShortcutEditor));
    assert_eq!(m.flow.returns_to(), Some(Return::Help(HelpTarget::Tool(Tool::Pencil), 2)));
    assert!(m.flow.editor().unwrap().capturing().is_some());
    // Capturar `x` (con el pase de eventos del fotograma).
    assert_eq!(m.teclas(&tecla(Key::X, Some("x"))), Strip::All);
    assert_eq!(slots_of(m.flow.editor().unwrap().keymap(), lapiz()).slots, vec![chord('x')]);
    assert_eq!(m.keymap, Keymap::default_scheme(), "la tabla viva no cambia hasta aceptar");
    // `Aceptar`: la tabla cambia y vuelve la ayuda, con `x` y en su línea.
    assert_eq!(m.acepta(), Outcome::KeymapChanged(lapiz()));
    assert_eq!(slots_of(&m.keymap, lapiz()).slots, vec![chord('x')]);
    assert_eq!(m.clase(), Some(DialogKind::Help));
    assert_eq!(m.dialogo().first_line(HELP_ZONE), 2);
    assert_eq!(m.dialogo().button_text(HELP_PRIMARY).as_deref(), Some("x"));
    assert!(m.flow.editor().is_none() && m.flow.returns_to().is_none());
    // `Cerrar` la cierra del todo.
    assert_eq!(m.acepta(), Outcome::None);
    assert_eq!(m.clase(), None);
}

#[test]
fn fila_8_cancelar_y_escape_vuelven_a_la_ayuda_sin_tocar_la_tabla() {
    let mut m = Mundo::nuevo();
    m.abre_ayuda(HelpTarget::ZoomIn);
    m.pulsa(HELP_SECONDARY, false);
    assert_eq!(m.clase(), Some(DialogKind::ShortcutEditor));
    m.teclas(&tecla(Key::X, Some("x")));
    assert_eq!(m.cancela(), Outcome::None);
    assert_eq!(m.clase(), Some(DialogKind::Help));
    assert_eq!(m.keymap, Keymap::default_scheme());
    assert_eq!(m.dialogo().spec().title, "Acercar");
    // Un `Cancelar` tras reasignar tampoco deja huella.
    m.pulsa(HELP_PRIMARY, false);
    m.teclas(&tecla(Key::V, Some("v")));
    assert!(m.flow.editor().unwrap().pending().is_some());
    m.sincroniza();
    m.pulsa(EDITOR_CONFLICT, false);
    m.cancela();
    assert_eq!(m.keymap, Keymap::default_scheme());
    // `Cerrar` de la ayuda: sin vuelta.
    assert_eq!(m.cancela(), Outcome::None);
    assert_eq!(m.clase(), None);
    assert!(m.flow.returns_to().is_none());
}

#[test]
fn fila_8_escape_en_cadena_captura_conflicto_dialogo() {
    let mut m = Mundo::nuevo();
    m.abre_ayuda(HelpTarget::Tool(Tool::Pencil));
    m.pulsa(HELP_PRIMARY, false);
    // 1. Con la captura armada, Escape la desarma y se retiran todos los eventos.
    assert_eq!(m.teclas(&tecla(Key::Escape, None)), Strip::All);
    assert!(m.flow.editor().unwrap().capturing().is_none());
    // 2. Sin captura ni conflicto, el Escape es del diálogo.
    assert_eq!(m.teclas(&tecla(Key::Escape, None)), Strip::None);
    // 3. Con un conflicto pendiente, Escape lo descarta y se retira solo el Escape.
    m.pulsa(EDITOR_PRIMARY, false);
    assert_eq!(m.teclas(&tecla(Key::V, Some("v"))), Strip::All);
    assert!(m.flow.editor().unwrap().pending().is_some());
    assert_eq!(m.teclas(&tecla(Key::Escape, None)), Strip::Escape);
    assert!(m.flow.editor().unwrap().pending().is_none());
    // 4. Y el siguiente Escape es del diálogo.
    assert_eq!(m.teclas(&tecla(Key::Escape, None)), Strip::None);
}

#[test]
fn fila_8_un_clic_desarma_la_captura_y_las_teclas_no_llegan_a_las_acciones() {
    let mut m = Mundo::nuevo();
    m.abre_ayuda(HelpTarget::Tool(Tool::Pencil));
    m.pulsa(HELP_PRIMARY, false);
    assert!(m.flow.editor().unwrap().capturing().is_some());
    assert_eq!(m.flow.keys(&[], true), Strip::None, "un clic desarma sin quitar nada");
    assert!(m.flow.editor().unwrap().capturing().is_none());
    // Sin captura, las teclas no se retiran.
    assert_eq!(m.teclas(&tecla(Key::D, Some("d"))), Strip::None);
    // Armada otra vez: `Ctrl+Q` ni siquiera llega a las acciones (se retira) y `F1` tampoco.
    m.sincroniza();
    m.pulsa(EDITOR_PRIMARY, false);
    let mut ctrl_q = tecla(Key::Q, None);
    if let Event::Key { modifiers, .. } = &mut ctrl_q[0] {
        *modifiers = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
    }
    assert_eq!(m.teclas(&ctrl_q), Strip::All);
    assert!(m.flow.editor().unwrap().capturing().is_some(), "rechazada: sigue armada");
    assert_eq!(m.teclas(&tecla(Key::F1, None)), Strip::All);
    // Las repeticiones también se retiran y no capturan.
    let mut repetida = tecla(Key::X, Some("x"));
    if let Event::Key { repeat, .. } = &mut repetida[0] {
        *repeat = true;
    }
    assert_eq!(m.teclas(&repetida), Strip::All);
    assert!(slots_of(m.flow.editor().unwrap().keymap(), lapiz()).slots == vec![chord('d')]);
}

#[test]
fn fila_8_el_editor_responde_a_sus_botones() {
    let mut m = Mundo::nuevo();
    m.abre_ayuda(HelpTarget::ZoomIn);
    m.pulsa(HELP_PRIMARY, false);
    m.teclas(&tecla(Key::Escape, None));
    // `Quitar` de la principal: la secundaria sube.
    m.pulsa(EDITOR_PRIMARY, true);
    assert_eq!(
        slots_of(m.flow.editor().unwrap().keymap(), AppAction::ZoomIn).slots,
        vec![chord('+'), chord('=')]
    );
    // `Restablecer` devuelve la acción a lo de fábrica.
    m.pulsa(EDITOR_RESET, false);
    assert_eq!(
        slots_of(m.flow.editor().unwrap().keymap(), AppAction::ZoomIn).slots,
        vec![chord('>'), chord('+'), chord('=')]
    );
    // Pulsar una casilla la arma; `Reasignar` (botón principal) con un conflicto aplica.
    m.pulsa(EDITOR_PRIMARY, false);
    assert!(m.flow.editor().unwrap().capturing().is_some());
    m.teclas(&tecla(Key::V, Some("v")));
    m.sincroniza();
    m.pulsa(EDITOR_CONFLICT, false);
    assert!(m.flow.editor().unwrap().pending().is_none());
    assert_eq!(m.acepta(), Outcome::KeymapChanged(AppAction::ZoomIn));
    assert_eq!(slots_of(&m.keymap, AppAction::ZoomIn).slots[0], chord('v'));
    assert!(slots_of(&m.keymap, AppAction::SelectTool(Tool::Line)).slots.is_empty());
    assert!(m.keymap.conflicts().is_empty());
}

#[test]
fn fila_8_cualquier_otro_cierre_borra_la_vuelta_y_el_editor() {
    let mut m = Mundo::nuevo();
    m.abre_ayuda(HelpTarget::Tool(Tool::Pencil));
    m.pulsa(HELP_PRIMARY, false);
    assert!(m.flow.returns_to().is_some());
    m.flow.abandon(&mut m.modal);
    assert_eq!(m.clase(), None);
    assert!(m.flow.returns_to().is_none() && m.flow.editor().is_none());
    // Y con el diálogo ya cerrado por otro camino, lo que llegue no abre nada.
    assert_eq!(m.acepta(), Outcome::None);
    assert_eq!(m.clase(), None);
}

#[test]
fn fila_8_no_se_abre_con_bloqueos_ni_con_otro_dialogo() {
    let mut m = Mundo::nuevo();
    for locks in [
        Locks {
            gesture: true,
            ..Locks::default()
        },
        Locks {
            text_focus: true,
            ..Locks::default()
        },
        Locks {
            dialog: true,
            ..Locks::default()
        },
        Locks {
            removal: true,
            ..Locks::default()
        },
        Locks {
            modal: true,
            ..Locks::default()
        },
    ] {
        assert!(!m.flow.open_help(
            &mut m.modal,
            HelpTarget::Active,
            Tool::Pencil,
            &m.keymap,
            LINUX,
            locks
        ));
        assert!(!m.flow.open_list(&mut m.modal, &m.keymap, LINUX, locks));
        assert_eq!(m.clase(), None);
    }
    m.abre_ayuda(HelpTarget::Active);
    assert!(!m.flow.open_list(&mut m.modal, &m.keymap, LINUX, Locks::default()));
}

// ---------------------------------------------------------------------------------------------
// Condición 2: cerrar con la `x` con una captura armada
// ---------------------------------------------------------------------------------------------

#[test]
fn condicion_2_cerrar_con_la_x_pasa_por_la_regla_de_repeticion_aunque_haya_captura_armada() {
    let mut m = Mundo::nuevo();
    m.abre_ayuda(HelpTarget::Tool(Tool::Pencil));
    m.pulsa(HELP_PRIMARY, false);
    assert!(m.flow.editor().unwrap().capturing().is_some());
    let mut confirm = Confirm::new();
    // Con cambios sin guardar: la primera petición se cancela (y el editor se abandona sin
    // aplicar nada); la segunda, dentro de 5 s, cierra.
    assert!(!m.flow.close_requested(&mut m.modal, true, &mut confirm, 10.0));
    assert_eq!(m.clase(), None);
    assert!(m.flow.editor().is_none() && m.flow.returns_to().is_none());
    assert_eq!(m.keymap, Keymap::default_scheme());
    assert!(m.flow.close_requested(&mut m.modal, true, &mut confirm, 12.0));
    // Pasados 5 s, vuelve a pedir confirmación.
    let mut otra = Confirm::new();
    assert!(!m.flow.close_requested(&mut m.modal, true, &mut otra, 20.0));
    assert!(!m.flow.close_requested(&mut m.modal, true, &mut otra, 26.0));
    // Sin cambios, cierra a la primera con o sin captura.
    m.abre_ayuda(HelpTarget::Tool(Tool::Pencil));
    m.pulsa(HELP_PRIMARY, false);
    let mut limpia = Confirm::new();
    assert!(m.flow.close_requested(&mut m.modal, false, &mut limpia, 1.0));
}

// ---------------------------------------------------------------------------------------------
// Fila 12 y condición 1: lista, restablecer todo y marca de conflicto
// ---------------------------------------------------------------------------------------------

fn texto_del_boton(d: &Dialog, id: &str) -> String {
    d.button_text(id).unwrap_or_else(|| panic!("no hay botón {id}"))
}

#[test]
fn fila_12_la_lista_abre_el_editor_de_las_editables_y_vuelve_a_su_posicion() {
    let mut m = Mundo::nuevo();
    m.abre_lista();
    assert_eq!(m.clase(), Some(DialogKind::ShortcutList));
    if let Modal::Open(d) = &mut m.modal {
        d.set_scroll_u(40);
    }
    // Las de archivo no abren nada (están atenuadas).
    assert_eq!(m.pulsa("file.save", false), Outcome::None);
    assert_eq!(m.clase(), Some(DialogKind::ShortcutList));
    // Una editable abre el editor con la principal armada.
    assert_eq!(m.pulsa("tool.fill", false), Outcome::None);
    assert_eq!(m.clase(), Some(DialogKind::ShortcutEditor));
    assert_eq!(m.flow.returns_to(), Some(Return::List(40)));
    assert!(m.flow.editor().unwrap().capturing().is_some());
    m.teclas(&tecla(Key::Y, Some("y")));
    assert_eq!(m.acepta(), Outcome::KeymapChanged(relleno()));
    assert_eq!(m.clase(), Some(DialogKind::ShortcutList));
    assert_eq!(m.dialogo().scroll_u(), 40);
    assert!(texto_del_boton(m.dialogo(), "tool.fill").contains('y'));
    // La línea es `{nombre:<20}{atajos}` de 44 columnas como mucho.
    assert!(texto_del_boton(m.dialogo(), "tool.pencil").starts_with(&format!("{:<20}d", "Lápiz")));
    for id in ["tool.pencil", "edit.undo", "view.zoom-in", "file.quit", "color.bg-next"] {
        assert!(texto_del_boton(m.dialogo(), id).chars().count() <= 44, "{id}");
    }
}

#[test]
fn fila_12_restablecer_todo_pide_confirmacion_cancelar_no_hace_nada_y_aceptar_restablece() {
    let mut m = Mundo::nuevo();
    m.keymap = {
        // Una tabla con un atajo cambiado.
        let mut f = Mundo::nuevo();
        f.abre_ayuda(HelpTarget::Tool(Tool::Pencil));
        f.pulsa(HELP_PRIMARY, false);
        f.teclas(&tecla(Key::X, Some("x")));
        f.acepta();
        f.keymap
    };
    assert_ne!(m.keymap, Keymap::default_scheme());
    m.abre_lista();
    assert_eq!(m.pulsa(SHORTCUTS_RESET, false), Outcome::None);
    assert_eq!(m.clase(), Some(DialogKind::ResetShortcuts));
    // Cancelar: vuelve a la lista y no cambia nada.
    let antes = m.keymap.clone();
    assert_eq!(m.cancela(), Outcome::None);
    assert_eq!(m.clase(), Some(DialogKind::ShortcutList));
    assert_eq!(m.keymap, antes);
    // Aceptar: el esquema por defecto en vivo, y de vuelta a la lista.
    m.pulsa(SHORTCUTS_RESET, false);
    assert_eq!(m.acepta(), Outcome::ResetAll);
    assert_eq!(m.keymap, Keymap::default_scheme());
    assert_eq!(m.clase(), Some(DialogKind::ShortcutList));
    assert!(texto_del_boton(m.dialogo(), "tool.pencil").contains('d'));
}

#[test]
fn condicion_1_la_marca_conflicto_sale_en_la_lista_hasta_que_se_edita_la_accion() {
    let mut m = Mundo::nuevo();
    m.flow = Flow::new(vec![relleno()]);
    m.abre_lista();
    assert!(texto_del_boton(m.dialogo(), "tool.fill").contains("conflicto"));
    assert!(!texto_del_boton(m.dialogo(), "tool.line").contains("conflicto"));
    assert!(texto_del_boton(m.dialogo(), "tool.fill").chars().count() <= 44);
    // Editar la acción quita la marca.
    m.pulsa("tool.fill", false);
    m.teclas(&tecla(Key::Y, Some("y")));
    m.acepta();
    assert!(m.flow.conflicted().is_empty());
    assert!(!texto_del_boton(m.dialogo(), "tool.fill").contains("conflicto"));
    // `Restablecer todo` también las quita.
    m.flow = Flow::new(vec![relleno()]);
    m.abre_lista_cerrando();
    m.pulsa(SHORTCUTS_RESET, false);
    m.acepta();
    assert!(m.flow.conflicted().is_empty());
}

impl Mundo {
    fn abre_lista_cerrando(&mut self) {
        self.modal = Modal::Closed;
        self.abre_lista();
    }
}
