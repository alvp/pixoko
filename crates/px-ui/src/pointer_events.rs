//! Eventos de egui del ratón y del trackpad a la entrada de la tabla de gestos
//! (specs/interfaz-raton.md). `egui::Event` como dato, sin `Context`: se prueba sin ventana.

use egui::{Event, Modifiers, MouseWheelUnit, PointerButton, Vec2};

use crate::actions::{AppAction, Platform};
use crate::gestures::{Button, GestureAction, GestureMap, Input, Pointing};

/// El botón de la tabla que corresponde al de egui. `egui-winit` ya descartó del sexto en adelante
/// (`MouseButton::Other`): no llegan.
pub fn button_of(button: PointerButton) -> Button {
    match button {
        PointerButton::Primary => Button::Left,
        PointerButton::Secondary => Button::Right,
        PointerButton::Middle => Button::Middle,
        PointerButton::Extra1 => Button::Back,
        PointerButton::Extra2 => Button::Forward,
    }
}

/// Una rueda de egui como entrada y delta. `Line` y `Page` son muescas (`Wheel`, el delta en
/// muescas) y `Point` son los dos dedos (`Scroll`, el delta en puntos). Con `command` no hay
/// entrada: ese gesto es de `Pinch`, que egui ya sintetiza como `zoom_delta`, y no se cuenta dos
/// veces.
pub fn wheel_of(
    unit: MouseWheelUnit,
    delta: Vec2,
    modifiers: Modifiers,
) -> Option<(Input, f32, f32)> {
    if modifiers.command {
        return None;
    }
    let input = match unit {
        MouseWheelUnit::Line | MouseWheelUnit::Page => Input::Wheel,
        MouseWheelUnit::Point => Input::Scroll,
    };
    Some((input, delta.x, delta.y))
}

/// Un botón por cada `PointerButton` **pulsado** (nunca por soltado), con `command` y `shift`.
pub fn pointer_presses(events: &[Event]) -> Vec<(Button, bool, bool)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::PointerButton {
                button,
                pressed: true,
                modifiers,
                ..
            } => Some((button_of(*button), modifiers.command, modifiers.shift)),
            _ => None,
        })
        .collect()
}

/// Las acciones de la tabla que valen en toda la ventana (los botones laterales) para los botones
/// pulsados de `events`, en orden. Las filas del lienzo no cuentan aquí, y `Espacio` tampoco (la
/// ventana no lo mira: una fila de ventana con `space: Some(true)` no se alcanza).
pub fn window_actions(events: &[Event], map: &GestureMap, platform: Platform) -> Vec<AppAction> {
    pointer_presses(events)
        .into_iter()
        .filter_map(|(button, command, shift)| {
            let pointing = Pointing {
                input: Input::Press(button),
                command,
                shift,
                space: false,
            };
            match map.resolve(&pointing, false, platform) {
                Some(GestureAction::App(action)) => Some(action),
                _ => None,
            }
        })
        .collect()
}
