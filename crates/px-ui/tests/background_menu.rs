//! Tests del menú «Fondo del lienzo» y de los bloqueos de su acción (specs/hito-2-damero-lienzo.md,
//! «Casos» fila 12 y, de la fila 13, la parte de `is_blocked` y `enabled`; el despacho de la fila 13
//! está en `px-app/src/dispatch/tests.rs`). Es lógica pura: la tabla `MENUS`, `Menu::submenus`,
//! `background_checked`, `shortcut_for`, `action_for_shortcut`, `is_blocked` y `enabled` no usan
//! ventana; solo los rótulos con la fuente real y la comprobación de que `show_menu_with_background`
//! se pinta usan un `egui::Context` con `RawInput` sintético (sin GPU ni capturas).
//!
//! - Fila 12: `Ver` conserva sus 4 entradas y gana **un** submenú `Fondo del lienzo` con `Damero`,
//!   `Negro`, `Gris` y `Blanco` en ese orden; el resto de menús sin submenús; `Menu::items` no
//!   cambia (las 11 entradas de la 17); `background_checked` cierto solo para la acción del fondo
//!   activo; ningún atajo en ninguna plataforma; rótulos dentro de `INTERFACE_CHARSET`, sin `?`,
//!   que `printable` deja igual y sin repetir los de `MENUS`.
//! - Fila 13 (parte pura): `AppAction::CanvasBackground` se bloquea **solo** con `dialog`, con `modal`
//!   y con `text_focus` (desde cualquier origen) y no con `gesture` ni con `removal`; no es zoom ni
//!   repite; `enabled` es lo contrario de `is_blocked` con `Origin::Menu`; las acciones anteriores
//!   se bloquean igual que antes (con el modelo de `actions.rs`).
//! - `show_menu` conserva su firma y es `show_menu_with_background` con el fondo por defecto.
//!
//! Unidad 19b (specs/hito-2-fondo-color.md), al final del fichero: la fila 2 (`Color…` como quinta
//! entrada, la marca siempre en una sola entrada con `Custom`, sin atajo, `submenu_width_u` = 131 u
//! con cualquier fondo activo y a las cinco escalas, el valor `#rrggbb` pintado solo con `Custom` y
//! con contraste) y la parte pura de la fila 3 (`BackgroundColor` es `opens_dialog`: se bloquea con
//! los cinco bloqueos; las cuatro fijas siguen sin bloquearse con `gesture` ni `removal`).
//!
//! Suposiciones sobre las firmas de «Piezas»: `Submenu` es `Clone, Copy, Debug, PartialEq, Eq` (como
//! `Menu`, que lo contiene) y `AppAction` sigue siendo `Clone, Copy, Debug, PartialEq, Eq`. La marca
//! de la entrada activa dentro del desplegable no se prueba aquí (es de la prueba manual: depende de
//! cómo se dibuje); sí `background_checked`, que decide dónde va.

use egui::epaint::{ClippedShape, Shape};
use egui::{Color32, Context, Event, Key, Pos2, RawInput, Rect, Ui, Vec2, vec2};
use px_core::Rgba;
use px_ui::actions::{
    AppAction, Locks, Origin, Platform, UiState, action_for_shortcut, can_open_dialog, enabled,
    is_blocked,
};
use px_ui::background::{CanvasBackground, background_value_text};
use px_ui::canvas::BACKGROUND;
use px_ui::menu::{
    MENUS, Menu, MenuItem, Submenu, background_checked, shortcut_for, shortcut_text, show_menu,
    show_menu_with_background, submenu_width_u,
};
use px_ui::text::{INTERFACE_CHARSET, apply_scale, printable, replace_missing, ui_unit};
use px_ui::theme::{contrast_ratio, visuals};
use px_ui::tool::Tool;
use px_ui::view::Zoom;

mod common;
use common::{PPPS, boton, contexto, fotograma, modificadores, mover, pintado, tolerancia_de_egui};

const FONDOS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];
const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const ORIGENES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];
const HERRAMIENTAS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

/// Las 11 entradas de `Menu::items`, en el orden del menú (las de la unidad 17).
const ACCIONES_DE_LA_TABLA: [AppAction; 11] = [
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
];

fn accion(b: CanvasBackground) -> AppAction {
    AppAction::CanvasBackground(b)
}

fn las_cuatro() -> [AppAction; 4] {
    FONDOS.map(accion)
}

/// Las acciones que ya existían: las 11 de la tabla, `CloseLicenses`, las cinco herramientas y las
/// nueve ranuras de color.
fn acciones_anteriores() -> Vec<AppAction> {
    let mut v = ACCIONES_DE_LA_TABLA.to_vec();
    v.push(AppAction::CloseLicenses);
    v.extend(HERRAMIENTAS.map(AppAction::SelectTool));
    v.extend((1..=9).map(AppAction::SelectColor));
    v
}

fn todas_las_acciones() -> Vec<AppAction> {
    let mut v = acciones_anteriores();
    v.extend(las_cuatro());
    v
}

fn el_menu(titulo: &str) -> &'static Menu {
    MENUS
        .iter()
        .find(|m| m.title == titulo)
        .unwrap_or_else(|| panic!("no hay menú {titulo:?}"))
}

fn rotulos_y_acciones(items: &[MenuItem]) -> Vec<(&'static str, AppAction)> {
    items.iter().map(|i| (i.label, i.action)).collect()
}

fn bloqueos(bits: u8) -> Locks {
    Locks {
        gesture: bits & 1 != 0,
        dialog: bits & 2 != 0,
        text_focus: bits & 4 != 0,
        removal: bits & 8 != 0,
        modal: bits & 16 != 0,
    }
}

fn assert_plano<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}

// ---------------------------------------------------------------------------------------------
// Fila 12: la tabla
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_los_tipos_son_planos() {
    assert_plano::<AppAction>();
    assert_plano::<CanvasBackground>();
    assert_plano::<Menu>();
    assert_plano::<Submenu>();
    assert_plano::<MenuItem>();
}

#[test]
fn fila_12_ver_conserva_sus_4_entradas() {
    let ver = el_menu("Ver");
    assert_eq!(
        rotulos_y_acciones(ver.items),
        [
            ("Acercar", AppAction::ZoomIn),
            ("Alejar", AppAction::ZoomOut),
            ("Ajustar a la ventana", AppAction::ZoomFit),
            ("1:1", AppAction::ZoomActual),
        ]
    );
}

#[test]
fn fila_12_ver_gana_un_submenu_fondo_del_lienzo_con_las_cinco_opciones_en_orden() {
    let ver = el_menu("Ver");
    assert_eq!(ver.submenus.len(), 1, "un solo submenú");
    let fondo = &ver.submenus[0];
    assert_eq!(fondo.label, "Fondo del lienzo");
    assert_eq!(
        rotulos_y_acciones(fondo.items),
        [
            (
                "Damero",
                AppAction::CanvasBackground(CanvasBackground::Checker)
            ),
            (
                "Negro",
                AppAction::CanvasBackground(CanvasBackground::Black)
            ),
            ("Gris", AppAction::CanvasBackground(CanvasBackground::Gray)),
            (
                "Blanco",
                AppAction::CanvasBackground(CanvasBackground::White)
            ),
            ("Color…", AppAction::BackgroundColor),
        ]
    );
    for item in fondo.items {
        assert!(!item.separator_before, "{}", item.label);
    }
}

#[test]
fn fila_12_el_resto_de_menus_no_tiene_submenus_y_son_cinco_en_orden() {
    let titulos: Vec<&str> = MENUS.iter().map(|m| m.title).collect();
    assert_eq!(titulos, ["Archivo", "Editar", "Imagen", "Ver", "Ayuda"]);
    for titulo in ["Archivo", "Editar", "Imagen", "Ayuda"] {
        assert!(el_menu(titulo).submenus.is_empty(), "{titulo}");
    }
}

#[test]
fn fila_12_menu_items_son_las_11_entradas_de_la_17_nueva_imagen_de_la_19_y_convertir_de_la_20() {
    let acciones: Vec<AppAction> = MENUS
        .iter()
        .flat_map(|m| m.items.iter().map(|i| i.action))
        .collect();
    // `Nueva imagen…` (unidad 19) es la primera y `Convertir modo…` (unidad 20) va tras
    // `Rehacer`; las otras 11 son las de la 17, sin cambios.
    let mut esperado = vec![AppAction::NewImage];
    esperado.extend(&ACCIONES_DE_LA_TABLA[..6]);
    esperado.push(AppAction::ConvertMode);
    esperado.extend(&ACCIONES_DE_LA_TABLA[6..]);
    assert_eq!(acciones, esperado);
    // Ninguna acción del fondo cuela en `items`.
    for b in FONDOS {
        assert!(!acciones.contains(&accion(b)));
    }
    // Los rótulos y los separadores de la 17, como estaban.
    let archivo = el_menu("Archivo");
    assert_eq!(
        rotulos_y_acciones(archivo.items),
        [
            ("Nueva imagen…", AppAction::NewImage),
            ("Abrir…", AppAction::Open),
            ("Guardar", AppAction::Save),
            ("Guardar como…", AppAction::SaveAs),
            ("Salir", AppAction::Quit),
        ]
    );
    assert_eq!(
        rotulos_y_acciones(el_menu("Editar").items),
        [("Deshacer", AppAction::Undo), ("Rehacer", AppAction::Redo)]
    );
    assert_eq!(
        rotulos_y_acciones(el_menu("Imagen").items),
        [("Convertir modo…", AppAction::ConvertMode)]
    );
    assert_eq!(
        rotulos_y_acciones(el_menu("Ayuda").items),
        [("Licencias", AppAction::ShowLicenses)]
    );
}

#[test]
fn fila_12_las_cuatro_acciones_del_submenu_son_distintas_y_no_estan_en_items() {
    let ver = el_menu("Ver");
    let acciones: Vec<AppAction> = ver.submenus[0].items.iter().map(|i| i.action).collect();
    // Las cuatro fijas y, al final, `Color…` (unidad 19b).
    let mut esperado = las_cuatro().to_vec();
    esperado.push(AppAction::BackgroundColor);
    assert_eq!(acciones, esperado);
    for (i, a) in acciones.iter().enumerate() {
        assert!(!acciones[i + 1..].contains(a), "{a:?} repetida");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: la marca
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_background_checked_es_cierto_solo_para_la_accion_del_fondo_activo() {
    for activo in FONDOS {
        for action in todas_las_acciones() {
            assert_eq!(
                background_checked(action, activo),
                action == accion(activo),
                "{action:?} con {activo:?} activo"
            );
        }
    }
}

#[test]
fn fila_12_con_cada_fondo_activo_hay_exactamente_una_marca_en_el_submenu() {
    let ver = el_menu("Ver");
    for activo in FONDOS {
        let marcadas: Vec<&str> = ver.submenus[0]
            .items
            .iter()
            .filter(|i| background_checked(i.action, activo))
            .map(|i| i.label)
            .collect();
        let esperado = match activo {
            CanvasBackground::Checker => "Damero",
            CanvasBackground::Black => "Negro",
            CanvasBackground::Gray => "Gris",
            CanvasBackground::White => "Blanco",
            // Con un color propio la marca va en `Color…` (unidad 19b).
            CanvasBackground::Custom(_) => "Color…",
        };
        assert_eq!(marcadas, [esperado], "{activo:?} activo");
    }
}

#[test]
fn fila_12_background_checked_es_falso_para_las_entradas_de_los_menus_de_la_17() {
    for activo in FONDOS {
        for menu in MENUS.iter() {
            for item in menu.items {
                assert!(!background_checked(item.action, activo), "{}", item.label);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: sin atajo
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_ningun_atajo_en_ninguna_plataforma() {
    for platform in PLATAFORMAS {
        for b in FONDOS {
            assert_eq!(
                shortcut_for(accion(b), platform),
                None,
                "{b:?} en {platform:?}"
            );
        }
    }
}

#[test]
fn fila_12_ninguna_tecla_con_ningun_modificador_da_una_accion_de_fondo() {
    for platform in PLATAFORMAS {
        for key in Key::ALL {
            for bits in 0..32u8 {
                let resultado = action_for_shortcut(*key, modificadores(bits), platform);
                assert!(
                    !matches!(resultado, Some(AppAction::CanvasBackground(_))),
                    "{key:?} con modificadores {bits:#07b} en {platform:?}: {resultado:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: rótulos
// ---------------------------------------------------------------------------------------------

/// Los rótulos nuevos: el del submenú y los de sus cuatro entradas.
fn rotulos_nuevos() -> Vec<&'static str> {
    let fondo = &el_menu("Ver").submenus[0];
    let mut v = vec![fondo.label];
    v.extend(fondo.items.iter().map(|i| i.label));
    v
}

#[test]
fn fila_12_los_rotulos_nuevos_estan_dentro_de_interface_charset() {
    for texto in rotulos_nuevos() {
        for c in texto.chars() {
            assert!(
                INTERFACE_CHARSET.contains(c),
                "{c:?} de {texto:?} no está en INTERFACE_CHARSET"
            );
        }
    }
}

#[test]
fn fila_12_ningun_rotulo_nuevo_pierde_un_caracter_ni_lleva_un_interrogante() {
    let tiene = |c: char| INTERFACE_CHARSET.contains(c);
    for texto in rotulos_nuevos() {
        assert_eq!(replace_missing(texto, tiene), texto);
        assert!(!texto.contains('?'), "{texto:?} lleva un `?`");
        assert!(!texto.is_empty());
    }
}

#[test]
fn fila_12_con_la_fuente_real_printable_no_cambia_ningun_rotulo_nuevo() {
    let ctx = contexto(1.0);
    for texto in rotulos_nuevos() {
        assert_eq!(printable(&ctx, texto), texto);
    }
}

#[test]
fn fila_12_sin_rotulos_repetidos_con_los_de_menus() {
    // Todos los rótulos de la barra: títulos, entradas y los nuevos.
    let mut todos: Vec<&str> = MENUS.iter().map(|m| m.title).collect();
    todos.extend(MENUS.iter().flat_map(|m| m.items.iter().map(|i| i.label)));
    let n_anteriores = todos.len();
    todos.extend(rotulos_nuevos());
    for (i, r) in todos.iter().enumerate() {
        assert!(!todos[i + 1..].contains(r), "rótulo repetido: {r}");
    }
    assert_eq!(
        todos.len(),
        n_anteriores + 6,
        "el submenú y sus cinco entradas"
    );
}

#[test]
fn fila_12_el_texto_de_los_atajos_de_la_17_no_cambia() {
    // `shortcut_text` y `shortcut_for` de las entradas de siempre, como estaban.
    let s = shortcut_for(AppAction::ZoomActual, Platform::Linux).expect("tiene atajo");
    assert_eq!(shortcut_text(s, Platform::Linux), "Ctrl+1");
    assert_eq!(shortcut_for(AppAction::ShowLicenses, Platform::Linux), None);
}

// ---------------------------------------------------------------------------------------------
// show_menu y show_menu_with_background
// ---------------------------------------------------------------------------------------------

#[test]
fn show_menu_conserva_su_firma_y_la_version_con_el_fondo_es_nueva() {
    let _: fn(&mut Ui, &UiState, Platform) -> Option<AppAction> = show_menu;
    let _: fn(&mut Ui, &UiState, Platform, CanvasBackground) -> Option<AppAction> =
        show_menu_with_background;
}

fn estado() -> UiState {
    UiState {
        can_undo: true,
        can_redo: false,
        zoom: Zoom::from_int(5),
        tool: Tool::Pencil,
        locks: Locks::default(),
    }
}

#[test]
fn show_menu_with_background_pinta_la_barra_con_cada_fondo_activo_como_show_menu() {
    let pantalla = egui::vec2(400.0, 300.0);
    for ppp in common::PPPS {
        let ctx = contexto(ppp);
        let estado = estado();
        let mut elegida = None;
        let con = fotograma(&ctx, pantalla, vec![], |ui| {
            elegida = show_menu(ui, &estado, Platform::Linux);
        });
        assert_eq!(elegida, None, "sin pulsar nada no se elige nada");
        let con = pintado(&con);
        for titulo in ["Archivo", "Editar", "Ver", "Ayuda"] {
            assert!(con.pinta(titulo), "ppp {ppp}: falta el título {titulo:?}");
        }
        for fondo in FONDOS {
            let mut elegida = None;
            let ctx = contexto(ppp);
            let otro = fotograma(&ctx, pantalla, vec![], |ui| {
                elegida = show_menu_with_background(ui, &estado, Platform::Linux, fondo);
            });
            assert_eq!(elegida, None);
            let otro = pintado(&otro);
            for titulo in ["Archivo", "Editar", "Ver", "Ayuda"] {
                assert!(otro.pinta(titulo), "ppp {ppp} {fondo:?}: falta {titulo:?}");
            }
            // Cerrado, el desplegable no se ve: la barra es la misma con cualquier fondo activo.
            assert_eq!(
                format!("{:?}", otro.todos_los_textos()),
                format!("{:?}", con.todos_los_textos()),
                "ppp {ppp} {fondo:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 13, parte pura: bloqueos y habilitación
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_13_is_blocked_es_cierto_solo_con_dialog_modal_o_text_focus_con_cualquier_origen() {
    for b in FONDOS {
        for bits in 0..32u8 {
            let locks = bloqueos(bits);
            for origin in ORIGENES {
                assert_eq!(
                    is_blocked(accion(b), locks, origin),
                    locks.dialog || locks.modal || locks.text_focus,
                    "{b:?} con {locks:?} y {origin:?}"
                );
            }
        }
    }
}

#[test]
fn fila_13_cada_bloqueo_solo() {
    let solo = |f: fn(&mut Locks)| {
        let mut l = Locks::default();
        f(&mut l);
        l
    };
    let casos: [(&str, Locks, bool); 6] = [
        ("ninguno", Locks::default(), false),
        ("gesture", solo(|l| l.gesture = true), false),
        ("removal", solo(|l| l.removal = true), false),
        ("dialog", solo(|l| l.dialog = true), true),
        ("modal", solo(|l| l.modal = true), true),
        ("text_focus", solo(|l| l.text_focus = true), true),
    ];
    for b in FONDOS {
        for (nombre, locks, bloqueada) in casos {
            for origin in ORIGENES {
                assert_eq!(
                    is_blocked(accion(b), locks, origin),
                    bloqueada,
                    "{b:?} con {nombre} y {origin:?}"
                );
            }
        }
    }
}

#[test]
fn fila_13_no_la_bloquean_ni_el_gesto_ni_la_quita_ni_los_dos_juntos() {
    let gesto_y_quita = Locks {
        gesture: true,
        removal: true,
        ..Locks::default()
    };
    for b in FONDOS {
        for origin in ORIGENES {
            assert!(
                !is_blocked(accion(b), gesto_y_quita, origin),
                "{b:?} {origin:?}"
            );
        }
    }
}

#[test]
fn fila_13_no_es_zoom_ni_repite() {
    for b in FONDOS {
        assert!(!accion(b).is_zoom(), "{b:?}");
        assert!(!accion(b).ignores_repeat(), "{b:?}");
    }
}

fn estados() -> Vec<UiState> {
    let mut v = Vec::new();
    for can_undo in [false, true] {
        for can_redo in [false, true] {
            for zoom in [None, Some(Zoom::MIN), Zoom::from_int(5), Some(Zoom::MAX)] {
                for tool in HERRAMIENTAS {
                    for bits in 0..32u8 {
                        v.push(UiState {
                            can_undo,
                            can_redo,
                            zoom,
                            tool,
                            locks: bloqueos(bits),
                        });
                    }
                }
            }
        }
    }
    v
}

#[test]
fn fila_13_enabled_es_lo_contrario_de_is_blocked_con_el_origen_menu() {
    for state in estados() {
        for b in FONDOS {
            assert_eq!(
                enabled(accion(b), &state),
                !is_blocked(accion(b), state.locks, Origin::Menu),
                "{b:?} con {state:?}"
            );
        }
    }
}

#[test]
fn fila_13_con_un_campo_de_texto_enfocado_el_submenu_esta_deshabilitado_y_sin_el_habilitado() {
    let con_texto = UiState {
        locks: Locks {
            text_focus: true,
            ..Locks::default()
        },
        ..estado()
    };
    let sin_texto = estado();
    for b in FONDOS {
        assert!(!enabled(accion(b), &con_texto), "{b:?}");
        assert!(enabled(accion(b), &sin_texto), "{b:?}");
    }
}

/// Modelo de `is_blocked` de las acciones de antes, escrito aparte (el de `actions.rs`): (1)
/// `CloseLicenses` nunca; (2) `modal`: todo salvo `Quit`; (3) `dialog`: todo; (4) zoom: con
/// `text_focus` solo si el origen es `Key`; (5) el resto: `gesture` o `text_focus`, y `removal` solo
/// `SelectColor` y las que abren un diálogo (`ShowLicenses`, unidad 18, D2); (6) nada.
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
    if action.is_zoom() {
        return locks.text_focus && origin == Origin::Key;
    }
    if locks.gesture || locks.text_focus {
        return true;
    }
    locks.removal && matches!(action, AppAction::SelectColor(_) | AppAction::ShowLicenses)
}

#[test]
fn fila_13_las_acciones_anteriores_se_bloquean_igual_que_antes() {
    for action in acciones_anteriores() {
        for bits in 0..32u8 {
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
fn fila_13_las_acciones_anteriores_se_habilitan_igual_que_antes() {
    for state in estados() {
        for action in acciones_anteriores() {
            let mut esperado = !bloqueada_segun_el_modelo(action, state.locks, Origin::Menu);
            if esperado {
                esperado = match action {
                    AppAction::Undo => state.can_undo,
                    AppAction::Redo => state.can_redo,
                    AppAction::ZoomIn => state.zoom.is_some_and(|z| z < Zoom::MAX),
                    AppAction::ZoomOut => state.zoom.is_some_and(|z| z > Zoom::MIN),
                    AppAction::ZoomFit => state.zoom.is_some(),
                    AppAction::ZoomActual => state.zoom.is_some_and(|z| z != Zoom::ONE),
                    _ => true,
                };
            }
            assert_eq!(
                enabled(action, &state),
                esperado,
                "{action:?} con {state:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19b, fila 2: la quinta entrada `Color…`
// ---------------------------------------------------------------------------------------------
//
// Suposiciones: `AppAction::BackgroundColor` es `Clone, Copy, Debug, PartialEq, Eq` como las demás;
// `submenu_width_u(&Submenu) -> i64` y `background_value_text(CanvasBackground) -> Option<String>`
// son las de «Piezas». El banco abre el submenú **con clics** (`Ver`, `Fondo del lienzo`), como lo
// hace `dialog_open_focus.rs` con `Ayuda → Licencias`, y mira lo pintado en orden.

const AZUL_ACERO: Rgba = Rgba::opaque(0x33, 0x66, 0x99);
const NEGRO: Rgba = Rgba::opaque(0, 0, 0);

/// Los seis fondos activos de la fila 2: los cuatro de siempre y dos colores propios (el segundo,
/// negro: no salta a `Negro`).
const ACTIVOS: [CanvasBackground; 6] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
    CanvasBackground::Custom(AZUL_ACERO),
    CanvasBackground::Custom(NEGRO),
];

fn el_submenu_de_fondo() -> &'static Submenu {
    &el_menu("Ver").submenus[0]
}

/// La acción cuya entrada lleva la marca con `activo`.
fn accion_marcada(activo: CanvasBackground) -> AppAction {
    match activo {
        CanvasBackground::Custom(_) => AppAction::BackgroundColor,
        otro => accion(otro),
    }
}

/// Las acciones de antes, las cuatro del fondo y la nueva.
fn todas_con_color() -> Vec<AppAction> {
    let mut v = todas_las_acciones();
    v.push(AppAction::BackgroundColor);
    v
}

#[test]
fn fila_2_la_accion_nueva_es_un_tipo_plano_y_distinta_de_las_otras() {
    assert_plano::<AppAction>();
    for otra in todas_las_acciones() {
        assert_ne!(AppAction::BackgroundColor, otra);
    }
}

#[test]
fn fila_2_fondo_del_lienzo_tiene_cinco_entradas_y_la_quinta_es_color_sin_separador() {
    let fondo = el_submenu_de_fondo();
    assert_eq!(fondo.label, "Fondo del lienzo");
    let rotulos: Vec<&str> = fondo.items.iter().map(|i| i.label).collect();
    assert_eq!(rotulos, ["Damero", "Negro", "Gris", "Blanco", "Color…"]);
    let ultima = fondo.items.last().expect("hay entradas");
    assert_eq!(ultima.action, AppAction::BackgroundColor);
    assert!(fondo.items.iter().all(|i| !i.separator_before));
    // Los puntos suspensivos son un solo carácter (U+2026) del juego, no tres puntos ni `?`.
    assert_eq!(ultima.label.chars().count(), 6);
    assert!(ultima.label.ends_with('…'));
    assert!(!ultima.label.contains('?'));
    // `Ver` conserva sus cuatro entradas y no gana ninguna: la nueva está solo en el submenú.
    assert_eq!(el_menu("Ver").items.len(), 4);
    for menu in MENUS.iter() {
        assert!(
            menu.items
                .iter()
                .all(|i| i.action != AppAction::BackgroundColor),
            "{}",
            menu.title
        );
    }
}

#[test]
fn fila_2_con_cada_fondo_activo_hay_exactamente_una_marca_y_con_custom_es_color() {
    let fondo = el_submenu_de_fondo();
    for activo in ACTIVOS {
        let marcadas: Vec<&str> = fondo
            .items
            .iter()
            .filter(|i| background_checked(i.action, activo))
            .map(|i| i.label)
            .collect();
        let esperado = match activo {
            CanvasBackground::Checker => "Damero",
            CanvasBackground::Black => "Negro",
            CanvasBackground::Gray => "Gris",
            CanvasBackground::White => "Blanco",
            CanvasBackground::Custom(_) => "Color…",
        };
        assert_eq!(marcadas, [esperado], "{activo:?} activo");
    }
}

#[test]
fn fila_2_background_checked_solo_es_cierto_para_la_accion_marcada_con_cualquier_fondo() {
    for activo in ACTIVOS {
        for action in todas_con_color() {
            assert_eq!(
                background_checked(action, activo),
                action == accion_marcada(activo),
                "{action:?} con {activo:?} activo"
            );
        }
    }
}

#[test]
fn fila_2_con_un_color_propio_no_se_marca_ninguna_de_las_cuatro_fijas_ni_siquiera_con_el_mismo_color()
 {
    for propio in [
        CanvasBackground::Custom(AZUL_ACERO),
        CanvasBackground::Custom(NEGRO),
        CanvasBackground::Custom(Rgba::opaque(255, 255, 255)),
        CanvasBackground::Custom(Rgba::opaque(40, 40, 40)),
    ] {
        for fija in las_cuatro() {
            assert!(!background_checked(fija, propio), "{fija:?} con {propio:?}");
        }
        assert!(background_checked(AppAction::BackgroundColor, propio));
    }
    // Y al revés: con una fija activa, `Color…` no se marca.
    for activo in FONDOS {
        assert!(!background_checked(AppAction::BackgroundColor, activo));
    }
}

#[test]
fn fila_2_color_no_tiene_atajo_en_ninguna_plataforma_y_ninguna_tecla_la_da() {
    for platform in PLATAFORMAS {
        assert_eq!(
            shortcut_for(AppAction::BackgroundColor, platform),
            None,
            "{platform:?}"
        );
        for key in Key::ALL {
            for bits in 0..32u8 {
                assert_ne!(
                    action_for_shortcut(*key, modificadores(bits), platform),
                    Some(AppAction::BackgroundColor),
                    "{key:?} con modificadores {bits:#07b} en {platform:?}"
                );
            }
        }
    }
}

#[test]
fn fila_2_el_rotulo_color_esta_en_el_juego_sin_interrogante_y_printable_lo_deja_igual() {
    let color = "Color…";
    assert!(color.chars().all(|c| INTERFACE_CHARSET.contains(c)));
    let tiene = |c: char| INTERFACE_CHARSET.contains(c);
    assert_eq!(replace_missing(color, tiene), color);
    assert!(!color.contains('?'));
    for ppp in PPPS {
        let ctx = contexto(ppp);
        assert_eq!(printable(&ctx, color), color, "ppp {ppp}");
    }
    // Está en la tabla y no se repite con ningún otro rótulo de la barra.
    assert!(rotulos_nuevos().contains(&color));
    let mut todos: Vec<&str> = MENUS.iter().map(|m| m.title).collect();
    todos.extend(MENUS.iter().flat_map(|m| m.items.iter().map(|i| i.label)));
    assert!(!todos.contains(&color));
}

#[test]
fn fila_2_submenu_width_u_es_131_con_el_rotulo_mas_largo_mas_la_columna_del_valor() {
    let fondo = el_submenu_de_fondo();
    // 12 + (2 + rótulo más largo + 2 + 7) · 7 con «Color…» (6 caracteres, como `Damero`).
    let mas_largo = fondo
        .items
        .iter()
        .map(|i| i.label.chars().count() as i64)
        .max()
        .expect("hay entradas");
    assert_eq!(mas_largo, 6);
    assert_eq!(submenu_width_u(fondo), 12 + (2 + mas_largo + 2 + 7) * 7);
    assert_eq!(submenu_width_u(fondo), 131);
}

/// Lo que se ve de una forma, en orden de pintado.
#[derive(Clone, Debug)]
enum Forma {
    Texto {
        texto: String,
        rect: Rect,
        clip: Rect,
        colores: Vec<Color32>,
        opacidad: f32,
        /// Color del primer vértice de glifo tal como lo pinta la fuente: lo que cambia al atenuar.
        glifo: Option<Color32>,
    },
    Relleno {
        rect: Rect,
        fill: Color32,
        clip: Rect,
    },
}

impl Forma {
    fn aspecto(&self) -> (Vec<Color32>, u32) {
        match self {
            Forma::Texto {
                colores,
                opacidad,
                glifo,
                ..
            } => {
                let mut v = colores.clone();
                v.extend(glifo);
                (v, opacidad.to_bits())
            }
            Forma::Relleno { .. } => (vec![], 0),
        }
    }
}

fn aplana(shape: &Shape, clip: Rect, fuera: &mut Vec<Forma>) {
    match shape {
        Shape::Text(t) => {
            let job = &t.galley.job;
            let colores = job
                .sections
                .iter()
                .map(|s| {
                    t.override_text_color
                        .unwrap_or(if s.format.color == Color32::PLACEHOLDER {
                            t.fallback_color
                        } else {
                            s.format.color
                        })
                })
                .collect();
            let glifo = t.galley.rows.iter().find_map(|fila| {
                let visuales = &fila.row.visuals;
                visuales.mesh.vertices[visuales.glyph_vertex_range.clone()]
                    .first()
                    .map(|v| v.color)
            });
            fuera.push(Forma::Texto {
                texto: t.galley.text().to_owned(),
                rect: Rect::from_min_size(t.pos, t.galley.size()),
                clip,
                colores,
                opacidad: t.opacity_factor,
                glifo,
            });
        }
        Shape::Rect(r) => fuera.push(Forma::Relleno {
            rect: r.rect,
            fill: r.fill,
            clip,
        }),
        Shape::Vec(formas) => {
            for forma in formas {
                aplana(forma, clip, fuera);
            }
        }
        _ => {}
    }
}

fn formas_en_orden(formas: &[ClippedShape]) -> Vec<Forma> {
    let mut fuera = Vec::new();
    for clipped in formas {
        aplana(&clipped.shape, clipped.clip_rect, &mut fuera);
    }
    fuera
}

/// `src` (premultiplicado, como egui) por encima de `dst` opaco.
fn encima(dst: Color32, src: Color32) -> Color32 {
    let k = 1.0 - f32::from(src.a()) / 255.0;
    let canal = |s: u8, d: u8| (f32::from(s) + f32::from(d) * k).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(
        canal(src.r(), dst.r()),
        canal(src.g(), dst.g()),
        canal(src.b(), dst.b()),
    )
}

/// Contraste mínimo de texto (WCAG AA).
const MIN_TEXTO: f64 = 4.5;

const PANTALLA_DEL_MENU: Vec2 = vec2(600.0, 400.0);

/// La barra con el menú `Ver` y su submenú `Fondo del lienzo` abiertos con el ratón, un fotograma
/// tras otro como `App::ui`: `show_menu_with_background` con el fondo activo y los bloqueos dados.
struct BancoDelMenu {
    ctx: Context,
    fondo: CanvasBackground,
    locks: Locks,
    formas: Vec<Forma>,
    elegida: Option<AppAction>,
    /// El reloj de egui, en segundos: avanza en cada fotograma para que terminen las animaciones de
    /// los desplegables (con el tiempo parado se verían a medio aparecer, translúcidos).
    tiempo: f64,
}

impl BancoDelMenu {
    fn nuevo(ppp: f32, fondo: CanvasBackground, locks: Locks) -> BancoDelMenu {
        let ctx = contexto(ppp);
        ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
        let mut banco = BancoDelMenu {
            ctx,
            fondo,
            locks,
            formas: Vec::new(),
            elegida: None,
            tiempo: 0.0,
        };
        banco.reposo();
        banco
    }

    /// Con `Ver → Fondo del lienzo` abierto con dos clics reales.
    fn abierto(ppp: f32, fondo: CanvasBackground, locks: Locks) -> BancoDelMenu {
        let mut banco = BancoDelMenu::nuevo(ppp, fondo, locks);
        let ver = banco.centro_de("Ver");
        banco.clic(ver);
        banco.reposo();
        let submenu = banco.centro_de("Fondo del lienzo");
        banco.clic(submenu);
        banco.reposo();
        banco
    }

    fn fotograma(&mut self, events: Vec<Event>) {
        let estado = UiState {
            can_undo: true,
            can_redo: true,
            zoom: Zoom::from_int(5),
            tool: Tool::Pencil,
            locks: self.locks,
        };
        let fondo = self.fondo;
        let mut elegida = None;
        self.tiempo += 0.25;
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, PANTALLA_DEL_MENU)),
            events,
            time: Some(self.tiempo),
            ..RawInput::default()
        };
        let mut salida = self.ctx.run_ui(entrada, |ui| {
            apply_scale(ui.ctx());
            elegida = show_menu_with_background(ui, &estado, Platform::Linux, fondo);
        });
        salida.textures_delta.clear();
        self.formas = formas_en_orden(&salida.shapes);
        if elegida.is_some() {
            self.elegida = elegida;
        }
    }

    /// Unos fotogramas sin eventos (1,5 s de reloj): lo que tarda en asentarse la animación.
    fn reposo(&mut self) {
        for _ in 0..6 {
            self.fotograma(vec![]);
        }
    }

    /// Mueve, pulsa y suelta en `pos`: tres fotogramas, como los entrega winit.
    fn clic(&mut self, pos: Pos2) {
        self.fotograma(vec![mover(pos)]);
        self.fotograma(vec![boton(pos, true)]);
        self.fotograma(vec![boton(pos, false)]);
    }

    /// El índice de la primera forma de texto que contiene `buscado`.
    fn indice_de(&self, buscado: &str) -> Option<usize> {
        self.formas
            .iter()
            .position(|f| matches!(f, Forma::Texto { texto, .. } if texto.contains(buscado)))
    }

    fn texto(&self, buscado: &str) -> Option<&Forma> {
        self.indice_de(buscado).map(|i| &self.formas[i])
    }

    fn rect_de(&self, buscado: &str) -> Rect {
        match self.texto(buscado) {
            Some(Forma::Texto { rect, .. }) => *rect,
            _ => panic!(
                "no se pinta {buscado:?}; hay {:?}",
                self.formas
                    .iter()
                    .filter_map(|f| match f {
                        Forma::Texto { texto, .. } => Some(texto.as_str()),
                        Forma::Relleno { .. } => None,
                    })
                    .collect::<Vec<_>>()
            ),
        }
    }

    fn centro_de(&self, buscado: &str) -> Pos2 {
        self.rect_de(buscado).center()
    }

    /// La fila donde está el texto `buscado` (el último relleno anterior que lo contiene, aunque
    /// sea transparente) y el color real que hay debajo: todos los rellenos anteriores que lo
    /// contienen, mezclados en orden sobre el fondo del lienzo (el que hay detrás del menú).
    fn fondo_bajo(&self, buscado: &str) -> Option<(Rect, Color32)> {
        let i = self.indice_de(buscado)?;
        let Forma::Texto { rect, clip, .. } = &self.formas[i] else {
            return None;
        };
        let visible = rect.intersect(*clip);
        let sonda = visible.shrink2(vec2(0.0, visible.height() * 0.25));
        let mut fila = None;
        let mut fondo = BACKGROUND;
        for forma in &self.formas[..i] {
            if let Forma::Relleno { rect, fill, clip } = forma
                && rect.intersect(*clip).expand(0.01).contains_rect(sonda)
            {
                fila = Some(*rect);
                fondo = encima(fondo, *fill);
            }
        }
        fila.map(|fila| (fila, fondo))
    }

    /// El color con el que se pinta el texto `buscado`: el del primer glifo (lo que sale por
    /// pantalla, con el atenuado y el color forzado ya aplicados).
    fn color_pintado(&self, buscado: &str) -> Color32 {
        match self.texto(buscado) {
            Some(Forma::Texto { glifo, colores, .. }) => glifo
                .or_else(|| colores.first().copied())
                .unwrap_or(Color32::PLACEHOLDER),
            _ => panic!("no se pinta {buscado:?}"),
        }
    }
}

fn sin_bloqueos() -> Locks {
    Locks::default()
}

#[test]
fn fila_2_el_submenu_se_abre_con_clics_y_pinta_sus_cinco_rotulos() {
    for ppp in PPPS {
        let banco = BancoDelMenu::abierto(ppp, CanvasBackground::Checker, sin_bloqueos());
        for rotulo in ["Damero", "Negro", "Gris", "Blanco", "Color…"] {
            assert!(
                banco.texto(rotulo).is_some(),
                "ppp {ppp}: falta {rotulo:?} en el submenú abierto"
            );
        }
    }
}

#[test]
fn fila_2_el_valor_solo_se_pinta_con_un_color_propio_y_a_la_derecha_de_color() {
    for ppp in PPPS {
        // Sin color propio no hay valor en ninguna fila: ni `#` ni dígitos hexadecimales sueltos.
        for fijo in FONDOS {
            let banco = BancoDelMenu::abierto(ppp, fijo, sin_bloqueos());
            assert!(banco.texto("Color…").is_some(), "ppp {ppp} {fijo:?}");
            assert!(
                banco.indice_de("#").is_none(),
                "ppp {ppp} {fijo:?}: sin color propio no se pinta ningún valor"
            );
        }
        for (c, texto) in [(AZUL_ACERO, "#336699"), (NEGRO, "#000000")] {
            let activo = CanvasBackground::Custom(c);
            assert_eq!(background_value_text(activo).as_deref(), Some(texto));
            let banco = BancoDelMenu::abierto(ppp, activo, sin_bloqueos());
            let valor = banco.rect_de(texto);
            let etiqueta = banco.rect_de("Color…");
            // A la derecha de la etiqueta y en la misma fila.
            assert!(
                valor.min.x >= etiqueta.max.x - 0.01,
                "ppp {ppp}: el valor {valor:?} va a la derecha de {etiqueta:?}"
            );
            assert!(
                (valor.center().y - etiqueta.center().y).abs() <= 1.0,
                "ppp {ppp}: el valor {valor:?} está en la fila de {etiqueta:?}"
            );
            // Dentro de la fila (el relleno de la entrada marcada) y sin salirse por la derecha.
            let (fila, _) = banco
                .fondo_bajo(texto)
                .unwrap_or_else(|| panic!("ppp {ppp}: el valor no tiene relleno debajo"));
            assert!(
                valor.max.x <= fila.max.x + 0.01,
                "ppp {ppp}: el valor {valor:?} cabe en la fila {fila:?}"
            );
            // Un solo valor pintado.
            let n = banco
                .formas
                .iter()
                .filter(|f| matches!(f, Forma::Texto { texto: t, .. } if t.contains(texto)))
                .count();
            assert_eq!(n, 1, "ppp {ppp}: {texto} se pinta una vez");
            // NO PROBADO: que el valor vaya en `TEXT` (D3). La fila marcada se rellena con
            // `ACCENT` y `TEXT` sobre `ACCENT` da un contraste de 1,47, que contradice el
            // «≥ 4,5 sobre la fila marcada» de la misma D3: se prueba el contraste (la prueba de
            // abajo) y no el color concreto.
        }
    }
}

#[test]
fn fila_2_el_valor_tiene_contraste_al_menos_4_5_sobre_su_fila_marcada_y_con_el_raton() {
    // Con un color propio la entrada `Color…` es siempre la marcada: el valor se ve sobre la fila
    // marcada y, con el puntero encima, sobre la marcada resaltada. El relleno «normal» (sin marca ni
    // puntero) nunca lleva el valor: solo aparece con `Custom`, que marca la fila. NO PROBADO ese
    // tercer estado por eso. El color que se mide es el que sale por pantalla (el del glifo).
    for ppp in PPPS {
        for (c, texto) in [(AZUL_ACERO, "#336699"), (NEGRO, "#000000")] {
            let activo = CanvasBackground::Custom(c);
            let mut banco = BancoDelMenu::abierto(ppp, activo, sin_bloqueos());
            for estado in ["marcada", "con el ratón"] {
                if estado == "con el ratón" {
                    let encima = banco.centro_de("Color…");
                    banco.fotograma(vec![mover(encima)]);
                    banco.reposo();
                }
                let (_, fondo) = banco
                    .fondo_bajo(texto)
                    .unwrap_or_else(|| panic!("ppp {ppp} {estado}: sin relleno bajo el valor"));
                let color = banco.color_pintado(texto);
                let ratio = contrast_ratio(color, fondo);
                assert!(
                    ratio >= MIN_TEXTO,
                    "ppp {ppp} {estado}: el valor {texto} ({color:?}) sobre {fondo:?} da {ratio:.2}"
                );
            }
        }
    }
}

#[test]
fn fila_2_el_submenu_mide_lo_mismo_a_las_cinco_escalas_con_y_sin_color_propio() {
    for ppp in PPPS {
        let u = ui_unit(ppp);
        // 131 u, el de la spec, y el que dice `submenu_width_u`: lo pintado cumple los dos.
        assert_eq!(submenu_width_u(el_submenu_de_fondo()), 131);
        let esperado = 131.0 * u;
        for activo in ACTIVOS {
            let banco = BancoDelMenu::abierto(ppp, activo, sin_bloqueos());
            let marcada = match activo {
                CanvasBackground::Checker => "Damero",
                CanvasBackground::Black => "Negro",
                CanvasBackground::Gray => "Gris",
                CanvasBackground::White => "Blanco",
                CanvasBackground::Custom(_) => "Color…",
            };
            // El ancho del submenú es el del relleno de la fila marcada (una fila mide el ancho
            // del desplegable).
            let (fila, _) = banco
                .fondo_bajo(marcada)
                .unwrap_or_else(|| panic!("ppp {ppp} {activo:?}: sin fila marcada"));
            let tolerancia = tolerancia_de_egui(ppp) / ppp;
            assert!(
                (fila.width() - esperado).abs() <= tolerancia,
                "ppp {ppp} {activo:?}: la fila mide {} pt y debía medir {esperado} pt (131 u)",
                fila.width()
            );
        }
    }
}

#[test]
fn fila_2_color_se_atenua_con_un_gesto_o_una_quita_y_las_cuatro_fijas_no() {
    let con_gesto = Locks {
        gesture: true,
        ..Locks::default()
    };
    let con_quita = Locks {
        removal: true,
        ..Locks::default()
    };
    for ppp in PPPS {
        let libre = BancoDelMenu::abierto(ppp, CanvasBackground::Checker, sin_bloqueos());
        for (nombre, locks) in [("gesto", con_gesto), ("quita", con_quita)] {
            let banco = BancoDelMenu::abierto(ppp, CanvasBackground::Checker, locks);
            let aspecto = |b: &BancoDelMenu, r: &str| b.texto(r).expect("se pinta").aspecto();
            assert_ne!(
                aspecto(&banco, "Color…"),
                aspecto(&libre, "Color…"),
                "ppp {ppp}, {nombre}: `Color…` se pinta atenuada"
            );
            for fija in ["Damero", "Negro", "Gris", "Blanco"] {
                assert_eq!(
                    aspecto(&banco, fija),
                    aspecto(&libre, fija),
                    "ppp {ppp}, {nombre}: {fija} sigue habilitada"
                );
            }
        }
    }
}

#[test]
fn fila_2_enabled_de_color_es_falso_con_un_gesto_o_una_quita_y_el_de_las_cuatro_fijas_cierto() {
    let base = estado();
    for locks in [
        Locks {
            gesture: true,
            ..Locks::default()
        },
        Locks {
            removal: true,
            ..Locks::default()
        },
        Locks {
            gesture: true,
            removal: true,
            ..Locks::default()
        },
    ] {
        let state = UiState { locks, ..base };
        assert!(!enabled(AppAction::BackgroundColor, &state), "{locks:?}");
        for fija in las_cuatro() {
            assert!(enabled(fija, &state), "{fija:?} con {locks:?}");
        }
    }
    // Sin bloqueos, las cinco están habilitadas.
    for action in las_cuatro().into_iter().chain([AppAction::BackgroundColor]) {
        assert!(enabled(action, &base), "{action:?}");
    }
    // Con un diálogo, un diálogo modal o un campo de texto enfocado, las cinco se atenúan.
    for locks in [
        Locks {
            dialog: true,
            ..Locks::default()
        },
        Locks {
            modal: true,
            ..Locks::default()
        },
        Locks {
            text_focus: true,
            ..Locks::default()
        },
    ] {
        let state = UiState { locks, ..base };
        for action in las_cuatro().into_iter().chain([AppAction::BackgroundColor]) {
            assert!(!enabled(action, &state), "{action:?} con {locks:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19b, fila 3 (parte pura): `BackgroundColor` abre un diálogo y se bloquea como los demás
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_3_background_color_abre_un_dialogo_y_no_repite_ni_es_zoom() {
    assert!(AppAction::BackgroundColor.opens_dialog());
    assert!(!AppAction::BackgroundColor.ignores_repeat());
    assert!(!AppAction::BackgroundColor.is_zoom());
    // Las cuatro fijas no abren ningún diálogo.
    for fija in las_cuatro() {
        assert!(!fija.opens_dialog(), "{fija:?}");
    }
}

#[test]
fn fila_3_background_color_libre_con_todo_en_falso_y_bloqueada_con_cada_bloqueo_solo() {
    let solo = |f: fn(&mut Locks)| {
        let mut l = Locks::default();
        f(&mut l);
        l
    };
    let casos: [(&str, Locks, bool); 6] = [
        ("ninguno", Locks::default(), false),
        ("gesture", solo(|l| l.gesture = true), true),
        ("dialog", solo(|l| l.dialog = true), true),
        ("text_focus", solo(|l| l.text_focus = true), true),
        ("removal", solo(|l| l.removal = true), true),
        ("modal", solo(|l| l.modal = true), true),
    ];
    for (nombre, locks, bloqueada) in casos {
        for origin in ORIGENES {
            assert_eq!(
                is_blocked(AppAction::BackgroundColor, locks, origin),
                bloqueada,
                "{nombre} y {origin:?}"
            );
        }
        // Y la regla de la 18 para abrir un diálogo da lo mismo.
        assert_eq!(can_open_dialog(locks), !bloqueada, "{nombre}");
    }
}

#[test]
fn fila_3_con_las_32_combinaciones_se_bloquea_exactamente_con_cualquier_bloqueo() {
    for bits in 0..32u8 {
        let locks = bloqueos(bits);
        let alguno =
            locks.gesture || locks.dialog || locks.text_focus || locks.removal || locks.modal;
        for origin in ORIGENES {
            assert_eq!(
                is_blocked(AppAction::BackgroundColor, locks, origin),
                alguno,
                "{locks:?} y {origin:?}"
            );
        }
        assert_eq!(can_open_dialog(locks), !alguno, "{locks:?}");
        // Con cualquier combinación, bloquea igual que las otras que abren un diálogo.
        for otra in [
            AppAction::NewImage,
            AppAction::ConvertMode,
            AppAction::ShowLicenses,
        ] {
            assert_eq!(
                is_blocked(AppAction::BackgroundColor, locks, Origin::Menu),
                is_blocked(otra, locks, Origin::Menu),
                "{otra:?} con {locks:?}"
            );
        }
    }
}

#[test]
fn fila_3_enabled_de_background_color_es_lo_contrario_de_is_blocked_con_el_origen_menu() {
    for state in estados() {
        assert_eq!(
            enabled(AppAction::BackgroundColor, &state),
            !is_blocked(AppAction::BackgroundColor, state.locks, Origin::Menu),
            "{state:?}"
        );
    }
}

#[test]
fn fila_3_las_cuatro_fijas_siguen_sin_bloquearse_con_un_gesto_ni_con_una_quita() {
    // Lo que cambia con la nueva acción es solo ella: con un gesto en curso o una quita pendiente,
    // `Negro` se puede elegir y `Color…` no.
    for solo in [
        Locks {
            gesture: true,
            ..Locks::default()
        },
        Locks {
            removal: true,
            ..Locks::default()
        },
    ] {
        for origin in ORIGENES {
            for fija in las_cuatro() {
                assert!(!is_blocked(fija, solo, origin), "{fija:?} con {solo:?}");
            }
            assert!(is_blocked(AppAction::BackgroundColor, solo, origin));
        }
    }
    // Y las cuatro fijas se bloquean exactamente como antes con las 32 combinaciones.
    for bits in 0..32u8 {
        let locks = bloqueos(bits);
        for fija in las_cuatro() {
            for origin in ORIGENES {
                assert_eq!(
                    is_blocked(fija, locks, origin),
                    locks.dialog || locks.modal || locks.text_focus,
                    "{fija:?} con {locks:?} y {origin:?}"
                );
            }
        }
    }
}
