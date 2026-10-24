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
    CaptureResult, EDITOR_CONFLICT, EDITOR_MESSAGE, EDITOR_PRIMARY, EDITOR_RESET, EDITOR_SECONDARY,
    ShortcutEditor, Slot, editor_form, slots_of, sync_editor_dialog,
};
use px_ui::tool::Tool;

const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

fn chord(c: char) -> Chord {
    Chord {
        key: KeyId::Char(c),
        command: false,
        shift: false,
    }
}

fn chord_with(key: KeyId, command: bool, shift: bool) -> Chord {
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

fn pencil() -> AppAction {
    AppAction::SelectTool(Tool::Pencil)
}

fn line() -> AppAction {
    AppAction::SelectTool(Tool::Line)
}

fn editor(action: AppAction, table: &Keymap) -> ShortcutEditor {
    ShortcutEditor::new(action, table, Platform::Linux).expect("editable")
}

/// Asigna `c` a la casilla `slot` de `action` (con el conflicto, `reassign`) y acepta.
fn assigns(table: &Keymap, action: AppAction, slot: Slot, c: char, reassign: bool) -> Keymap {
    let mut e = editor(action, table);
    e.begin_capture(slot);
    if let CaptureResult::Pending(_) = e.capture(Capture::Press(press(c))) {
        e.resolve(reassign);
    }
    e.accept()
}

fn editable() -> Vec<AppAction> {
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
fn row_5_the_boxes_of_the_default_table() {
    let table = Keymap::default_scheme();
    let slots = |a| slots_of(&table, a).slots;
    assert_eq!(slots(pencil()), vec![chord('d')]);
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
        vec![chord(']'), chord_with(KeyId::PageDown, false, false)]
    );
    assert_eq!(
        slots(AppAction::BackgroundNext),
        vec![chord('}'), chord_with(KeyId::PageDown, false, true)],
        "la principal es la que se muestra, no la primera fila"
    );
    assert_eq!(
        slots(AppAction::ClearPage),
        vec![chord_with(KeyId::Char('k'), false, true)]
    );
    assert_eq!(slots(AppAction::ZoomActualSize), vec![]);
    // Fijas: las filas con `command` activas en la plataforma.
    assert_eq!(
        slots_of(&table, AppAction::Undo).fixed,
        vec![chord_with(KeyId::Char('z'), true, false)]
    );
    assert_eq!(
        slots_of(&table, AppAction::Redo).fixed,
        vec![
            chord_with(KeyId::Char('y'), true, false),
            chord_with(KeyId::Char('z'), true, true)
        ]
    );
}

#[test]
fn row_5_only_the_editable_actions_have_an_editor() {
    let table = Keymap::default_scheme();
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
            ShortcutEditor::new(action, &table, Platform::Linux).is_none(),
            "{action:?}"
        );
    }
    assert_eq!(editable().len(), 39);
    for action in editable() {
        assert!(ShortcutEditor::new(action, &table, Platform::Linux).is_some());
    }
}

#[test]
fn row_5_changing_and_returning_leaves_the_default_table_exact() {
    let default = Keymap::default_scheme();
    for action in editable() {
        let mut e = editor(action, &default);
        e.begin_capture(Slot::Primary);
        assert_eq!(e.capture(Capture::Press(press('x'))), CaptureResult::Set);
        let changed = e.accept();
        assert_ne!(changed, default, "{action:?}");
        // Volver a lo de antes: restablecer.
        let mut e = editor(action, &changed);
        e.reset();
        assert_eq!(e.accept(), default, "{action:?} tras restablecer");
    }
    // Cambiar `d` por `x` y volver a `d`: la tabla por defecto exacta.
    let x = assigns(&default, pencil(), Slot::Primary, 'x', false);
    let d = assigns(&x, pencil(), Slot::Primary, 'd', false);
    assert_eq!(d, default);
}

#[test]
fn row_5_assign_regenerates_the_rows_and_keeps_the_rest() {
    let default = Keymap::default_scheme();
    // Lápiz: la `d` pasa a `x`; el menú, el tooltip y la tabla dicen `x`.
    let x = assigns(&default, pencil(), Slot::Primary, 'x', false);
    assert_eq!(
        x.shown_text(pencil(), Platform::Linux).as_deref(),
        Some("x")
    );
    assert_eq!(slots_of(&x, pencil()).slots, vec![chord('x')]);
    // Las filas con `command` no cambian: exactamente las nueve modernas.
    let with_command = |k: &Keymap| {
        k.rows()
            .iter()
            .filter(|r| r.command)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(with_command(&x).len(), 9);
    for row_2 in with_command(&default) {
        assert!(with_command(&x).contains(&row_2), "{row_2:?}");
    }
    // Lo que no se tocó conserva sus filas.
    for row_2 in default.rows().iter().filter(|r| r.action != pencil()) {
        assert!(x.rows().contains(row_2), "{row_2:?}");
    }
    // Una casilla vacía se compacta: al quitar la principal, sube la secundaria.
    let mut e = editor(AppAction::ColorNext, &default);
    e.clear(Slot::Primary);
    let t = e.accept();
    assert_eq!(
        slots_of(&t, AppAction::ColorNext).slots,
        vec![chord_with(KeyId::PageDown, false, false)]
    );
    // Deshacer sigue mostrando `Ctrl+Z` (la fija se muestra) aunque se cambie `u`.
    let u = assigns(&default, AppAction::Undo, Slot::Primary, 'x', false);
    assert_eq!(
        u.shown_text(AppAction::Undo, Platform::Linux).as_deref(),
        Some("Ctrl+Z")
    );
    // Rehacer no tiene casillas: asignar a la principal la crea (`shown` al ser la fija la que se muestra).
    let r = assigns(&default, AppAction::Redo, Slot::Primary, 'r', false);
    assert_eq!(slots_of(&r, AppAction::Redo).slots, vec![chord('r')]);
    assert_eq!(
        r.shown_text(AppAction::Redo, Platform::Linux).as_deref(),
        Some("Ctrl+Y")
    );
    // Un acción que solo tenía casillas editables muestra la nueva.
    let c = assigns(
        &default,
        AppAction::CenterOnCursor,
        Slot::Primary,
        'q',
        false,
    );
    assert_eq!(
        c.shown_text(AppAction::CenterOnCursor, Platform::Linux)
            .as_deref(),
        Some("q")
    );
    // Con el extra `=` de Acercar: se conserva al cambiar la principal.
    let z = assigns(&default, AppAction::ZoomIn, Slot::Primary, 'x', false);
    assert_eq!(
        slots_of(&z, AppAction::ZoomIn).slots,
        vec![chord('x'), chord('+'), chord('=')]
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 7: conflictos
// ---------------------------------------------------------------------------------------------

#[test]
fn row_7_a_key_of_another_action_warns_and_offers_to_reassign_or_leave_it() {
    let default = Keymap::default_scheme();
    let mut e = editor(pencil(), &default);
    e.begin_capture(Slot::Primary);
    assert_eq!(
        e.capture(Capture::Press(press('v'))),
        CaptureResult::Pending(line())
    );
    assert_eq!(e.pending(), Some((chord('v'), line())));
    assert!(
        !e.can_accept(),
        "con un conflicto sin resolver no se acepta"
    );
    assert!(
        e.message().unwrap().contains("ya es de Línea"),
        "{:?}",
        e.message()
    );
    // Dejarlo: la casilla vuelve a como estaba y no queda huella.
    e.resolve(false);
    assert!(e.pending().is_none() && e.can_accept());
    assert_eq!(e.accept(), default);
    // Reasignar: Línea pierde su `v` (queda sin principal) y el lápiz la gana.
    let mut e = editor(pencil(), &default);
    e.begin_capture(Slot::Primary);
    e.capture(Capture::Press(press('v')));
    e.resolve(true);
    let t = e.accept();
    assert_eq!(slots_of(&t, pencil()).slots, vec![chord('v')]);
    assert_eq!(slots_of(&t, line()).slots, vec![], "Línea queda sin atajo");
    assert!(t.conflicts().is_empty());
    // Cancelar tras reasignar no deja huella: el editor se descarta y la tabla de partida sigue igual.
    let mut e = editor(pencil(), &default);
    e.begin_capture(Slot::Primary);
    e.capture(Capture::Press(press('v')));
    e.resolve(true);
    drop(e);
    assert_eq!(default, Keymap::default_scheme());
}

#[test]
fn row_7_the_rows_that_are_not_shown_also_count_and_the_name_comes_from_the_table() {
    let default = Keymap::default_scheme();
    for (c, owner) in [
        ('u', AppAction::Undo),
        ('+', AppAction::ZoomIn),
        ('-', AppAction::ZoomOut),
        ('}', AppAction::BackgroundNext),
        ('n', AppAction::CenterOnCursor),
    ] {
        let mut e = editor(pencil(), &default);
        e.begin_capture(Slot::Primary);
        assert_eq!(
            e.capture(Capture::Press(press(c))),
            CaptureResult::Pending(owner),
            "{c}"
        );
    }
    // `Av Pág` es de ColorNext.
    let mut e = editor(pencil(), &default);
    e.begin_capture(Slot::Secondary);
    let av_page = Press {
        key: Some(KeyId::PageDown),
        ..press('a')
    };
    assert_eq!(
        e.capture(Capture::Press(av_page)),
        CaptureResult::Pending(AppAction::ColorNext)
    );
    // Reasignar un extra (`=` de Acercar) lo quita de Acercar.
    let mut e = editor(pencil(), &default);
    e.begin_capture(Slot::Secondary);
    assert_eq!(
        e.capture(Capture::Press(press('='))),
        CaptureResult::Pending(AppAction::ZoomIn)
    );
    e.resolve(true);
    let t = e.accept();
    assert_eq!(
        slots_of(&t, AppAction::ZoomIn).slots,
        vec![chord('>'), chord('+')]
    );
    assert!(t.conflicts().is_empty());
}

#[test]
fn row_7_the_same_key_in_the_two_boxes_and_what_cannot_be_assigned_are_rejected() {
    let default = Keymap::default_scheme();
    // El Lápiz ya tiene `d`: ponerla en la secundaria es de esta acción.
    let mut e = editor(pencil(), &default);
    e.begin_capture(Slot::Secondary);
    match e.capture(Capture::Press(press('d'))) {
        CaptureResult::Rejected(m) => assert!(m.contains("esta acción"), "{m}"),
        other_2 => panic!("{other_2:?}"),
    }
    assert_eq!(
        e.capturing(),
        Some(Slot::Secondary),
        "la captura sigue armada"
    );
    // Reservadas y combinaciones modernas: rechazadas, captura armada.
    let mut e = editor(pencil(), &default);
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
    assert_eq!(e.accept(), default);
}

#[test]
fn row_7_escape_disarms_the_capture_and_the_chained_conflict() {
    let default = Keymap::default_scheme();
    let mut e = editor(pencil(), &default);
    e.begin_capture(Slot::Primary);
    assert!(e.cancel_capture());
    assert!(!e.cancel_capture(), "ya estaba desarmada");
    assert_eq!(
        e.capture(Capture::Press(press('x'))),
        CaptureResult::Ignored
    );
    e.begin_capture(Slot::Primary);
    assert_eq!(e.capture(Capture::Cancel), CaptureResult::Ignored);
    assert_eq!(e.capturing(), None, "Escape desarma");
    // Con un conflicto pendiente, `resolve(false)` es lo que hace el segundo Escape.
    e.begin_capture(Slot::Primary);
    e.capture(Capture::Press(press('v')));
    assert!(e.pending().is_some() && e.capturing().is_none());
    e.resolve(false);
    assert!(e.pending().is_none());
    assert_eq!(e.accept(), default);
}

// ---------------------------------------------------------------------------------------------
// Fila 8 (parte pura): el formulario del editor
// ---------------------------------------------------------------------------------------------

#[test]
fn row_8_the_editor_form_follows_the_model() {
    let default = Keymap::default_scheme();
    let spec = editor_form();
    assert!(spec.validate().is_ok());
    assert_eq!((spec.accept, spec.cancel), ("Aceptar", Some("Cancelar")));
    let mut dialog = Dialog::new(DialogKind::ShortcutEditor, spec).unwrap();
    let mut e = editor(AppAction::ZoomIn, &default);
    sync_editor_dialog(&mut dialog, &e, Platform::Linux);
    assert_eq!(dialog.button_text(EDITOR_PRIMARY).as_deref(), Some(">"));
    assert_eq!(dialog.button_text(EDITOR_SECONDARY).as_deref(), Some("+"));
    // Sin conflicto, los botones del conflicto están atenuados y el conjunto es válido.
    assert_eq!(dialog.button_disabled(EDITOR_CONFLICT), Some([true, true]));
    assert!(dialog.is_valid());
    // Armada: la casilla lo dice y el mensaje invita a pulsar.
    e.begin_capture(Slot::Primary);
    sync_editor_dialog(&mut dialog, &e, Platform::Linux);
    assert!(
        dialog
            .button_text(EDITOR_PRIMARY)
            .unwrap()
            .contains("pulsa")
    );
    let message = zone(&dialog, EDITOR_MESSAGE);
    assert!(
        message.contains("Acercar") && message.contains("Esc cancela"),
        "{message}"
    );
    // Con un conflicto: botones habilitados, `Accept` rechazado con el motivo.
    e.capture(Capture::Press(press('v')));
    sync_editor_dialog(&mut dialog, &e, Platform::Linux);
    assert_eq!(
        dialog.button_disabled(EDITOR_CONFLICT),
        Some([false, false])
    );
    assert!(!dialog.is_valid());
    assert_eq!(dialog.reason().as_deref(), Some("resuelve el conflicto"));
    assert!(matches!(dialog.event(Event::Accept), Reaction::Rejected(_)));
    // `Reset` es un botón pulsable.
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
    let mut e2 = editor(pencil(), &default);
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
    assert_eq!(
        dialog.button_text(EDITOR_PRIMARY).as_deref(),
        Some("sin atajo")
    );
}

fn zone(dialog: &Dialog, id: &str) -> String {
    use px_ui::dialog::FieldKind;
    match &dialog
        .spec()
        .fields
        .iter()
        .find(|f| f.id == id)
        .unwrap()
        .kind
    {
        FieldKind::ReadOnly { text, .. } => text.clone(),
        other_2 => panic!("{other_2:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Step {
    Open(usize),
    Capture2(u8, bool, bool),
    Remove(bool),
    Reset,
    Resolve(bool),
    Cancel,
    Accept,
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0usize..17).prop_map(Step::Open),
        (0u8..40, any::<bool>(), any::<bool>()).prop_map(|(c, s, sh)| Step::Capture2(c, s, sh)),
        any::<bool>().prop_map(Step::Remove),
        Just(Step::Reset),
        any::<bool>().prop_map(Step::Resolve),
        Just(Step::Cancel),
        Just(Step::Accept),
    ]
}

/// Un carácter asignable: letras, cifras y símbolos.
fn character(n: u8) -> char {
    const C: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789,.;/[]{}<>-=+";
    C[usize::from(n) % C.len()] as char
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 2: tras cualquier secuencia de operaciones del editor, la tabla aceptada no tiene
    /// conflictos, sus filas con `command` son las nueve modernas, cada acción editable tiene sus
    /// casillas, las acciones no tocadas conservan sus filas y `Cancel` no deja huella.
    #[test]
    fn property_2_the_editor_never_corrupts_the_table(
        steps_2 in proptest::collection::vec(step(), 0..24),
        platform_2 in 0usize..3,
    ) {
        let platform = PLATFORMS[platform_2];
        let default = Keymap::default_scheme();
        let mut table = default.clone();
        let mut touched: Vec<AppAction> = Vec::new();
        let mut open_2_2: Option<(AppAction, ShortcutEditor)> = None;
        for step in steps_2 {
            match step {
                Step::Open(i) => {
                    let action = editable()[i];
                    open_2_2 = ShortcutEditor::new(action, &table, platform).map(|e| (action, e));
                }
                Step::Capture2(c, secondary, shift) => {
                    if let Some((_, e)) = &mut open_2_2 {
                        e.begin_capture(if secondary { Slot::Secondary } else { Slot::Primary });
                        let ch = character(c);
                        let p = Press { shift: shift && ch.is_ascii_alphabetic(), ..press(ch) };
                        e.capture(Capture::Press(p));
                    }
                }
                Step::Remove(secondary) => {
                    if let Some((_, e)) = &mut open_2_2 {
                        e.clear(if secondary { Slot::Secondary } else { Slot::Primary });
                    }
                }
                Step::Reset => {
                    if let Some((_, e)) = &mut open_2_2 {
                        e.reset();
                    }
                }
                Step::Resolve(reassign) => {
                    if let Some((_, e)) = &mut open_2_2
                        && e.pending().is_some()
                    {
                        e.resolve(reassign);
                    }
                }
                Step::Cancel => open_2_2 = None,
                Step::Accept => {
                    if let Some((action, e)) = open_2_2.take()
                        && e.can_accept()
                    {
                        let new = e.accept();
                        // El borrador es una tabla válida.
                        prop_assert!(new.conflicts().is_empty());
                        table = new;
                        touched.push(action);
                    }
                }
            }
            if let Some((_, e)) = &open_2_2 {
                prop_assert!(e.keymap().conflicts().is_empty());
            }
        }
        prop_assert!(table.conflicts().is_empty());
        let modern = |k: &Keymap| k.rows().iter().filter(|r| r.command).cloned().collect::<Vec<_>>();
        prop_assert_eq!(modern(&table).len(), 9);
        for row_2 in modern(&default) {
            prop_assert!(modern(&table).contains(&row_2), "{row_2:?}");
        }
        for action in editable() {
            let boxes_2 = slots_of(&table, action).slots;
            // Ninguna casilla repetida dentro de la acción.
            for (i, c) in boxes_2.iter().enumerate() {
                prop_assert!(!boxes_2[..i].contains(c));
            }
            if !touched.contains(&action) {
                // Una acción que ningún `Accept` tocó puede haber perdido una casilla por un
                // `Reasignar` de otra; si no, conserva las suyas.
                let before = slots_of(&default, action).slots;
                prop_assert!(boxes_2.iter().all(|c| before.contains(c)));
            }
        }
        // Lo que se muestra funciona: la `Press` de cada atajo mostrado da su acción.
        for action in editable() {
            if let Some(c) = table.shown(action, platform) {
                let p = Press {
                    key: Some(c.key),
                    command: c.command,
                    shift: c.shift,
                    ..press('a')
                };
                prop_assert_eq!(table.action_for(&p, platform), Some(action));
            }
        }
    }
}
