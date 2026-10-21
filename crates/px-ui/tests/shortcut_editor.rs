//! Tests del editor de atajo (specs/interfaz-ayuda.md, unidad I.5). En ROJO hasta la lógica:
//! `shortcut_editor` no existe todavía.
//!
//! - Fila 5: modelo de casillas y filas (`slots_of`, `ShortcutEditor::new` y `accept`).
//! - Fila 7: conflictos y reasignación.
//! - Fila 8 (parte pura): el formulario del editor y su sincronización con el modelo.
//! - Propiedad 2: el editor nunca corrompe la tabla.

use proptest::prelude::*;
use px_ui::actions::{AppAction, Platform};
use px_ui::dialog::{Dialog, DialogKind, Event, Reaction};
use px_ui::help_texts::{action_of_key, help_table, is_editable};
use px_ui::key_events::Capture;
use px_ui::keymap::{Chord, KeyId, Keymap, Press};
use px_ui::shortcut_editor::{
    CaptureResult, EDITOR_CONFLICT, EDITOR_MESSAGE, EDITOR_PRIMARY, EDITOR_RESET,
    EDITOR_SECONDARY, ShortcutEditor, Slot, editor_form, slots_of, sync_editor_dialog,
};
use px_ui::tool::Tool;

const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

fn chord(c: char) -> Chord {
    Chord {
        key: KeyId::Char(c),
        command: false,
        shift: false,
    }
}

fn chord_con(key: KeyId, command: bool, shift: bool) -> Chord {
    Chord {
        key,
        command,
        shift,
    }
}

fn press(c: char) -> Press {
    Press {
        key: Some(KeyId::Char(c)),
        command: false,
        shift: false,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }
}

fn lapiz() -> AppAction {
    AppAction::SelectTool(Tool::Pencil)
}

fn linea() -> AppAction {
    AppAction::SelectTool(Tool::Line)
}

fn editor(action: AppAction, tabla: &Keymap) -> ShortcutEditor {
    ShortcutEditor::new(action, tabla, Platform::Linux).expect("editable")
}

/// Asigna `c` a la casilla `slot` de `action` (con el conflicto, `reasignar`) y acepta.
fn asigna(tabla: &Keymap, action: AppAction, slot: Slot, c: char, reasignar: bool) -> Keymap {
    let mut e = editor(action, tabla);
    e.begin_capture(slot);
    if let CaptureResult::Pending(_) = e.capture(Capture::Press(press(c))) {
        e.resolve(reasignar);
    }
    e.accept()
}

fn editables() -> Vec<AppAction> {
    help_table()
        .iter()
        .filter_map(|e| action_of_key(e.key))
        .filter(|a| is_editable(*a))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Fila 5: casillas y filas
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_5_las_casillas_de_la_tabla_por_defecto() {
    let tabla = Keymap::default_scheme();
    let slots = |a| slots_of(&tabla, a).slots;
    assert_eq!(slots(lapiz()), vec![chord('d')]);
    assert_eq!(slots(AppAction::Undo), vec![chord('u')]);
    assert_eq!(slots(AppAction::Redo), vec![]);
    assert_eq!(
        slots(AppAction::ZoomIn),
        vec![chord('>'), chord('+'), chord('=')],
        "principal, secundaria y el extra `=`"
    );
    assert_eq!(slots(AppAction::ZoomOut), vec![chord('<'), chord('-')]);
    assert_eq!(
        slots(AppAction::ColorNext),
        vec![chord(']'), chord_con(KeyId::PageDown, false, false)]
    );
    assert_eq!(
        slots(AppAction::BackgroundNext),
        vec![chord('}'), chord_con(KeyId::PageDown, false, true)],
        "la principal es la que se muestra, no la primera fila"
    );
    assert_eq!(slots(AppAction::ClearPage), vec![chord_con(KeyId::Char('k'), false, true)]);
    assert_eq!(slots(AppAction::ZoomActual), vec![]);
    // Fijas: las filas con `command` activas en la plataforma.
    assert_eq!(
        slots_of(&tabla, AppAction::Undo).fixed,
        vec![chord_con(KeyId::Char('z'), true, false)]
    );
    assert_eq!(
        slots_of(&tabla, AppAction::Redo).fixed,
        vec![
            chord_con(KeyId::Char('y'), true, false),
            chord_con(KeyId::Char('z'), true, true)
        ]
    );
}

#[test]
fn fila_5_solo_las_acciones_editables_tienen_editor() {
    let tabla = Keymap::default_scheme();
    for action in [
        AppAction::Open,
        AppAction::Save,
        AppAction::SaveAs,
        AppAction::NewImage,
        AppAction::Quit,
        AppAction::ShowLicenses,
        AppAction::SelectColor(3),
    ] {
        assert!(
            ShortcutEditor::new(action, &tabla, Platform::Linux).is_none(),
            "{action:?}"
        );
    }
    assert_eq!(editables().len(), 17);
    for action in editables() {
        assert!(ShortcutEditor::new(action, &tabla, Platform::Linux).is_some());
    }
}

#[test]
fn fila_5_cambiar_y_volver_deja_la_tabla_por_defecto_exacta() {
    let defecto = Keymap::default_scheme();
    for action in editables() {
        let mut e = editor(action, &defecto);
        e.begin_capture(Slot::Primary);
        assert_eq!(e.capture(Capture::Press(press('x'))), CaptureResult::Set);
        let cambiada = e.accept();
        assert_ne!(cambiada, defecto, "{action:?}");
        // Volver a lo de antes: restablecer.
        let mut e = editor(action, &cambiada);
        e.reset();
        assert_eq!(e.accept(), defecto, "{action:?} tras restablecer");
    }
    // Cambiar `d` por `x` y volver a `d`: la tabla por defecto exacta.
    let x = asigna(&defecto, lapiz(), Slot::Primary, 'x', false);
    let d = asigna(&x, lapiz(), Slot::Primary, 'd', false);
    assert_eq!(d, defecto);
}

#[test]
fn fila_5_asignar_regenera_las_filas_y_conserva_lo_demas() {
    let defecto = Keymap::default_scheme();
    // Lápiz: la `d` pasa a `x`; el menú, el tooltip y la tabla dicen `x`.
    let x = asigna(&defecto, lapiz(), Slot::Primary, 'x', false);
    assert_eq!(x.shown_text(lapiz(), Platform::Linux).as_deref(), Some("x"));
    assert_eq!(slots_of(&x, lapiz()).slots, vec![chord('x')]);
    // Las filas con `command` no cambian: exactamente las nueve modernas.
    let con_command = |k: &Keymap| k.rows().iter().filter(|r| r.command).cloned().collect::<Vec<_>>();
    assert_eq!(con_command(&x), con_command(&defecto));
    assert_eq!(con_command(&x).len(), 9);
    // Lo que no se tocó conserva sus filas.
    for fila in defecto.rows().iter().filter(|r| r.action != lapiz()) {
        assert!(x.rows().contains(fila), "{fila:?}");
    }
    // Una casilla vacía se compacta: al quitar la principal, sube la secundaria.
    let mut e = editor(AppAction::ColorNext, &defecto);
    e.clear(Slot::Primary);
    let t = e.accept();
    assert_eq!(
        slots_of(&t, AppAction::ColorNext).slots,
        vec![chord_con(KeyId::PageDown, false, false)]
    );
    // Deshacer sigue mostrando `Ctrl+Z` (la fija se muestra) aunque se cambie `u`.
    let u = asigna(&defecto, AppAction::Undo, Slot::Primary, 'x', false);
    assert_eq!(u.shown_text(AppAction::Undo, Platform::Linux).as_deref(), Some("Ctrl+Z"));
    // Rehacer no tiene casillas: asignar a la principal la crea (`shown` al ser la fija la que se muestra).
    let r = asigna(&defecto, AppAction::Redo, Slot::Primary, 'r', false);
    assert_eq!(slots_of(&r, AppAction::Redo).slots, vec![chord('r')]);
    assert_eq!(r.shown_text(AppAction::Redo, Platform::Linux).as_deref(), Some("Ctrl+Y"));
    // Un acción que solo tenía casillas editables muestra la nueva.
    let c = asigna(&defecto, AppAction::CenterOnCursor, Slot::Primary, 'q', false);
    assert_eq!(c.shown_text(AppAction::CenterOnCursor, Platform::Linux).as_deref(), Some("q"));
    // Con el extra `=` de Acercar: se conserva al cambiar la principal.
    let z = asigna(&defecto, AppAction::ZoomIn, Slot::Primary, 'x', false);
    assert_eq!(
        slots_of(&z, AppAction::ZoomIn).slots,
        vec![chord('x'), chord('+'), chord('=')]
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 7: conflictos
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_7_una_tecla_de_otra_accion_avisa_y_ofrece_reasignar_o_dejarlo() {
    let defecto = Keymap::default_scheme();
    let mut e = editor(lapiz(), &defecto);
    e.begin_capture(Slot::Primary);
    assert_eq!(
        e.capture(Capture::Press(press('v'))),
        CaptureResult::Pending(linea())
    );
    assert_eq!(e.pending(), Some((chord('v'), linea())));
    assert!(!e.can_accept(), "con un conflicto sin resolver no se acepta");
    assert!(e.message().unwrap().contains("ya es de Línea"), "{:?}", e.message());
    // Dejarlo: la casilla vuelve a como estaba y no queda huella.
    e.resolve(false);
    assert!(e.pending().is_none() && e.can_accept());
    assert_eq!(e.accept(), defecto);
    // Reasignar: Línea pierde su `v` (queda sin principal) y el lápiz la gana.
    let mut e = editor(lapiz(), &defecto);
    e.begin_capture(Slot::Primary);
    e.capture(Capture::Press(press('v')));
    e.resolve(true);
    let t = e.accept();
    assert_eq!(slots_of(&t, lapiz()).slots, vec![chord('v')]);
    assert_eq!(slots_of(&t, linea()).slots, vec![], "Línea queda sin atajo");
    assert!(t.conflicts().is_empty());
    // Cancelar tras reasignar no deja huella: el editor se descarta y la tabla de partida sigue igual.
    let mut e = editor(lapiz(), &defecto);
    e.begin_capture(Slot::Primary);
    e.capture(Capture::Press(press('v')));
    e.resolve(true);
    drop(e);
    assert_eq!(defecto, Keymap::default_scheme());
}

#[test]
fn fila_7_las_filas_que_no_se_muestran_tambien_cuentan_y_el_nombre_sale_de_la_tabla() {
    let defecto = Keymap::default_scheme();
    for (c, dueno) in [
        ('u', AppAction::Undo),
        ('+', AppAction::ZoomIn),
        ('-', AppAction::ZoomOut),
        ('}', AppAction::BackgroundNext),
        ('n', AppAction::CenterOnCursor),
    ] {
        let mut e = editor(lapiz(), &defecto);
        e.begin_capture(Slot::Primary);
        assert_eq!(
            e.capture(Capture::Press(press(c))),
            CaptureResult::Pending(dueno),
            "{c}"
        );
    }
    // `Av Pág` es de ColorNext.
    let mut e = editor(lapiz(), &defecto);
    e.begin_capture(Slot::Secondary);
    let av_pag = Press {
        key: Some(KeyId::PageDown),
        ..press('a')
    };
    assert_eq!(
        e.capture(Capture::Press(av_pag)),
        CaptureResult::Pending(AppAction::ColorNext)
    );
    // Reasignar un extra (`=` de Acercar) lo quita de Acercar.
    let mut e = editor(lapiz(), &defecto);
    e.begin_capture(Slot::Secondary);
    assert_eq!(
        e.capture(Capture::Press(press('='))),
        CaptureResult::Pending(AppAction::ZoomIn)
    );
    e.resolve(true);
    let t = e.accept();
    assert_eq!(slots_of(&t, AppAction::ZoomIn).slots, vec![chord('>'), chord('+')]);
    assert!(t.conflicts().is_empty());
}

#[test]
fn fila_7_la_misma_tecla_en_las_dos_casillas_y_lo_no_asignable_se_rechazan() {
    let defecto = Keymap::default_scheme();
    // El Lápiz ya tiene `d`: ponerla en la secundaria es de esta acción.
    let mut e = editor(lapiz(), &defecto);
    e.begin_capture(Slot::Secondary);
    match e.capture(Capture::Press(press('d'))) {
        CaptureResult::Rejected(m) => assert!(m.contains("esta acción"), "{m}"),
        otro => panic!("{otro:?}"),
    }
    assert_eq!(e.capturing(), Some(Slot::Secondary), "la captura sigue armada");
    // Reservadas y combinaciones modernas: rechazadas, captura armada.
    let mut e = editor(lapiz(), &defecto);
    e.begin_capture(Slot::Primary);
    for p in [
        Press {
            command: true,
            ..press('z')
        },
        Press {
            command: true,
            ..press('c')
        },
        press(' '),
        Press {
            key: None,
            ..press('a')
        },
        Press {
            alt: true,
            ..press('d')
        },
    ] {
        assert!(
            matches!(e.capture(Capture::Press(p)), CaptureResult::Rejected(_)),
            "{p:?}"
        );
        assert_eq!(e.capturing(), Some(Slot::Primary));
    }
    // Tras los rechazos nada cambió.
    assert!(e.can_accept());
    assert_eq!(e.accept(), defecto);
}

#[test]
fn fila_7_escape_desarma_la_captura_y_el_conflicto_en_cadena() {
    let defecto = Keymap::default_scheme();
    let mut e = editor(lapiz(), &defecto);
    e.begin_capture(Slot::Primary);
    assert!(e.cancel_capture());
    assert!(!e.cancel_capture(), "ya estaba desarmada");
    assert_eq!(e.capture(Capture::Press(press('x'))), CaptureResult::Ignored);
    e.begin_capture(Slot::Primary);
    assert_eq!(e.capture(Capture::Cancel), CaptureResult::Ignored);
    assert_eq!(e.capturing(), None, "Escape desarma");
    // Con un conflicto pendiente, `resolve(false)` es lo que hace el segundo Escape.
    e.begin_capture(Slot::Primary);
    e.capture(Capture::Press(press('v')));
    assert!(e.pending().is_some() && e.capturing().is_none());
    e.resolve(false);
    assert!(e.pending().is_none());
    assert_eq!(e.accept(), defecto);
}

// ---------------------------------------------------------------------------------------------
// Fila 8 (parte pura): el formulario del editor
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_8_el_formulario_del_editor_sigue_al_modelo() {
    let defecto = Keymap::default_scheme();
    let spec = editor_form();
    assert!(spec.validate().is_ok());
    assert_eq!((spec.accept, spec.cancel), ("Aceptar", Some("Cancelar")));
    let mut dialog = Dialog::new(DialogKind::ShortcutEditor, spec).unwrap();
    let mut e = editor(AppAction::ZoomIn, &defecto);
    sync_editor_dialog(&mut dialog, &e, Platform::Linux);
    assert_eq!(dialog.button_text(EDITOR_PRIMARY).as_deref(), Some(">"));
    assert_eq!(dialog.button_text(EDITOR_SECONDARY).as_deref(), Some("+"));
    // Sin conflicto, los botones del conflicto están atenuados y el conjunto es válido.
    assert_eq!(dialog.button_disabled(EDITOR_CONFLICT), Some([true, true]));
    assert!(dialog.is_valid());
    // Armada: la casilla lo dice y el mensaje invita a pulsar.
    e.begin_capture(Slot::Primary);
    sync_editor_dialog(&mut dialog, &e, Platform::Linux);
    assert!(dialog.button_text(EDITOR_PRIMARY).unwrap().contains("pulsa"));
    let mensaje = zona(&dialog, EDITOR_MESSAGE);
    assert!(mensaje.contains("Acercar") && mensaje.contains("Esc cancela"), "{mensaje}");
    // Con un conflicto: botones habilitados, `Aceptar` rechazado con el motivo.
    e.capture(Capture::Press(press('v')));
    sync_editor_dialog(&mut dialog, &e, Platform::Linux);
    assert_eq!(dialog.button_disabled(EDITOR_CONFLICT), Some([false, false]));
    assert!(!dialog.is_valid());
    assert_eq!(dialog.reason().as_deref(), Some("resuelve el conflicto"));
    assert!(matches!(dialog.event(Event::Accept), Reaction::Rejected(_)));
    // `Restablecer` es un botón pulsable.
    assert_eq!(
        dialog.event(Event::Press {
            field: EDITOR_RESET,
            aside: false
        }),
        Reaction::Pressed {
            field: EDITOR_RESET,
            aside: false
        }
    );
    // Un botón atenuado no responde.
    let mut e2 = editor(lapiz(), &defecto);
    sync_editor_dialog(&mut dialog, &e2, Platform::Linux);
    assert_eq!(
        dialog.event(Event::Press {
            field: EDITOR_CONFLICT,
            aside: false
        }),
        Reaction::None
    );
    e2.clear(Slot::Primary);
    sync_editor_dialog(&mut dialog, &e2, Platform::Linux);
    assert_eq!(dialog.button_text(EDITOR_PRIMARY).as_deref(), Some("sin atajo"));
}

fn zona(dialog: &Dialog, id: &str) -> String {
    use px_ui::dialog::FieldKind;
    match &dialog.spec().fields.iter().find(|f| f.id == id).unwrap().kind {
        FieldKind::ReadOnly { text, .. } => text.clone(),
        otro => panic!("{otro:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Paso {
    Abrir(usize),
    Capturar(u8, bool, bool),
    Quitar(bool),
    Restablecer,
    Resolver(bool),
    Cancelar,
    Aceptar,
}

fn paso() -> impl Strategy<Value = Paso> {
    prop_oneof![
        (0usize..17).prop_map(Paso::Abrir),
        (0u8..40, any::<bool>(), any::<bool>()).prop_map(|(c, s, sh)| Paso::Capturar(c, s, sh)),
        any::<bool>().prop_map(Paso::Quitar),
        Just(Paso::Restablecer),
        any::<bool>().prop_map(Paso::Resolver),
        Just(Paso::Cancelar),
        Just(Paso::Aceptar),
    ]
}

/// Un carácter asignable: letras, cifras y símbolos.
fn caracter(n: u8) -> char {
    const C: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789,.;/[]{}<>-=+";
    C[usize::from(n) % C.len()] as char
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 2: tras cualquier secuencia de operaciones del editor, la tabla aceptada no tiene
    /// conflictos, sus filas con `command` son las nueve modernas, cada acción editable tiene sus
    /// casillas, las acciones no tocadas conservan sus filas y `Cancelar` no deja huella.
    #[test]
    fn propiedad_2_el_editor_nunca_corrompe_la_tabla(
        pasos in proptest::collection::vec(paso(), 0..24),
        plataforma in 0usize..3,
    ) {
        let platform = PLATAFORMAS[plataforma];
        let defecto = Keymap::default_scheme();
        let mut tabla = defecto.clone();
        let mut tocadas: Vec<AppAction> = Vec::new();
        let mut abierto: Option<(AppAction, ShortcutEditor)> = None;
        for paso in pasos {
            match paso {
                Paso::Abrir(i) => {
                    let action = editables()[i];
                    abierto = ShortcutEditor::new(action, &tabla, platform).map(|e| (action, e));
                }
                Paso::Capturar(c, secundaria, shift) => {
                    if let Some((_, e)) = &mut abierto {
                        e.begin_capture(if secundaria { Slot::Secondary } else { Slot::Primary });
                        let ch = caracter(c);
                        let p = Press { shift: shift && ch.is_ascii_alphabetic(), ..press(ch) };
                        e.capture(Capture::Press(p));
                    }
                }
                Paso::Quitar(secundaria) => {
                    if let Some((_, e)) = &mut abierto {
                        e.clear(if secundaria { Slot::Secondary } else { Slot::Primary });
                    }
                }
                Paso::Restablecer => {
                    if let Some((_, e)) = &mut abierto {
                        e.reset();
                    }
                }
                Paso::Resolver(reasignar) => {
                    if let Some((_, e)) = &mut abierto {
                        if e.pending().is_some() {
                            e.resolve(reasignar);
                        }
                    }
                }
                Paso::Cancelar => abierto = None,
                Paso::Aceptar => {
                    if let Some((action, e)) = abierto.take() {
                        if e.can_accept() {
                            let nueva = e.accept();
                            // El borrador es una tabla válida.
                            prop_assert!(nueva.conflicts().is_empty());
                            tabla = nueva;
                            tocadas.push(action);
                        }
                    }
                }
            }
            if let Some((_, e)) = &abierto {
                prop_assert!(e.keymap().conflicts().is_empty());
            }
        }
        prop_assert!(tabla.conflicts().is_empty());
        let modernas = |k: &Keymap| k.rows().iter().filter(|r| r.command).cloned().collect::<Vec<_>>();
        prop_assert_eq!(modernas(&tabla), modernas(&defecto));
        prop_assert_eq!(modernas(&tabla).len(), 9);
        for action in editables() {
            let casillas = slots_of(&tabla, action).slots;
            // Ninguna casilla repetida dentro de la acción.
            for (i, c) in casillas.iter().enumerate() {
                prop_assert!(!casillas[..i].contains(c));
            }
            if !tocadas.contains(&action) {
                // Una acción que ningún `Aceptar` tocó puede haber perdido una casilla por un
                // `Reasignar` de otra; si no, conserva las suyas.
                let antes = slots_of(&defecto, action).slots;
                prop_assert!(casillas.iter().all(|c| antes.contains(c)));
            }
        }
        // Lo que se muestra funciona: la `Press` de cada atajo mostrado da su acción.
        for action in editables() {
            if let Some(c) = tabla.shown(action, platform) {
                let p = Press {
                    key: Some(c.key),
                    command: c.command,
                    shift: c.shift,
                    ..press('a')
                };
                prop_assert_eq!(tabla.action_for(&p, platform), Some(action));
            }
        }
    }
}
