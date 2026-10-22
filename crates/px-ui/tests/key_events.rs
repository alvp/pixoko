//! Tests de `px_ui::key_events::presses` y su paso por `Keymap::action_for` (specs/interfaz-teclado.md,
//! unidad I.1). En ROJO hasta la lógica: `key_events` y `keymap` no existen todavía.
//!
//! Filas y propiedades de la spec que cubre este fichero:
//!
//! - Fila 4: secuencias de eventos (`Key` + `Text`) a `presses` y `Keymap::action_for`, en las
//!   tres plataformas: español (AltGr+`[`, tecla ISO `<`, `Mayús`+ISO `>`), inglés (`Mayús`+`,`
//!   con `Text("<")`, `Mayús`+`.`), alemán (AltGr+8), macOS español (`Option`+`[`), Dvorak, ruso
//!   (`Key::D` con `Text("в")`), Bloq Mayús (`Text("D")` sin `shift`), AZERTY (`Ctrl` + la tecla
//!   de la cifra 1). Y lo que **no** activa nada: la tecla muerta del acento (`Key::OpenBracket`,
//!   sin texto y sin modificadores), la vocal compuesta (`Key::U` con `Text("ú")`), la composición
//!   (`Key::D` con `Text("´d")`), `Option`+`d` en macOS (`Text("∂")` con `alt`), el IME
//!   (`Event::Ime` de preedición y de confirmación) y un `Text` suelto.
//! - Fila 12 (parte de `presses`): el campo `repeat`; `u`, `]`, `Av Pág` y `<` repetidos dan una
//!   acción por pulsación; los atajos de fichero repetidos llevan `repeat` (los frena
//!   `ignores_repeat`, que se prueba en px-app).
//! - Propiedad 4: emparejado de eventos con proptest (256 casos).
//!
//! Suposiciones sobre lo que la spec no fija (anotadas en el informe):
//! - **Propiedad 4, «más una por cada `Text` suelto».** La frase contradice la decisión 1, la
//!   fila 4 y el visto bueno 2 (que prevalece): un `Text` suelto **no** da `Press`. Aquí
//!   `presses` da exactamente una `Press` por cada `Key` pulsada.
//! - `other_ctrl` es `modifiers.ctrl && !modifiers.command` (el `Ctrl` de macOS).
//! - Para `Key::IntlBackslash` (la tecla ISO `<`/`>` del español y el alemán, sin `Key` propio
//!   para `<` en egui 0.37) se usa esa tecla física; el resultado solo depende del texto.
//! - «Ajena a Latin-1 y Latín extendido» se prueba con el cirílico (`в`, `я`) y el griego (`δ`);
//!   las letras con acento (`ú`, `é`, `ď`) y los símbolos no ASCII (`∂`, `€`) no dan carácter.
//! - Una `Key` sin carácter (por ejemplo `Fin`, `F1`, `Escape`) da una `Press` con `key: None`.
//! - Un `Text` que **no** sigue inmediatamente a una `Key` pulsada (hay otro evento en medio, o
//!   la tecla es una suelta) es un `Text` suelto.

use egui::{Event, ImeEvent, Key, Modifiers, pos2};
use proptest::prelude::*;
use px_ui::actions::{AppAction, Platform};
use px_ui::key_events::presses;
use px_ui::keymap::{KeyId, Keymap, Press};
use px_ui::tool::Tool;

const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

/// La tecla pulsada, con la lógica igual a la física.
fn key_2(key: Key, modifiers: Modifiers) -> Event {
    key_physical(key, key, modifiers)
}

/// Una tecla pulsada con su posición física (Dvorak, ruso…).
fn key_physical(key: Key, physical: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(physical),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn repeated(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: true,
        modifiers,
    }
}

fn releases(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: false,
        repeat: false,
        modifiers,
    }
}

fn text_2(t: &str) -> Event {
    Event::Text(t.to_string())
}

/// `Ctrl` como lo entrega egui en Linux y Windows (`ctrl` y `command`).
const CTRL: Modifiers = Modifiers {
    alt: false,
    ctrl: true,
    shift: false,
    mac_cmd: false,
    command: true,
};
/// `Cmd` de macOS (`mac_cmd` y `command`).
const CMD: Modifiers = Modifiers {
    alt: false,
    ctrl: false,
    shift: false,
    mac_cmd: true,
    command: true,
};
/// El `Ctrl` de macOS: no es `command`.
const CTRL_DE_MAC: Modifiers = Modifiers {
    alt: false,
    ctrl: true,
    shift: false,
    mac_cmd: false,
    command: false,
};

fn press(key: Option<KeyId>, command: bool, shift: bool, alt: bool, other_ctrl: bool) -> Press {
    Press {
        key,
        command,
        shift,
        alt,
        other_ctrl,
        repeat: false,
    }
}

/// Las acciones que salen de `events`, una por cada `Press` (con `None` si no da ninguna).
fn actions(events: &[Event], platform: Platform) -> Vec<Option<AppAction>> {
    let km = Keymap::default_scheme();
    presses(events)
        .iter()
        .map(|p| km.action_for(p, platform))
        .collect()
}

/// Comprueba la secuencia en las tres plataformas.
fn assert_actions(name_2: &str, events: &[Event], expected_2: &[Option<AppAction>]) {
    for platform in PLATFORMS {
        assert_eq!(
            actions(events, platform),
            expected_2,
            "{name_2} en {platform:?}: {events:?}"
        );
    }
}

fn pencil() -> Option<AppAction> {
    Some(AppAction::SelectTool(Tool::Pencil))
}

// ---------------------------------------------------------------------------------------------
// Fila 4: lo que sí activa un atajo
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_spanish_altgr_brackets_give_color_previous_and_next() {
    // AltGr+`[` y AltGr+`]`: la tecla lógica es el corchete y el texto llega justo después. En
    // Linux AltGr no es `Alt`; en Windows `winit` lo quita; con `alt` también debe valer.
    for modifiers in [Modifiers::NONE, Modifiers::ALT] {
        assert_actions(
            "AltGr+[",
            &[key_2(Key::OpenBracket, modifiers), text_2("[")],
            &[Some(AppAction::ColorPrev)],
        );
        assert_actions(
            "AltGr+]",
            &[key_2(Key::CloseBracket, modifiers), text_2("]")],
            &[Some(AppAction::ColorNext)],
        );
    }
}

#[test]
fn row_4_the_bracket_press_carries_the_character_of_the_text() {
    let p = presses(&[key_2(Key::OpenBracket, Modifiers::NONE), text_2("[")]);
    assert_eq!(
        p,
        vec![press(Some(KeyId::Char('[')), false, false, false, false)]
    );
    let p = presses(&[key_2(Key::OpenBracket, Modifiers::ALT), text_2("[")]);
    assert_eq!(
        p,
        vec![press(Some(KeyId::Char('[')), false, false, true, false)]
    );
}

#[test]
fn row_4_spanish_the_iso_key_gives_less_than_and_with_shift_greater_than() {
    assert_actions(
        "< (tecla ISO)",
        &[key_2(Key::IntlBackslash, Modifiers::NONE), text_2("<")],
        &[Some(AppAction::ZoomOut)],
    );
    assert_actions(
        "> (Mayús + tecla ISO)",
        &[key_2(Key::IntlBackslash, Modifiers::SHIFT), text_2(">")],
        &[Some(AppAction::ZoomIn)],
    );
}

#[test]
fn row_4_english_shift_comma_is_less_than_and_shift_period_is_greater_than() {
    assert_actions(
        "Mayús+, con Text(<)",
        &[key_2(Key::Comma, Modifiers::SHIFT), text_2("<")],
        &[Some(AppAction::ZoomOut)],
    );
    assert_actions(
        "Mayús+. con Text(>)",
        &[key_2(Key::Period, Modifiers::SHIFT), text_2(">")],
        &[Some(AppAction::ZoomIn)],
    );
    // Sin `Mayús`, la misma tecla es otra cosa: `,` es el cuentagotas y `.` no es nada.
    assert_actions(
        ",",
        &[key_2(Key::Comma, Modifiers::NONE), text_2(",")],
        &[Some(AppAction::SelectTool(Tool::Dropper))],
    );
    assert_actions(
        ".",
        &[key_2(Key::Period, Modifiers::NONE), text_2(".")],
        &[None],
    );
}

#[test]
fn row_4_the_less_than_with_shift_comma_is_not_the_comma() {
    // La misma tecla física (`,`) da cosas distintas según el texto: se compara el carácter.
    let smaller = presses(&[key_2(Key::Comma, Modifiers::SHIFT), text_2("<")]);
    let comma = presses(&[key_2(Key::Comma, Modifiers::NONE), text_2(",")]);
    assert_eq!(smaller[0].key, Some(KeyId::Char('<')));
    assert_eq!(comma[0].key, Some(KeyId::Char(',')));
}

#[test]
fn row_4_german_altgr_8_and_9_give_the_brackets() {
    // AltGr+8 → `[` y AltGr+9 → `]`: la tecla lógica es la de la cifra o el propio corchete; vale
    // el texto, también con `alt`.
    for modifiers in [Modifiers::NONE, Modifiers::ALT] {
        for (logic, physical) in [(Key::OpenBracket, Key::Num8), (Key::Num8, Key::Num8)] {
            assert_actions(
                "AltGr+8",
                &[key_physical(logic, physical, modifiers), text_2("[")],
                &[Some(AppAction::ColorPrev)],
            );
        }
        for (logic, physical) in [(Key::CloseBracket, Key::Num9), (Key::Num9, Key::Num9)] {
            assert_actions(
                "AltGr+9",
                &[key_physical(logic, physical, modifiers), text_2("]")],
                &[Some(AppAction::ColorNext)],
            );
        }
    }
}

#[test]
fn row_4_macos_spanish_option_bracket_with_alt_gives_color_previous() {
    assert_actions(
        "Option+[",
        &[key_2(Key::OpenBracket, Modifiers::ALT), text_2("[")],
        &[Some(AppAction::ColorPrev)],
    );
    assert_actions(
        "Option+]",
        &[key_2(Key::CloseBracket, Modifiers::ALT), text_2("]")],
        &[Some(AppAction::ColorNext)],
    );
}

#[test]
fn row_4_dvorak_the_d_at_another_physical_position_is_still_the_pencil() {
    // En Dvorak la `d` está donde la `e` de QWERTY.
    assert_actions(
        "d en la posición de la E",
        &[key_physical(Key::D, Key::E, Modifiers::NONE), text_2("d")],
        &[pencil()],
    );
    // Y la tecla física `D` de QWERTY escribe `e`: no es el lápiz.
    assert_actions(
        "la posición de la D escribe e",
        &[key_physical(Key::E, Key::D, Modifiers::NONE), text_2("e")],
        &[None],
    );
    // La coma de Dvorak está donde la `w`.
    assert_actions(
        ", en la posición de la W",
        &[
            key_physical(Key::Comma, Key::W, Modifiers::NONE),
            text_2(","),
        ],
        &[Some(AppAction::SelectTool(Tool::Dropper))],
    );
}

#[test]
fn row_4_russian_key_d_with_cyrillic_text_is_the_pencil_by_position() {
    assert_actions(
        "в sobre la D",
        &[key_2(Key::D, Modifiers::NONE), text_2("в")],
        &[pencil()],
    );
    let p = presses(&[key_2(Key::D, Modifiers::NONE), text_2("в")]);
    assert_eq!(
        p,
        vec![press(Some(KeyId::Char('d')), false, false, false, false)]
    );
    // El griego también (`δ` sobre la D).
    assert_actions(
        "δ sobre la D",
        &[key_2(Key::D, Modifiers::NONE), text_2("δ")],
        &[pencil()],
    );
    // Una letra de otra escritura sobre una tecla sin fila es esa tecla: sin acción.
    assert_actions(
        "я sobre la Z",
        &[key_2(Key::Z, Modifiers::NONE), text_2("я")],
        &[None],
    );
}

#[test]
fn row_4_russian_with_ctrl_the_egui_key_counts() {
    // Con `Ctrl` no hay texto: vale la tecla de egui (que en ruso es la física).
    assert_actions(
        "Ctrl+Z en ruso",
        &[key_2(Key::Z, CTRL)],
        &[Some(AppAction::Undo)],
    );
}

#[test]
fn row_4_caps_lock_text_capital_d_without_shift_is_the_pencil() {
    assert_actions(
        "Text(D) sin shift",
        &[key_2(Key::D, Modifiers::NONE), text_2("D")],
        &[pencil()],
    );
    // Es la misma `Press` que con `Text("d")` sin `shift`.
    let with_capital = presses(&[key_2(Key::D, Modifiers::NONE), text_2("D")]);
    let with_lowercase = presses(&[key_2(Key::D, Modifiers::NONE), text_2("d")]);
    assert_eq!(with_capital, with_lowercase);
    assert_eq!(
        with_capital,
        vec![press(Some(KeyId::Char('d')), false, false, false, false)]
    );
}

#[test]
fn row_4_a_real_shift_with_d_is_not_the_pencil() {
    // `Mayús` pulsada con la `d` (en las letras `shift` es exacto): sin acción.
    assert_actions(
        "Mayús+D",
        &[key_2(Key::D, Modifiers::SHIFT), text_2("D")],
        &[None],
    );
    // Bloq Mayús y `Mayús` a la vez escribe `d`, pero con `shift`: tampoco.
    assert_actions(
        "Bloq Mayús + Mayús + D",
        &[key_2(Key::D, Modifiers::SHIFT), text_2("d")],
        &[None],
    );
}

#[test]
fn row_4_azerty_ctrl_plus_the_key_of_the_digit_1_is_zoom_actual() {
    // En AZERTY el carácter de esa tecla sin `Mayús` es `&`, pero con `Ctrl` no hay texto y
    // vale la tecla de egui: la cifra.
    assert_actions(
        "Ctrl+tecla de la cifra 1",
        &[key_2(Key::Num1, CTRL)],
        &[Some(AppAction::ZoomActualSize)],
    );
    assert_actions(
        "Cmd+tecla de la cifra 1",
        &[key_2(Key::Num1, CMD)],
        &[Some(AppAction::ZoomActualSize)],
    );
    // Sin `Ctrl`, esa tecla escribe `&`: no es ningún atajo.
    assert_actions(
        "& sin Ctrl",
        &[key_2(Key::Num1, Modifiers::NONE), text_2("&")],
        &[None],
    );
    // Con `Mayús` escribe `1`, pero con `shift` y sin `command`: nada.
    assert_actions(
        "1 con Mayús",
        &[key_2(Key::Num1, Modifiers::SHIFT), text_2("1")],
        &[None],
    );
}

#[test]
fn row_4_digits_with_their_text_do_nothing() {
    for (key, figure) in [(Key::Num1, "1"), (Key::Num3, "3"), (Key::Num9, "9")] {
        assert_actions(
            figure,
            &[key_2(key, Modifiers::NONE), text_2(figure)],
            &[None],
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 4: lo que NO activa ningún atajo
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_the_accent_dead_key_without_text_does_not_fire_the_bracket() {
    // La tecla muerta del teclado español es la física de `[`: sin texto y sin modificadores.
    let events = [key_2(Key::OpenBracket, Modifiers::NONE)];
    assert_eq!(
        presses(&events),
        vec![press(None, false, false, false, false)],
        "una Press sin carácter"
    );
    assert_actions("tecla muerta", &events, &[None]);
    // También la del apóstrofo (`'`), que es la otra tecla muerta posible.
    assert_actions(
        "tecla muerta del `'`",
        &[key_2(Key::Quote, Modifiers::NONE)],
        &[None],
    );
    // Y con `Mayús` (la diéresis).
    assert_actions(
        "tecla muerta con Mayús",
        &[key_2(Key::OpenBracket, Modifiers::SHIFT)],
        &[None],
    );
    assert_actions(
        "tecla muerta de `]`",
        &[key_2(Key::CloseBracket, Modifiers::NONE)],
        &[None],
    );
}

#[test]
fn row_4_the_composed_vowel_after_the_dead_key_is_not_the_letter() {
    // `´` y luego `u`: la `Key::U` llega con `Text("ú")`: no es el `u` de deshacer.
    let events = [
        key_2(Key::OpenBracket, Modifiers::NONE),
        key_2(Key::U, Modifiers::NONE),
        text_2("ú"),
    ];
    assert_eq!(presses(&events).len(), 2);
    assert_eq!(presses(&events)[1].key, None, "ú no da carácter");
    assert_actions("´ + u", &events, &[None, None]);
    for (key, composite) in [(Key::E, "é"), (Key::D, "ď"), (Key::A, "á")] {
        assert_actions(
            composite,
            &[key_2(key, Modifiers::NONE), text_2(composite)],
            &[None],
        );
    }
}

#[test]
fn row_4_a_composition_of_several_characters_gives_no_shortcut() {
    let events = [key_2(Key::D, Modifiers::NONE), text_2("´d")];
    assert_eq!(
        presses(&events),
        vec![press(None, false, false, false, false)]
    );
    assert_actions("composición ´d", &events, &[None]);
    assert_actions(
        "dd",
        &[key_2(Key::D, Modifiers::NONE), text_2("dd")],
        &[None],
    );
}

#[test]
fn row_4_option_plus_a_letter_on_macos_writes_a_symbol_and_does_nothing() {
    // `Option`+`d` escribe `∂`: la `Key::D` llega con `alt` y `Text("∂")`.
    let events = [key_2(Key::D, Modifiers::ALT), text_2("∂")];
    assert_eq!(
        presses(&events),
        vec![press(None, false, false, true, false)],
        "un símbolo que no es ASCII no da carácter"
    );
    assert_actions("Option+d", &events, &[None]);
    // Otros símbolos no ASCII de `Option`.
    for symbol in ["ƒ", "ß", "€", "©"] {
        assert_actions(
            symbol,
            &[key_2(Key::F, Modifiers::ALT), text_2(symbol)],
            &[None],
        );
    }
}

#[test]
fn row_4_alt_plus_a_letter_with_its_text_is_not_the_letter() {
    // En Linux y Windows `Alt`+`d` llega con `Text("d")`: la letra con `alt` no hace nada.
    assert_actions(
        "Alt+d",
        &[key_2(Key::D, Modifiers::ALT), text_2("d")],
        &[None],
    );
    let p = presses(&[key_2(Key::D, Modifiers::ALT), text_2("d")]);
    assert_eq!(
        p,
        vec![press(Some(KeyId::Char('d')), false, false, true, false)]
    );
}

#[test]
fn row_4_the_ime_gives_no_press_or_shortcut() {
    let preedicion = Event::Ime(ImeEvent::Preedit {
        text: "d".to_string(),
        active_range_chars: None,
    });
    let confirmation = Event::Ime(ImeEvent::Commit("d".to_string()));
    let empty = Event::Ime(ImeEvent::Preedit {
        text: String::new(),
        active_range_chars: None,
    });
    let deleted = Event::Ime(ImeEvent::DeleteSurrounding {
        before_chars: 1,
        after_chars: 0,
    });
    for events in [
        vec![preedicion.clone()],
        vec![confirmation.clone()],
        vec![empty],
        vec![deleted],
        // La secuencia completa de una composición: preedición, confirmación y el texto suelto.
        vec![preedicion.clone(), confirmation.clone(), text_2("d")],
    ] {
        assert!(presses(&events).is_empty(), "{events:?}");
        assert_actions("IME", &events, &[]);
    }
}

#[test]
fn row_4_the_key_pressed_during_an_ime_composition_gives_no_character() {
    let preedicion = Event::Ime(ImeEvent::Preedit {
        text: "d".to_string(),
        active_range_chars: None,
    });
    let confirmation = Event::Ime(ImeEvent::Commit("d".to_string()));
    let events = [preedicion, key_2(Key::D, Modifiers::NONE), confirmation];
    assert_eq!(
        presses(&events),
        vec![press(None, false, false, false, false)],
        "una Press sin carácter por la `Key`, ninguna por el IME"
    );
    assert_actions("tecla durante el IME", &events, &[None]);
}

#[test]
fn row_4_a_loose_text_gives_no_press() {
    assert!(presses(&[text_2("d")]).is_empty());
    assert!(presses(&[text_2(",")]).is_empty());
    assert_actions("Text(,) suelto", &[text_2(",")], &[]);
    assert_actions("Text(d) suelto", &[text_2("d")], &[]);
    // Tras la suelta de una tecla, el texto también es suelto.
    assert!(presses(&[releases(Key::Comma, Modifiers::NONE), text_2(",")]).is_empty());
    // Un segundo `Text` tras la tecla es suelto: solo el primero se empareja.
    let events = [key_2(Key::D, Modifiers::NONE), text_2("d"), text_2("d")];
    assert_eq!(
        presses(&events),
        vec![press(Some(KeyId::Char('d')), false, false, false, false)]
    );
}

#[test]
fn row_4_a_text_that_does_not_immediately_follow_the_key_is_loose() {
    let events = [
        key_2(Key::D, Modifiers::NONE),
        Event::PointerMoved(pos2(1.0, 1.0)),
        text_2("d"),
    ];
    assert_eq!(
        presses(&events),
        vec![press(None, false, false, false, false)],
        "la tecla queda sin carácter y el texto no da Press"
    );
    assert_actions("texto separado", &events, &[None]);
}

#[test]
fn row_4_other_events_give_no_press() {
    let events = [
        Event::PointerMoved(pos2(1.0, 1.0)),
        Event::WindowFocused(true),
        Event::ModifiersChanged(Modifiers::SHIFT),
        Event::Zoom(1.1),
    ];
    assert!(presses(&events).is_empty());
}

#[test]
fn row_4_the_released_ones_are_ignored() {
    let events = [
        releases(Key::D, Modifiers::NONE),
        key_2(Key::D, Modifiers::NONE),
        text_2("d"),
        releases(Key::D, Modifiers::NONE),
    ];
    assert_eq!(
        presses(&events),
        vec![press(Some(KeyId::Char('d')), false, false, false, false)]
    );
}

#[test]
fn row_4_several_keys_in_a_frame_give_one_press_per_key_and_in_order() {
    let events = [
        key_2(Key::D, Modifiers::NONE),
        text_2("d"),
        key_2(Key::V, Modifiers::NONE),
        text_2("v"),
        key_2(Key::F, Modifiers::NONE),
        text_2("f"),
        key_2(Key::Z, CTRL),
    ];
    assert_actions(
        "d v f Ctrl+Z",
        &events,
        &[
            pencil(),
            Some(AppAction::SelectTool(Tool::Line)),
            Some(AppAction::SelectTool(Tool::Fill)),
            Some(AppAction::Undo),
        ],
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 4: con `command` vale la tecla de egui; teclas con nombre; Ctrl de macOS
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_with_command_the_character_is_that_of_the_egui_key_without_text() {
    for (key, expected) in [
        (Key::Z, Some(KeyId::Char('z'))),
        (Key::Y, Some(KeyId::Char('y'))),
        (Key::O, Some(KeyId::Char('o'))),
        (Key::N, Some(KeyId::Char('n'))),
        (Key::S, Some(KeyId::Char('s'))),
        (Key::Q, Some(KeyId::Char('q'))),
        (Key::Num1, Some(KeyId::Char('1'))),
        (Key::Comma, Some(KeyId::Char(','))),
        (Key::Home, Some(KeyId::Home)),
    ] {
        for modifiers in [CTRL, CMD] {
            let p = presses(&[key_2(key, modifiers)]);
            assert_eq!(p.len(), 1);
            assert_eq!(p[0].key, expected, "{key:?} con {modifiers:?}");
            assert!(
                p[0].command && !p[0].other_ctrl,
                "{key:?} con {modifiers:?}: {:?}",
                p[0]
            );
        }
    }
}

#[test]
fn row_4_the_nine_modern_ones_over_events_on_the_three_platforms() {
    let linux_y_windows = [
        (Key::Z, CTRL, AppAction::Undo),
        (Key::Y, CTRL, AppAction::Redo),
        (Key::Z, CTRL | Modifiers::SHIFT, AppAction::Redo),
        (Key::O, CTRL, AppAction::Open),
        (Key::N, CTRL, AppAction::NewImage),
        (Key::S, CTRL, AppAction::Save),
        (Key::S, CTRL | Modifiers::SHIFT, AppAction::SaveAs),
        (Key::Num1, CTRL, AppAction::ZoomActualSize),
    ];
    for (key, modifiers, expected) in linux_y_windows {
        assert_actions(
            "moderno con Ctrl",
            &[key_2(key, modifiers)],
            &[Some(expected)],
        );
    }
    for (key, modifiers, expected) in [
        (Key::Z, CMD, AppAction::Undo),
        (Key::Z, CMD | Modifiers::SHIFT, AppAction::Redo),
        (Key::S, CMD | Modifiers::SHIFT, AppAction::SaveAs),
        (Key::Num1, CMD, AppAction::ZoomActualSize),
    ] {
        assert_actions(
            "moderno con Cmd",
            &[key_2(key, modifiers)],
            &[Some(expected)],
        );
    }
    // `Ctrl+Q` solo sale en Linux.
    let events = [key_2(Key::Q, CTRL)];
    assert_eq!(actions(&events, Platform::Linux), [Some(AppAction::Quit)]);
    assert_eq!(actions(&events, Platform::Windows), [None]);
    assert_eq!(actions(&events, Platform::Mac), [None]);
}

#[test]
fn row_4_ctrl_alt_and_a_letter_is_not_the_shortcut() {
    // `Ctrl+Alt+Z` (y el AltGr de Windows que llegara como `Ctrl`+`Alt`) no deshace.
    assert_actions(
        "Ctrl+Alt+Z",
        &[key_2(Key::Z, CTRL | Modifiers::ALT)],
        &[None],
    );
}

#[test]
fn row_4_the_macos_ctrl_is_other_ctrl_and_cancels_the_letters() {
    let p = presses(&[key_2(Key::Z, CTRL_DE_MAC)]);
    assert_eq!(p.len(), 1);
    assert!(!p[0].command && p[0].other_ctrl, "{:?}", p[0]);
    assert_actions("Ctrl de macOS + Z", &[key_2(Key::Z, CTRL_DE_MAC)], &[None]);
    // Con texto de una letra, también: `Ctrl` suelto + `d` no es el lápiz.
    let with_text = [key_2(Key::D, CTRL_DE_MAC), text_2("d")];
    let p = presses(&with_text);
    assert!(p[0].other_ctrl);
    assert_actions("Ctrl de macOS + d", &with_text, &[None]);
}

#[test]
fn row_4_named_keys_always_count_by_the_key_and_with_any_modifier() {
    for modifiers in [
        Modifiers::NONE,
        Modifiers::SHIFT,
        Modifiers::ALT,
        Modifiers::SHIFT | Modifiers::ALT,
        CTRL_DE_MAC,
    ] {
        assert_actions(
            "Inicio",
            &[key_2(Key::Home, modifiers)],
            &[Some(AppAction::ZoomFit)],
        );
        // I.2a: con `Mayús` son el color de fondo; sin ella, el primer plano.
        let (previous, next) = if modifiers.shift {
            (AppAction::BackgroundPrev, AppAction::BackgroundNext)
        } else {
            (AppAction::ColorPrev, AppAction::ColorNext)
        };
        assert_actions(
            "Re Pág",
            &[key_2(Key::PageUp, modifiers)],
            &[Some(previous)],
        );
        assert_actions("Av Pág", &[key_2(Key::PageDown, modifiers)], &[Some(next)]);
    }
    // Con `command` no hay acción.
    assert_actions("Ctrl+Inicio", &[key_2(Key::Home, CTRL)], &[None]);
    assert_actions("Ctrl+Re Pág", &[key_2(Key::PageUp, CTRL)], &[None]);
    assert_actions("Cmd+Av Pág", &[key_2(Key::PageDown, CMD)], &[None]);
    // Las pulsaciones de teclas con nombre llevan la tecla aunque no haya texto.
    assert_eq!(
        presses(&[key_2(Key::PageDown, Modifiers::NONE)]),
        vec![press(Some(KeyId::PageDown), false, false, false, false)]
    );
}

#[test]
fn row_4_keys_without_a_character_give_a_press_without_a_key() {
    for key in [Key::End, Key::F1, Key::Escape, Key::Tab] {
        assert_eq!(
            presses(&[key_2(key, Modifiers::NONE)]),
            vec![press(None, false, false, false, false)],
            "{key:?}"
        );
        assert_actions("sin carácter", &[key_2(key, Modifiers::NONE)], &[None]);
    }
}

#[test]
fn row_4_the_modifiers_pass_to_the_fields_of_the_press() {
    let p = presses(&[key_2(Key::Home, Modifiers::SHIFT | Modifiers::ALT)]);
    assert_eq!(p, vec![press(Some(KeyId::Home), false, true, true, false)]);
    let p = presses(&[key_2(Key::Home, CTRL)]);
    assert_eq!(p, vec![press(Some(KeyId::Home), true, false, false, false)]);
    let p = presses(&[key_2(Key::Home, CTRL_DE_MAC)]);
    assert_eq!(p, vec![press(Some(KeyId::Home), false, false, false, true)]);
}

// ---------------------------------------------------------------------------------------------
// Fila 12 (parte de presses): repetición
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_presses_marks_repeat_on_the_repetitions_and_not_on_the_first() {
    let events = [
        key_2(Key::U, Modifiers::NONE),
        text_2("u"),
        repeated(Key::U, Modifiers::NONE),
        text_2("u"),
        repeated(Key::U, Modifiers::NONE),
        text_2("u"),
    ];
    let p = presses(&events);
    assert_eq!(p.len(), 3);
    assert_eq!(
        p.iter().map(|p| p.repeat).collect::<Vec<_>>(),
        [false, true, true]
    );
    for p in &p {
        assert_eq!(p.key, Some(KeyId::Char('u')));
    }
}

#[test]
fn row_12_u_bracket_page_down_and_smaller_repeated_give_one_action_per_press() {
    let cases: Vec<(&str, Vec<Event>, AppAction)> = vec![
        (
            "u",
            vec![
                key_2(Key::U, Modifiers::NONE),
                text_2("u"),
                repeated(Key::U, Modifiers::NONE),
                text_2("u"),
                repeated(Key::U, Modifiers::NONE),
                text_2("u"),
                repeated(Key::U, Modifiers::NONE),
                text_2("u"),
            ],
            AppAction::Undo,
        ),
        (
            "]",
            vec![
                key_2(Key::CloseBracket, Modifiers::NONE),
                text_2("]"),
                repeated(Key::CloseBracket, Modifiers::NONE),
                text_2("]"),
                repeated(Key::CloseBracket, Modifiers::NONE),
                text_2("]"),
                repeated(Key::CloseBracket, Modifiers::NONE),
                text_2("]"),
            ],
            AppAction::ColorNext,
        ),
        (
            "Av Pág",
            vec![
                key_2(Key::PageDown, Modifiers::NONE),
                repeated(Key::PageDown, Modifiers::NONE),
                repeated(Key::PageDown, Modifiers::NONE),
                repeated(Key::PageDown, Modifiers::NONE),
            ],
            AppAction::ColorNext,
        ),
        (
            "<",
            vec![
                key_2(Key::IntlBackslash, Modifiers::NONE),
                text_2("<"),
                repeated(Key::IntlBackslash, Modifiers::NONE),
                text_2("<"),
                repeated(Key::IntlBackslash, Modifiers::NONE),
                text_2("<"),
                repeated(Key::IntlBackslash, Modifiers::NONE),
                text_2("<"),
            ],
            AppAction::ZoomOut,
        ),
    ];
    for (name_2, events, expected) in cases {
        for platform in PLATFORMS {
            let actions = actions(&events, platform);
            assert_eq!(
                actions,
                vec![Some(expected); 4],
                "{name_2} repetida en {platform:?}"
            );
            assert!(!expected.ignores_repeat(), "{name_2}: {expected:?} repite");
        }
    }
}

#[test]
fn row_12_the_held_file_shortcuts_arrive_with_repeat_and_are_of_those_that_do_not_repeat() {
    for (key, modifiers, expected) in [
        (Key::O, CTRL, AppAction::Open),
        (Key::S, CTRL, AppAction::Save),
        (Key::S, CTRL | Modifiers::SHIFT, AppAction::SaveAs),
        (Key::N, CTRL, AppAction::NewImage),
        (Key::Q, CTRL, AppAction::Quit),
    ] {
        let events = [key_2(key, modifiers), repeated(key, modifiers)];
        let p = presses(&events);
        assert_eq!(
            p.iter().map(|p| p.repeat).collect::<Vec<_>>(),
            [false, true],
            "{expected:?}"
        );
        assert_eq!(
            actions(&events, Platform::Linux),
            vec![Some(expected); 2],
            "{expected:?}"
        );
        assert!(expected.ignores_repeat(), "{expected:?} no repite");
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 4: emparejado de eventos
// ---------------------------------------------------------------------------------------------

/// Las teclas de la propiedad y el carácter que da su tecla de egui (`None`: sin carácter).
const KEYS: [(Key, Option<KeyId>); 12] = [
    (Key::D, Some(KeyId::Char('d'))),
    (Key::U, Some(KeyId::Char('u'))),
    (Key::Z, Some(KeyId::Char('z'))),
    (Key::Num1, Some(KeyId::Char('1'))),
    (Key::Comma, Some(KeyId::Char(','))),
    (Key::OpenBracket, Some(KeyId::Char('['))),
    (Key::Home, Some(KeyId::Home)),
    (Key::PageUp, Some(KeyId::PageUp)),
    (Key::PageDown, Some(KeyId::PageDown)),
    (Key::End, None),
    (Key::F1, None),
    (Key::Escape, None),
];

/// Los textos de la propiedad: ASCII, mayúsculas, varios caracteres, acentos, símbolos no ASCII y
/// letras de otras escrituras.
const TEST_TEXTS: [&str; 16] = [
    "d", "D", ",", "<", ">", "[", "1", "ú", "´d", "dd", "∂", "в", "δ", "é", "€", "я",
];

#[derive(Clone, Debug)]
enum TestEvent {
    TestKey {
        index_2: usize,
        pressed_2: bool,
        repeat: bool,
        command: bool,
        ctrl_loose: bool,
        shift: bool,
        alt: bool,
    },
    Text(usize),
    Ime(usize),
}

fn modifiers_of(command: bool, ctrl_loose: bool, shift: bool, alt: bool) -> Modifiers {
    Modifiers {
        alt,
        ctrl: command || ctrl_loose,
        shift,
        mac_cmd: false,
        command,
    }
}

fn a_egui(event: &TestEvent) -> Event {
    match event {
        TestEvent::TestKey {
            index_2,
            pressed_2,
            repeat,
            command,
            ctrl_loose,
            shift,
            alt,
        } => Event::Key {
            key: KEYS[*index_2].0,
            physical_key: Some(KEYS[*index_2].0),
            pressed: *pressed_2,
            repeat: *pressed_2 && *repeat,
            modifiers: modifiers_of(*command, *ctrl_loose && !*command, *shift, *alt),
        },
        TestEvent::Text(i) => Event::Text(TEST_TEXTS[*i].to_string()),
        TestEvent::Ime(i) => match i % 3 {
            0 => Event::Ime(ImeEvent::Preedit {
                text: TEST_TEXTS[*i].to_string(),
                active_range_chars: None,
            }),
            1 => Event::Ime(ImeEvent::Commit(TEST_TEXTS[*i].to_string())),
            _ => Event::Ime(ImeEvent::DeleteSurrounding {
                before_chars: 1,
                after_chars: 0,
            }),
        },
    }
}

fn event_any() -> impl Strategy<Value = TestEvent> {
    prop_oneof![
        4 => (
            0..KEYS.len(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
        )
            .prop_map(|(index_2, pressed_2, repeat, command, ctrl_loose, shift, alt)| {
                TestEvent::TestKey {
                    index_2,
                    pressed_2,
                    repeat,
                    command,
                    ctrl_loose,
                    shift,
                    alt,
                }
            }),
        3 => (0..TEST_TEXTS.len()).prop_map(TestEvent::Text),
        1 => (0..TEST_TEXTS.len()).prop_map(TestEvent::Ime),
    ]
}

/// Si `c` es una letra de otra escritura: alfabética y fuera de Latin-1 y Latín extendido.
fn is_of_another_script(ch: char) -> bool {
    ch.is_alphabetic() && ch > '\u{24F}'
}

/// El carácter esperado de una `Press` (oráculo escrito a mano): teclas con nombre por la tecla;
/// con `command`, la tecla de egui; sin él, solo el texto que le sigue inmediatamente, si es un
/// único carácter ASCII imprimible (en minúscula) o, como respaldo por posición, una letra de
/// otra escritura (entonces, la tecla de egui).
fn character_expected(index_2: usize, command: bool, text_next: Option<&str>) -> Option<KeyId> {
    let of_the_key = KEYS[index_2].1;
    if matches!(
        of_the_key,
        Some(KeyId::Home | KeyId::PageUp | KeyId::PageDown)
    ) {
        return of_the_key;
    }
    if command {
        return of_the_key;
    }
    let text_2 = text_next?;
    let mut chars = text_2.chars();
    let (Some(ch), None) = (chars.next(), chars.next()) else {
        return None;
    };
    if ch.is_ascii_graphic() {
        return Some(KeyId::Char(ch.to_ascii_lowercase()));
    }
    if is_of_another_script(ch) {
        return of_the_key;
    }
    None
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 4: `presses` no entra en pánico; da una `Press` por cada `Key` pulsada, en
    /// orden; ignora las soltadas; un `Text` suelto o un `Event::Ime` no dan `Press`; un `Text`
    /// de varios caracteres, de un símbolo no ASCII o de una letra latina con acento (y una tecla
    /// sin texto ni `command`) no dan carácter, y solo una letra de otra escritura cae al de la
    /// tecla.
    #[test]
    fn property_4_event_pairing(
        events_2 in prop::collection::vec(event_any(), 0..24),
    ) {
        let egui: Vec<Event> = events_2.iter().map(a_egui).collect();
        let obtained = presses(&egui);

        // Lo esperado, recorriendo la secuencia con las reglas de la spec.
        let mut expected_2_2: Vec<Press> = Vec::new();
        for (i, event) in events_2.iter().enumerate() {
            let TestEvent::TestKey { index_2, pressed_2: true, repeat, command, ctrl_loose, shift, alt } = event else {
                continue;
            };
            let next = match events_2.get(i + 1) {
                Some(TestEvent::Text(t)) => Some(TEST_TEXTS[*t]),
                _ => None,
            };
            expected_2_2.push(Press {
                key: character_expected(*index_2, *command, next),
                command: *command,
                shift: *shift,
                alt: *alt,
                other_ctrl: *ctrl_loose && !*command,
                repeat: *repeat,
            });
        }
        prop_assert_eq!(obtained.len(), expected_2_2.len(), "una Press por cada Key pulsada: {:?}", egui);
        prop_assert_eq!(obtained, expected_2_2, "{:?}", egui);
    }

    /// Propiedad 4, última cláusula: `Text("D")` sin `shift` da la misma `Press` que `Text("d")`
    /// sin `shift`, con cualquier otro modificador y estando entre otros eventos.
    #[test]
    fn property_4_text_capital_d_without_shift_is_text_lowercase_d(
        alt in any::<bool>(),
        ctrl_loose in any::<bool>(),
        repeat in any::<bool>(),
        before in prop::collection::vec(event_any(), 0..6),
    ) {
        let modifiers = modifiers_of(false, ctrl_loose, false, alt);
        let with = |t: &str| {
            let mut v: Vec<Event> = before.iter().map(a_egui).collect();
            let mut k = key_2(Key::D, modifiers);
            if let Event::Key { repeat: r, .. } = &mut k {
                *r = repeat;
            }
            v.push(k);
            v.push(text_2(t));
            presses(&v)
        };
        prop_assert_eq!(with("D"), with("d"));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 4: no entra en pánico con ninguna tecla de `Key::ALL`, pulsada o soltada, con
    /// ningún texto ni modificadores.
    #[test]
    fn property_4_without_panic_with_any_egui_key(
        index_2 in 0..Key::ALL.len(),
        pressed_2 in any::<bool>(),
        bits in 0u8..32,
        with_text in prop::option::of(0..TEST_TEXTS.len()),
    ) {
        let m = Modifiers {
            alt: bits & 1 != 0,
            ctrl: bits & 2 != 0,
            shift: bits & 4 != 0,
            mac_cmd: bits & 8 != 0,
            command: bits & 16 != 0,
        };
        let mut events = vec![Event::Key {
            key: Key::ALL[index_2],
            physical_key: None,
            pressed: pressed_2,
            repeat: false,
            modifiers: m,
        }];
        if let Some(t) = with_text {
            events.push(text_2(TEST_TEXTS[t]));
        }
        let p = presses(&events);
        prop_assert_eq!(p.len(), usize::from(pressed_2), "{:?}", events);
    }
}
