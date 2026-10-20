//! Unidad I.2a (specs/interfaz-dos-colores.md, visto bueno 6): el clic en el indicador activa el
//! cuentagotas **una sola vez** y, al soltar, vuelve la herramienta anterior.

use super::*;

#[test]
fn vuelve_a_la_herramienta_anterior_al_soltar_el_primer_gesto() {
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
fn elegir_otra_herramienta_antes_del_gesto_lo_cancela() {
    for otra in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Eraser] {
        let mut once = DropperOnce::new();
        once.arm(Tool::Line);
        assert_eq!(once.update(otra, false), None);
        // Aunque luego vuelva a elegir el cuentagotas a mano y lo use, no hay retorno.
        assert_eq!(once.update(Tool::Dropper, true), None);
        assert_eq!(once.update(Tool::Dropper, false), None, "{otra:?}");
    }
}

#[test]
fn con_el_cuentagotas_ya_elegido_se_queda_y_no_hay_retorno() {
    let mut once = DropperOnce::new();
    once.arm(Tool::Dropper);
    assert_eq!(once.update(Tool::Dropper, true), None);
    assert_eq!(once.update(Tool::Dropper, false), None);
}

#[test]
fn volver_a_pulsar_el_indicador_con_el_cuentagotas_armado_conserva_la_herramienta_anterior() {
    let mut once = DropperOnce::new();
    once.arm(Tool::Fill);
    once.arm(Tool::Dropper);
    once.update(Tool::Dropper, true);
    assert_eq!(once.update(Tool::Dropper, false), Some(Tool::Fill));
}

#[test]
fn rearmar_desde_otra_herramienta_cambia_la_anterior_y_cancel_borra_todo() {
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
