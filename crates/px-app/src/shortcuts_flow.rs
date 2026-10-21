//! El flujo de las ventanas de ayuda y de atajos (specs/interfaz-ayuda.md, decisiones 5 a 8 y 10 a
//! 12): **sustitución con vuelta** (hay un solo modal: el editor sustituye a la ayuda o a la lista y,
//! al cerrarse, estas se reabren desde la tabla viva), la captura de teclas con `Esc` en cadena, la
//! marca de conflicto y `Restablecer todo`. Sin egui: se prueba alimentándolo con las `Reaction` de
//! los diálogos y con los eventos del fotograma.

use px_ui::actions::{AppAction, Locks, Platform};
use px_ui::dialog::{DialogKind, Modal, Reaction};
use px_ui::help::{
    HELP_PRIMARY, HELP_SECONDARY, HELP_ZONE, HelpTarget, SHORTCUTS_RESET, confirm_reset_form,
    help_form, prepare_shortcut_list, shortcuts_form,
};
use px_ui::help_texts::{action_of_key, is_editable};
use px_ui::key_events::captures;
use px_ui::keymap::Keymap;
use px_ui::shortcut_editor::{
    EDITOR_CONFLICT, EDITOR_PRIMARY, EDITOR_RESET, EDITOR_SECONDARY, ShortcutEditor, Slot,
    editor_form, sync_editor_dialog,
};
use px_ui::tool::Tool;

use crate::confirm::{Confirm, close_allowed};

/// A dónde se vuelve al cerrar el editor o la confirmación: la ayuda (con su destino y la primera
/// línea visible de su zona) o la lista (con su desplazamiento, en `u`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Return {
    Help(HelpTarget, usize),
    List(i64),
}

/// Qué ventana del flujo está abierta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
    Help(HelpTarget),
    List,
    Editor,
    Reset,
}

/// Lo que dejó una reacción para quien guarda la tabla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlowOutcome {
    None,
    /// El editor aceptó: la tabla viva cambió por esa acción (hay que guardarla).
    KeymapChanged(AppAction),
    /// `Restablecer todo` confirmado: la tabla es la de fábrica (hay que guardarla, con copia).
    ResetAll,
}

/// Qué eventos del fotograma hay que retirar antes de que los vean las acciones y egui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strip {
    None,
    /// Todos los `Key`, `Text` e `Ime`: la captura estaba armada.
    All,
    /// Solo los `Escape`: se descartó un conflicto.
    Escape,
}

#[derive(Debug, Default)]
pub struct Flow {
    shown: Option<Shown>,
    ret: Option<Return>,
    editor: Option<ShortcutEditor>,
    conflicted: Vec<AppAction>,
}

impl Flow {
    /// `conflicted`: las acciones que perdieron un atajo por un choque al leer el fichero.
    pub fn new(conflicted: Vec<AppAction>) -> Flow {
        Flow {
            conflicted,
            ..Flow::default()
        }
    }

    #[cfg(test)]
    pub fn conflicted(&self) -> &[AppAction] {
        &self.conflicted
    }

    #[cfg(test)]
    pub fn returns_to(&self) -> Option<Return> {
        self.ret
    }

    #[cfg(test)]
    pub fn editor(&self) -> Option<&ShortcutEditor> {
        self.editor.as_ref()
    }

    /// Si `kind` es una ventana de este flujo.
    pub fn handles(kind: DialogKind) -> bool {
        matches!(
            kind,
            DialogKind::Help
                | DialogKind::ShortcutList
                | DialogKind::ShortcutEditor
                | DialogKind::ResetShortcuts
        )
    }

    /// Abre la ayuda de `target` (`active` es la herramienta activa, para `Active`). `false` si hay
    /// un bloqueo o ya hay un diálogo.
    pub fn open_help(
        &mut self,
        modal: &mut Modal,
        target: HelpTarget,
        active: Tool,
        keymap: &Keymap,
        platform: Platform,
        locks: Locks,
    ) -> bool {
        let resolved = match target {
            HelpTarget::Active => HelpTarget::Tool(active),
            other => other,
        };
        let opened = modal.open(
            DialogKind::Help,
            help_form(resolved, active, keymap, platform),
            locks,
        );
        if opened {
            self.reset();
            self.shown = Some(Shown::Help(resolved));
        }
        opened
    }

    /// Abre la lista de atajos.
    pub fn open_list(
        &mut self,
        modal: &mut Modal,
        keymap: &Keymap,
        platform: Platform,
        locks: Locks,
    ) -> bool {
        let opened = modal.open(
            DialogKind::ShortcutList,
            shortcuts_form(keymap, platform, &self.conflicted),
            locks,
        );
        if opened {
            self.reset();
            self.shown = Some(Shown::List);
            if let Some(dialog) = modal.dialog_mut() {
                prepare_shortcut_list(dialog);
            }
        }
        opened
    }

    fn reset(&mut self) {
        self.shown = None;
        self.ret = None;
        self.editor = None;
    }

    /// Cierra lo que hubiera sin aplicar nada (la `x` de la ventana, `Quit`, un diálogo de archivo):
    /// se borra también la vuelta.
    pub fn abandon(&mut self, modal: &mut Modal) -> bool {
        self.reset();
        modal.abandon()
    }

    /// El cierre de la ventana: abandona lo abierto (también con una captura armada: la captura no
    /// intercepta el cierre) y aplica la regla de cierre por repetición. `true` si se cierra.
    pub fn close_requested(
        &mut self,
        modal: &mut Modal,
        modified: bool,
        confirm: &mut Confirm,
        now: f64,
    ) -> bool {
        self.abandon(modal);
        close_allowed(modified, confirm, now)
    }

    /// Los eventos de teclado del fotograma frente a la captura del editor. Con la captura armada,
    /// la primera pulsación se intenta asignar (`Escape` la desarma) y hay que retirar **todos** los
    /// eventos de teclado (que ni `Ctrl+Q` llegue a las acciones ni `Intro` vuelva a pulsar la
    /// casilla); un clic (`pointer_pressed`) la desarma antes de atenderse. Con un conflicto
    /// pendiente, `Escape` lo descarta y solo se retira ese `Escape`.
    pub fn keys(&mut self, events: &[eframe::egui::Event], pointer_pressed: bool) -> Strip {
        let Some(editor) = self.editor.as_mut() else {
            return Strip::None;
        };
        if editor.capturing().is_none() {
            let escape = events.iter().any(|event| {
                matches!(
                    event,
                    eframe::egui::Event::Key {
                        key: eframe::egui::Key::Escape,
                        pressed: true,
                        repeat: false,
                        ..
                    }
                )
            });
            if editor.pending().is_some() && escape {
                editor.resolve(false);
                return Strip::Escape;
            }
            return Strip::None;
        }
        if pointer_pressed {
            editor.cancel_capture();
            return Strip::None;
        }
        if let Some(first) = captures(events).into_iter().next() {
            editor.capture(first);
        }
        Strip::All
    }

    /// Pone en el formulario del editor lo que dice el modelo.
    pub fn sync(&self, modal: &mut Modal, platform: Platform) {
        if let (Some(editor), Some(dialog)) = (&self.editor, modal.dialog_mut())
            && dialog.kind() == DialogKind::ShortcutEditor
        {
            sync_editor_dialog(dialog, editor, platform);
        }
    }

    /// La reacción de una ventana del flujo. Aplica la tabla viva (`keymap`) al aceptar el editor o
    /// `Restablecer todo`, y sustituye una ventana por otra o vuelve a la de origen.
    pub fn react(
        &mut self,
        modal: &mut Modal,
        reaction: Reaction,
        keymap: &mut Keymap,
        platform: Platform,
        active: Tool,
    ) -> FlowOutcome {
        let Some(shown) = self.shown else {
            return FlowOutcome::None;
        };
        match (shown, reaction) {
            (
                Shown::Help(target),
                Reaction::Pressed {
                    field,
                    aside: false,
                },
            ) if field == HELP_PRIMARY || field == HELP_SECONDARY => {
                let line = modal.dialog().map_or(0, |d| d.first_line(HELP_ZONE));
                let slot = if field == HELP_PRIMARY {
                    Slot::Primary
                } else {
                    Slot::Secondary
                };
                self.ret = Some(Return::Help(target, line));
                self.start_editor(modal, target.action(active), slot, keymap, platform, active);
                FlowOutcome::None
            }
            (Shown::List, Reaction::Pressed { field, .. }) => {
                let scroll = modal.dialog().map_or(0, |d| d.scroll_u());
                if field == SHORTCUTS_RESET {
                    self.ret = Some(Return::List(scroll));
                    modal.abandon();
                    if modal.open(
                        DialogKind::ResetShortcuts,
                        confirm_reset_form(),
                        Locks::default(),
                    ) {
                        self.shown = Some(Shown::Reset);
                    } else {
                        self.reset();
                    }
                } else if let Some(action) = action_of_key(field).filter(|a| is_editable(*a)) {
                    self.ret = Some(Return::List(scroll));
                    self.start_editor(modal, action, Slot::Primary, keymap, platform, active);
                }
                FlowOutcome::None
            }
            (Shown::Editor, Reaction::Pressed { field, aside }) => {
                if let Some(editor) = self.editor.as_mut() {
                    match (field, aside) {
                        (EDITOR_PRIMARY, false) => editor.begin_capture(Slot::Primary),
                        (EDITOR_PRIMARY, true) => editor.clear(Slot::Primary),
                        (EDITOR_SECONDARY, false) => editor.begin_capture(Slot::Secondary),
                        (EDITOR_SECONDARY, true) => editor.clear(Slot::Secondary),
                        (EDITOR_CONFLICT, aside) => editor.resolve(!aside),
                        (EDITOR_RESET, _) => editor.reset(),
                        _ => {}
                    }
                }
                self.sync(modal, platform);
                FlowOutcome::None
            }
            (Shown::Editor, Reaction::Accepted(_)) => {
                let Some(editor) = self.editor.take() else {
                    return FlowOutcome::None;
                };
                if !editor.can_accept() {
                    self.editor = Some(editor);
                    return FlowOutcome::None;
                }
                let action = editor.action();
                *keymap = editor.accept();
                self.conflicted.retain(|a| *a != action);
                self.back(modal, keymap, platform, active);
                FlowOutcome::KeymapChanged(action)
            }
            (Shown::Editor, Reaction::Cancelled) => {
                self.editor = None;
                self.back(modal, keymap, platform, active);
                FlowOutcome::None
            }
            (Shown::Reset, Reaction::Accepted(_)) => {
                *keymap = Keymap::default_scheme();
                self.conflicted.clear();
                self.back(modal, keymap, platform, active);
                FlowOutcome::ResetAll
            }
            (Shown::Reset, Reaction::Cancelled) => {
                self.back(modal, keymap, platform, active);
                FlowOutcome::None
            }
            (Shown::Help(_) | Shown::List, Reaction::Accepted(_) | Reaction::Cancelled) => {
                self.reset();
                modal.abandon();
                FlowOutcome::None
            }
            _ => FlowOutcome::None,
        }
    }

    /// Sustituye la ventana abierta por el editor de `action` con `slot` armada.
    fn start_editor(
        &mut self,
        modal: &mut Modal,
        action: AppAction,
        slot: Slot,
        keymap: &Keymap,
        platform: Platform,
        active: Tool,
    ) {
        let Some(mut editor) = ShortcutEditor::new(action, keymap, platform) else {
            self.ret = None;
            return;
        };
        editor.begin_capture(slot);
        modal.abandon();
        if modal.open(DialogKind::ShortcutEditor, editor_form(), Locks::default()) {
            self.editor = Some(editor);
            self.shown = Some(Shown::Editor);
            self.sync(modal, platform);
        } else {
            self.reset();
            let _ = active;
        }
    }

    /// Cierra el editor o la confirmación y reabre la ventana de origen desde la tabla viva, en su
    /// posición; sin origen, solo cierra.
    fn back(&mut self, modal: &mut Modal, keymap: &Keymap, platform: Platform, active: Tool) {
        modal.abandon();
        self.editor = None;
        let origin = self.ret.take();
        self.shown = None;
        match origin {
            Some(Return::Help(target, line)) => {
                if self.open_help(modal, target, active, keymap, platform, Locks::default())
                    && let Some(dialog) = modal.dialog_mut()
                {
                    dialog.set_first_line(HELP_ZONE, line);
                }
            }
            Some(Return::List(scroll)) => {
                if self.open_list(modal, keymap, platform, Locks::default())
                    && let Some(dialog) = modal.dialog_mut()
                {
                    dialog.set_scroll_u(scroll);
                }
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests;
