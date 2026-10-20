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

const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

/// La tecla pulsada, con la lógica igual a la física.
fn tecla(key: Key, modifiers: Modifiers) -> Event {
    tecla_fisica(key, key, modifiers)
}

/// Una tecla pulsada con su posición física (Dvorak, ruso…).
fn tecla_fisica(key: Key, fisica: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(fisica),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn repetida(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: true,
        modifiers,
    }
}

fn suelta(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: false,
        repeat: false,
        modifiers,
    }
}

fn texto(t: &str) -> Event {
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
fn acciones(events: &[Event], platform: Platform) -> Vec<Option<AppAction>> {
    let km = Keymap::default_scheme();
    presses(events)
        .iter()
        .map(|p| km.action_for(p, platform))
        .collect()
}

/// Comprueba la secuencia en las tres plataformas.
fn assert_acciones(nombre: &str, events: &[Event], esperado: &[Option<AppAction>]) {
    for platform in PLATAFORMAS {
        assert_eq!(
            acciones(events, platform),
            esperado,
            "{nombre} en {platform:?}: {events:?}"
        );
    }
}

fn lapiz() -> Option<AppAction> {
    Some(AppAction::SelectTool(Tool::Pencil))
}

// ---------------------------------------------------------------------------------------------
// Fila 4: lo que sí activa un atajo
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_4_espanol_altgr_corchetes_dan_color_anterior_y_siguiente() {
    // AltGr+`[` y AltGr+`]`: la tecla lógica es el corchete y el texto llega justo después. En
    // Linux AltGr no es `Alt`; en Windows `winit` lo quita; con `alt` también debe valer.
    for modifiers in [Modifiers::NONE, Modifiers::ALT] {
        assert_acciones(
            "AltGr+[",
            &[tecla(Key::OpenBracket, modifiers), texto("[")],
            &[Some(AppAction::ColorPrev)],
        );
        assert_acciones(
            "AltGr+]",
            &[tecla(Key::CloseBracket, modifiers), texto("]")],
            &[Some(AppAction::ColorNext)],
        );
    }
}

#[test]
fn fila_4_la_press_del_corchete_lleva_el_caracter_del_texto() {
    let p = presses(&[tecla(Key::OpenBracket, Modifiers::NONE), texto("[")]);
    assert_eq!(
        p,
        vec![press(Some(KeyId::Char('[')), false, false, false, false)]
    );
    let p = presses(&[tecla(Key::OpenBracket, Modifiers::ALT), texto("[")]);
    assert_eq!(
        p,
        vec![press(Some(KeyId::Char('[')), false, false, true, false)]
    );
}

#[test]
fn fila_4_espanol_la_tecla_iso_da_menor_y_con_mayus_mayor() {
    assert_acciones(
        "< (tecla ISO)",
        &[tecla(Key::IntlBackslash, Modifiers::NONE), texto("<")],
        &[Some(AppAction::ZoomOut)],
    );
    assert_acciones(
        "> (Mayús + tecla ISO)",
        &[tecla(Key::IntlBackslash, Modifiers::SHIFT), texto(">")],
        &[Some(AppAction::ZoomIn)],
    );
}

#[test]
fn fila_4_ingles_mayus_coma_es_menor_y_mayus_punto_es_mayor() {
    assert_acciones(
        "Mayús+, con Text(<)",
        &[tecla(Key::Comma, Modifiers::SHIFT), texto("<")],
        &[Some(AppAction::ZoomOut)],
    );
    assert_acciones(
        "Mayús+. con Text(>)",
        &[tecla(Key::Period, Modifiers::SHIFT), texto(">")],
        &[Some(AppAction::ZoomIn)],
    );
    // Sin `Mayús`, la misma tecla es otra cosa: `,` es el cuentagotas y `.` no es nada.
    assert_acciones(
        ",",
        &[tecla(Key::Comma, Modifiers::NONE), texto(",")],
        &[Some(AppAction::SelectTool(Tool::Dropper))],
    );
    assert_acciones(
        ".",
        &[tecla(Key::Period, Modifiers::NONE), texto(".")],
        &[None],
    );
}

#[test]
fn fila_4_el_menor_con_mayus_coma_no_es_la_coma() {
    // La misma tecla física (`,`) da cosas distintas según el texto: se compara el carácter.
    let menor = presses(&[tecla(Key::Comma, Modifiers::SHIFT), texto("<")]);
    let coma = presses(&[tecla(Key::Comma, Modifiers::NONE), texto(",")]);
    assert_eq!(menor[0].key, Some(KeyId::Char('<')));
    assert_eq!(coma[0].key, Some(KeyId::Char(',')));
}

#[test]
fn fila_4_aleman_altgr_8_y_9_dan_los_corchetes() {
    // AltGr+8 → `[` y AltGr+9 → `]`: la tecla lógica es la de la cifra o el propio corchete; vale
    // el texto, también con `alt`.
    for modifiers in [Modifiers::NONE, Modifiers::ALT] {
        for (logica, fisica) in [(Key::OpenBracket, Key::Num8), (Key::Num8, Key::Num8)] {
            assert_acciones(
                "AltGr+8",
                &[tecla_fisica(logica, fisica, modifiers), texto("[")],
                &[Some(AppAction::ColorPrev)],
            );
        }
        for (logica, fisica) in [(Key::CloseBracket, Key::Num9), (Key::Num9, Key::Num9)] {
            assert_acciones(
                "AltGr+9",
                &[tecla_fisica(logica, fisica, modifiers), texto("]")],
                &[Some(AppAction::ColorNext)],
            );
        }
    }
}

#[test]
fn fila_4_macos_espanol_option_corchete_con_alt_da_color_anterior() {
    assert_acciones(
        "Option+[",
        &[tecla(Key::OpenBracket, Modifiers::ALT), texto("[")],
        &[Some(AppAction::ColorPrev)],
    );
    assert_acciones(
        "Option+]",
        &[tecla(Key::CloseBracket, Modifiers::ALT), texto("]")],
        &[Some(AppAction::ColorNext)],
    );
}

#[test]
fn fila_4_dvorak_la_d_en_otra_posicion_fisica_sigue_siendo_el_lapiz() {
    // En Dvorak la `d` está donde la `e` de QWERTY.
    assert_acciones(
        "d en la posición de la E",
        &[tecla_fisica(Key::D, Key::E, Modifiers::NONE), texto("d")],
        &[lapiz()],
    );
    // Y la tecla física `D` de QWERTY escribe `e`: no es el lápiz.
    assert_acciones(
        "la posición de la D escribe e",
        &[tecla_fisica(Key::E, Key::D, Modifiers::NONE), texto("e")],
        &[None],
    );
    // La coma de Dvorak está donde la `w`.
    assert_acciones(
        ", en la posición de la W",
        &[
            tecla_fisica(Key::Comma, Key::W, Modifiers::NONE),
            texto(","),
        ],
        &[Some(AppAction::SelectTool(Tool::Dropper))],
    );
}

#[test]
fn fila_4_ruso_key_d_con_texto_cirilico_es_el_lapiz_por_posicion() {
    assert_acciones(
        "в sobre la D",
        &[tecla(Key::D, Modifiers::NONE), texto("в")],
        &[lapiz()],
    );
    let p = presses(&[tecla(Key::D, Modifiers::NONE), texto("в")]);
    assert_eq!(
        p,
        vec![press(Some(KeyId::Char('d')), false, false, false, false)]
    );
    // El griego también (`δ` sobre la D).
    assert_acciones(
        "δ sobre la D",
        &[tecla(Key::D, Modifiers::NONE), texto("δ")],
        &[lapiz()],
    );
    // Una letra de otra escritura sobre una tecla sin fila es esa tecla: sin acción.
    assert_acciones(
        "я sobre la Z",
        &[tecla(Key::Z, Modifiers::NONE), texto("я")],
        &[None],
    );
}

#[test]
fn fila_4_ruso_con_ctrl_vale_la_tecla_de_egui() {
    // Con `Ctrl` no hay texto: vale la tecla de egui (que en ruso es la física).
    assert_acciones(
        "Ctrl+Z en ruso",
        &[tecla(Key::Z, CTRL)],
        &[Some(AppAction::Undo)],
    );
}

#[test]
fn fila_4_bloq_mayus_text_d_mayuscula_sin_shift_es_el_lapiz() {
    assert_acciones(
        "Text(D) sin shift",
        &[tecla(Key::D, Modifiers::NONE), texto("D")],
        &[lapiz()],
    );
    // Es la misma `Press` que con `Text("d")` sin `shift`.
    let con_mayuscula = presses(&[tecla(Key::D, Modifiers::NONE), texto("D")]);
    let con_minuscula = presses(&[tecla(Key::D, Modifiers::NONE), texto("d")]);
    assert_eq!(con_mayuscula, con_minuscula);
    assert_eq!(
        con_mayuscula,
        vec![press(Some(KeyId::Char('d')), false, false, false, false)]
    );
}

#[test]
fn fila_4_mayus_de_verdad_con_d_no_es_el_lapiz() {
    // `Mayús` pulsada con la `d` (en las letras `shift` es exacto): sin acción.
    assert_acciones(
        "Mayús+D",
        &[tecla(Key::D, Modifiers::SHIFT), texto("D")],
        &[None],
    );
    // Bloq Mayús y `Mayús` a la vez escribe `d`, pero con `shift`: tampoco.
    assert_acciones(
        "Bloq Mayús + Mayús + D",
        &[tecla(Key::D, Modifiers::SHIFT), texto("d")],
        &[None],
    );
}

#[test]
fn fila_4_azerty_ctrl_mas_la_tecla_de_la_cifra_1_es_zoom_actual() {
    // En AZERTY el carácter de esa tecla sin `Mayús` es `&`, pero con `Ctrl` no hay texto y
    // vale la tecla de egui: la cifra.
    assert_acciones(
        "Ctrl+tecla de la cifra 1",
        &[tecla(Key::Num1, CTRL)],
        &[Some(AppAction::ZoomActual)],
    );
    assert_acciones(
        "Cmd+tecla de la cifra 1",
        &[tecla(Key::Num1, CMD)],
        &[Some(AppAction::ZoomActual)],
    );
    // Sin `Ctrl`, esa tecla escribe `&`: no es ningún atajo.
    assert_acciones(
        "& sin Ctrl",
        &[tecla(Key::Num1, Modifiers::NONE), texto("&")],
        &[None],
    );
    // Con `Mayús` escribe `1`, pero con `shift` y sin `command`: nada.
    assert_acciones(
        "1 con Mayús",
        &[tecla(Key::Num1, Modifiers::SHIFT), texto("1")],
        &[None],
    );
}

#[test]
fn fila_4_las_cifras_con_su_texto_no_hacen_nada() {
    for (key, cifra) in [(Key::Num1, "1"), (Key::Num3, "3"), (Key::Num9, "9")] {
        assert_acciones(cifra, &[tecla(key, Modifiers::NONE), texto(cifra)], &[None]);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 4: lo que NO activa ningún atajo
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_4_la_tecla_muerta_del_acento_sin_texto_no_dispara_el_corchete() {
    // La tecla muerta del teclado español es la física de `[`: sin texto y sin modificadores.
    let events = [tecla(Key::OpenBracket, Modifiers::NONE)];
    assert_eq!(
        presses(&events),
        vec![press(None, false, false, false, false)],
        "una Press sin carácter"
    );
    assert_acciones("tecla muerta", &events, &[None]);
    // También la del apóstrofo (`'`), que es la otra tecla muerta posible.
    assert_acciones(
        "tecla muerta del `'`",
        &[tecla(Key::Quote, Modifiers::NONE)],
        &[None],
    );
    // Y con `Mayús` (la diéresis).
    assert_acciones(
        "tecla muerta con Mayús",
        &[tecla(Key::OpenBracket, Modifiers::SHIFT)],
        &[None],
    );
    assert_acciones(
        "tecla muerta de `]`",
        &[tecla(Key::CloseBracket, Modifiers::NONE)],
        &[None],
    );
}

#[test]
fn fila_4_la_vocal_compuesta_tras_la_tecla_muerta_no_es_la_letra() {
    // `´` y luego `u`: la `Key::U` llega con `Text("ú")`: no es el `u` de deshacer.
    let events = [
        tecla(Key::OpenBracket, Modifiers::NONE),
        tecla(Key::U, Modifiers::NONE),
        texto("ú"),
    ];
    assert_eq!(presses(&events).len(), 2);
    assert_eq!(presses(&events)[1].key, None, "ú no da carácter");
    assert_acciones("´ + u", &events, &[None, None]);
    for (key, compuesta) in [(Key::E, "é"), (Key::D, "ď"), (Key::A, "á")] {
        assert_acciones(
            compuesta,
            &[tecla(key, Modifiers::NONE), texto(compuesta)],
            &[None],
        );
    }
}

#[test]
fn fila_4_una_composicion_de_varios_caracteres_no_da_atajo() {
    let events = [tecla(Key::D, Modifiers::NONE), texto("´d")];
    assert_eq!(
        presses(&events),
        vec![press(None, false, false, false, false)]
    );
    assert_acciones("composición ´d", &events, &[None]);
    assert_acciones(
        "dd",
        &[tecla(Key::D, Modifiers::NONE), texto("dd")],
        &[None],
    );
}

#[test]
fn fila_4_option_mas_letra_en_macos_escribe_un_simbolo_y_no_hace_nada() {
    // `Option`+`d` escribe `∂`: la `Key::D` llega con `alt` y `Text("∂")`.
    let events = [tecla(Key::D, Modifiers::ALT), texto("∂")];
    assert_eq!(
        presses(&events),
        vec![press(None, false, false, true, false)],
        "un símbolo que no es ASCII no da carácter"
    );
    assert_acciones("Option+d", &events, &[None]);
    // Otros símbolos no ASCII de `Option`.
    for simbolo in ["ƒ", "ß", "€", "©"] {
        assert_acciones(
            simbolo,
            &[tecla(Key::F, Modifiers::ALT), texto(simbolo)],
            &[None],
        );
    }
}

#[test]
fn fila_4_alt_mas_letra_con_su_texto_no_es_la_letra() {
    // En Linux y Windows `Alt`+`d` llega con `Text("d")`: la letra con `alt` no hace nada.
    assert_acciones(
        "Alt+d",
        &[tecla(Key::D, Modifiers::ALT), texto("d")],
        &[None],
    );
    let p = presses(&[tecla(Key::D, Modifiers::ALT), texto("d")]);
    assert_eq!(
        p,
        vec![press(Some(KeyId::Char('d')), false, false, true, false)]
    );
}

#[test]
fn fila_4_el_ime_no_da_press_ni_atajo() {
    let preedicion = Event::Ime(ImeEvent::Preedit {
        text: "d".to_string(),
        active_range_chars: None,
    });
    let confirmacion = Event::Ime(ImeEvent::Commit("d".to_string()));
    let vacia = Event::Ime(ImeEvent::Preedit {
        text: String::new(),
        active_range_chars: None,
    });
    let borrado = Event::Ime(ImeEvent::DeleteSurrounding {
        before_chars: 1,
        after_chars: 0,
    });
    for events in [
        vec![preedicion.clone()],
        vec![confirmacion.clone()],
        vec![vacia],
        vec![borrado],
        // La secuencia completa de una composición: preedición, confirmación y el texto suelto.
        vec![preedicion.clone(), confirmacion.clone(), texto("d")],
    ] {
        assert!(presses(&events).is_empty(), "{events:?}");
        assert_acciones("IME", &events, &[]);
    }
}

#[test]
fn fila_4_la_tecla_pulsada_durante_una_composicion_de_ime_no_da_caracter() {
    let preedicion = Event::Ime(ImeEvent::Preedit {
        text: "d".to_string(),
        active_range_chars: None,
    });
    let confirmacion = Event::Ime(ImeEvent::Commit("d".to_string()));
    let events = [preedicion, tecla(Key::D, Modifiers::NONE), confirmacion];
    assert_eq!(
        presses(&events),
        vec![press(None, false, false, false, false)],
        "una Press sin carácter por la `Key`, ninguna por el IME"
    );
    assert_acciones("tecla durante el IME", &events, &[None]);
}

#[test]
fn fila_4_un_text_suelto_no_da_press() {
    assert!(presses(&[texto("d")]).is_empty());
    assert!(presses(&[texto(",")]).is_empty());
    assert_acciones("Text(,) suelto", &[texto(",")], &[]);
    assert_acciones("Text(d) suelto", &[texto("d")], &[]);
    // Tras la suelta de una tecla, el texto también es suelto.
    assert!(presses(&[suelta(Key::Comma, Modifiers::NONE), texto(",")]).is_empty());
    // Un segundo `Text` tras la tecla es suelto: solo el primero se empareja.
    let events = [tecla(Key::D, Modifiers::NONE), texto("d"), texto("d")];
    assert_eq!(
        presses(&events),
        vec![press(Some(KeyId::Char('d')), false, false, false, false)]
    );
}

#[test]
fn fila_4_un_text_que_no_sigue_inmediatamente_a_la_tecla_es_suelto() {
    let events = [
        tecla(Key::D, Modifiers::NONE),
        Event::PointerMoved(pos2(1.0, 1.0)),
        texto("d"),
    ];
    assert_eq!(
        presses(&events),
        vec![press(None, false, false, false, false)],
        "la tecla queda sin carácter y el texto no da Press"
    );
    assert_acciones("texto separado", &events, &[None]);
}

#[test]
fn fila_4_otros_eventos_no_dan_press() {
    let events = [
        Event::PointerMoved(pos2(1.0, 1.0)),
        Event::WindowFocused(true),
        Event::ModifiersChanged(Modifiers::SHIFT),
        Event::Zoom(1.1),
    ];
    assert!(presses(&events).is_empty());
}

#[test]
fn fila_4_las_soltadas_se_ignoran() {
    let events = [
        suelta(Key::D, Modifiers::NONE),
        tecla(Key::D, Modifiers::NONE),
        texto("d"),
        suelta(Key::D, Modifiers::NONE),
    ];
    assert_eq!(
        presses(&events),
        vec![press(Some(KeyId::Char('d')), false, false, false, false)]
    );
}

#[test]
fn fila_4_varias_teclas_en_un_fotograma_dan_una_press_por_tecla_y_en_orden() {
    let events = [
        tecla(Key::D, Modifiers::NONE),
        texto("d"),
        tecla(Key::V, Modifiers::NONE),
        texto("v"),
        tecla(Key::F, Modifiers::NONE),
        texto("f"),
        tecla(Key::Z, CTRL),
    ];
    assert_acciones(
        "d v f Ctrl+Z",
        &events,
        &[
            lapiz(),
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
fn fila_4_con_command_el_caracter_es_el_de_la_tecla_de_egui_sin_texto() {
    for (key, esperada) in [
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
            let p = presses(&[tecla(key, modifiers)]);
            assert_eq!(p.len(), 1);
            assert_eq!(p[0].key, esperada, "{key:?} con {modifiers:?}");
            assert!(
                p[0].command && !p[0].other_ctrl,
                "{key:?} con {modifiers:?}: {:?}",
                p[0]
            );
        }
    }
}

#[test]
fn fila_4_los_nueve_modernos_sobre_eventos_en_las_tres_plataformas() {
    let linux_y_windows = [
        (Key::Z, CTRL, AppAction::Undo),
        (Key::Y, CTRL, AppAction::Redo),
        (Key::Z, CTRL | Modifiers::SHIFT, AppAction::Redo),
        (Key::O, CTRL, AppAction::Open),
        (Key::N, CTRL, AppAction::NewImage),
        (Key::S, CTRL, AppAction::Save),
        (Key::S, CTRL | Modifiers::SHIFT, AppAction::SaveAs),
        (Key::Num1, CTRL, AppAction::ZoomActual),
    ];
    for (key, modifiers, esperada) in linux_y_windows {
        assert_acciones(
            "moderno con Ctrl",
            &[tecla(key, modifiers)],
            &[Some(esperada)],
        );
    }
    for (key, modifiers, esperada) in [
        (Key::Z, CMD, AppAction::Undo),
        (Key::Z, CMD | Modifiers::SHIFT, AppAction::Redo),
        (Key::S, CMD | Modifiers::SHIFT, AppAction::SaveAs),
        (Key::Num1, CMD, AppAction::ZoomActual),
    ] {
        assert_acciones(
            "moderno con Cmd",
            &[tecla(key, modifiers)],
            &[Some(esperada)],
        );
    }
    // `Ctrl+Q` solo sale en Linux.
    let events = [tecla(Key::Q, CTRL)];
    assert_eq!(acciones(&events, Platform::Linux), [Some(AppAction::Quit)]);
    assert_eq!(acciones(&events, Platform::Windows), [None]);
    assert_eq!(acciones(&events, Platform::Mac), [None]);
}

#[test]
fn fila_4_ctrl_alt_y_letra_no_es_el_atajo() {
    // `Ctrl+Alt+Z` (y el AltGr de Windows que llegara como `Ctrl`+`Alt`) no deshace.
    assert_acciones(
        "Ctrl+Alt+Z",
        &[tecla(Key::Z, CTRL | Modifiers::ALT)],
        &[None],
    );
}

#[test]
fn fila_4_el_ctrl_de_macos_es_other_ctrl_y_anula_las_letras() {
    let p = presses(&[tecla(Key::Z, CTRL_DE_MAC)]);
    assert_eq!(p.len(), 1);
    assert!(!p[0].command && p[0].other_ctrl, "{:?}", p[0]);
    assert_acciones("Ctrl de macOS + Z", &[tecla(Key::Z, CTRL_DE_MAC)], &[None]);
    // Con texto de una letra, también: `Ctrl` suelto + `d` no es el lápiz.
    let con_texto = [tecla(Key::D, CTRL_DE_MAC), texto("d")];
    let p = presses(&con_texto);
    assert!(p[0].other_ctrl);
    assert_acciones("Ctrl de macOS + d", &con_texto, &[None]);
}

#[test]
fn fila_4_las_teclas_con_nombre_valen_siempre_por_la_tecla_y_con_cualquier_modificador() {
    for modifiers in [
        Modifiers::NONE,
        Modifiers::SHIFT,
        Modifiers::ALT,
        Modifiers::SHIFT | Modifiers::ALT,
        CTRL_DE_MAC,
    ] {
        assert_acciones(
            "Inicio",
            &[tecla(Key::Home, modifiers)],
            &[Some(AppAction::ZoomFit)],
        );
        assert_acciones(
            "Re Pág",
            &[tecla(Key::PageUp, modifiers)],
            &[Some(AppAction::ColorPrev)],
        );
        assert_acciones(
            "Av Pág",
            &[tecla(Key::PageDown, modifiers)],
            &[Some(AppAction::ColorNext)],
        );
    }
    // Con `command` no hay acción.
    assert_acciones("Ctrl+Inicio", &[tecla(Key::Home, CTRL)], &[None]);
    assert_acciones("Ctrl+Re Pág", &[tecla(Key::PageUp, CTRL)], &[None]);
    assert_acciones("Cmd+Av Pág", &[tecla(Key::PageDown, CMD)], &[None]);
    // Las pulsaciones de teclas con nombre llevan la tecla aunque no haya texto.
    assert_eq!(
        presses(&[tecla(Key::PageDown, Modifiers::NONE)]),
        vec![press(Some(KeyId::PageDown), false, false, false, false)]
    );
}

#[test]
fn fila_4_las_teclas_sin_caracter_dan_una_press_sin_tecla() {
    for key in [Key::End, Key::F1, Key::Escape, Key::Tab] {
        assert_eq!(
            presses(&[tecla(key, Modifiers::NONE)]),
            vec![press(None, false, false, false, false)],
            "{key:?}"
        );
        assert_acciones("sin carácter", &[tecla(key, Modifiers::NONE)], &[None]);
    }
}

#[test]
fn fila_4_los_modificadores_pasan_a_los_campos_de_la_press() {
    let p = presses(&[tecla(Key::Home, Modifiers::SHIFT | Modifiers::ALT)]);
    assert_eq!(p, vec![press(Some(KeyId::Home), false, true, true, false)]);
    let p = presses(&[tecla(Key::Home, CTRL)]);
    assert_eq!(p, vec![press(Some(KeyId::Home), true, false, false, false)]);
    let p = presses(&[tecla(Key::Home, CTRL_DE_MAC)]);
    assert_eq!(p, vec![press(Some(KeyId::Home), false, false, false, true)]);
}

// ---------------------------------------------------------------------------------------------
// Fila 12 (parte de presses): repetición
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_presses_marca_repeat_en_las_repeticiones_y_no_en_la_primera() {
    let events = [
        tecla(Key::U, Modifiers::NONE),
        texto("u"),
        repetida(Key::U, Modifiers::NONE),
        texto("u"),
        repetida(Key::U, Modifiers::NONE),
        texto("u"),
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
fn fila_12_u_corchete_av_pag_y_menor_repetidos_dan_una_accion_por_pulsacion() {
    let casos: Vec<(&str, Vec<Event>, AppAction)> = vec![
        (
            "u",
            vec![
                tecla(Key::U, Modifiers::NONE),
                texto("u"),
                repetida(Key::U, Modifiers::NONE),
                texto("u"),
                repetida(Key::U, Modifiers::NONE),
                texto("u"),
                repetida(Key::U, Modifiers::NONE),
                texto("u"),
            ],
            AppAction::Undo,
        ),
        (
            "]",
            vec![
                tecla(Key::CloseBracket, Modifiers::NONE),
                texto("]"),
                repetida(Key::CloseBracket, Modifiers::NONE),
                texto("]"),
                repetida(Key::CloseBracket, Modifiers::NONE),
                texto("]"),
                repetida(Key::CloseBracket, Modifiers::NONE),
                texto("]"),
            ],
            AppAction::ColorNext,
        ),
        (
            "Av Pág",
            vec![
                tecla(Key::PageDown, Modifiers::NONE),
                repetida(Key::PageDown, Modifiers::NONE),
                repetida(Key::PageDown, Modifiers::NONE),
                repetida(Key::PageDown, Modifiers::NONE),
            ],
            AppAction::ColorNext,
        ),
        (
            "<",
            vec![
                tecla(Key::IntlBackslash, Modifiers::NONE),
                texto("<"),
                repetida(Key::IntlBackslash, Modifiers::NONE),
                texto("<"),
                repetida(Key::IntlBackslash, Modifiers::NONE),
                texto("<"),
                repetida(Key::IntlBackslash, Modifiers::NONE),
                texto("<"),
            ],
            AppAction::ZoomOut,
        ),
    ];
    for (nombre, events, esperada) in casos {
        for platform in PLATAFORMAS {
            let acciones = acciones(&events, platform);
            assert_eq!(
                acciones,
                vec![Some(esperada); 4],
                "{nombre} repetida en {platform:?}"
            );
            assert!(!esperada.ignores_repeat(), "{nombre}: {esperada:?} repite");
        }
    }
}

#[test]
fn fila_12_los_atajos_de_fichero_mantenidos_llegan_con_repeat_y_son_de_los_que_no_repiten() {
    for (key, modifiers, esperada) in [
        (Key::O, CTRL, AppAction::Open),
        (Key::S, CTRL, AppAction::Save),
        (Key::S, CTRL | Modifiers::SHIFT, AppAction::SaveAs),
        (Key::N, CTRL, AppAction::NewImage),
        (Key::Q, CTRL, AppAction::Quit),
    ] {
        let events = [tecla(key, modifiers), repetida(key, modifiers)];
        let p = presses(&events);
        assert_eq!(
            p.iter().map(|p| p.repeat).collect::<Vec<_>>(),
            [false, true],
            "{esperada:?}"
        );
        assert_eq!(
            acciones(&events, Platform::Linux),
            vec![Some(esperada); 2],
            "{esperada:?}"
        );
        assert!(esperada.ignores_repeat(), "{esperada:?} no repite");
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 4: emparejado de eventos
// ---------------------------------------------------------------------------------------------

/// Las teclas de la propiedad y el carácter que da su tecla de egui (`None`: sin carácter).
const TECLAS: [(Key, Option<KeyId>); 12] = [
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
const TEXTOS: [&str; 16] = [
    "d", "D", ",", "<", ">", "[", "1", "ú", "´d", "dd", "∂", "в", "δ", "é", "€", "я",
];

#[derive(Clone, Debug)]
enum Evento {
    Tecla {
        indice: usize,
        pulsada: bool,
        repeat: bool,
        command: bool,
        ctrl_suelto: bool,
        shift: bool,
        alt: bool,
    },
    Texto(usize),
    Ime(usize),
}

fn modificadores_de(command: bool, ctrl_suelto: bool, shift: bool, alt: bool) -> Modifiers {
    Modifiers {
        alt,
        ctrl: command || ctrl_suelto,
        shift,
        mac_cmd: false,
        command,
    }
}

fn a_egui(evento: &Evento) -> Event {
    match evento {
        Evento::Tecla {
            indice,
            pulsada,
            repeat,
            command,
            ctrl_suelto,
            shift,
            alt,
        } => Event::Key {
            key: TECLAS[*indice].0,
            physical_key: Some(TECLAS[*indice].0),
            pressed: *pulsada,
            repeat: *pulsada && *repeat,
            modifiers: modificadores_de(*command, *ctrl_suelto && !*command, *shift, *alt),
        },
        Evento::Texto(i) => Event::Text(TEXTOS[*i].to_string()),
        Evento::Ime(i) => match i % 3 {
            0 => Event::Ime(ImeEvent::Preedit {
                text: TEXTOS[*i].to_string(),
                active_range_chars: None,
            }),
            1 => Event::Ime(ImeEvent::Commit(TEXTOS[*i].to_string())),
            _ => Event::Ime(ImeEvent::DeleteSurrounding {
                before_chars: 1,
                after_chars: 0,
            }),
        },
    }
}

fn evento_cualquiera() -> impl Strategy<Value = Evento> {
    prop_oneof![
        4 => (
            0..TECLAS.len(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
        )
            .prop_map(|(indice, pulsada, repeat, command, ctrl_suelto, shift, alt)| {
                Evento::Tecla {
                    indice,
                    pulsada,
                    repeat,
                    command,
                    ctrl_suelto,
                    shift,
                    alt,
                }
            }),
        3 => (0..TEXTOS.len()).prop_map(Evento::Texto),
        1 => (0..TEXTOS.len()).prop_map(Evento::Ime),
    ]
}

/// Si `c` es una letra de otra escritura: alfabética y fuera de Latin-1 y Latín extendido.
fn es_de_otra_escritura(ch: char) -> bool {
    ch.is_alphabetic() && ch > '\u{24F}'
}

/// El carácter esperado de una `Press` (oráculo escrito a mano): teclas con nombre por la tecla;
/// con `command`, la tecla de egui; sin él, solo el texto que le sigue inmediatamente, si es un
/// único carácter ASCII imprimible (en minúscula) o, como respaldo por posición, una letra de
/// otra escritura (entonces, la tecla de egui).
fn caracter_esperado(indice: usize, command: bool, texto_siguiente: Option<&str>) -> Option<KeyId> {
    let de_la_tecla = TECLAS[indice].1;
    if matches!(
        de_la_tecla,
        Some(KeyId::Home | KeyId::PageUp | KeyId::PageDown)
    ) {
        return de_la_tecla;
    }
    if command {
        return de_la_tecla;
    }
    let texto = texto_siguiente?;
    let mut chars = texto.chars();
    let (Some(ch), None) = (chars.next(), chars.next()) else {
        return None;
    };
    if ch.is_ascii_graphic() {
        return Some(KeyId::Char(ch.to_ascii_lowercase()));
    }
    if es_de_otra_escritura(ch) {
        return de_la_tecla;
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
    fn propiedad_4_emparejado_de_eventos(
        eventos in prop::collection::vec(evento_cualquiera(), 0..24),
    ) {
        let egui: Vec<Event> = eventos.iter().map(a_egui).collect();
        let obtenidas = presses(&egui);

        // Lo esperado, recorriendo la secuencia con las reglas de la spec.
        let mut esperadas: Vec<Press> = Vec::new();
        for (i, evento) in eventos.iter().enumerate() {
            let Evento::Tecla { indice, pulsada: true, repeat, command, ctrl_suelto, shift, alt } = evento else {
                continue;
            };
            let siguiente = match eventos.get(i + 1) {
                Some(Evento::Texto(t)) => Some(TEXTOS[*t]),
                _ => None,
            };
            esperadas.push(Press {
                key: caracter_esperado(*indice, *command, siguiente),
                command: *command,
                shift: *shift,
                alt: *alt,
                other_ctrl: *ctrl_suelto && !*command,
                repeat: *repeat,
            });
        }
        prop_assert_eq!(obtenidas.len(), esperadas.len(), "una Press por cada Key pulsada: {:?}", egui);
        prop_assert_eq!(obtenidas, esperadas, "{:?}", egui);
    }

    /// Propiedad 4, última cláusula: `Text("D")` sin `shift` da la misma `Press` que `Text("d")`
    /// sin `shift`, con cualquier otro modificador y estando entre otros eventos.
    #[test]
    fn propiedad_4_text_d_mayuscula_sin_shift_es_text_d_minuscula(
        alt in any::<bool>(),
        ctrl_suelto in any::<bool>(),
        repeat in any::<bool>(),
        antes in prop::collection::vec(evento_cualquiera(), 0..6),
    ) {
        let modifiers = modificadores_de(false, ctrl_suelto, false, alt);
        let con = |t: &str| {
            let mut v: Vec<Event> = antes.iter().map(a_egui).collect();
            let mut k = tecla(Key::D, modifiers);
            if let Event::Key { repeat: r, .. } = &mut k {
                *r = repeat;
            }
            v.push(k);
            v.push(texto(t));
            presses(&v)
        };
        prop_assert_eq!(con("D"), con("d"));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 4: no entra en pánico con ninguna tecla de `Key::ALL`, pulsada o soltada, con
    /// ningún texto ni modificadores.
    #[test]
    fn propiedad_4_sin_panico_con_cualquier_tecla_de_egui(
        indice in 0..Key::ALL.len(),
        pulsada in any::<bool>(),
        bits in 0u8..32,
        con_texto in prop::option::of(0..TEXTOS.len()),
    ) {
        let m = Modifiers {
            alt: bits & 1 != 0,
            ctrl: bits & 2 != 0,
            shift: bits & 4 != 0,
            mac_cmd: bits & 8 != 0,
            command: bits & 16 != 0,
        };
        let mut events = vec![Event::Key {
            key: Key::ALL[indice],
            physical_key: None,
            pressed: pulsada,
            repeat: false,
            modifiers: m,
        }];
        if let Some(t) = con_texto {
            events.push(texto(TEXTOS[t]));
        }
        let p = presses(&events);
        prop_assert_eq!(p.len(), usize::from(pulsada), "{:?}", events);
    }
}
