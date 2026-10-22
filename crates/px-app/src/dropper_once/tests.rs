//! Unidad I.2a (specs/interfaz-dos-colores.md, visto bueno 6): el clic en el indicador activa el
//! cuentagotas **una sola vez** y, al soltar, vuelve la herramienta anterior.

use super::*;

#[test]
fn it_returns_to_the_previous_tool_on_releasing_the_first_gesture() {
    let mut once = DropperOnce::new();
    once.arm(Tool::Pencil);
    assert_eq!(
        once.update(Tool::Dropper, false),
        None,
        "aún no se ha usado"
    );
    assert_eq!(once.update(Tool::Dropper, true), None, "gesto en curso");
    assert_eq!(once.update(Tool::Dropper, true), None);
    assert_eq!(
        once.update(Tool::Dropper, false),
        Some(Tool::Pencil),
        "al soltar"
    );
    assert_eq!(once.update(Tool::Pencil, false), None, "solo una vez");
    assert_eq!(once.update(Tool::Dropper, true), None);
    assert_eq!(once.update(Tool::Dropper, false), None);
}

#[test]
fn choosing_another_tool_before_the_gesture_cancels_it() {
    for other in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Eraser] {
        let mut once = DropperOnce::new();
        once.arm(Tool::Line);
        assert_eq!(once.update(other, false), None);
        // Aunque luego vuelva a elegir el cuentagotas a mano y lo use, no hay retorno.
        assert_eq!(once.update(Tool::Dropper, true), None);
        assert_eq!(once.update(Tool::Dropper, false), None, "{other:?}");
    }
}

#[test]
fn with_the_dropper_already_chosen_it_stays_and_there_is_no_return() {
    let mut once = DropperOnce::new();
    once.arm(Tool::Dropper);
    assert_eq!(once.update(Tool::Dropper, true), None);
    assert_eq!(once.update(Tool::Dropper, false), None);
}

#[test]
fn pressing_the_indicator_again_with_the_dropper_armed_keeps_the_previous_tool() {
    let mut once = DropperOnce::new();
    once.arm(Tool::Fill);
    once.arm(Tool::Dropper);
    once.update(Tool::Dropper, true);
    assert_eq!(once.update(Tool::Dropper, false), Some(Tool::Fill));
}

#[test]
fn rearming_from_another_tool_changes_the_previous_one_and_cancel_clears_everything() {
    let mut once = DropperOnce::new();
    once.arm(Tool::Fill);
    once.arm(Tool::Line);
    once.update(Tool::Dropper, true);
    assert_eq!(once.update(Tool::Dropper, false), Some(Tool::Line));
    once.arm(Tool::Pencil);
    once.cancel();
    once.update(Tool::Dropper, true);
    assert_eq!(once.update(Tool::Dropper, false), None);
}
