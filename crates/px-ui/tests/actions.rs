//! Tests de la capa de acciones `px_ui::actions` en su parte pura (specs/hito-2-menu-herramientas.md,
//! «Casos» filas 3, 3b, 5 y 5b, y las reglas de «Piezas»). Sin egui más allá de `Key` y `Modifiers`:
//! no hay ventana ni banco.
//!
//! - Fila 3: `action_for_shortcut` sobre `Key::ALL` por las 32 combinaciones de modificadores y las
//!   tres plataformas, contra un oráculo escrito aparte; ida y vuelta con la tabla del menú; las
//!   tres funciones de teclas anteriores se componen sin cambios.
//! - Fila 3b: las teclas `1` a `9` siguen eligiendo color y `Ctrl`/`Cmd`+`1` es solo `ZoomActual`.
//! - Fila 5: `is_blocked` en el orden de la spec (con la quita pendiente también frena abrir un
//!   diálogo: unidad 18, D2), con un modelo independiente que se compara con la
//!   implementación sobre todas las acciones, orígenes y combinaciones de bloqueos; `enabled`.
//! - Fila 5b (parte pura): los bloqueos del zoom, uno por bloqueo, con los tres orígenes.
//!
//! Decisiones donde la spec no fija un detalle (anotadas aquí y en el informe):
//! - `Ctrl+Mayús+Q` en Linux da `None`: `Salir` se trata como `Abrir` (`command` sin `Mayús` ni
//!   `Alt`); la spec solo dice «`command`+`Q` solo en Linux».
//! - `Quit` con `modal` y otro bloqueo (por ejemplo `gesture`): la regla (2) deja pasar a `Quit` y
//!   las siguientes reglas se siguen aplicando en orden, así que el otro bloqueo lo frena.

use egui::{Key, Modifiers};
use px_ui::actions::{
    AppAction, Locks, Origin, Platform, UiState, action_for_shortcut, enabled, is_blocked,
};
use px_ui::menu::{MENUS, Shortcut, shortcut_for};
use px_ui::shortcuts::{Action, FileAction, action_for_key, file_action_for_key};
use px_ui::tool::{Tool, ToolAction, tool_action_for_key};
use px_ui::view::Zoom;

mod common;
use common::{modificadores, modificadores_command};

const HERRAMIENTAS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];
const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const ORIGENES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];
const ZOOMS: [AppAction; 4] = [
    AppAction::ZoomIn,
    AppAction::ZoomOut,
    AppAction::ZoomFit,
    AppAction::ZoomActual,
];

/// Todas las acciones posibles: las 11 de la tabla del menú, `CloseLicenses`, las cinco
/// herramientas y las nueve ranuras de color.
fn todas_las_acciones() -> Vec<AppAction> {
    let mut v = vec![
        AppAction::Open,
        AppAction::Save,
        AppAction::SaveAs,
        AppAction::Quit,
        AppAction::Undo,
        AppAction::Redo,
        AppAction::ZoomIn,
        AppAction::ZoomOut,
        AppAction::ZoomFit,
        AppAction::ZoomActual,
        AppAction::ShowLicenses,
        AppAction::CloseLicenses,
    ];
    v.extend(HERRAMIENTAS.map(AppAction::SelectTool));
    v.extend((1..=9).map(AppAction::SelectColor));
    v
}

fn es_zoom(action: AppAction) -> bool {
    matches!(
        action,
        AppAction::ZoomIn | AppAction::ZoomOut | AppAction::ZoomFit | AppAction::ZoomActual
    )
}

/// Las 32 combinaciones de los cinco bloqueos.
fn bloqueos(bits: u8) -> Locks {
    Locks {
        gesture: bits & 1 != 0,
        dialog: bits & 2 != 0,
        text_focus: bits & 4 != 0,
        removal: bits & 8 != 0,
        modal: bits & 16 != 0,
    }
}

fn solo(f: impl FnOnce(&mut Locks)) -> Locks {
    let mut locks = Locks::default();
    f(&mut locks);
    locks
}

/// Estado con historia, zoom ×5 y todo lo demás en reposo: nada deshabilita por sí mismo.
fn estado(zoom: Option<Zoom>, locks: Locks) -> UiState {
    UiState {
        can_undo: true,
        can_redo: true,
        zoom,
        tool: Tool::Pencil,
        locks,
    }
}

fn zoom_x5() -> Option<Zoom> {
    Zoom::from_int(5)
}

// ---------------------------------------------------------------------------------------------
// Tipos y funciones sencillas
// ---------------------------------------------------------------------------------------------

fn assert_plano<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
fn assert_con_defecto<T: Default>() {}

#[test]
fn los_tipos_de_la_capa_de_acciones_son_planos() {
    assert_plano::<AppAction>();
    assert_plano::<Origin>();
    assert_plano::<Platform>();
    assert_plano::<Locks>();
    assert_con_defecto::<Locks>();
}

#[test]
fn locks_por_defecto_es_todo_falso() {
    let locks = Locks::default();
    assert!(!locks.gesture);
    assert!(!locks.dialog);
    assert!(!locks.text_focus);
    assert!(!locks.removal);
    assert!(!locks.modal);
}

#[test]
fn ignores_repeat_es_solo_open_save_saveas_y_quit() {
    for action in todas_las_acciones() {
        let esperado = matches!(
            action,
            AppAction::Open | AppAction::Save | AppAction::SaveAs | AppAction::Quit
        );
        assert_eq!(action.ignores_repeat(), esperado, "{action:?}");
    }
}

#[test]
fn is_zoom_es_solo_las_cuatro_de_zoom() {
    for action in todas_las_acciones() {
        assert_eq!(action.is_zoom(), es_zoom(action), "{action:?}");
    }
}

#[test]
fn platform_current_sigue_el_sistema_de_compilacion() {
    #[cfg(target_os = "linux")]
    assert_eq!(Platform::current(), Platform::Linux);
    #[cfg(target_os = "windows")]
    assert_eq!(Platform::current(), Platform::Windows);
    #[cfg(target_os = "macos")]
    assert_eq!(Platform::current(), Platform::Mac);
}

// ---------------------------------------------------------------------------------------------
// Fila 5: is_blocked en el orden de la spec
// ---------------------------------------------------------------------------------------------

/// Modelo de `is_blocked`, escrito aparte de la implementación y en el orden de la spec:
/// (1) `CloseLicenses` nunca; (2) `modal`: todo salvo `CloseLicenses` y `Quit`; (3) `dialog`:
/// todo; (4) zoom: con `text_focus` solo si el origen es `Key`; (5) el resto: `gesture` o
/// `text_focus`, y `removal` solo `SelectColor` y las acciones que abren un diálogo (hoy, solo
/// `ShowLicenses`; unidad 18, decisión D2); (6) nada.
fn bloqueada_segun_el_modelo(action: AppAction, locks: Locks, origin: Origin) -> bool {
    if action == AppAction::CloseLicenses {
        return false;
    }
    if locks.modal && action != AppAction::Quit {
        return true;
    }
    if locks.dialog {
        return true;
    }
    if es_zoom(action) {
        return locks.text_focus && origin == Origin::Key;
    }
    if locks.gesture || locks.text_focus {
        return true;
    }
    locks.removal && matches!(action, AppAction::SelectColor(_) | AppAction::ShowLicenses)
}

#[test]
fn fila_5_is_blocked_coincide_con_el_modelo_en_todas_las_combinaciones() {
    for action in todas_las_acciones() {
        for bits in 0..32 {
            let locks = bloqueos(bits);
            for origin in ORIGENES {
                assert_eq!(
                    is_blocked(action, locks, origin),
                    bloqueada_segun_el_modelo(action, locks, origin),
                    "{action:?} con {locks:?} y {origin:?}"
                );
            }
        }
    }
}

#[test]
fn fila_5_sin_bloqueos_nada_esta_bloqueado() {
    for action in todas_las_acciones() {
        for origin in ORIGENES {
            assert!(
                !is_blocked(action, Locks::default(), origin),
                "{action:?} con {origin:?}"
            );
        }
    }
}

#[test]
fn fila_5_regla_1_close_licenses_nunca_se_bloquea() {
    let todo = Locks {
        gesture: true,
        dialog: true,
        text_focus: true,
        removal: true,
        modal: true,
    };
    for origin in ORIGENES {
        assert!(!is_blocked(AppAction::CloseLicenses, todo, origin));
        for bits in 0..32 {
            assert!(!is_blocked(
                AppAction::CloseLicenses,
                bloqueos(bits),
                origin
            ));
        }
    }
}

#[test]
fn fila_5_regla_2_con_modal_solo_pasan_close_licenses_y_quit() {
    let locks = solo(|l| l.modal = true);
    for action in todas_las_acciones() {
        for origin in ORIGENES {
            let pasa = matches!(action, AppAction::CloseLicenses | AppAction::Quit);
            assert_eq!(
                is_blocked(action, locks, origin),
                !pasa,
                "{action:?} con modal y {origin:?}"
            );
        }
    }
}

#[test]
fn fila_5_regla_3_con_dialogo_todo_esta_bloqueado_tambien_el_zoom() {
    let locks = solo(|l| l.dialog = true);
    for action in todas_las_acciones() {
        for origin in ORIGENES {
            let esperado = action != AppAction::CloseLicenses;
            assert_eq!(
                is_blocked(action, locks, origin),
                esperado,
                "{action:?} con dialog y {origin:?}"
            );
        }
    }
}

#[test]
fn fila_5_regla_4_el_zoom_ignora_gesto_y_quita_y_solo_las_teclas_paran_con_texto() {
    for action in ZOOMS {
        for origin in ORIGENES {
            for locks in [
                solo(|l| l.gesture = true),
                solo(|l| l.removal = true),
                Locks {
                    gesture: true,
                    removal: true,
                    ..Locks::default()
                },
            ] {
                assert!(
                    !is_blocked(action, locks, origin),
                    "{action:?} con {locks:?} y {origin:?}"
                );
            }
            let texto = solo(|l| l.text_focus = true);
            assert_eq!(
                is_blocked(action, texto, origin),
                origin == Origin::Key,
                "{action:?} con text_focus y {origin:?}"
            );
            // Gesto y texto a la vez: manda el texto, que solo para a las teclas.
            let ambos = Locks {
                gesture: true,
                text_focus: true,
                ..Locks::default()
            };
            assert_eq!(
                is_blocked(action, ambos, origin),
                origin == Origin::Key,
                "{action:?} con gesture y text_focus y {origin:?}"
            );
        }
    }
}

#[test]
fn fila_5_regla_5_el_resto_para_con_gesto_o_texto_y_la_quita_solo_para_las_ranuras_y_los_dialogos()
{
    for action in todas_las_acciones() {
        if es_zoom(action) || action == AppAction::CloseLicenses {
            continue;
        }
        for origin in ORIGENES {
            assert!(
                is_blocked(action, solo(|l| l.gesture = true), origin),
                "{action:?} con gesture y {origin:?}"
            );
            assert!(
                is_blocked(action, solo(|l| l.text_focus = true), origin),
                "{action:?} con text_focus y {origin:?}"
            );
            assert_eq!(
                is_blocked(action, solo(|l| l.removal = true), origin),
                matches!(action, AppAction::SelectColor(_) | AppAction::ShowLicenses),
                "{action:?} con removal y {origin:?}"
            );
        }
    }
}

#[test]
fn fila_5_quit_y_show_licenses_tambien_las_frena_un_gesto_y_solo_show_licenses_una_quita() {
    for action in [AppAction::Quit, AppAction::ShowLicenses] {
        for origin in ORIGENES {
            assert!(is_blocked(action, solo(|l| l.gesture = true), origin));
            assert!(is_blocked(action, solo(|l| l.dialog = true), origin));
            assert!(is_blocked(action, solo(|l| l.text_focus = true), origin));
            // Una quita pendiente no frena `Quit`; sí frena abrir un diálogo (D2).
            assert_eq!(
                is_blocked(action, solo(|l| l.removal = true), origin),
                action == AppAction::ShowLicenses
            );
        }
    }
}

#[test]
fn fila_5_quit_con_modal_pasa_pero_un_gesto_a_la_vez_lo_para() {
    let modal = solo(|l| l.modal = true);
    let modal_y_gesto = Locks {
        modal: true,
        gesture: true,
        ..Locks::default()
    };
    for origin in ORIGENES {
        assert!(!is_blocked(AppAction::Quit, modal, origin));
        assert!(is_blocked(AppAction::Quit, modal_y_gesto, origin));
    }
}

#[test]
fn fila_5_enabled_es_falso_exactamente_donde_is_blocked_con_origen_menu() {
    // Con historia y zoom ×5, `enabled` solo depende del bloqueo en todas las acciones.
    for action in todas_las_acciones() {
        for bits in 0..32 {
            let locks = bloqueos(bits);
            assert_eq!(
                enabled(action, &estado(zoom_x5(), locks)),
                !is_blocked(action, locks, Origin::Menu),
                "{action:?} con {locks:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5b: bloqueos del zoom, uno por bloqueo
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_5b_a_b_y_c_gesto_deslizador_y_quita_no_bloquean_el_zoom_con_ningun_origen() {
    // (a) trazo del lápiz, (b) deslizador del editor y (c) quita pendiente: `gesture` es la
    // reunión de (a) y (b) y `removal` es (c).
    for locks in [
        solo(|l| l.gesture = true),
        solo(|l| l.removal = true),
        Locks {
            gesture: true,
            removal: true,
            ..Locks::default()
        },
    ] {
        for action in ZOOMS {
            for origin in ORIGENES {
                assert!(!is_blocked(action, locks, origin), "{action:?} {origin:?}");
            }
            assert!(
                enabled(action, &estado(zoom_x5(), locks)),
                "{action:?} con {locks:?}"
            );
        }
    }
}

#[test]
fn fila_5b_d_con_un_campo_enfocado_solo_las_teclas_se_bloquean() {
    let locks = solo(|l| l.text_focus = true);
    for action in ZOOMS {
        assert!(is_blocked(action, locks, Origin::Key), "{action:?}");
        assert!(!is_blocked(action, locks, Origin::Menu), "{action:?}");
        assert!(!is_blocked(action, locks, Origin::Button), "{action:?}");
        assert!(enabled(action, &estado(zoom_x5(), locks)), "{action:?}");
    }
}

#[test]
fn fila_5b_e_un_dialogo_abierto_bloquea_el_zoom_con_los_tres_origenes() {
    let locks = solo(|l| l.dialog = true);
    for action in ZOOMS {
        for origin in ORIGENES {
            assert!(is_blocked(action, locks, origin), "{action:?} {origin:?}");
        }
        assert!(!enabled(action, &estado(zoom_x5(), locks)), "{action:?}");
    }
}

#[test]
fn fila_5b_f_el_panel_de_licencias_bloquea_el_zoom_y_deja_pasar_solo_cerrar_y_salir() {
    let locks = solo(|l| l.modal = true);
    for action in ZOOMS {
        for origin in ORIGENES {
            assert!(is_blocked(action, locks, origin), "{action:?} {origin:?}");
        }
        assert!(!enabled(action, &estado(zoom_x5(), locks)), "{action:?}");
    }
    for origin in ORIGENES {
        assert!(!is_blocked(AppAction::CloseLicenses, locks, origin));
        assert!(!is_blocked(AppAction::Quit, locks, origin));
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3: action_for_shortcut
// ---------------------------------------------------------------------------------------------

/// Oráculo de `action_for_shortcut`, escrito aparte de la implementación.
///
/// Con `command` (y sin `Alt`): `Z` deshacer, `Mayús`+`Z` e `Y` rehacer, `S` guardar, `Mayús`+`S`
/// guardar como, `O` abrir, `N` nueva imagen (sin `Mayús`; unidad 19), `1` 1:1 (sin `Mayús`) y `Q`
/// salir solo en Linux (sin `Mayús`). Sin
/// `command`: `+`, `=`, `-` y `Home` con cualquier otro modificador, y con **ningún** modificador
/// `P L F I E` y `1` a `9`.
fn atajo_esperado(key: Key, m: Modifiers, platform: Platform) -> Option<AppAction> {
    if m.command {
        if m.alt {
            return None;
        }
        return match (key, m.shift) {
            (Key::Z, false) => Some(AppAction::Undo),
            (Key::Z, true) | (Key::Y, false) => Some(AppAction::Redo),
            (Key::S, false) => Some(AppAction::Save),
            (Key::S, true) => Some(AppAction::SaveAs),
            (Key::O, false) => Some(AppAction::Open),
            (Key::N, false) => Some(AppAction::NewImage),
            (Key::Num1, false) => Some(AppAction::ZoomActual),
            (Key::Q, false) if platform == Platform::Linux => Some(AppAction::Quit),
            _ => None,
        };
    }
    match key {
        Key::Plus | Key::Equals => return Some(AppAction::ZoomIn),
        Key::Minus => return Some(AppAction::ZoomOut),
        Key::Home => return Some(AppAction::ZoomFit),
        _ => {}
    }
    if !m.is_none() {
        return None;
    }
    match key {
        Key::P => Some(AppAction::SelectTool(Tool::Pencil)),
        Key::L => Some(AppAction::SelectTool(Tool::Line)),
        Key::F => Some(AppAction::SelectTool(Tool::Fill)),
        Key::I => Some(AppAction::SelectTool(Tool::Dropper)),
        Key::E => Some(AppAction::SelectTool(Tool::Eraser)),
        Key::Num1 => Some(AppAction::SelectColor(1)),
        Key::Num2 => Some(AppAction::SelectColor(2)),
        Key::Num3 => Some(AppAction::SelectColor(3)),
        Key::Num4 => Some(AppAction::SelectColor(4)),
        Key::Num5 => Some(AppAction::SelectColor(5)),
        Key::Num6 => Some(AppAction::SelectColor(6)),
        Key::Num7 => Some(AppAction::SelectColor(7)),
        Key::Num8 => Some(AppAction::SelectColor(8)),
        Key::Num9 => Some(AppAction::SelectColor(9)),
        _ => None,
    }
}

#[test]
fn fila_3_action_for_shortcut_sobre_key_all_por_32_combinaciones_y_3_plataformas() {
    assert!(!Key::ALL.is_empty());
    for platform in PLATAFORMAS {
        for &key in Key::ALL {
            for bits in 0..32 {
                let m = modificadores(bits);
                assert_eq!(
                    action_for_shortcut(key, m, platform),
                    atajo_esperado(key, m, platform),
                    "{key:?} con {m:?} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_3_escape_da_none_con_cualquier_combinacion() {
    // `Escape` no es una acción: lo ordena `Canvas::interact_ordered` con los eventos de trazo.
    for platform in PLATAFORMAS {
        for bits in 0..32 {
            assert_eq!(
                action_for_shortcut(Key::Escape, modificadores(bits), platform),
                None,
                "{platform:?} con {bits:#07b}"
            );
        }
    }
}

#[test]
fn fila_3_command_q_solo_sale_en_linux() {
    let linux = modificadores_command(false);
    assert_eq!(
        action_for_shortcut(Key::Q, linux, Platform::Linux),
        Some(AppAction::Quit)
    );
    assert_eq!(
        action_for_shortcut(Key::Q, Modifiers::COMMAND, Platform::Linux),
        Some(AppAction::Quit)
    );
    for platform in [Platform::Windows, Platform::Mac] {
        for bits in 0..32 {
            assert_eq!(
                action_for_shortcut(Key::Q, modificadores(bits), platform),
                None,
                "{platform:?} con {bits:#07b}"
            );
        }
    }
    // Sin `command`, la `Q` sola no hace nada en ninguna plataforma.
    for platform in PLATAFORMAS {
        assert_eq!(action_for_shortcut(Key::Q, Modifiers::NONE, platform), None);
    }
}

#[test]
fn fila_3_ctrl_mayus_q_y_alt_q_no_salen_en_linux() {
    // Decisión: `Salir` se trata como `Abrir` (`command` sin `Mayús` ni `Alt`).
    let mut con_mayus = modificadores_command(false);
    con_mayus.shift = true;
    let mut con_alt = modificadores_command(false);
    con_alt.alt = true;
    for m in [con_mayus, con_alt] {
        assert_eq!(
            action_for_shortcut(Key::Q, m, Platform::Linux),
            None,
            "{m:?}"
        );
    }
}

#[test]
fn fila_3_las_teclas_de_zoom_valen_sin_command_y_con_cualquier_otro_modificador() {
    for platform in PLATAFORMAS {
        for (key, accion) in [
            (Key::Plus, AppAction::ZoomIn),
            (Key::Equals, AppAction::ZoomIn),
            (Key::Minus, AppAction::ZoomOut),
            (Key::Home, AppAction::ZoomFit),
        ] {
            for bits in 0..32 {
                let m = modificadores(bits);
                let esperado = (!m.command).then_some(accion);
                assert_eq!(
                    action_for_shortcut(key, m, platform),
                    esperado,
                    "{key:?} con {m:?} en {platform:?}"
                );
            }
            // `+` se teclea con Mayús en muchas distribuciones: sigue siendo zoom.
            assert_eq!(
                action_for_shortcut(key, Modifiers::SHIFT, platform),
                Some(accion)
            );
        }
    }
}

#[test]
fn fila_3_cada_tecla_de_herramienta_y_de_ranura_vale_sin_modificadores_en_las_tres_plataformas() {
    for platform in PLATAFORMAS {
        for (key, tool) in [
            (Key::P, Tool::Pencil),
            (Key::L, Tool::Line),
            (Key::F, Tool::Fill),
            (Key::I, Tool::Dropper),
            (Key::E, Tool::Eraser),
        ] {
            assert_eq!(
                action_for_shortcut(key, Modifiers::NONE, platform),
                Some(AppAction::SelectTool(tool))
            );
            // Con cualquier modificador (también `command`) no hacen nada.
            for bits in 1..32 {
                let m = modificadores(bits);
                assert_eq!(
                    action_for_shortcut(key, m, platform),
                    None,
                    "{key:?} con {m:?} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_3_compone_las_tres_funciones_de_teclas_existentes_sin_cambiarlas() {
    for platform in PLATAFORMAS {
        for &key in Key::ALL {
            for bits in 0..32 {
                let m = modificadores(bits);
                let atajo = action_for_shortcut(key, m, platform);
                if let Some(file) = file_action_for_key(key, m) {
                    let esperado = match file {
                        FileAction::Open => AppAction::Open,
                        FileAction::Save => AppAction::Save,
                        FileAction::SaveAs => AppAction::SaveAs,
                    };
                    assert_eq!(atajo, Some(esperado), "{key:?} con {m:?}");
                }
                match tool_action_for_key(key, m) {
                    Some(ToolAction::Select(tool)) => {
                        assert_eq!(
                            atajo,
                            Some(AppAction::SelectTool(tool)),
                            "{key:?} con {m:?}"
                        );
                    }
                    Some(ToolAction::Cancel) => {
                        assert_eq!(atajo, None, "Escape no es una acción: {m:?}");
                    }
                    None => {}
                }
                match action_for_key(key, m) {
                    Some(Action::Undo) => assert_eq!(atajo, Some(AppAction::Undo)),
                    Some(Action::Redo) => assert_eq!(atajo, Some(AppAction::Redo)),
                    Some(Action::SelectColor(n)) => {
                        assert_eq!(atajo, Some(AppAction::SelectColor(n)), "{key:?} con {m:?}");
                    }
                    None => {}
                }
            }
        }
    }
}

#[test]
fn fila_3_las_tres_funciones_anteriores_siguen_dando_none_para_command_q_y_command_1() {
    // Sin choque: las funciones existentes no conocen las teclas nuevas.
    for m in [
        Modifiers::COMMAND,
        modificadores_command(false),
        modificadores_command(true),
    ] {
        for key in [Key::Q, Key::Num1] {
            assert_eq!(file_action_for_key(key, m), None, "{key:?} con {m:?}");
            assert_eq!(action_for_key(key, m), None, "{key:?} con {m:?}");
            assert_eq!(tool_action_for_key(key, m), None, "{key:?} con {m:?}");
        }
    }
}

/// Modificadores con los que se pulsa un `Shortcut` en `platform`.
fn pulsado(shortcut: Shortcut, platform: Platform) -> Modifiers {
    Modifiers {
        alt: false,
        ctrl: shortcut.command && platform != Platform::Mac,
        shift: shortcut.shift,
        mac_cmd: shortcut.command && platform == Platform::Mac,
        command: shortcut.command,
    }
}

#[test]
fn fila_3_cada_atajo_mostrado_da_la_accion_de_su_entrada_ida_y_vuelta() {
    let mut con_atajo = 0;
    for menu in MENUS.iter() {
        for item in menu.items {
            for platform in PLATAFORMAS {
                let Some(shortcut) = shortcut_for(item.action, platform) else {
                    continue;
                };
                con_atajo += 1;
                assert_eq!(
                    action_for_shortcut(shortcut.key, pulsado(shortcut, platform), platform),
                    Some(item.action),
                    "{} ({:?}) en {platform:?}",
                    item.label,
                    shortcut
                );
            }
        }
    }
    // 11 entradas con atajo en Linux y 10 en Windows y Mac (sin `Licencias` ni, fuera de Linux,
    // `Salir`; con `Nueva imagen…` de la unidad 19): 11 + 10 + 10 = 31 pares.
    assert_eq!(con_atajo, 11 + 10 + 10);
}

// ---------------------------------------------------------------------------------------------
// Fila 3b: las teclas 1 a 9 siguen eligiendo color
// ---------------------------------------------------------------------------------------------

const DIGITOS: [(Key, u8); 9] = [
    (Key::Num1, 1),
    (Key::Num2, 2),
    (Key::Num3, 3),
    (Key::Num4, 4),
    (Key::Num5, 5),
    (Key::Num6, 6),
    (Key::Num7, 7),
    (Key::Num8, 8),
    (Key::Num9, 9),
];

#[test]
fn fila_3b_los_digitos_sin_modificadores_dan_select_color_en_las_tres_plataformas() {
    for platform in PLATAFORMAS {
        for (key, slot) in DIGITOS {
            assert_eq!(
                action_for_shortcut(key, Modifiers::NONE, platform),
                Some(AppAction::SelectColor(slot)),
                "{key:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_3b_command_y_1_da_solo_zoom_actual_y_nunca_select_color() {
    for platform in PLATAFORMAS {
        for m in [
            Modifiers::COMMAND,
            modificadores_command(false),
            modificadores_command(true),
        ] {
            assert_eq!(
                action_for_shortcut(Key::Num1, m, platform),
                Some(AppAction::ZoomActual),
                "{m:?} en {platform:?}"
            );
        }
        // Ningún otro dígito con `command` hace nada.
        for (key, _) in DIGITOS.iter().filter(|(_, slot)| *slot != 1) {
            assert_eq!(
                action_for_shortcut(
                    *key,
                    modificadores_command(platform == Platform::Mac),
                    platform
                ),
                None,
                "{key:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_3b_num1_con_mayus_alt_o_combinados_da_none() {
    for platform in PLATAFORMAS {
        let command = modificadores_command(platform == Platform::Mac);
        let command_mayus = Modifiers {
            shift: true,
            ..command
        };
        let command_alt = Modifiers {
            alt: true,
            ..command
        };
        for (nombre, m) in [
            ("Mayús", Modifiers::SHIFT),
            ("Alt", Modifiers::ALT),
            ("command+Mayús", command_mayus),
            ("command+Alt", command_alt),
        ] {
            assert_eq!(
                action_for_shortcut(Key::Num1, m, platform),
                None,
                "{nombre} en {platform:?}"
            );
        }
    }
}
