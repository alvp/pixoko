//! Tests del bloqueo por diálogo modal en `px-ui` (specs/hito-2-dialogos-modales.md, «Casos» fila 6,
//! la parte pura: `dispatch`, `ignored_file_notice` y el cierre de ventana están en `px-app`).
//! Sin ventana: `Key` y `Modifiers` de egui y lógica pura.
//!
//! - (a) `can_open_dialog` con las 32 combinaciones de los cinco bloqueos.
//! - (b) `Modal::open` con cada bloqueo solo, con todos a falso, con uno ya abierto y con una spec
//!   inválida.
//! - (c) con `Locks { modal: true }`: `is_blocked` y `enabled` de **todas** las `AppAction`
//!   (herramientas, ranuras 1 a 9, los cuatro fondos, zoom, fichero, deshacer, rehacer y
//!   licencias) con los tres `Origin`: bloqueadas salvo `CloseLicenses` y `Quit` (decisión D1).
//! - (d) `action_for_shortcut` sobre `Key::ALL` × 32 combinaciones de modificadores × tres
//!   plataformas: lo traducido sigue sujeto a `is_blocked` con modal (solo `Quit` pasa, y solo
//!   `Ctrl+Q` en Linux lo traduce); `AppAction::opens_dialog()` es verdadero solo para
//!   `ShowLicenses`; y `is_blocked` bloquea con `removal` exactamente `SelectColor(_)` y las
//!   acciones que abren un diálogo (decisión 2, regla literal), con las demás acciones y los tres
//!   orígenes.
//!
//! Suposiciones sobre las firmas (derives no listados): `Dialog` es `Clone + PartialEq` y `Modal`
//! tiene `Default` (= `Closed`); no se exige nada más (para mirar dentro de un `Modal` abierto se
//! hace `match` sobre `Modal::Open(dialog)`). `Modal::open` con una spec inválida da `false` y
//! deja el `Modal` cerrado (spec: «false si `can_open_dialog` es falso o la spec no valida»).
//!
//! Decisión donde la spec no fija un detalle: `Locks` con `removal` solo no bloquea `Quit`, el
//! zoom, el fondo, deshacer ni rehacer (lo que ya comprobaban las pruebas de la 17); el modelo
//! completo de las 32 combinaciones está en `actions.rs`.

use egui::Key;
use px_ui::actions::{
    AppAction, Locks, Origin, Platform, UiState, action_for_shortcut, can_open_dialog, enabled,
    is_blocked,
};
use px_ui::background::CanvasBackground;
use px_ui::dialog::{Dialog, DialogKind, FieldKind, FieldSpec, FormSpec, Modal};
use px_ui::licenses_view::licenses_form;
use px_ui::tool::Tool;
use px_ui::view::Zoom;

mod common;
use common::{modificadores, modificadores_command};

#[path = "common/dialog_forms.rs"]
mod dialog_forms;

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

const ORIGENES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];
const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const HERRAMIENTAS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];
const FONDOS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];

/// Todas las acciones: fichero, salir, deshacer, rehacer, zoom, licencias (abrir y cerrar), las
/// cinco herramientas, las nueve ranuras y los cuatro fondos del lienzo.
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
    v.extend(FONDOS.map(AppAction::CanvasBackground));
    v
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

/// Los cinco bloqueos, uno por uno, con su nombre.
fn bloqueos_uno_a_uno() -> [(&'static str, Locks); 5] {
    [
        ("gesture", solo(|l| l.gesture = true)),
        ("dialog", solo(|l| l.dialog = true)),
        ("text_focus", solo(|l| l.text_focus = true)),
        ("removal", solo(|l| l.removal = true)),
        ("modal", solo(|l| l.modal = true)),
    ]
}

fn estado(locks: Locks) -> UiState {
    UiState {
        can_undo: true,
        can_redo: true,
        zoom: Zoom::from_int(5),
        tool: Tool::Pencil,
        locks,
    }
}

fn pasa_con_modal(action: AppAction) -> bool {
    matches!(action, AppAction::CloseLicenses | AppAction::Quit)
}

/// Una spec inválida: dos campos con el mismo id.
fn spec_invalida() -> FormSpec {
    let casilla = |label| FieldSpec {
        id: "repetido",
        label,
        kind: FieldKind::Check { default: false },
        shown_when: None,
    };
    FormSpec {
        title: "Inválida",
        accept: "Aceptar",
        cancel: Some("Cancelar"),
        fields: vec![casilla("Uno"), casilla("Dos")],
        check: None,
    }
}

fn abierto(modal: &Modal) -> Option<&Dialog> {
    match modal {
        Modal::Open(dialog) => Some(dialog),
        Modal::Closed => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6 (a): can_open_dialog
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_6a_can_open_dialog_es_cierto_solo_con_los_cinco_bloqueos_en_falso() {
    let mut ciertos = 0;
    for bits in 0..32u8 {
        let locks = bloqueos(bits);
        let esperado =
            !(locks.gesture || locks.dialog || locks.text_focus || locks.removal || locks.modal);
        assert_eq!(can_open_dialog(locks), esperado, "{locks:?}");
        assert_eq!(can_open_dialog(locks), bits == 0, "{locks:?}");
        ciertos += usize::from(can_open_dialog(locks));
    }
    assert_eq!(ciertos, 1, "solo una de las 32 combinaciones permite abrir");
}

#[test]
fn fila_6a_con_todo_en_falso_se_puede_abrir() {
    assert!(can_open_dialog(Locks::default()));
}

#[test]
fn fila_6a_cada_bloqueo_solo_impide_abrir_y_todos_juntos_tambien() {
    for (nombre, locks) in bloqueos_uno_a_uno() {
        assert!(!can_open_dialog(locks), "{nombre}");
    }
    let todos = Locks {
        gesture: true,
        dialog: true,
        text_focus: true,
        removal: true,
        modal: true,
    };
    assert!(!can_open_dialog(todos));
}

// ---------------------------------------------------------------------------------------------
// Fila 6 (b): Modal::open
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_6b_open_con_todo_a_falso_abre_el_dialogo() {
    let mut modal = Modal::default();
    assert!(!modal.is_open());
    assert_eq!(modal.kind(), None);
    assert!(modal.open(
        DialogKind::Licenses,
        licenses_form("texto"),
        Locks::default()
    ));
    assert!(modal.is_open());
    assert_eq!(modal.kind(), Some(DialogKind::Licenses));
    assert!(abierto(&modal).is_some());
}

#[test]
fn fila_6b_open_con_cada_bloqueo_solo_da_false_y_sigue_closed() {
    for (nombre, locks) in bloqueos_uno_a_uno() {
        let mut modal = Modal::default();
        assert!(
            !modal.open(DialogKind::Licenses, licenses_form("texto"), locks),
            "{nombre}"
        );
        assert!(!modal.is_open(), "{nombre}");
        assert_eq!(modal.kind(), None, "{nombre}");
        assert!(matches!(modal, Modal::Closed), "{nombre}");
    }
}

#[test]
fn fila_6b_open_con_las_32_combinaciones_abre_solo_con_todo_en_falso() {
    for bits in 0..32u8 {
        let locks = bloqueos(bits);
        let mut modal = Modal::default();
        let abre = modal.open(DialogKind::Licenses, licenses_form("texto"), locks);
        assert_eq!(abre, bits == 0, "{locks:?}");
        assert_eq!(modal.is_open(), bits == 0, "{locks:?}");
        assert_eq!(modal.kind().is_some(), bits == 0, "{locks:?}");
    }
}

#[test]
fn fila_6b_con_uno_abierto_otro_open_da_false_con_cualquier_bloqueo_y_no_toca_sus_valores() {
    for bits in 0..32u8 {
        let locks = bloqueos(bits);
        let mut modal = Modal::default();
        assert!(modal.open(
            DialogKind::Licenses,
            dialog_forms::all_kinds(),
            Locks::default()
        ));
        let antes = abierto(&modal).expect("abierto").clone();
        // La app pone `modal: true` mientras hay uno abierto, pero la regla también se sostiene
        // sin ese bloqueo: un segundo `open` nunca sustituye al primero.
        assert!(
            !modal.open(DialogKind::Licenses, licenses_form("otro"), locks),
            "{locks:?}"
        );
        assert!(modal.is_open(), "{locks:?}");
        assert_eq!(
            abierto(&modal).expect("sigue abierto"),
            &antes,
            "{locks:?}: el diálogo abierto no cambia"
        );
    }
}

#[test]
fn fila_6b_open_con_el_bloqueo_modal_del_propio_dialogo_abierto_da_false() {
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::Licenses, licenses_form("uno"), Locks::default()));
    let con_modal = solo(|l| l.modal = modal.is_open());
    assert!(con_modal.modal);
    assert!(!modal.open(DialogKind::Licenses, licenses_form("dos"), con_modal));
    assert!(modal.is_open());
}

#[test]
fn fila_6b_open_con_una_spec_invalida_da_false_y_sigue_closed() {
    let mut modal = Modal::default();
    assert!(spec_invalida().validate().is_err());
    assert!(!modal.open(DialogKind::Licenses, spec_invalida(), Locks::default()));
    assert!(!modal.is_open());
    assert!(matches!(modal, Modal::Closed));
}

#[test]
fn fila_6b_se_puede_volver_a_abrir_despues_de_cerrar() {
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::Licenses, licenses_form("uno"), Locks::default()));
    assert!(modal.abandon());
    assert!(!modal.is_open());
    // Cerrado, con un bloqueo no abre; sin bloqueos, sí.
    assert!(!modal.open(
        DialogKind::Licenses,
        licenses_form("dos"),
        solo(|l| l.gesture = true)
    ));
    assert!(modal.open(DialogKind::Licenses, licenses_form("dos"), Locks::default()));
}

// ---------------------------------------------------------------------------------------------
// Fila 6 (c): con modal, todo bloqueado salvo CloseLicenses y Quit
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_6c_con_modal_is_blocked_es_cierto_salvo_para_close_licenses_y_quit() {
    let locks = solo(|l| l.modal = true);
    let acciones = todas_las_acciones();
    // 12 + 5 herramientas + 9 ranuras + 4 fondos.
    assert_eq!(acciones.len(), 30);
    for action in acciones {
        for origin in ORIGENES {
            assert_eq!(
                is_blocked(action, locks, origin),
                !pasa_con_modal(action),
                "{action:?} con modal y {origin:?}"
            );
        }
    }
}

#[test]
fn fila_6c_con_modal_enabled_es_falso_salvo_para_close_licenses_y_quit() {
    let locks = solo(|l| l.modal = true);
    for action in todas_las_acciones() {
        assert_eq!(
            enabled(action, &estado(locks)),
            pasa_con_modal(action),
            "{action:?} con modal"
        );
    }
}

#[test]
fn fila_6c_enabled_con_modal_no_depende_de_la_historia_ni_del_zoom_ni_de_la_herramienta() {
    let locks = solo(|l| l.modal = true);
    for can_undo in [false, true] {
        for can_redo in [false, true] {
            for zoom in [None, Zoom::from_int(1), Zoom::from_int(5), Some(Zoom::MAX)] {
                for tool in HERRAMIENTAS {
                    let state = UiState {
                        can_undo,
                        can_redo,
                        zoom,
                        tool,
                        locks,
                    };
                    for action in todas_las_acciones() {
                        // `CloseLicenses` y `Quit` no dependen de la historia ni del zoom.
                        assert_eq!(
                            enabled(action, &state),
                            pasa_con_modal(action),
                            "{action:?} con {state:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn fila_6c_quit_y_close_licenses_pasan_con_modal_y_con_los_tres_origenes() {
    let locks = solo(|l| l.modal = true);
    for action in [AppAction::Quit, AppAction::CloseLicenses] {
        for origin in ORIGENES {
            assert!(!is_blocked(action, locks, origin), "{action:?} {origin:?}");
        }
        assert!(enabled(action, &estado(locks)), "{action:?}");
    }
}

#[test]
fn fila_6c_cada_accion_bloqueada_con_modal_tambien_lo_esta_con_el_menu_el_boton_y_la_tecla() {
    // Las ranuras 1 a 9, las cinco herramientas, los cuatro fondos y las cuatro de zoom, una a
    // una (el zoom con el campo de texto sin foco: solo el modal lo bloquea).
    let locks = solo(|l| l.modal = true);
    let sin_modal = Locks::default();
    for action in todas_las_acciones() {
        if pasa_con_modal(action) {
            continue;
        }
        for origin in ORIGENES {
            assert!(!is_blocked(action, sin_modal, origin), "{action:?} libre");
            assert!(is_blocked(action, locks, origin), "{action:?} {origin:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6 (d): teclas con modal
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_6d_lo_que_traduce_action_for_shortcut_sigue_bloqueado_con_modal_salvo_quit() {
    let locks = solo(|l| l.modal = true);
    let mut traducidas = 0usize;
    let mut quits = 0usize;
    for key in Key::ALL {
        for bits in 0..32u8 {
            let modifiers = modificadores(bits);
            for platform in PLATAFORMAS {
                let Some(action) = action_for_shortcut(*key, modifiers, platform) else {
                    continue;
                };
                traducidas += 1;
                let contexto = format!("{key:?} con {modifiers:?} en {platform:?}: {action:?}");
                for origin in ORIGENES {
                    assert_eq!(
                        is_blocked(action, locks, origin),
                        action != AppAction::Quit,
                        "{contexto} ({origin:?})"
                    );
                }
                // Las teclas llegan con `Origin::Key`; el menú las ve con `enabled`.
                assert_eq!(
                    enabled(action, &estado(locks)),
                    action == AppAction::Quit,
                    "{contexto}"
                );
                assert_ne!(
                    action,
                    AppAction::CloseLicenses,
                    "{contexto}: ningún atajo cierra las licencias"
                );
                quits += usize::from(action == AppAction::Quit);
            }
        }
    }
    assert!(traducidas > 100, "se tradujeron {traducidas} combinaciones");
    assert!(quits > 0, "hay combinaciones que dan Quit");
}

#[test]
fn fila_6d_solo_ctrl_q_en_linux_da_quit_y_es_lo_unico_que_pasa_con_modal() {
    let locks = solo(|l| l.modal = true);
    for key in Key::ALL {
        for bits in 0..32u8 {
            let modifiers = modificadores(bits);
            for platform in PLATAFORMAS {
                let pasa = action_for_shortcut(*key, modifiers, platform)
                    .is_some_and(|a| !is_blocked(a, locks, Origin::Key));
                if pasa {
                    assert_eq!(*key, Key::Q, "{key:?} {modifiers:?} {platform:?}");
                    assert_eq!(platform, Platform::Linux, "{key:?} {modifiers:?}");
                    assert!(modifiers.command, "{key:?} {modifiers:?}");
                    assert!(!modifiers.alt && !modifiers.shift, "{key:?} {modifiers:?}");
                }
            }
        }
    }
    assert_eq!(
        action_for_shortcut(Key::Q, modificadores_command(false), Platform::Linux),
        Some(AppAction::Quit)
    );
    assert!(!is_blocked(AppAction::Quit, locks, Origin::Key));
    // En Windows y macOS `Ctrl`/`Cmd`+`Q` no es un atajo de la app.
    assert_eq!(
        action_for_shortcut(Key::Q, modificadores_command(false), Platform::Windows),
        None
    );
    assert_eq!(
        action_for_shortcut(Key::Q, modificadores_command(true), Platform::Mac),
        None
    );
}

#[test]
fn fila_6d_las_teclas_de_herramienta_color_zoom_y_fichero_estan_bloqueadas_con_modal() {
    let locks = solo(|l| l.modal = true);
    let casos = [
        (Key::D, modificadores(0)),
        (Key::CloseBracket, modificadores(0)),
        (Key::OpenBracket, modificadores(0)),
        (Key::Plus, modificadores(0)),
        (Key::Minus, modificadores(0)),
        (Key::Home, modificadores(0)),
        (Key::Z, modificadores_command(false)),
        (Key::O, modificadores_command(false)),
        (Key::S, modificadores_command(false)),
        (Key::Num1, modificadores_command(false)),
    ];
    for (key, modifiers) in casos {
        let action = action_for_shortcut(key, modifiers, Platform::Linux)
            .unwrap_or_else(|| panic!("{key:?} con {modifiers:?} debería traducirse"));
        assert!(
            is_blocked(action, locks, Origin::Key),
            "{key:?}: {action:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6 (d): opens_dialog y la regla de la quita pendiente (decisión 2)
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_6d_opens_dialog_es_cierto_solo_para_show_licenses() {
    for action in todas_las_acciones() {
        assert_eq!(
            action.opens_dialog(),
            action == AppAction::ShowLicenses,
            "{action:?}"
        );
    }
}

#[test]
fn fila_6d_con_removal_is_blocked_bloquea_exactamente_select_color_y_show_licenses() {
    let locks = solo(|l| l.removal = true);
    let mut bloqueadas = Vec::new();
    for action in todas_las_acciones() {
        for origin in ORIGENES {
            let esperado = matches!(action, AppAction::SelectColor(_) | AppAction::ShowLicenses);
            assert_eq!(
                is_blocked(action, locks, origin),
                esperado,
                "{action:?} con removal y {origin:?}"
            );
        }
        if is_blocked(action, locks, Origin::Menu) {
            bloqueadas.push(action);
        }
    }
    // Las nueve ranuras y `ShowLicenses`, nada más.
    assert_eq!(bloqueadas.len(), 10, "{bloqueadas:?}");
}

#[test]
fn fila_6d_la_regla_5_es_select_color_o_una_accion_que_abre_un_dialogo() {
    let locks = solo(|l| l.removal = true);
    for action in todas_las_acciones() {
        for origin in ORIGENES {
            assert_eq!(
                is_blocked(action, locks, origin),
                matches!(action, AppAction::SelectColor(_)) || action.opens_dialog(),
                "{action:?} {origin:?}"
            );
        }
    }
}

#[test]
fn fila_6d_con_removal_siguen_libres_quit_zoom_fondo_deshacer_y_rehacer_y_herramientas() {
    let locks = solo(|l| l.removal = true);
    let libres = [
        AppAction::Quit,
        AppAction::CloseLicenses,
        AppAction::Undo,
        AppAction::Redo,
        AppAction::Open,
        AppAction::Save,
        AppAction::SaveAs,
        AppAction::ZoomIn,
        AppAction::ZoomOut,
        AppAction::ZoomFit,
        AppAction::ZoomActual,
        AppAction::SelectTool(Tool::Fill),
        AppAction::CanvasBackground(CanvasBackground::Black),
    ];
    for action in libres {
        for origin in ORIGENES {
            assert!(!is_blocked(action, locks, origin), "{action:?} {origin:?}");
        }
        assert!(enabled(action, &estado(locks)), "{action:?}");
    }
    assert!(!enabled(AppAction::ShowLicenses, &estado(locks)));
    assert!(!enabled(AppAction::SelectColor(1), &estado(locks)));
}

#[test]
fn fila_6d_show_licenses_esta_bloqueada_exactamente_cuando_no_se_puede_abrir_un_dialogo() {
    for bits in 0..32u8 {
        let locks = bloqueos(bits);
        for origin in ORIGENES {
            assert_eq!(
                is_blocked(AppAction::ShowLicenses, locks, origin),
                !can_open_dialog(locks),
                "{locks:?} {origin:?}"
            );
        }
        assert_eq!(
            enabled(AppAction::ShowLicenses, &estado(locks)),
            can_open_dialog(locks),
            "{locks:?}"
        );
    }
}

#[test]
fn fila_6d_ninguna_accion_que_abre_un_dialogo_se_deja_pasar_si_no_se_puede_abrir() {
    for action in todas_las_acciones()
        .into_iter()
        .filter(|a| a.opens_dialog())
    {
        for bits in 0..32u8 {
            let locks = bloqueos(bits);
            for origin in ORIGENES {
                if !can_open_dialog(locks) {
                    assert!(
                        is_blocked(action, locks, origin),
                        "{action:?} {locks:?} {origin:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn fila_6d_cada_bloqueo_solo_frena_show_licenses_con_los_tres_origenes() {
    for (nombre, locks) in bloqueos_uno_a_uno() {
        for origin in ORIGENES {
            assert!(
                is_blocked(AppAction::ShowLicenses, locks, origin),
                "{nombre} {origin:?}"
            );
        }
    }
    for origin in ORIGENES {
        assert!(!is_blocked(
            AppAction::ShowLicenses,
            Locks::default(),
            origin
        ));
    }
}
