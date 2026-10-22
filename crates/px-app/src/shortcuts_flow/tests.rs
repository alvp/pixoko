//! Tests del flujo de ayuda y atajos de la app (specs/interfaz-ayuda.md, unidad I.5): sustitución
//! con vuelta, `Esc` en cadena, lista de atajos, `Restablecer all`, marca de conflicto y cierre de
//! la ventana con una captura armada. Filas 8 y 12 y condiciones 1 y 2 del visto bueno. Sin
//! ventana: se alimentan las `Reaction` de los diálogos como lo hace `show_modal`. En ROJO hasta la
//! lógica: `shortcuts_flow` no existe todavía.

use eframe::egui::{Event, Key, Modifiers};
use px_ui::actions::{AppAction, Locks, Platform};
use px_ui::dialog::{Dialog, DialogKind, Event as TestEvent, Modal, Reaction};
use px_ui::help::{HELP_PRIMARY, HELP_SECONDARY, HELP_ZONE, HelpTarget, SHORTCUTS_RESET};
use px_ui::keymap::{Chord, KeyId, Keymap};
use px_ui::shortcut_editor::{EDITOR_CONFLICT, EDITOR_PRIMARY, EDITOR_RESET, slots_of};
use px_ui::tool::Tool;

use super::{Flow, FlowOutcome, Return, Strip};
use crate::confirm::Confirm;

const LINUX: Platform = Platform::Linux;

fn pencil() -> AppAction {
    AppAction::SelectTool(Tool::Pencil)
}

fn fill() -> AppAction {
    AppAction::SelectTool(Tool::Fill)
}

fn key_2(key: Key, text_2: Option<&str>) -> Vec<Event> {
    let mut v = vec![Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }];
    if let Some(t) = text_2 {
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

struct World {
    modal: Modal,
    flow: Flow,
    keymap: Keymap,
}

impl World {
    fn new_2() -> World {
        World {
            modal: Modal::Closed,
            flow: Flow::new(Vec::new()),
            keymap: Keymap::default_scheme(),
        }
    }

    fn opens_help(&mut self, target: HelpTarget) {
        assert!(self.flow.open_help(
            &mut self.modal,
            target,
            Tool::Pencil,
            &self.keymap,
            LINUX,
            Locks::default()
        ));
    }

    fn opens_list(&mut self) {
        assert!(
            self.flow
                .open_list(&mut self.modal, &self.keymap, LINUX, Locks::default())
        );
    }

    fn dialog(&self) -> &Dialog {
        match &self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("no hay diálogo abierto"),
        }
    }

    fn class(&self) -> Option<DialogKind> {
        self.modal.kind()
    }

    /// Un evento al diálogo (como el pintado) y su reacción al flujo.
    fn event_2(&mut self, event_2: TestEvent) -> FlowOutcome {
        let reaction = match &mut self.modal {
            Modal::Open(d) => d.event(event_2),
            Modal::Closed => Reaction::None,
        };
        self.reacts(reaction)
    }

    fn reacts(&mut self, reaction: Reaction) -> FlowOutcome {
        self.flow.react(
            &mut self.modal,
            reaction,
            &mut self.keymap,
            LINUX,
            Tool::Pencil,
        )
    }

    fn presses(&mut self, field: &'static str, aside: bool) -> FlowOutcome {
        self.event_2(TestEvent::Press { field, aside })
    }

    /// `Accept` o `Cancel` del diálogo abierto, como lo da `show_dialog`.
    fn accepts(&mut self) -> FlowOutcome {
        self.event_2(TestEvent::Accept)
    }

    fn cancels(&mut self) -> FlowOutcome {
        self.event_2(TestEvent::Cancel)
    }

    fn keys_2(&mut self, events: &[Event]) -> Strip {
        self.flow.keys(events, false)
    }

    fn synchronizes(&mut self) {
        self.flow.sync(&mut self.modal, LINUX);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: sustitución con vuelta y tabla en vivo
// ---------------------------------------------------------------------------------------------

#[test]
fn row_8_help_editor_accept_returns_to_the_help_with_the_live_table_and_its_line() {
    let mut m = World::new_2();
    m.opens_help(HelpTarget::Tool(Tool::Pencil));
    assert_eq!(m.class(), Some(DialogKind::Help));
    if let Modal::Open(d) = &mut m.modal {
        d.set_first_line(HELP_ZONE, 2);
    }
    // `Principal` abre el editor: la ayuda se cierra (un solo modal) y la casilla queda armada.
    assert_eq!(m.presses(HELP_PRIMARY, false), FlowOutcome::None);
    assert_eq!(m.class(), Some(DialogKind::ShortcutEditor));
    assert_eq!(
        m.flow.returns_to(),
        Some(Return::Help(HelpTarget::Tool(Tool::Pencil), 2))
    );
    assert!(m.flow.editor().unwrap().capturing().is_some());
    // Capturar `x` (con el pase de eventos del fotograma).
    assert_eq!(m.keys_2(&key_2(Key::X, Some("x"))), Strip::All);
    assert_eq!(
        slots_of(m.flow.editor().unwrap().keymap(), pencil()).slots,
        vec![chord('x')]
    );
    assert_eq!(
        m.keymap,
        Keymap::default_scheme(),
        "la tabla viva no cambia hasta aceptar"
    );
    // `Accept`: la tabla cambia y vuelve la ayuda, con `x` y en su línea.
    assert_eq!(m.accepts(), FlowOutcome::KeymapChanged(pencil()));
    assert_eq!(slots_of(&m.keymap, pencil()).slots, vec![chord('x')]);
    assert_eq!(m.class(), Some(DialogKind::Help));
    assert_eq!(m.dialog().first_line(HELP_ZONE), 2);
    assert_eq!(m.dialog().button_text(HELP_PRIMARY).as_deref(), Some("x"));
    assert!(m.flow.editor().is_none() && m.flow.returns_to().is_none());
    // `Cerrar` la cierra del todo.
    assert_eq!(m.accepts(), FlowOutcome::None);
    assert_eq!(m.class(), None);
}

#[test]
fn row_8_cancel_and_escape_return_to_the_help_without_touching_the_table() {
    let mut m = World::new_2();
    m.opens_help(HelpTarget::ZoomIn);
    m.presses(HELP_SECONDARY, false);
    assert_eq!(m.class(), Some(DialogKind::ShortcutEditor));
    m.keys_2(&key_2(Key::X, Some("x")));
    assert_eq!(m.cancels(), FlowOutcome::None);
    assert_eq!(m.class(), Some(DialogKind::Help));
    assert_eq!(m.keymap, Keymap::default_scheme());
    assert_eq!(m.dialog().spec().title, "Acercar");
    // Un `Cancel` tras reasignar tampoco deja huella.
    m.presses(HELP_PRIMARY, false);
    m.keys_2(&key_2(Key::V, Some("v")));
    assert!(m.flow.editor().unwrap().pending().is_some());
    m.synchronizes();
    m.presses(EDITOR_CONFLICT, false);
    m.cancels();
    assert_eq!(m.keymap, Keymap::default_scheme());
    // `Cerrar` de la ayuda: sin vuelta.
    assert_eq!(m.cancels(), FlowOutcome::None);
    assert_eq!(m.class(), None);
    assert!(m.flow.returns_to().is_none());
}

#[test]
fn row_8_escape_in_chain_capture_conflict_dialog() {
    let mut m = World::new_2();
    m.opens_help(HelpTarget::Tool(Tool::Pencil));
    m.presses(HELP_PRIMARY, false);
    // 1. Con la captura armada, Escape la desarma y se retiran todos los eventos.
    assert_eq!(m.keys_2(&key_2(Key::Escape, None)), Strip::All);
    assert!(m.flow.editor().unwrap().capturing().is_none());
    // 2. Sin captura ni conflicto, el Escape es del diálogo.
    assert_eq!(m.keys_2(&key_2(Key::Escape, None)), Strip::None);
    // 3. Con un conflicto pendiente, Escape lo descarta y se retira solo el Escape.
    m.presses(EDITOR_PRIMARY, false);
    assert_eq!(m.keys_2(&key_2(Key::V, Some("v"))), Strip::All);
    assert!(m.flow.editor().unwrap().pending().is_some());
    assert_eq!(m.keys_2(&key_2(Key::Escape, None)), Strip::Escape);
    assert!(m.flow.editor().unwrap().pending().is_none());
    // 4. Y el siguiente Escape es del diálogo.
    assert_eq!(m.keys_2(&key_2(Key::Escape, None)), Strip::None);
}

#[test]
fn row_8_a_click_disarms_the_capture_and_the_keys_do_not_reach_the_actions() {
    let mut m = World::new_2();
    m.opens_help(HelpTarget::Tool(Tool::Pencil));
    m.presses(HELP_PRIMARY, false);
    assert!(m.flow.editor().unwrap().capturing().is_some());
    assert_eq!(
        m.flow.keys(&[], true),
        Strip::None,
        "un clic desarma sin quitar nada"
    );
    assert!(m.flow.editor().unwrap().capturing().is_none());
    // Sin captura, las teclas no se retiran.
    assert_eq!(m.keys_2(&key_2(Key::D, Some("d"))), Strip::None);
    // Armada otra vez: `Ctrl+Q` ni siquiera llega a las acciones (se retira) y `F1` tampoco.
    m.synchronizes();
    m.presses(EDITOR_PRIMARY, false);
    let mut ctrl_q = key_2(Key::Q, None);
    if let Event::Key { modifiers, .. } = &mut ctrl_q[0] {
        *modifiers = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
    }
    assert_eq!(m.keys_2(&ctrl_q), Strip::All);
    assert!(
        m.flow.editor().unwrap().capturing().is_some(),
        "rechazada: sigue armada"
    );
    assert_eq!(m.keys_2(&key_2(Key::F1, None)), Strip::All);
    // Las repeticiones también se retiran y no capturan.
    let mut repeated = key_2(Key::X, Some("x"));
    if let Event::Key { repeat, .. } = &mut repeated[0] {
        *repeat = true;
    }
    assert_eq!(m.keys_2(&repeated), Strip::All);
    assert!(slots_of(m.flow.editor().unwrap().keymap(), pencil()).slots == vec![chord('d')]);
}

#[test]
fn row_8_the_editor_responds_to_its_buttons() {
    let mut m = World::new_2();
    m.opens_help(HelpTarget::ZoomIn);
    m.presses(HELP_PRIMARY, false);
    m.keys_2(&key_2(Key::Escape, None));
    // `Remove` de la principal: la secundaria sube.
    m.presses(EDITOR_PRIMARY, true);
    assert_eq!(
        slots_of(m.flow.editor().unwrap().keymap(), AppAction::ZoomIn).slots,
        vec![chord('+'), chord('=')]
    );
    // `Restablecer` devuelve la acción a lo de fábrica.
    m.presses(EDITOR_RESET, false);
    assert_eq!(
        slots_of(m.flow.editor().unwrap().keymap(), AppAction::ZoomIn).slots,
        vec![chord('>'), chord('+'), chord('=')]
    );
    // Pulsar una casilla la arma; `Reasignar` (botón principal) con un conflicto aplica.
    m.presses(EDITOR_PRIMARY, false);
    assert!(m.flow.editor().unwrap().capturing().is_some());
    m.keys_2(&key_2(Key::V, Some("v")));
    m.synchronizes();
    m.presses(EDITOR_CONFLICT, false);
    assert!(m.flow.editor().unwrap().pending().is_none());
    assert_eq!(m.accepts(), FlowOutcome::KeymapChanged(AppAction::ZoomIn));
    assert_eq!(slots_of(&m.keymap, AppAction::ZoomIn).slots[0], chord('v'));
    assert!(
        slots_of(&m.keymap, AppAction::SelectTool(Tool::Line))
            .slots
            .is_empty()
    );
    assert!(m.keymap.conflicts().is_empty());
}

#[test]
fn row_8_any_other_close_clears_the_return_and_the_editor() {
    let mut m = World::new_2();
    m.opens_help(HelpTarget::Tool(Tool::Pencil));
    m.presses(HELP_PRIMARY, false);
    assert!(m.flow.returns_to().is_some());
    m.flow.abandon(&mut m.modal);
    assert_eq!(m.class(), None);
    assert!(m.flow.returns_to().is_none() && m.flow.editor().is_none());
    // Y con el diálogo ya cerrado por otro camino, lo que llegue no abre nada.
    assert_eq!(m.accepts(), FlowOutcome::None);
    assert_eq!(m.class(), None);
}

#[test]
fn row_8_it_is_not_opened_with_locks_or_with_another_dialog() {
    let mut m = World::new_2();
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
        assert_eq!(m.class(), None);
    }
    m.opens_help(HelpTarget::Active);
    assert!(
        !m.flow
            .open_list(&mut m.modal, &m.keymap, LINUX, Locks::default())
    );
}

// ---------------------------------------------------------------------------------------------
// Condición 2: cerrar con la `x` con una captura armada
// ---------------------------------------------------------------------------------------------

#[test]
fn condition_2_closing_with_the_x_goes_through_the_repeat_rule_even_with_an_armed_capture() {
    let mut m = World::new_2();
    m.opens_help(HelpTarget::Tool(Tool::Pencil));
    m.presses(HELP_PRIMARY, false);
    assert!(m.flow.editor().unwrap().capturing().is_some());
    let mut confirm = Confirm::new();
    // Con cambios sin guardar: la primera petición se cancela (y el editor se abandona sin
    // aplicar nada); la segunda, dentro de 5 s, cierra.
    assert!(
        !m.flow
            .close_requested(&mut m.modal, true, &mut confirm, 10.0)
    );
    assert_eq!(m.class(), None);
    assert!(m.flow.editor().is_none() && m.flow.returns_to().is_none());
    assert_eq!(m.keymap, Keymap::default_scheme());
    assert!(
        m.flow
            .close_requested(&mut m.modal, true, &mut confirm, 12.0)
    );
    // Pasados 5 s, vuelve a pedir confirmación.
    let mut other = Confirm::new();
    assert!(!m.flow.close_requested(&mut m.modal, true, &mut other, 20.0));
    assert!(!m.flow.close_requested(&mut m.modal, true, &mut other, 26.0));
    // Sin cambios, cierra a la primera con o sin captura.
    m.opens_help(HelpTarget::Tool(Tool::Pencil));
    m.presses(HELP_PRIMARY, false);
    let mut cleans = Confirm::new();
    assert!(
        m.flow
            .close_requested(&mut m.modal, false, &mut cleans, 1.0)
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 12 y condición 1: lista, restablecer todo y marca de conflicto
// ---------------------------------------------------------------------------------------------

fn text_of_the_button(d: &Dialog, id: &'static str) -> String {
    d.button_text(id)
        .unwrap_or_else(|| panic!("no hay botón {id}"))
}

#[test]
fn row_12_the_list_opens_the_editor_of_the_editable_ones_and_returns_to_its_position() {
    let mut m = World::new_2();
    m.opens_list();
    assert_eq!(m.class(), Some(DialogKind::ShortcutList));
    if let Modal::Open(d) = &mut m.modal {
        d.set_scroll_u(40);
    }
    // Las de archivo no abren nada (están atenuadas).
    assert_eq!(m.presses("file.save", false), FlowOutcome::None);
    assert_eq!(m.class(), Some(DialogKind::ShortcutList));
    // Una editable abre el editor con la principal armada.
    assert_eq!(m.presses("tool.fill", false), FlowOutcome::None);
    assert_eq!(m.class(), Some(DialogKind::ShortcutEditor));
    assert_eq!(m.flow.returns_to(), Some(Return::List(40)));
    assert!(m.flow.editor().unwrap().capturing().is_some());
    m.keys_2(&key_2(Key::Y, Some("y")));
    assert_eq!(m.accepts(), FlowOutcome::KeymapChanged(fill()));
    assert_eq!(m.class(), Some(DialogKind::ShortcutList));
    assert_eq!(m.dialog().scroll_u(), 40);
    assert!(text_of_the_button(m.dialog(), "tool.fill").contains('y'));
    // La línea es `{name_2:<20}{atajos}` de 44 columnas como mucho.
    assert!(
        text_of_the_button(m.dialog(), "tool.pencil").starts_with(&format!("{:<20}d", "Lápiz"))
    );
    for id in [
        "tool.pencil",
        "edit.undo",
        "view.zoom-in",
        "file.quit",
        "color.bg-next",
    ] {
        assert!(
            text_of_the_button(m.dialog(), id).chars().count() <= 44,
            "{id}"
        );
    }
}

#[test]
fn row_12_reset_all_asks_for_confirmation_cancel_does_nothing_and_accept_resets() {
    let mut m = World::new_2();
    m.keymap = {
        // Una tabla con un atajo cambiado.
        let mut f = World::new_2();
        f.opens_help(HelpTarget::Tool(Tool::Pencil));
        f.presses(HELP_PRIMARY, false);
        f.keys_2(&key_2(Key::X, Some("x")));
        f.accepts();
        f.keymap
    };
    assert_ne!(m.keymap, Keymap::default_scheme());
    m.opens_list();
    assert_eq!(m.presses(SHORTCUTS_RESET, false), FlowOutcome::None);
    assert_eq!(m.class(), Some(DialogKind::ResetShortcuts));
    // Cancelar: vuelve a la lista y no cambia nada.
    let before = m.keymap.clone();
    assert_eq!(m.cancels(), FlowOutcome::None);
    assert_eq!(m.class(), Some(DialogKind::ShortcutList));
    assert_eq!(m.keymap, before);
    // Aceptar: el esquema por defecto en vivo, y de vuelta a la lista.
    m.presses(SHORTCUTS_RESET, false);
    assert_eq!(m.accepts(), FlowOutcome::ResetAll);
    assert_eq!(m.keymap, Keymap::default_scheme());
    assert_eq!(m.class(), Some(DialogKind::ShortcutList));
    assert!(text_of_the_button(m.dialog(), "tool.pencil").contains('d'));
}

#[test]
fn condition_1_the_conflict_mark_stays_in_the_list_until_the_action_is_edited() {
    let mut m = World::new_2();
    m.flow = Flow::new(vec![fill()]);
    m.opens_list();
    assert!(text_of_the_button(m.dialog(), "tool.fill").contains("conflicto"));
    assert!(!text_of_the_button(m.dialog(), "tool.line").contains("conflicto"));
    assert!(text_of_the_button(m.dialog(), "tool.fill").chars().count() <= 44);
    // Editar la acción quita la marca.
    m.presses("tool.fill", false);
    m.keys_2(&key_2(Key::Y, Some("y")));
    m.accepts();
    assert!(m.flow.conflicted().is_empty());
    assert!(!text_of_the_button(m.dialog(), "tool.fill").contains("conflicto"));
    // `Restablecer all` también las quita.
    m.flow = Flow::new(vec![fill()]);
    m.opens_list_closing();
    m.presses(SHORTCUTS_RESET, false);
    m.accepts();
    assert!(m.flow.conflicted().is_empty());
}

impl World {
    fn opens_list_closing(&mut self) {
        self.modal = Modal::Closed;
        self.opens_list();
    }
}
