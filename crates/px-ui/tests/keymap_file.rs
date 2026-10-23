//! Tests del fichero de atajos (specs/interfaz-ayuda.md, unidad I.5, decisiones 9 y 10 y las
//! condiciones 1 y 4 del visto bueno). En ROJO hasta la lógica: `keymap_file` no existe todavía.
//!
//! - Fila 9: formato y lectura (`to_text`, `parse`, gramática de los atajos).
//! - Fila 10: tolerancia (bytes malos, ficheros ilegibles, choques a mano, aviso).
//! - Condición 1: ante un choque con un atajo por defecto nuevo, gana el del usuario y nunca se le
//!   quita nada.
//! - Propiedad 1a: bytes aleatorios (500 casos), sin pánico y con lo construido acotado.
//! - Propiedad 1b: ida y vuelta con todas las combinaciones asignables y con secuencias del editor.

use proptest::prelude::*;
use px_ui::actions::{AppAction, Platform};
use px_ui::help_texts::{action_of_key, help_table, is_editable};
use px_ui::key_events::Capture;
use px_ui::keymap::{Binding, Chord, KeyId, Keymap, Platforms, Press};
use px_ui::keymap_file::{Fatal, Loaded, MAX_BYTES, MAX_LINES, load_notice, parse, to_text};
use px_ui::shortcut_editor::{CaptureResult, ShortcutEditor, Slot, slots_of};
use px_ui::tool::Tool;

fn default() -> Keymap {
    Keymap::default_scheme()
}

fn load_text(text_2: &str) -> Loaded {
    parse(text_2.as_bytes(), &default())
}

fn chord(c: char) -> Chord {
    Chord {
        key: KeyId::Char(c),
        command: false,
        shift: false,
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

fn fill() -> AppAction {
    AppAction::SelectTool(Tool::Fill)
}

fn editable() -> Vec<AppAction> {
    help_table()
        .iter()
        .filter_map(|e| action_of_key(e.key))
        .filter(|a| is_editable(*a))
        .collect()
}

/// Asigna una `Press` a una casilla de una acción, resolviendo un conflicto.
fn assigns(table: &Keymap, action: AppAction, slot: Slot, p: Press, reassign: bool) -> Keymap {
    let mut e = ShortcutEditor::new(action, table, Platform::Linux).unwrap();
    e.begin_capture(slot);
    if let CaptureResult::Pending(_) = e.capture(Capture::Press(p)) {
        e.resolve(reassign);
    }
    e.accept()
}

/// Los atajos asignables: letras (con y sin `Mayús`), cifras, símbolos y las teclas con nombre.
fn shortcuts_assignable() -> Vec<Press> {
    let mut v = Vec::new();
    for c in b' '..=b'~' {
        let c = c as char;
        if !c.is_ascii_graphic() || c.is_ascii_uppercase() {
            continue;
        }
        v.push(press(c));
        if c.is_ascii_lowercase() {
            v.push(Press {
                shift: true,
                ..press(c)
            });
        }
    }
    for key in [KeyId::Home, KeyId::PageUp, KeyId::PageDown] {
        v.push(Press {
            key: Some(key),
            ..press('a')
        });
        if key != KeyId::Home {
            v.push(Press {
                key: Some(key),
                shift: true,
                ..press('a')
            });
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------
// Fila 9: formato y lectura
// ---------------------------------------------------------------------------------------------

#[test]
fn row_9_the_default_table_gives_only_the_header() {
    let text_2 = to_text(&default(), &default());
    let useful: Vec<&str> = text_2
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert_eq!(useful, ["pixoko-atajos 1"], "{text_2}");
    assert_eq!(text_2.lines().next(), Some("pixoko-atajos 1"));
    let loaded = load_text(&text_2);
    assert_eq!(loaded.keymap, default());
    assert_eq!((loaded.ignored_lines, loaded.dropped_rows), (0, 0));
    assert!(loaded.fatal.is_none() && loaded.conflicted.is_empty());
    assert_eq!(load_notice(&loaded), None);
}

#[test]
fn row_9_only_what_differs_is_written_compacted_and_in_order() {
    // Lápiz: `d` pasa a `x`; Siguiente color: se quita la principal; Borrar página: `Mayús+K`.
    let mut t = assigns(&default(), pencil(), Slot::Primary, press('x'), false);
    let mut e = ShortcutEditor::new(AppAction::ColorNext, &t, Platform::Linux).unwrap();
    e.clear(Slot::Primary);
    t = e.accept();
    let text_2 = to_text(&t, &default());
    let lines: Vec<&str> = text_2
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert_eq!(
        lines,
        [
            "pixoko-atajos 1",
            "tool.pencil 1 x",
            // Compactada: la secundaria (`Av Pág`) sube y la casilla 2 queda vacía.
            "color.next 1 pagedown",
            "color.next 2 none",
        ],
        "{text_2}"
    );
    assert_eq!(load_text(&text_2).keymap, t, "ida y vuelta exacta");
}

#[test]
fn row_9_the_grammar_of_the_shortcuts() {
    let valid = |shortcut_2: &str| {
        let l = load_text(&format!("pixoko-atajos 1\ntool.pencil 1 {shortcut_2}\n"));
        (l.ignored_lines, slots_of(&l.keymap, pencil()).slots)
    };
    assert_eq!(valid("x"), (0, vec![chord('x')]));
    assert_eq!(valid(">"), (0, vec![chord('>')]));
    assert_eq!(valid("["), (0, vec![chord('[')]));
    assert_eq!(valid("+"), (0, vec![chord('+')]));
    assert_eq!(valid("3"), (0, vec![chord('3')]));
    assert_eq!(
        valid("shift+k"),
        (
            0,
            vec![Chord {
                key: KeyId::Char('k'),
                command: false,
                shift: true
            }]
        )
    );
    assert_eq!(
        valid("pagedown"),
        (
            0,
            vec![Chord {
                key: KeyId::PageDown,
                command: false,
                shift: false
            }]
        )
    );
    assert_eq!(
        valid("shift+pageup").1,
        vec![Chord {
            key: KeyId::PageUp,
            command: false,
            shift: true
        }]
    );
    assert_eq!(valid("home").1.len(), 1);
    assert_eq!(valid("none"), (0, vec![]), "none vacía la casilla");
    // Mal formados o no asignables: la línea se ignora y la tabla queda como estaba.
    for bad in [
        "X",
        "shift+",
        "shift+x ",
        "shift+>",
        "shift+home",
        "ctrl+x",
        "f1",
        "F1",
        "escape",
        "space",
        "xx",
        "é",
        "",
        "none none",
    ] {
        let l = load_text(&format!("pixoko-atajos 1\ntool.pencil 1 {bad}\n"));
        if bad == "shift+x " {
            // El espacio final se recorta: es `shift+x`, válida.
            assert_eq!(l.ignored_lines, 0);
            continue;
        }
        assert_eq!(l.ignored_lines, 1, "{bad:?}");
        assert_eq!(l.keymap, default(), "{bad:?}");
    }
    // Tabuladores, CRLF, comentarios y líneas vacías valen.
    let l = load_text("pixoko-atajos 1\r\n# nota\r\n\r\ntool.pencil\t1\tx\r\n");
    assert_eq!(
        (l.ignored_lines, slots_of(&l.keymap, pencil()).slots),
        (0, vec![chord('x')])
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 10: tolerancia
// ---------------------------------------------------------------------------------------------

#[test]
fn row_10_the_fatal_cases_give_the_default_table_and_its_notice() {
    let unreadable = parse(&[0xff, 0xfe, 0x80], &default());
    assert_eq!(unreadable.fatal, Some(Fatal::Unreadable));
    assert_eq!(unreadable.keymap, default());
    assert_eq!(
        load_notice(&unreadable).as_deref(),
        Some("atajos: fichero ilegible, se usan los de fábrica")
    );
    let version = load_text("pixoko-atajos 2\ntool.pencil 1 x\n");
    assert_eq!(version.fatal, Some(Fatal::Version));
    assert_eq!(version.keymap, default());
    assert_eq!(
        load_notice(&version).as_deref(),
        Some("atajos: versión desconocida, se usan los de fábrica")
    );
    // Sin cabecera: se ignora entero.
    assert!(load_text("tool.pencil 1 x\n").fatal.is_some());
    assert!(load_text("").fatal.is_some());
    // Demasiado grande y demasiadas líneas.
    let large = vec![b'#'; MAX_BYTES + 1];
    assert_eq!(parse(&large, &default()).fatal, Some(Fatal::TooBig));
    let mut many = String::from("pixoko-atajos 1\n");
    for _ in 0..=MAX_LINES {
        many.push_str("tool.pencil 1 x\n");
    }
    assert_eq!(load_text(&many).fatal, Some(Fatal::TooBig));
    for l in [&unreadable, &version] {
        assert!(load_notice(l).unwrap().chars().count() <= 60);
    }
}

#[test]
fn row_10_bad_lines_are_ignored_and_good_ones_are_applied() {
    let l = load_text(
        "pixoko-atajos 1\n\
        tool.pencil 1 x\n\
        file.save 1 g\n\
        no.existe 1 g\n\
        tool.line 3 none\n\
        tool.line 0 q\n\
        tool.line 1 ??\n\
        tool.fill 1 w\n\
        tool.fill 2 e\n\
        tool.fill 2 r\n",
    );
    // Ignoradas: acción fija, clave desconocida, casilla 0, atajo mal formado y la segunda línea de
    // la casilla 2 de Relleno (gana la primera); la casilla 3 de Línea no existe pero es válida
    // (vacía un extra que no hay) y no cuenta como mala.
    assert_eq!(l.ignored_lines, 5, "{l:?}");
    assert_eq!(slots_of(&l.keymap, pencil()).slots, vec![chord('x')]);
    assert_eq!(
        slots_of(&l.keymap, fill()).slots,
        vec![chord('w'), chord('e')]
    );
    assert!(l.keymap.conflicts().is_empty());
    assert_eq!(
        load_notice(&l).as_deref(),
        Some("atajos: 5 líneas ignoradas")
    );
}

#[test]
fn row_10_a_hand_made_clash_the_first_row_wins_and_it_is_counted() {
    // Lápiz con `f` (la de Relleno): en la tabla el Lápiz va antes, gana el Lápiz y a Relleno se le
    // quita; las dos son del usuario si Relleno también se escribe.
    let l = load_text("pixoko-atajos 1\ntool.pencil 1 f\ntool.fill 1 f\n");
    assert!(l.keymap.conflicts().is_empty());
    assert_eq!(slots_of(&l.keymap, pencil()).slots, vec![chord('f')]);
    assert_eq!(
        slots_of(&l.keymap, fill()).slots,
        vec![],
        "pierde la segunda"
    );
    assert_eq!(l.dropped_rows, 1);
    assert!(l.conflicted.contains(&fill()));
    let notice = load_notice(&l).unwrap();
    assert_eq!(notice, "atajos: 1 repetidos");
    assert!(notice.chars().count() <= 60);
    let the_two = load_text(
        "pixoko-atajos 1\ntool.pencil 1 x\ntool.line 1 y\ntool.fill 1 q\ntool.fill 2 q\n",
    );
    // La misma tecla dos veces en una acción: queda una.
    assert_eq!(slots_of(&the_two.keymap, fill()).slots, vec![chord('q')]);
    assert!(the_two.keymap.conflicts().is_empty());
    assert_eq!(
        load_notice(&load_text("pixoko-atajos 1\ntool.fill 9 x\n")).as_deref(),
        Some("atajos: 1 línea ignorada")
    );
}

// ---------------------------------------------------------------------------------------------
// Condición 1: siempre gana el atajo del usuario
// ---------------------------------------------------------------------------------------------

#[test]
fn condition_1_a_new_default_shortcut_never_removes_the_users() {
    // Esquema «de una versión futura»: el Relleno estrena la `x`, que el usuario ya tiene en el Lápiz.
    let saved = to_text(
        &assigns(&default(), pencil(), Slot::Primary, press('x'), false),
        &default(),
    );
    let mut rows_2: Vec<Binding> = default().rows().to_vec();
    for row_2 in &mut rows_2 {
        if row_2.action == fill() {
            row_2.key = KeyId::Char('x');
        }
    }
    // El Relleno nuevo va ANTES que el Lápiz en la tabla: la «primera fila» sería la suya.
    let (fill_rows, rest_2): (Vec<_>, Vec<_>) =
        rows_2.into_iter().partition(|f| f.action == fill());
    let new_default = Keymap::new(fill_rows.into_iter().chain(rest_2).collect());
    let l = parse(saved.as_bytes(), &new_default);
    assert_eq!(
        slots_of(&l.keymap, pencil()).slots,
        vec![chord('x')],
        "el atajo del usuario se conserva"
    );
    assert_eq!(
        slots_of(&l.keymap, fill()).slots,
        vec![],
        "la acción nueva queda sin atajo"
    );
    assert!(l.keymap.conflicts().is_empty());
    assert_eq!(l.conflicted, vec![fill()], "marca `conflicto`");
    assert_eq!(l.dropped_rows, 1);
    assert_eq!(load_notice(&l).as_deref(), Some("atajos: 1 repetidos"));
    // Lo guardado después sigue diciendo lo del usuario.
    let rereads = parse(to_text(&l.keymap, &new_default).as_bytes(), &new_default);
    assert_eq!(slots_of(&rereads.keymap, pencil()).slots, vec![chord('x')]);
    assert!(rereads.keymap.conflicts().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Propiedades
// ---------------------------------------------------------------------------------------------

/// La cota de lo que `parse` construye: las filas por defecto más dos por acción editable (más un
/// margen de casillas por acción), nunca más.
fn bound_of_rows() -> usize {
    default().rows().len() + 8 * editable().len()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Propiedad 1a: bytes aleatorios (a veces con la cabecera y líneas casi válidas): `parse` no
    /// entra en pánico, lo construido está acotado, no hay conflictos y las nueve filas con
    /// `command` siguen intactas.
    #[test]
    fn property_1a_random_bytes_do_not_panic_or_reserve_too_much(
        bytes in proptest::collection::vec(any::<u8>(), 0..4096),
        with_header in any::<bool>(),
        fill in 0usize..200_000,
    ) {
        let mut input_2 = Vec::new();
        if with_header {
            input_2.extend_from_slice(b"pixoko-atajos 1\n");
        }
        input_2.extend_from_slice(&bytes);
        // Y una entrada enorme de una sola línea: se rechaza o se ignora sin recorrerla entera.
        let mut huge = input_2.clone();
        huge.extend(std::iter::repeat_n(b'a', fill));
        for e in [&input_2, &huge] {
            let l = parse(e, &default());
            prop_assert!(l.keymap.rows().len() <= bound_of_rows());
            prop_assert!(l.keymap.conflicts().is_empty());
            let modern: Vec<_> = l.keymap.rows().iter().filter(|r| r.command).collect();
            prop_assert_eq!(modern.len(), 9);
            prop_assert!(l.ignored_lines <= MAX_LINES + 1);
            if e.len() > MAX_BYTES {
                prop_assert_eq!(l.fatal, Some(Fatal::TooBig));
                prop_assert_eq!(&l.keymap, &default());
            }
            if l.fatal.is_some() {
                prop_assert_eq!(&l.keymap, &default());
                prop_assert!(load_notice(&l).is_some());
            }
            if let Some(notice) = load_notice(&l) {
                prop_assert!(notice.chars().count() <= 60);
            }
        }
    }

    /// Propiedad 1b (secuencias): lo que se escribe se lee igual, para cualquier tabla alcanzable
    /// desde el editor.
    #[test]
    fn property_1b_round_trip_of_any_reachable_table(
        steps_2 in proptest::collection::vec(
            (0usize..17, any::<bool>(), 0usize..120, any::<bool>(), any::<bool>()),
            0..16,
        ),
    ) {
        let shortcuts = shortcuts_assignable();
        let actions = editable();
        let mut table = default();
        for (action_2, secondary, shortcut_2, reassign, remove) in steps_2 {
            let action = actions[action_2 % actions.len()];
            let mut e = ShortcutEditor::new(action, &table, Platform::Linux).unwrap();
            let slot = if secondary { Slot::Secondary } else { Slot::Primary };
            if remove {
                e.clear(slot);
            } else {
                e.begin_capture(slot);
                if let CaptureResult::Pending(_) =
                    e.capture(Capture::Press(shortcuts[shortcut_2 % shortcuts.len()]))
                {
                    e.resolve(reassign);
                }
            }
            table = e.accept();
        }
        let text_2 = to_text(&table, &default());
        let l = parse(text_2.as_bytes(), &default());
        prop_assert_eq!((l.ignored_lines, l.dropped_rows), (0, 0), "{}", text_2);
        prop_assert!(l.fatal.is_none());
        prop_assert_eq!(&l.keymap, &table, "{}", text_2);
    }
}

/// Propiedad 1b (todas las combinaciones): cada acción editable con cada atajo asignable en cada
/// una de las dos casillas se escribe y se lee igual (con la reasignación si choca).
#[test]
fn property_1b_round_trip_with_all_the_combinations_of_the_table() {
    let shortcuts = shortcuts_assignable();
    assert!(shortcuts.len() > 90);
    let mut checked = 0usize;
    for action in editable() {
        for slot in [Slot::Primary, Slot::Secondary] {
            for p in &shortcuts {
                let table = assigns(&default(), action, slot, *p, true);
                let text_2 = to_text(&table, &default());
                let l = parse(text_2.as_bytes(), &default());
                assert_eq!(
                    (l.ignored_lines, l.dropped_rows, l.fatal),
                    (0, 0, None),
                    "{action:?} {slot:?} {p:?}\n{text_2}"
                );
                assert_eq!(l.keymap, table, "{action:?} {slot:?} {p:?}\n{text_2}");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 33 * 2 * shortcuts.len());
}

/// Los atajos mostrados siguen funcionando tras leer un fichero cualquiera (la tabla leída es una
/// tabla válida de verdad).
#[test]
fn what_was_read_works_as_a_table() {
    let l = load_text("pixoko-atajos 1\ntool.pencil 1 x\ntool.line 1 y\n");
    for (c, expected_2) in [
        ('x', Some(pencil())),
        ('y', Some(line())),
        ('d', None),
        ('v', None),
    ] {
        assert_eq!(
            l.keymap.action_for(&press(c), Platform::Linux),
            expected_2,
            "{c}"
        );
    }
    let _ = Platforms::All;
}
