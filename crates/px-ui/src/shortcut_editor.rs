//! El editor de atajo (specs/interfaz-ayuda.md, decisiones 4, 5, 6 y 7), sin egui: el modelo de
//! casillas de una acción sobre la tabla de teclas, el borrador con captura y conflictos, y el
//! formulario que lo enseña.
//!
//! **Casillas.** De una acción, las filas con `command` son *fijas* (las combinaciones modernas: no
//! se editan ni se asignan); el resto son sus *casillas*, con la que se muestra primero (la
//! principal), luego la secundaria y, si hay más (hoy solo `=` en Acercar), los extras, que se
//! conservan. Toda edición reconstruye la tabla en una forma canónica (acciones en el orden de la
//! tabla de ayuda, fijas y luego casillas) para que el resultado no dependa del camino seguido, y
//! si todas las acciones coinciden con las del esquema por defecto devuelve ese esquema exacto.

use crate::actions::{AppAction, Platform};
use crate::dialog::{Dialog, FieldId, FieldKind, FieldSpec, FormSpec, Rows};
use crate::help_texts::{help_entry, help_key, help_table, is_editable, ui};
use crate::key_events::{Capture, chord_of};
use crate::keymap::{Binding, Chord, Conflict, Keymap, Platforms, default_keymap};

/// Campos del formulario del editor.
pub const EDITOR_PRIMARY: FieldId = "principal";
pub const EDITOR_SECONDARY: FieldId = "secundario";
pub const EDITOR_MESSAGE: FieldId = "mensaje";
pub const EDITOR_CONFLICT: FieldId = "conflicto";
pub const EDITOR_FIXED: FieldId = "fijos";
pub const EDITOR_RESET: FieldId = "restablecer";

/// Las casillas de una acción en una tabla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slots {
    /// Las combinaciones con `command`, en el orden de la tabla.
    pub fixed: Vec<Chord>,
    /// Las casillas: principal, secundaria y extras, sin huecos.
    pub slots: Vec<Chord>,
}

/// Las casillas del editor que se ven: la principal y la secundaria.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Primary,
    Secondary,
}

impl Slot {
    fn index(self) -> usize {
        match self {
            Slot::Primary => 0,
            Slot::Secondary => 1,
        }
    }
}

/// Lo que dio una captura.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureResult {
    /// La casilla tiene el atajo nuevo.
    Set,
    /// El atajo es de otra acción: queda pendiente de `resolve`.
    Pending(AppAction),
    /// No vale; la captura sigue armada.
    Rejected(&'static str),
    /// No había captura armada, o era `Escape`.
    Ignored,
}

fn chord_of_row(row: &Binding) -> Chord {
    Chord {
        key: row.key,
        command: row.command,
        shift: row.shift,
    }
}

/// Una tabla separada por acciones: lo que el editor cambia y lo que conserva tal cual.
#[derive(Clone, Debug, PartialEq)]
struct Parts {
    /// En el orden de la tabla de ayuda: las filas tal cual de la acción que no son casillas y sus
    /// casillas (vacías en una acción que no se edita).
    actions: Vec<(AppAction, Vec<Binding>, Vec<Chord>)>,
    /// Las filas de acciones sin ayuda, tal cual y en su orden.
    unknown: Vec<Binding>,
}

impl Parts {
    fn of(keymap: &Keymap) -> Parts {
        let mut actions: Vec<(AppAction, Vec<Binding>, Vec<Chord>)> = help_table()
            .iter()
            .filter_map(|entry| crate::help_texts::action_of_key(entry.key))
            .map(|action| (action, Vec::new(), Vec::new()))
            .collect();
        let mut unknown = Vec::new();
        for action_slot in &mut actions {
            let action = action_slot.0;
            let rows: Vec<&Binding> = keymap
                .rows()
                .iter()
                .filter(|r| r.action == action)
                .collect();
            if is_editable(action) {
                action_slot.1 = rows
                    .iter()
                    .filter(|r| r.command)
                    .map(|r| (*r).clone())
                    .collect();
                let editable: Vec<&&Binding> = rows.iter().filter(|r| !r.command).collect();
                let primary = editable
                    .iter()
                    .position(|r| r.shown != Platforms::Never)
                    .unwrap_or(0);
                if !editable.is_empty() {
                    action_slot.2.push(chord_of_row(editable[primary]));
                    for (i, row) in editable.iter().enumerate() {
                        if i != primary {
                            action_slot.2.push(chord_of_row(row));
                        }
                    }
                }
            } else {
                action_slot.1 = rows.into_iter().cloned().collect();
            }
        }
        for row in keymap.rows() {
            if help_key(row.action).is_none() {
                unknown.push(row.clone());
            }
        }
        Parts { actions, unknown }
    }

    fn slots_mut(&mut self, action: AppAction) -> Option<&mut Vec<Chord>> {
        self.actions
            .iter_mut()
            .find(|(a, _, _)| *a == action)
            .map(|(_, _, slots)| slots)
    }

    fn compose(&self, default: &Keymap) -> Keymap {
        if *self == Parts::of(default) {
            return default.clone();
        }
        let mut rows = Vec::new();
        for (action, fixed, slots) in &self.actions {
            rows.extend(fixed.iter().cloned());
            let fixed_shown = fixed
                .iter()
                .any(|r| r.command && r.shown != Platforms::Never);
            for (i, chord) in slots.iter().enumerate() {
                rows.push(Binding {
                    key: chord.key,
                    command: false,
                    shift: chord.shift,
                    action: *action,
                    active: Platforms::All,
                    shown: if i == 0 && !fixed_shown {
                        Platforms::All
                    } else {
                        Platforms::Never
                    },
                });
            }
        }
        rows.extend(self.unknown.iter().cloned());
        Keymap::new(rows)
    }
}

/// Las casillas y las combinaciones fijas de `action` en `keymap`.
pub fn slots_of(keymap: &Keymap, action: AppAction) -> Slots {
    let parts = Parts::of(keymap);
    parts
        .actions
        .into_iter()
        .find(|(a, _, _)| *a == action)
        .map(|(_, fixed, slots)| Slots {
            fixed: fixed
                .iter()
                .filter(|r| r.command)
                .map(chord_of_row)
                .collect(),
            slots,
        })
        .unwrap_or(Slots {
            fixed: Vec::new(),
            slots: Vec::new(),
        })
}

/// Las combinaciones fijas de `action` que valen en `platform`.
pub fn fixed_chords(keymap: &Keymap, action: AppAction, platform: Platform) -> Vec<Chord> {
    keymap
        .rows()
        .iter()
        .filter(|r| r.action == action && r.command && r.active.includes(platform))
        .map(chord_of_row)
        .collect()
}

/// `keymap` con las casillas de `action` cambiadas a `slots` (compactadas y sin repetir). Si
/// `action` no es editable, no cambia nada.
pub fn with_slots(
    keymap: &Keymap,
    default: &Keymap,
    action: AppAction,
    slots: Vec<Chord>,
) -> Keymap {
    let mut parts = Parts::of(keymap);
    if !is_editable(action) {
        return keymap.clone();
    }
    if let Some(target) = parts.slots_mut(action) {
        let mut unique: Vec<Chord> = Vec::new();
        for chord in slots {
            if !unique.contains(&chord) {
                unique.push(chord);
            }
        }
        *target = unique;
    }
    parts.compose(default)
}

/// Quita de la tabla toda fila sin `command` de `action` con el atajo `chord` (para una acción que
/// no es editable o sin ayuda, directamente; si es editable, por sus casillas).
pub fn without_chord(keymap: &Keymap, default: &Keymap, action: AppAction, chord: Chord) -> Keymap {
    if is_editable(action) {
        let mut slots = slots_of(keymap, action).slots;
        slots.retain(|c| *c != chord);
        with_slots(keymap, default, action, slots)
    } else {
        Keymap::new(
            keymap
                .rows()
                .iter()
                .filter(|r| !(r.action == action && !r.command && chord_of_row(r) == chord))
                .cloned()
                .collect(),
        )
    }
}

/// Un atajo asignado a una casilla que choca con el de otra acción.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pending {
    slot: usize,
    chord: Chord,
    owner: AppAction,
}

/// El borrador de la edición de los atajos de una acción.
#[derive(Clone, Debug)]
pub struct ShortcutEditor {
    action: AppAction,
    platform: Platform,
    keymap: Keymap,
    capturing: Option<usize>,
    pending: Option<Pending>,
    message: Option<String>,
}

impl ShortcutEditor {
    /// El editor de `action` sobre `keymap`; `None` si la acción no se edita.
    pub fn new(action: AppAction, keymap: &Keymap, platform: Platform) -> Option<ShortcutEditor> {
        is_editable(action).then(|| ShortcutEditor {
            action,
            platform,
            keymap: keymap.clone(),
            capturing: None,
            pending: None,
            message: None,
        })
    }

    pub fn action(&self) -> AppAction {
        self.action
    }

    /// El borrador: una tabla sin conflictos.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Las casillas de la acción en el borrador (principal, secundaria y extras).
    pub fn slots(&self) -> Vec<Chord> {
        slots_of(&self.keymap, self.action).slots
    }

    /// Las combinaciones fijas de la acción que valen en esta plataforma.
    pub fn fixed(&self) -> Vec<Chord> {
        fixed_chords(&self.keymap, self.action, self.platform)
    }

    /// Los extras: las casillas después de la secundaria.
    pub fn extras(&self) -> Vec<Chord> {
        self.slots().into_iter().skip(2).collect()
    }

    /// Arma la captura de una casilla: lo siguiente que se pulse se intenta asignar.
    pub fn begin_capture(&mut self, slot: Slot) {
        self.pending = None;
        self.message = None;
        self.capturing = Some(slot.index());
    }

    /// La casilla armada, si la hay.
    pub fn capturing(&self) -> Option<Slot> {
        self.capturing.map(|i| {
            if i == 0 {
                Slot::Primary
            } else {
                Slot::Secondary
            }
        })
    }

    /// Desarma la captura. `true` si estaba armada.
    pub fn cancel_capture(&mut self) -> bool {
        self.message = None;
        self.capturing.take().is_some()
    }

    /// Lo capturado en un fotograma: asigna, avisa de un conflicto o rechaza (con la captura aún
    /// armada).
    pub fn capture(&mut self, input: Capture) -> CaptureResult {
        let Some(slot) = self.capturing else {
            return CaptureResult::Ignored;
        };
        match input {
            Capture::Cancel => {
                self.cancel_capture();
                CaptureResult::Ignored
            }
            Capture::Reserved(reason) => self.reject(reason),
            Capture::Press(press) => match chord_of(&press) {
                Ok(chord) => self.try_assign(slot, chord),
                Err(reason) => self.reject(reason),
            },
        }
    }

    fn reject(&mut self, reason: &'static str) -> CaptureResult {
        self.message = Some(reason.to_string());
        CaptureResult::Rejected(reason)
    }

    /// Las casillas con `chord` en la posición `slot` (se añade al final si no hay tantas).
    fn with_chord_at(&self, slot: usize, chord: Chord) -> Vec<Chord> {
        let mut slots = self.slots();
        if slot < slots.len() {
            slots[slot] = chord;
        } else {
            slots.push(chord);
        }
        slots
    }

    fn try_assign(&mut self, slot: usize, chord: Chord) -> CaptureResult {
        let current = self.slots();
        if current.get(slot) == Some(&chord) {
            self.capturing = None;
            self.message = None;
            return CaptureResult::Set;
        }
        if current
            .iter()
            .enumerate()
            .any(|(i, c)| i != slot && *c == chord)
        {
            return self.reject(ui().same_action);
        }
        let candidate = with_slots(
            &self.keymap,
            default_keymap(),
            self.action,
            self.with_chord_at(slot, chord),
        );
        // El borrador no tiene conflictos: cualquiera es del atajo nuevo.
        if let Some(conflict) = candidate.conflicts().into_iter().next() {
            match conflict {
                Conflict::Duplicate(i, j) => {
                    let (a, b) = (&candidate.rows()[i], &candidate.rows()[j]);
                    let owner = [a, b].into_iter().find(|r| r.action != self.action);
                    return match owner {
                        Some(row) => {
                            let owner = row.action;
                            self.capturing = None;
                            self.pending = Some(Pending { slot, chord, owner });
                            self.message = Some(format!(
                                "{} {} {}",
                                chord.text(self.platform),
                                ui().already_belongs,
                                help_key(owner)
                                    .and_then(help_entry)
                                    .map_or("otra acción", |entry| entry.name)
                            ));
                            CaptureResult::Pending(owner)
                        }
                        None => self.reject(ui().same_action),
                    };
                }
                Conflict::Reserved(_) => return self.reject(ui().not_a_key),
            }
        }
        self.keymap = candidate;
        self.capturing = None;
        self.message = None;
        CaptureResult::Set
    }

    /// El conflicto sin resolver: el atajo y la acción que ya lo usa.
    pub fn pending(&self) -> Option<(Chord, AppAction)> {
        self.pending.map(|p| (p.chord, p.owner))
    }

    /// Resuelve el conflicto: `reassign` quita el atajo a la otra acción y lo asigna a la casilla;
    /// si no, la casilla queda como estaba.
    pub fn resolve(&mut self, reassign: bool) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        self.message = None;
        if !reassign {
            return;
        }
        self.keymap = without_chord(&self.keymap, default_keymap(), pending.owner, pending.chord);
        let slots = self.with_chord_at(pending.slot, pending.chord);
        self.keymap = with_slots(&self.keymap, default_keymap(), self.action, slots);
    }

    /// Vacía una casilla (las siguientes suben).
    pub fn clear(&mut self, slot: Slot) {
        self.capturing = None;
        self.pending = None;
        self.message = None;
        let mut slots = self.slots();
        if slot.index() < slots.len() {
            slots.remove(slot.index());
            self.keymap = with_slots(&self.keymap, default_keymap(), self.action, slots);
        }
    }

    /// Devuelve las casillas de **esta acción** a las del esquema por defecto.
    pub fn reset(&mut self) {
        self.capturing = None;
        self.pending = None;
        self.message = None;
        let defaults = slots_of(default_keymap(), self.action).slots;
        self.keymap = with_slots(&self.keymap, default_keymap(), self.action, defaults);
    }

    /// Se puede aceptar si no hay un conflicto sin resolver.
    pub fn can_accept(&self) -> bool {
        self.pending.is_none()
    }

    /// El mensaje de la captura o del conflicto, si lo hay.
    pub fn message(&self) -> Option<String> {
        self.message.clone()
    }

    /// El borrador como la tabla nueva.
    pub fn accept(self) -> Keymap {
        self.keymap
    }
}

/// El formulario del editor: dos filas de casilla con `Quitar`, el mensaje, el conflicto con sus
/// dos botones, los fijos y `Restablecer`. `sync_editor_dialog` pone el contenido.
pub fn editor_form() -> FormSpec {
    let strings = ui();
    let button = |id, label, text: &str, aside| FieldSpec {
        id,
        label,
        kind: FieldKind::Button {
            text: text.to_string(),
            aside,
        },
        shown_when: None,
    };
    let zone = |id, rows| FieldSpec {
        id,
        label: "",
        kind: FieldKind::ReadOnly {
            text: String::new(),
            rows: Rows::Fixed(rows),
        },
        shown_when: None,
    };
    FormSpec {
        title: strings.editor_title,
        accept: strings.accept,
        cancel: Some(strings.cancel),
        fields: vec![
            button(EDITOR_PRIMARY, strings.primary, "", Some(strings.remove)),
            button(
                EDITOR_SECONDARY,
                strings.secondary,
                "",
                Some(strings.remove),
            ),
            zone(EDITOR_MESSAGE, 3),
            button(
                EDITOR_CONFLICT,
                strings.conflict_label,
                strings.reassign,
                Some(strings.keep),
            ),
            zone(EDITOR_FIXED, 2),
            button(EDITOR_RESET, strings.reset_label, strings.reset, None),
        ],
        check: None,
    }
}

/// Pone en el formulario lo que dice el modelo: el texto de las casillas, qué botones están
/// atenuados, el mensaje, los fijos y el motivo que impide aceptar.
pub fn sync_editor_dialog(dialog: &mut Dialog, editor: &ShortcutEditor, platform: Platform) {
    let strings = ui();
    let slots = editor.slots();
    for (id, index) in [(EDITOR_PRIMARY, 0usize), (EDITOR_SECONDARY, 1usize)] {
        let text = if editor.capturing.is_some_and(|armed| armed == index) {
            strings.armed.to_string()
        } else {
            slots
                .get(index)
                .map_or(strings.no_shortcut.to_string(), |chord| {
                    chord.text(platform)
                })
        };
        dialog.set_button_text(id, text);
        dialog.set_button_disabled(id, false, slots.len() <= index);
    }
    let name = help_key(editor.action)
        .and_then(help_entry)
        .map_or("", |entry| entry.name);
    let note = if editor.pending.is_none() && editor.capturing.is_some() {
        editor
            .message
            .clone()
            .unwrap_or_else(|| strings.capture_prompt.to_string())
    } else {
        editor.message.clone().unwrap_or_default()
    };
    dialog.set_report(
        EDITOR_MESSAGE,
        format!("{}{name}\n{note}", strings.action_prefix),
    );
    let pending = editor.pending.is_some();
    dialog.set_button_disabled(EDITOR_CONFLICT, !pending, !pending);
    dialog.set_block(pending.then(|| strings.resolve_conflict.to_string()));
    let join = |chords: Vec<Chord>| {
        chords
            .iter()
            .map(|chord| chord.text(platform))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut fixed = String::new();
    let fixed_chords = editor.fixed();
    if !fixed_chords.is_empty() {
        fixed.push_str(strings.fixed_prefix);
        fixed.push_str(&join(fixed_chords));
    }
    let extras = editor.extras();
    if !extras.is_empty() {
        if !fixed.is_empty() {
            fixed.push('\n');
        }
        fixed.push_str(strings.also_prefix);
        fixed.push_str(&join(extras));
    }
    dialog.set_report(EDITOR_FIXED, fixed);
}
