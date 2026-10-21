//! Foco y teclado de los diálogos modales, sin ventana (specs/hito-2-dialogos-modales.md,
//! decisiones 5 y 7): el orden de tabulación, el foco inicial y qué hacen `Intro` y `Escape`.

use egui::{Key, Modifiers};

use crate::dialog::{Dialog, FieldId, FieldKind};

/// Un elemento al que se puede dar el foco.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusTarget {
    /// El texto de un numérico o un hexadecimal, o una casilla.
    Field(FieldId),
    /// Una opción de un grupo.
    Option(FieldId, usize),
    Minus(FieldId),
    Plus(FieldId),
    /// Una zona de texto de solo lectura.
    Zone(FieldId),
    /// El botón principal de un campo `Button`.
    Button(FieldId),
    /// El segundo botón de un campo `Button`.
    ButtonAside(FieldId),
    Accept,
    Cancel,
}

/// Las dos teclas que tratan los diálogos, sin modificadores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKey {
    Enter,
    Escape,
}

/// Lo que hay que hacer con una tecla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    Accept,
    Cancel,
    ConfirmHex(FieldId),
    RevertHex(FieldId),
    /// Nada: el elemento enfocado (un botón) o la zona de texto se ocupan de la tecla.
    Nothing,
}

/// `Intro` o `Escape` sin modificadores; cualquier otra cosa, `None`.
pub fn dialog_key(key: Key, modifiers: Modifiers) -> Option<DialogKey> {
    if !modifiers.is_none() {
        return None;
    }
    match key {
        Key::Enter => Some(DialogKey::Enter),
        Key::Escape => Some(DialogKey::Escape),
        _ => None,
    }
}

/// El orden de tabulación: los campos visibles de arriba abajo (`Minus` y `Plus` tras cada
/// numérico, una parada por opción, la zona de texto), `Aceptar` solo si todo es válido y
/// `Cancelar` si lo hay.
pub fn focus_order(dialog: &Dialog) -> Vec<FocusTarget> {
    let mut order = Vec::new();
    for index in dialog.shown_indices() {
        let field = &dialog.spec().fields[index];
        match &field.kind {
            FieldKind::Number { .. } => {
                order.push(FocusTarget::Field(field.id));
                order.push(FocusTarget::Minus(field.id));
                order.push(FocusTarget::Plus(field.id));
            }
            FieldKind::Choice { options, .. } => {
                order.extend((0..options.len()).map(|i| FocusTarget::Option(field.id, i)));
            }
            FieldKind::Check { .. } | FieldKind::Hex { .. } => {
                order.push(FocusTarget::Field(field.id));
            }
            FieldKind::ReadOnly { .. } => order.push(FocusTarget::Zone(field.id)),
            FieldKind::Button { .. } => {
                if dialog.button_enabled(field.id, false) {
                    order.push(FocusTarget::Button(field.id));
                }
                if dialog.button_enabled(field.id, true) {
                    order.push(FocusTarget::ButtonAside(field.id));
                }
            }
        }
    }
    if dialog.is_valid() {
        order.push(FocusTarget::Accept);
    }
    if dialog.spec().cancel.is_some() {
        order.push(FocusTarget::Cancel);
    }
    order
}

/// El foco al abrir: el primer campo editable visible (en un grupo, la opción elegida); si no hay,
/// el primero que sea una zona de texto o un botón de campo habilitado, en el orden de los campos;
/// si no, `Aceptar`.
pub fn initial_focus(dialog: &Dialog) -> FocusTarget {
    for index in dialog.shown_indices() {
        let field = &dialog.spec().fields[index];
        match &field.kind {
            FieldKind::Number { .. } | FieldKind::Check { .. } | FieldKind::Hex { .. } => {
                return FocusTarget::Field(field.id);
            }
            FieldKind::Choice { .. } => {
                return FocusTarget::Option(field.id, dialog.chosen(field.id).unwrap_or(0));
            }
            FieldKind::ReadOnly { .. } | FieldKind::Button { .. } => {}
        }
    }
    for index in dialog.shown_indices() {
        let field = &dialog.spec().fields[index];
        match field.kind {
            FieldKind::ReadOnly { .. } => return FocusTarget::Zone(field.id),
            FieldKind::Button { .. } if dialog.button_enabled(field.id, false) => {
                return FocusTarget::Button(field.id);
            }
            _ => {}
        }
    }
    FocusTarget::Accept
}

/// El siguiente elemento (o el anterior con `backwards`), dando la vuelta. Con un foco que no está
/// en el orden (o ninguno), el primero (o el último hacia atrás). `None` si el orden está vacío.
pub fn next_focus(
    order: &[FocusTarget],
    current: Option<FocusTarget>,
    backwards: bool,
) -> Option<FocusTarget> {
    if order.is_empty() {
        return None;
    }
    let position = current.and_then(|target| order.iter().position(|t| *t == target));
    let index = match (position, backwards) {
        (Some(i), false) => (i + 1) % order.len(),
        (Some(i), true) => (i + order.len() - 1) % order.len(),
        (None, false) => 0,
        (None, true) => order.len() - 1,
    };
    Some(order[index])
}

/// Qué hacer con `Intro` o `Escape` con el foco en `focus` (el del principio del fotograma).
/// Las repeticiones de teclado no hacen nada.
pub fn key_action(
    dialog: &Dialog,
    focus: Option<FocusTarget>,
    key: DialogKey,
    repeat: bool,
) -> KeyAction {
    if repeat {
        return KeyAction::Nothing;
    }
    let pending_hex = match focus {
        Some(FocusTarget::Field(id)) if dialog.hex_pending(id) => Some(id),
        _ => None,
    };
    match key {
        DialogKey::Escape => match pending_hex {
            Some(id) => KeyAction::RevertHex(id),
            None => KeyAction::Cancel,
        },
        DialogKey::Enter => match focus {
            Some(
                FocusTarget::Zone(_)
                | FocusTarget::Button(_)
                | FocusTarget::ButtonAside(_)
                | FocusTarget::Minus(_)
                | FocusTarget::Plus(_)
                | FocusTarget::Accept
                | FocusTarget::Cancel,
            ) => KeyAction::Nothing,
            _ => match pending_hex {
                Some(id) => KeyAction::ConfirmHex(id),
                None if dialog.is_valid() => KeyAction::Accept,
                None => KeyAction::Nothing,
            },
        },
    }
}
