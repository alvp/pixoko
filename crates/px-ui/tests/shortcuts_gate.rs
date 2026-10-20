//! Tests de `shortcuts::gate_keys` (specs/hito-2-editar-color.md, unidad 11c, decisión 11; filas
//! 16 y 17 en su parte pura).
//!
//! Con un campo de texto enfocado (`wants_keyboard`), `gate_keys` no deja pasar ningún evento a
//! los atajos: teclear `ff00ab` no cambia de herramienta (`f`), de color (`0` a `9`) ni de vista
//! (`Home`, `+`, `-`), y `Ctrl+Z`, `Ctrl+Y` y `Ctrl+S` no tocan el documento ni el fichero. Sin
//! foco de texto devuelve todos los eventos, en el mismo orden, y `action_for_key` y
//! `tool_action_for_key` siguen viéndolos.

use egui::{Event, Key, Modifiers, PointerButton, pos2};
use px_ui::shortcuts::{Action, FileAction, action_for_key, file_action_for_key, gate_keys};
use px_ui::tool::{Tool, ToolAction, tool_action_for_key};

fn tecla(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn texto(texto: &str) -> Event {
    Event::Text(texto.to_string())
}

/// Las teclas de la fila 16: `f`, `f`, `0`, `0`, `a`, `b`, y `Home`, `+`, `-`.
fn teclas_de_la_fila_16() -> Vec<Event> {
    [
        Key::F,
        Key::F,
        Key::Num0,
        Key::Num0,
        Key::A,
        Key::B,
        Key::Home,
        Key::Plus,
        Key::Minus,
    ]
    .into_iter()
    .map(|key| tecla(key, Modifiers::NONE))
    .collect()
}

/// Los textos que llegan con esas teclas.
fn textos_de_la_fila_16() -> Vec<Event> {
    ["f", "f", "0", "0", "a", "b"]
        .into_iter()
        .map(texto)
        .collect()
}

/// Teclas con modificador de la fila 17.
fn atajos_de_la_fila_17() -> Vec<Event> {
    vec![
        tecla(Key::Z, Modifiers::COMMAND),
        tecla(Key::Y, Modifiers::COMMAND),
        tecla(Key::S, Modifiers::COMMAND),
    ]
}

/// Todo lo de las filas 16 y 17, mezclado como lo entregaría egui: texto y tecla por cada letra.
fn fotograma_completo() -> Vec<Event> {
    let mut eventos = Vec::new();
    for (key, letra) in [
        (Key::F, "f"),
        (Key::F, "f"),
        (Key::Num0, "0"),
        (Key::Num0, "0"),
        (Key::A, "a"),
        (Key::B, "b"),
    ] {
        eventos.push(tecla(key, Modifiers::NONE));
        eventos.push(texto(letra));
    }
    eventos.extend([
        tecla(Key::Home, Modifiers::NONE),
        tecla(Key::Plus, Modifiers::NONE),
        tecla(Key::Minus, Modifiers::NONE),
    ]);
    eventos.extend(atajos_de_la_fila_17());
    eventos
}

// Fila 16: con el campo enfocado.

#[test]
fn fila_16_con_foco_de_texto_no_pasa_ninguna_tecla() {
    assert!(gate_keys(&teclas_de_la_fila_16(), true).is_empty());
}

#[test]
fn fila_16_con_foco_de_texto_no_pasan_los_eventos_de_texto() {
    assert!(gate_keys(&textos_de_la_fila_16(), true).is_empty());
}

#[test]
fn fila_16_cada_evento_por_separado_queda_fuera() {
    for evento in fotograma_completo() {
        assert!(
            gate_keys(std::slice::from_ref(&evento), true).is_empty(),
            "{evento:?}"
        );
    }
}

#[test]
fn fila_16_el_fotograma_entero_con_foco_da_vacio() {
    assert!(gate_keys(&fotograma_completo(), true).is_empty());
}

#[test]
fn fila_16_ni_tool_action_for_key_ni_action_for_key_ven_nada_con_foco() {
    let visibles = gate_keys(&fotograma_completo(), true);
    let mut acciones_de_herramienta = 0;
    let mut acciones = 0;
    for evento in &visibles {
        if let Event::Key { key, modifiers, .. } = evento {
            acciones_de_herramienta += usize::from(tool_action_for_key(*key, *modifiers).is_some());
            acciones += usize::from(action_for_key(*key, *modifiers).is_some());
        }
    }
    assert_eq!((acciones_de_herramienta, acciones), (0, 0));
}

#[test]
fn fila_16_sin_el_filtro_la_f_elegiria_relleno() {
    // Lo que evita el filtro: la `f` sola es una acción de herramienta.
    assert_eq!(
        tool_action_for_key(Key::F, Modifiers::NONE),
        Some(ToolAction::Select(Tool::Fill))
    );
}

// Fila 17: Ctrl+Z, Ctrl+Y, Ctrl+S.

#[test]
fn fila_17_con_foco_de_texto_los_atajos_con_control_quedan_fuera() {
    assert!(gate_keys(&atajos_de_la_fila_17(), true).is_empty());
    for evento in atajos_de_la_fila_17() {
        let visibles = gate_keys(std::slice::from_ref(&evento), true);
        assert!(visibles.is_empty(), "{evento:?}");
    }
}

#[test]
fn fila_17_con_foco_de_texto_el_atajo_de_guardar_no_llega() {
    let visibles = gate_keys(&atajos_de_la_fila_17(), true);
    let guardar = visibles.iter().any(|evento| match evento {
        Event::Key { key, modifiers, .. } => file_action_for_key(*key, *modifiers).is_some(),
        _ => false,
    });
    assert!(!guardar);
}

#[test]
fn fila_17_sin_foco_devuelve_todos_en_el_mismo_orden() {
    let eventos = fotograma_completo();
    assert_eq!(gate_keys(&eventos, false), eventos);
    let teclas = teclas_de_la_fila_16();
    assert_eq!(gate_keys(&teclas, false), teclas);
    let textos = textos_de_la_fila_16();
    assert_eq!(gate_keys(&textos, false), textos);
    let atajos = atajos_de_la_fila_17();
    assert_eq!(gate_keys(&atajos, false), atajos);
}

#[test]
fn fila_17_sin_foco_los_atajos_vuelven_a_verse() {
    let eventos = vec![
        tecla(Key::Z, Modifiers::COMMAND),
        tecla(Key::CloseBracket, Modifiers::NONE),
        tecla(Key::Home, Modifiers::NONE),
    ];
    let visibles = gate_keys(&eventos, false);
    assert_eq!(visibles, eventos);
    let acciones: Vec<Option<Action>> = visibles
        .iter()
        .map(|evento| match evento {
            Event::Key { key, modifiers, .. } => action_for_key(*key, *modifiers),
            _ => None,
        })
        .collect();
    assert_eq!(
        acciones,
        [Some(Action::Undo), Some(Action::ColorNext), None]
    );
}

#[test]
fn fila_17_sin_foco_los_atajos_de_fichero_y_de_herramienta_siguen_viendose() {
    let visibles = gate_keys(&atajos_de_la_fila_17(), false);
    let guardar: Vec<Option<FileAction>> = visibles
        .iter()
        .map(|evento| match evento {
            Event::Key { key, modifiers, .. } => file_action_for_key(*key, *modifiers),
            _ => None,
        })
        .collect();
    assert_eq!(guardar, [None, None, Some(FileAction::Save)]);

    let visibles = gate_keys(&teclas_de_la_fila_16(), false);
    let herramienta: Vec<Option<ToolAction>> = visibles
        .iter()
        .map(|evento| match evento {
            Event::Key { key, modifiers, .. } => tool_action_for_key(*key, *modifiers),
            _ => None,
        })
        .collect();
    assert_eq!(herramienta[0], Some(ToolAction::Select(Tool::Fill)));
    assert_eq!(herramienta[1], Some(ToolAction::Select(Tool::Fill)));
    assert!(herramienta[2..].iter().all(Option::is_none));
}

// Casos generales.

#[test]
fn sin_eventos_da_vacio_con_y_sin_foco() {
    assert!(gate_keys(&[], true).is_empty());
    assert!(gate_keys(&[], false).is_empty());
}

#[test]
fn sin_foco_tambien_pasan_los_eventos_que_no_son_de_teclado() {
    let eventos = vec![
        Event::PointerMoved(pos2(1.0, 2.0)),
        Event::PointerButton {
            pos: pos2(1.0, 2.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        texto("x"),
        tecla(Key::Escape, Modifiers::NONE),
    ];
    assert_eq!(gate_keys(&eventos, false), eventos);
}

#[test]
fn con_foco_no_pasan_ni_las_teclas_soltadas_ni_repetidas() {
    let soltada = Event::Key {
        key: Key::Z,
        physical_key: Some(Key::Z),
        pressed: false,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    };
    let repetida = Event::Key {
        key: Key::Num3,
        physical_key: Some(Key::Num3),
        pressed: true,
        repeat: true,
        modifiers: Modifiers::NONE,
    };
    assert!(gate_keys(&[soltada, repetida], true).is_empty());
}

#[test]
fn el_filtro_no_modifica_la_entrada() {
    let eventos = fotograma_completo();
    let copia = eventos.clone();
    let _ = gate_keys(&eventos, true);
    let _ = gate_keys(&eventos, false);
    assert_eq!(eventos, copia);
}
