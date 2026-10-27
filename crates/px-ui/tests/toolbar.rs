//! Tests de la columna de herramientas `px_ui::toolbar` (specs/hito-2-menu-herramientas.md, parte
//! de la fila 7 «Geometría a las cinco escalas»; la pila completa está en `window_stack.rs` y la
//! aritmética pura en `chrome.rs`).
//!
//! Banco de egui sin ventana (`Context::run_ui` con `RawInput` sintético): `show_toolbox` se pinta
//! sola en una ventana vacía, así que la caja queda pegada al borde derecho (`common::zones::toolbox_single`,
//! con las 16 columnas de hoy: la columna única de verdad está en `window_stack.rs`). Se comprueba a las cinco
//! escalas (`ppp` 1,0, 1,25, 1,5, 2,0 y 3,0), en píxeles físicos:
//!
//! - las tablas `TOOL_BUTTONS` y `ZOOM_BUTTONS`;
//! - los 9 botones golpean en su sitio: casillas de 24 × 24 u en una rejilla de `c(k)` botones por
//!   fila (specs/ui-single-column.md; con 16 columnas, nueve: una sola fila de herramientas; primera
//!   y última coordenada física de cada botón dentro); el nivel de zoom y lo que queda fuera de la
//!   caja no golpean nada;
//! - el activo se hunde (cara `SURFACE_0`, icono `ACCENT`; contraste ≥ 4,5 con `contrast_ratio`),
//!   los demás llevan el icono en `TEXT`, y los deshabilitados se atenúan y no responden (con las
//!   reglas de bloqueo del zoom: gesto, quita y campo de texto no lo apagan);
//! - el nivel de zoom es un texto de 14 u en `y = 37`.
//!
//! Las posiciones pintadas se comparan con la tolerancia del redondeo de egui
//! (`common::tolerance_of_egui`: 1/32 pt, porque `Panel` y `allocate_rect` redondean a múltiplos pares de 1/32 pt).
//!
//! Supuestos sobre lo que la spec no fija (cualquier otro reparto cambiaría esta página):
//! - `show_toolbox` crea su propio panel a la derecha de `ui` (como `status::show` y
//!   `palette_panel::show`), de `column_width_u(16)·n` píxeles de ancho;
//! - el rótulo de zoom que recibe es el texto del nivel (`×12`, `1/8`, `—`) con o sin el prefijo
//!   `Zoom `: el banco solo exige que el texto pintado lo contenga.

use egui::{Context, Event, Vec2, vec2};
use px_ui::actions::{AppAction, Locks, Platform, UiState};
use px_ui::keymap::{Keymap, default_keymap};
use px_ui::theme::{ACCENT, SURFACE_0, TEXT, TEXT_DIM, TEXT_DISABLED, contrast_ratio};
use px_ui::tool::Tool;
use px_ui::toolbar::{TOOL_BUTTONS, ToolboxView, ZOOM_BUTTONS, show_toolbox};
use px_ui::view::Zoom;

mod common;
use common::icons::IconsOfTest;
use common::toolbox::center_u;
use common::zones::toolbox_single;
use common::{
    PPPS, Painted, button_2, context, frame, in_points, move_pointer, n_of, painted,
    tolerance_of_egui,
};

const TEST_TOOLS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];
const TEST_SCREEN: Vec2 = vec2(400.0, 600.0);
/// Ancho de la caja con 16 columnas, en `u`, y lado de un botón (casilla de 24 × 24 u).
const COLUMN_U: i64 = 233;
const TEST_BUTTON_U: i64 = 24;
const TEST_MARGIN_U: i64 = 4;

fn state_2(tool: Tool, zoom: Option<Zoom>, locks: Locks) -> UiState {
    UiState {
        can_undo: true,
        can_redo: true,
        zoom,
        tool,
        locks,
    }
}

fn x5() -> Option<Zoom> {
    Zoom::from_int(5)
}

struct Bench {
    ctx: Context,
    ppp: f32,
    state_2: UiState,
    label_2: &'static str,
    icons_2: IconsOfTest,
}

impl Bench {
    fn new_2(ppp: f32, state_2: UiState) -> Bench {
        let ctx = context(ppp);
        let icons_2 = IconsOfTest::new_2_2(&ctx);
        Bench {
            ctx,
            ppp,
            state_2,
            label_2: "×5",
            icons_2,
        }
    }

    fn n(&self) -> i64 {
        n_of(self.ppp)
    }

    /// `x` de la columna en píxeles físicos (pegada al borde derecho de la pantalla del banco).
    fn x0(&self) -> i64 {
        toolbox_single(
            (
                (TEST_SCREEN.x * self.ppp).round() as i64,
                (TEST_SCREEN.y * self.ppp).round() as i64,
            ),
            self.n() as u32,
        )
        .x
    }

    /// Un fotograma: lo pintado y lo que devolvió `show_toolbox`.
    fn frame(&self, events: Vec<Event>) -> (Painted, Option<AppAction>) {
        let mut action_2 = None;
        let shapes_2 = frame(&self.ctx, TEST_SCREEN, events, |ui| {
            let view = ToolboxView {
                state: &self.state_2,
                zoom_label: self.label_2,
                icons: &self.icons_2.set,
            };
            action_2 = show_toolbox(ui, &view, default_keymap(), Platform::Linux).action;
        });
        (painted(&shapes_2), action_2)
    }

    fn rest_2_2(&self) -> Painted {
        // Dos fotogramas: el primero reparte el espacio y el segundo es el estable.
        self.frame(vec![]);
        self.frame(vec![]).0
    }

    /// Mueve, pulsa y suelta en `(x_px, y_px)`; devuelve las acciones que salieron.
    fn click(&self, x_px: i64, y_px: i64) -> Vec<AppAction> {
        self.rest_2_2();
        let pos = in_points(self.ppp, x_px, y_px);
        let mut actions = Vec::new();
        for events in [
            vec![move_pointer(pos)],
            vec![button_2(pos, true)],
            vec![button_2(pos, false)],
        ] {
            actions.extend(self.frame(events).1);
        }
        // Aparta el puntero para no dejar nada pulsado ni bajo el cursor.
        self.frame(vec![move_pointer(in_points(self.ppp, 3000, 3000))]);
        actions
    }

    /// Clic en el centro del botón `i` (0 a 4 las herramientas, 5 a 8 el zoom).
    fn click_in_button(&self, i: usize) -> Vec<AppAction> {
        let n = self.n();
        let (cx, cy) = center_u(i);
        self.click(self.x0() + cx * n, cy * n)
    }

    /// Rectángulo `(x, y)` de la esquina del botón `i`, en `u` desde la esquina de la caja.
    fn corner_u(i: usize) -> (i64, i64) {
        let (cx, cy) = center_u(i);
        (cx - TEST_BUTTON_U / 2, cy - TEST_BUTTON_U / 2)
    }
}

// ---------------------------------------------------------------------------------------------
// Tablas y colores
// ---------------------------------------------------------------------------------------------

#[test]
fn the_active_one_has_enough_contrast_accent_over_surface_0() {
    assert!(
        contrast_ratio(ACCENT, SURFACE_0) >= 4.5,
        "contraste {}",
        contrast_ratio(ACCENT, SURFACE_0)
    );
}

/// La letra que la columna enseña para `action`: sale de la tabla de teclas (`Button.key` ya no
/// existe) y no hay ninguna si la acción no tiene fila.
fn letter_of(action: AppAction) -> Option<char> {
    Keymap::default_scheme()
        .shown_text(action, Platform::Linux)
        .and_then(|text_2| text_2.chars().next())
}

#[test]
fn tool_buttons_are_the_five_buttons_with_their_letter() {
    let table: Vec<(&str, AppAction, Option<char>)> = TOOL_BUTTONS
        .iter()
        .map(|b| (b.label, b.action, letter_of(b.action)))
        .collect();
    assert_eq!(
        table,
        [
            ("Lápiz", AppAction::SelectTool(Tool::Pencil), Some('d')),
            ("Línea", AppAction::SelectTool(Tool::Line), Some('v')),
            ("Relleno", AppAction::SelectTool(Tool::Fill), Some('f')),
            (
                "Cuentagotas",
                AppAction::SelectTool(Tool::Dropper),
                Some(',')
            ),
            ("Borrador", AppAction::SelectTool(Tool::Eraser), Some('k')),
        ]
    );
}

#[test]
fn zoom_buttons_are_the_four_buttons_without_a_letter() {
    let table: Vec<(&str, AppAction)> = ZOOM_BUTTONS.iter().map(|b| (b.label, b.action)).collect();
    assert_eq!(
        table,
        [
            ("Acercar", AppAction::ZoomIn),
            ("Alejar", AppAction::ZoomOut),
            ("Ajustar", AppAction::ZoomFit),
            ("1:1", AppAction::ZoomActualSize),
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 7: los botones golpean en su sitio
// ---------------------------------------------------------------------------------------------

#[test]
fn row_7_the_center_of_each_tool_button_gives_its_action_at_the_five_scales() {
    for ppp in PPPS {
        let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
        for (i, button) in TOOL_BUTTONS.iter().enumerate() {
            assert_eq!(
                bench.click_in_button(i),
                vec![button.action],
                "ppp {ppp}: {}",
                button.label
            );
        }
    }
}

#[test]
fn row_7_the_center_of_each_zoom_button_gives_its_action_at_the_five_scales() {
    for ppp in PPPS {
        let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
        for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
            assert_eq!(
                bench.click_in_button(TOOL_BUTTONS.len() + i),
                vec![button.action],
                "ppp {ppp}: {}",
                button.label
            );
        }
    }
}

#[test]
fn row_7_the_first_and_the_last_physical_coordinate_of_each_button_hit() {
    for ppp in PPPS {
        let n = n_of(ppp);
        let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
        let boxes_2: Vec<(usize, AppAction)> = TOOL_BUTTONS
            .iter()
            .chain(ZOOM_BUTTONS.iter())
            .enumerate()
            .map(|(i, b)| (i, b.action))
            .collect();
        for (i, action_2) in boxes_2 {
            let (cx_u, cy_u) = Bench::corner_u(i);
            let (x0, y0) = (bench.x0() + cx_u * n, cy_u * n);
            let (x1, y1) = (
                bench.x0() + (cx_u + TEST_BUTTON_U) * n - 1,
                (cy_u + TEST_BUTTON_U) * n - 1,
            );
            for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                assert_eq!(
                    bench.click(x, y),
                    vec![action_2],
                    "ppp {ppp}: ({x}, {y}) debería golpear {action_2:?}"
                );
            }
        }
    }
}

/// Los negativos solo se piden **lejos** de los botones: egui da el clic al widget más cercano
/// dentro de `style.interaction.interact_radius` (5 pt), así que un píxel fuera de un botón
/// golpea a ese botón salvo que la columna ponga el radio a 0 (la spec no lo exige).
#[test]
fn row_7_the_zoom_level_and_what_is_far_from_the_buttons_hit_nothing() {
    for ppp in PPPS {
        let n = n_of(ppp);
        let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
        let center_x = bench.x0() + center_u(1).0 * n;
        // El nivel de zoom (y = 37 a 51): a 9 u de la fila de herramientas y a 4 u de la de zoom;
        // se pulsa a 16 u y 11 u de ellas, fuera del radio de interacción de egui.
        assert!(
            bench.click(center_x, 44 * n).is_empty(),
            "ppp {ppp}: el nivel de zoom no es un botón"
        );
        // Fuera de la columna (a 40 u de su borde, en el lado del lienzo) no hay nada.
        for y_u in [10, 50, 130] {
            assert!(
                bench.click(bench.x0() - 40 * n, y_u * n).is_empty(),
                "ppp {ppp}: 40 u fuera de la columna no golpean nada (y = {y_u} u)"
            );
        }
    }
}

#[test]
fn row_7_everything_painted_falls_inside_the_column_and_its_buttons() {
    for ppp in PPPS {
        let n = n_of(ppp);
        let p = ppp;
        let bench = Bench::new_2(ppp, state_2(Tool::Line, x5(), Locks::default()));
        let painted = bench.rest_2_2();
        for text_2 in &painted.texts {
            let r = text_2.rect();
            assert!(
                r.min.x * p >= (bench.x0() + TEST_MARGIN_U * n) as f32 - tolerance_of_egui(ppp)
                    && r.max.x * p
                        <= (bench.x0() + (COLUMN_U - TEST_MARGIN_U) * n) as f32
                            + tolerance_of_egui(ppp),
                "ppp {ppp}: {:?} se sale de la columna: {r:?}",
                text_2.text_2
            );
        }
        for fill in &painted.fills {
            let r = fill.rect;
            assert!(
                r.max.x * p <= (bench.x0() + COLUMN_U * n) as f32 + tolerance_of_egui(ppp),
                "ppp {ppp}: relleno fuera de la columna: {r:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// El activo y la letra
// ---------------------------------------------------------------------------------------------

#[test]
fn the_active_one_sinks_with_face_surface_0_and_accent_icon_on_its_button() {
    for ppp in PPPS {
        let n = n_of(ppp);
        for (i, tool) in TEST_TOOLS.iter().enumerate() {
            let bench = Bench::new_2(ppp, state_2(*tool, x5(), Locks::default()));
            let painted = bench.rest_2_2();
            // Solo el icono del activo lleva `ACCENT`, y va 1 u abajo y a la derecha en su botón.
            let accents: Vec<_> = painted
                .meshes
                .iter()
                .filter(|m| m.color == ACCENT)
                .collect();
            assert_eq!(
                accents.len(),
                1,
                "ppp {ppp}: solo el activo ({tool:?}) lleva el icono ACCENT"
            );
            let (ex, ey) = Bench::corner_u(i);
            let expected_2 = [
                bench.x0() + (ex + 4 + 1) * n,
                (ey + 4 + 1) * n,
                bench.x0() + (ex + 4 + 1 + 16) * n,
                (ey + 4 + 1 + 16) * n,
            ];
            let r = accents[0].rect;
            let real = [r.min.x * ppp, r.min.y * ppp, r.max.x * ppp, r.max.y * ppp];
            for (a, b) in real.iter().zip(expected_2) {
                assert!(
                    (a - b as f32).abs() <= tolerance_of_egui(ppp),
                    "ppp {ppp}: {tool:?} {real:?} y no {expected_2:?}"
                );
            }
            // La cara del activo es `SURFACE_0` en el rectángulo del botón.
            let face = [
                bench.x0() + ex * n,
                ey * n,
                bench.x0() + (ex + TEST_BUTTON_U) * n,
                (ey + TEST_BUTTON_U) * n,
            ];
            assert!(
                painted.fills.iter().any(|r| {
                    r.fill == SURFACE_0
                        && [r.rect.min.x, r.rect.min.y, r.rect.max.x, r.rect.max.y]
                            .iter()
                            .zip(face)
                            .all(|(a, b)| (a * ppp - b as f32).abs() <= tolerance_of_egui(ppp))
                }),
                "ppp {ppp}: {tool:?}: falta la cara hundida en {face:?}"
            );
        }
    }
}

#[test]
fn the_shortcuts_are_no_longer_painted_on_the_buttons_and_come_out_in_the_tooltip() {
    // El atajo se dice en el tooltip (`tooltip_text`); en el botón solo queda el icono.
    for ppp in PPPS {
        let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
        let painted = bench.rest_2_2();
        for button in TOOL_BUTTONS.iter().skip(1) {
            let Some(letter) = letter_of(button.action).map(|c| c.to_string()) else {
                continue;
            };
            assert!(
                painted.span(&letter).is_none(),
                "ppp {ppp}: la letra {letter} no se pinta en el botón"
            );
        }
    }
}

#[test]
fn the_nine_icons_are_painted_and_no_button_label() {
    let bench = Bench::new_2(1.0, state_2(Tool::Fill, x5(), Locks::default()));
    let painted = bench.rest_2_2();
    let textures_2: Vec<_> = painted.meshes.iter().map(|m| m.texture_2).collect();
    assert_eq!(textures_2.len(), 9, "nueve iconos, uno por botón");
    for (i, a) in textures_2.iter().enumerate() {
        assert!(!textures_2[i + 1..].contains(a), "iconos repetidos");
    }
    for button in ZOOM_BUTTONS.iter() {
        assert!(
            painted.text_with(button.label).is_none(),
            "el rótulo {:?} no se pinta en el botón; hay {:?}",
            button.label,
            painted.all_the_texts()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Nivel de zoom
// ---------------------------------------------------------------------------------------------

#[test]
fn the_zoom_level_is_a_14_u_text_at_y_37() {
    for ppp in PPPS {
        let n = n_of(ppp);
        let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
        let painted = bench.rest_2_2();
        let text_2 = painted.text_with("×5").unwrap_or_else(|| {
            panic!(
                "ppp {ppp}: no se pinta el nivel; hay {:?}",
                painted.all_the_texts()
            )
        });
        assert!(
            (text_2.pos.y * ppp - (37 * n) as f32).abs() <= tolerance_of_egui(ppp),
            "ppp {ppp}: el nivel empieza en y = {} px y no en {}",
            text_2.pos.y * ppp,
            37 * n
        );
        assert!(
            (text_2.size_2.y * ppp - (14 * n) as f32).abs() <= 0.05,
            "ppp {ppp}: el nivel mide {} px de alto y no {}",
            text_2.size_2.y * ppp,
            14 * n
        );
    }
}

#[test]
fn without_an_image_the_zoom_level_is_painted_anyway() {
    let mut bench = Bench::new_2(1.0, state_2(Tool::Pencil, None, Locks::default()));
    bench.label_2 = "—";
    let painted = bench.rest_2_2();
    assert!(
        painted.text_with("—").is_some(),
        "{:?}",
        painted.all_the_texts()
    );
}

// ---------------------------------------------------------------------------------------------
// Deshabilitados
// ---------------------------------------------------------------------------------------------

fn lock(f: impl FnOnce(&mut Locks)) -> Locks {
    let mut locks = Locks::default();
    f(&mut locks);
    locks
}

/// Qué botones responden a un clic: herramientas (5) y zoom (4).
fn respond(bench: &Bench) -> ([bool; 5], [bool; 4]) {
    let mut tools = [false; 5];
    let mut zoom = [false; 4];
    for (i, button) in TOOL_BUTTONS.iter().enumerate() {
        tools[i] = bench.click_in_button(i) == vec![button.action];
    }
    for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
        zoom[i] = bench.click_in_button(TOOL_BUTTONS.len() + i) == vec![button.action];
    }
    (tools, zoom)
}

#[test]
fn with_a_gesture_or_a_focused_field_the_tools_do_not_respond_but_the_zoom_does() {
    for ppp in PPPS {
        for locks in [lock(|l| l.gesture = true), lock(|l| l.text_focus = true)] {
            let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), locks));
            assert_eq!(
                respond(&bench),
                ([false; 5], [true; 4]),
                "ppp {ppp} con {locks:?}"
            );
        }
    }
}

#[test]
fn with_a_pending_removal_the_tools_and_the_zoom_respond() {
    let bench = Bench::new_2(1.0, state_2(Tool::Pencil, x5(), lock(|l| l.removal = true)));
    assert_eq!(respond(&bench), ([true; 5], [true; 4]));
}

#[test]
fn with_an_open_dialog_or_the_licenses_panel_nothing_responds() {
    for locks in [lock(|l| l.dialog = true), lock(|l| l.modal = true)] {
        let bench = Bench::new_2(1.0, state_2(Tool::Pencil, x5(), locks));
        assert_eq!(respond(&bench), ([false; 5], [false; 4]), "{locks:?}");
    }
}

#[test]
fn without_an_image_the_zoom_does_not_respond_and_at_the_minimum_only_zoom_out_is_off() {
    let bench = Bench::new_2(1.0, state_2(Tool::Pencil, None, Locks::default()));
    assert_eq!(respond(&bench), ([true; 5], [false; 4]));
    let bench = Bench::new_2(
        1.0,
        state_2(Tool::Pencil, Some(Zoom::MIN), Locks::default()),
    );
    // Acercar, Alejar, Ajustar y 1:1: en el mínimo (1/64) solo `Alejar` está deshabilitado.
    assert_eq!(respond(&bench), ([true; 5], [true, false, true, true]));
    // En ×1 (el mínimo de antes) `Alejar` ya vale (baja a 1/2) y `1:1` no.
    let bench = Bench::new_2(
        1.0,
        state_2(Tool::Pencil, Zoom::from_int(1), Locks::default()),
    );
    assert_eq!(respond(&bench), ([true; 5], [true, true, true, false]));
}

#[test]
fn at_zoom_max_zoom_in_does_not_respond() {
    let bench = Bench::new_2(
        1.0,
        state_2(Tool::Pencil, Some(Zoom::MAX), Locks::default()),
    );
    assert_eq!(respond(&bench), ([true; 5], [false, true, true, true]));
}

#[test]
fn a_disabled_button_is_painted_dimmed() {
    // El icono de `Línea` cambia de tinta al deshabilitarlo (no es el activo en ninguno de los dos).
    let free = Bench::new_2(1.0, state_2(Tool::Pencil, x5(), Locks::default()));
    let with_gesture = Bench::new_2(1.0, state_2(Tool::Pencil, x5(), lock(|l| l.gesture = true)));
    let ink = |bench: &Bench, index_2: usize| {
        let id = bench.icons_2.set.get(
            TOOL_BUTTONS
                .iter()
                .chain(ZOOM_BUTTONS.iter())
                .nth(index_2)
                .expect("botón")
                .icon,
        );
        bench
            .rest_2_2()
            .mesh_of(id.expect("textura"))
            .expect("icono pintado")
            .color
    };
    assert_eq!(ink(&free, 1), TEXT);
    assert_eq!(
        ink(&with_gesture, 1),
        TEXT_DISABLED,
        "el deshabilitado va atenuado"
    );
    // El zoom sigue habilitado con el gesto: su icono no cambia.
    assert_eq!(ink(&free, 5), ink(&with_gesture, 5));
}

// ---------------------------------------------------------------------------------------------
// Unidad I.3 (specs/interfaz-caja-derecha.md) y C2 (specs/ui-single-column.md): ventana de opciones,
// clic derecho y tooltips
// ---------------------------------------------------------------------------------------------

mod toolbox_right {
    use super::*;
    use egui::{PointerButton, RawInput, Rect};
    use proptest::prelude::*;
    use px_ui::actions::{Origin, is_blocked};
    use px_ui::keymap::{KeyId, default_keymap};
    use px_ui::options::{
        NO_OPTIONS, OptionsWindow, key_line, options_max_scroll, options_row_count,
    };
    use px_ui::options_view::{OptionsView, show_options_window};
    use px_ui::text::INTERFACE_CHARSET;
    use px_ui::toolbar::{
        SecondaryOutcome, ToolboxOutput, ToolboxView, secondary_outcome, show_toolbox, tooltip_text,
    };
    use px_ui::view::ScreenRect;

    use super::common::{Text, button_of_2, wheel_line};

    const LINUX: Platform = Platform::Linux;

    /// Banco de la caja entera (`show_toolbox`) con el tiempo del fotograma (los tooltips lo usan).
    struct Toolbox {
        ctx: Context,
        ppp: f32,
        state_2: UiState,
        keymap: Keymap,
        time_2: f64,
        icons_2: IconsOfTest,
    }

    impl Toolbox {
        fn new(ppp: f32, state_2: UiState) -> Toolbox {
            let ctx = context(ppp);
            let icons_2 = IconsOfTest::new_2_2(&ctx);
            Toolbox {
                ctx,
                ppp,
                state_2,
                keymap: default_keymap().clone(),
                time_2: 0.0,
                icons_2,
            }
        }

        fn n(&self) -> i64 {
            n_of(self.ppp)
        }

        /// `x` de la caja en píxeles físicos (pegada al borde derecho de la pantalla del banco).
        fn x0(&self) -> i64 {
            toolbox_single(
                (
                    (TEST_SCREEN.x * self.ppp).round() as i64,
                    (TEST_SCREEN.y * self.ppp).round() as i64,
                ),
                self.n() as u32,
            )
            .x
        }

        fn frame(&mut self, events: Vec<Event>) -> (Painted, ToolboxOutput) {
            self.time_2 += 0.1;
            let input_2 = RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, TEST_SCREEN)),
                events,
                time: Some(self.time_2),
                ..RawInput::default()
            };
            let mut output_2 = None;
            let mut outside = self.ctx.run_ui(input_2, |ui| {
                let view = ToolboxView {
                    state: &self.state_2,
                    zoom_label: "×5",
                    icons: &self.icons_2.set,
                };
                output_2 = Some(show_toolbox(ui, &view, &self.keymap, LINUX));
            });
            outside.textures_delta.clear();
            let shapes_2 = std::mem::take(&mut outside.shapes);
            (painted(&shapes_2), output_2.expect("la caja se pinta"))
        }

        fn rest_2_2(&mut self) -> Painted {
            self.frame(vec![]);
            self.frame(vec![]).0
        }

        /// Mueve, pulsa y suelta `button_2` en `(x, y)`; devuelve la salida de cada uno de los tres
        /// fotogramas.
        fn click(&mut self, button_2: PointerButton, x: i64, y: i64) -> Vec<ToolboxOutput> {
            self.rest_2_2();
            let pos = in_points(self.ppp, x, y);
            let mut outputs = Vec::new();
            for events in [
                vec![move_pointer(pos)],
                vec![button_of_2(pos, button_2, true)],
                vec![button_of_2(pos, button_2, false)],
            ] {
                outputs.push(self.frame(events).1);
            }
            self.frame(vec![move_pointer(in_points(self.ppp, 3000, 3000))]);
            outputs
        }

        fn center_of_tool(&self, i: i64) -> (i64, i64) {
            let n = self.n();
            let (cx, cy) = center_u(i as usize);
            (self.x0() + cx * n, cy * n)
        }

        fn center_of_zoom(&self, i: i64) -> (i64, i64) {
            let n = self.n();
            let (cx, cy) = center_u(5 + i as usize);
            (self.x0() + cx * n, cy * n)
        }
    }

    /// El texto pintado cuya esquina cae en `(x_px, y_px)`.
    fn text_in(p: &Painted, ppp: f32, x_px: i64, y_px: i64) -> Option<&Text> {
        let tol = tolerance_of_egui(ppp);
        p.texts.iter().find(|t| {
            (t.pos.x * ppp - x_px as f32).abs() <= tol && (t.pos.y * ppp - y_px as f32).abs() <= tol
        })
    }

    fn actions(outputs: &[ToolboxOutput]) -> Vec<AppAction> {
        outputs.iter().filter_map(|s| s.action).collect()
    }

    fn toggles(outputs: &[ToolboxOutput]) -> bool {
        outputs.iter().any(|s| s.toggle_options)
    }

    fn per_label(label: &str) -> &'static px_ui::toolbar::Button {
        TOOL_BUTTONS
            .iter()
            .chain(ZOOM_BUTTONS.iter())
            .find(|b| b.label == label)
            .expect("el botón existe")
    }

    // -----------------------------------------------------------------------------------------
    // La ventana de opciones: contenido (specs/ui-single-column.md, decisión 11)
    // -----------------------------------------------------------------------------------------

    /// Banco de la ventana de opciones sola (`show_options_window`) en la pantalla del banco, con
    /// 16 columnas elegidas.
    struct WindowBench {
        ctx: Context,
        ppp: f32,
        tool: Tool,
        rows: usize,
        keymap: Keymap,
        state: OptionsWindow,
        time_2: f64,
    }

    impl WindowBench {
        fn new(ppp: f32, tool: Tool, rows: usize) -> WindowBench {
            let state = OptionsWindow {
                open: true,
                ..OptionsWindow::default()
            };
            WindowBench {
                ctx: context(ppp),
                ppp,
                tool,
                rows,
                keymap: default_keymap().clone(),
                state,
                time_2: 0.0,
            }
        }

        fn n(&self) -> i64 {
            n_of(self.ppp)
        }

        fn window_px(&self) -> (i64, i64) {
            (
                (TEST_SCREEN.x * self.ppp).round() as i64,
                (TEST_SCREEN.y * self.ppp).round() as i64,
            )
        }

        /// Rectángulo de la ventana en píxeles físicos.
        fn rect(&mut self) -> ScreenRect {
            self.state
                .rect(self.rows, self.window_px(), self.n() as u32, 16)
        }

        /// Punto `(x_u, y_u)` de la ventana, en píxeles físicos.
        fn at(&mut self, x_u: i64, y_u: i64) -> (i64, i64) {
            let (n, rect) = (self.n(), self.rect());
            (rect.x + x_u * n, rect.y + y_u * n)
        }

        fn frame(&mut self, events: Vec<Event>) -> Painted {
            self.time_2 += 0.1;
            let input_2 = RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, TEST_SCREEN)),
                events,
                time: Some(self.time_2),
                ..RawInput::default()
            };
            let window_px = self.window_px();
            let view = OptionsView {
                tool: self.tool,
                rows: self.rows,
                lines: &[],
                keymap: &self.keymap,
                platform: LINUX,
                enabled: true,
            };
            let state = &mut self.state;
            let mut outside = self.ctx.run_ui(input_2, |ui| {
                show_options_window(ui.ctx(), &view, state, window_px, 16);
            });
            outside.textures_delta.clear();
            painted(&std::mem::take(&mut outside.shapes))
        }

        fn rest_2_2(&mut self) -> Painted {
            self.frame(vec![]);
            self.frame(vec![])
        }
    }

    // -----------------------------------------------------------------------------------------
    // Fila 4: qué trae la ventana hoy y cómo se prepara
    // -----------------------------------------------------------------------------------------

    #[test]
    fn row_4_each_tool_paints_its_name_its_shortcut_and_no_options_in_the_window() {
        let expected_2 = [
            (Tool::Pencil, "Lápiz", "tecla d"),
            (Tool::Line, "Línea", "tecla v"),
            (Tool::Fill, "Relleno", "tecla f"),
            (Tool::Dropper, "Cuentagotas", "tecla ,"),
            (Tool::Eraser, "Borrador", "tecla k"),
        ];
        for ppp in PPPS {
            for (tool, name_2, shortcut_2) in expected_2 {
                let mut window = WindowBench::new(ppp, tool, 0);
                let p = window.rest_2_2();
                // Nombre en la barra de título, línea del atajo y cuerpo (`sin opciones`).
                let (x, y) = window.at(5, 2);
                let title = text_in(&p, ppp, x, y)
                    .unwrap_or_else(|| panic!("ppp {ppp} {tool:?}: falta el título"));
                assert_eq!(title.text_2, name_2, "ppp {ppp}");
                let (x, y) = window.at(5, 21);
                let line = text_in(&p, ppp, x, y)
                    .unwrap_or_else(|| panic!("ppp {ppp} {tool:?}: falta la línea del atajo"));
                assert_eq!(line.text_2, shortcut_2, "ppp {ppp}");
                assert_eq!(line.color(), TEXT_DIM, "ppp {ppp}");
                let (x, y) = window.at(5, 39);
                let body = text_in(&p, ppp, x, y)
                    .unwrap_or_else(|| panic!("ppp {ppp} {tool:?}: falta `sin opciones`"));
                assert_eq!(body.text_2, NO_OPTIONS, "ppp {ppp}");
                assert_eq!(NO_OPTIONS, "sin opciones");
                assert_eq!(body.color(), TEXT_DIM, "ppp {ppp}");
            }
        }
    }

    #[test]
    fn row_4_the_shortcut_line_reads_the_table_and_without_a_row_it_says_no_key() {
        for tool in TEST_TOOLS {
            assert_eq!(
                key_line(tool, default_keymap(), LINUX),
                match tool {
                    Tool::Pencil => "tecla d",
                    Tool::Line => "tecla v",
                    Tool::Fill => "tecla f",
                    Tool::Dropper => "tecla ,",
                    Tool::Eraser => "tecla k",
                }
            );
        }
        // Con otra tabla (la `d` pasa a `x`) cambia el texto.
        let rows_2 = default_keymap()
            .rows()
            .iter()
            .cloned()
            .map(|mut f| {
                if !f.command && f.key == KeyId::Char('d') {
                    f.key = KeyId::Char('x');
                }
                f
            })
            .collect();
        let other = Keymap::new(rows_2);
        assert_eq!(key_line(Tool::Pencil, &other, LINUX), "tecla x");
        // Sin fila en la tabla, la herramienta sigue diciendo `sin tecla`.
        let empty = Keymap::new(vec![]);
        assert_eq!(key_line(Tool::Eraser, &empty, LINUX), "sin tecla");
        let mut window = WindowBench::new(1.0, Tool::Pencil, 0);
        window.keymap = other;
        let p = window.rest_2_2();
        assert!(p.paints("tecla x"));
    }

    #[test]
    fn row_4_the_body_rows_and_their_scroll_are_prepared_but_empty() {
        for (tool, rows) in [
            (Tool::Pencil, 3),
            (Tool::Line, 4),
            (Tool::Fill, 0),
            (Tool::Dropper, 0),
            (Tool::Eraser, 0),
        ] {
            assert_eq!(options_row_count(tool), rows, "{tool:?}");
        }
        for (rows_2, maximum_2) in [(0, 0), (8, 0), (9, 1), (30, 22)] {
            assert_eq!(options_max_scroll(rows_2), maximum_2, "{rows_2} filas");
        }
        let mut state_2 = OptionsWindow::default();
        assert_eq!((state_2.open, state_2.scroll_of(Tool::Pencil)), (false, 0));
        state_2.scroll_by(Tool::Pencil, 3, 0);
        assert_eq!(
            state_2.scroll_of(Tool::Pencil),
            0,
            "sin filas no hay nada que desplazar"
        );
        state_2.scroll_by(Tool::Pencil, 5, 30);
        assert_eq!(state_2.scroll_of(Tool::Pencil), 5);
        state_2.scroll_by(Tool::Pencil, 100, 30);
        assert_eq!(
            state_2.scroll_of(Tool::Pencil),
            22,
            "se recorta a `filas − 8`"
        );
        state_2.scroll_by(Tool::Pencil, -100, 30);
        assert_eq!(state_2.scroll_of(Tool::Pencil), 0);
        // Cada herramienta recuerda su propio desplazamiento.
        state_2.scroll_by(Tool::Pencil, 4, 30);
        state_2.scroll_by(Tool::Line, 2, 30);
        assert_eq!(state_2.scroll_of(Tool::Pencil), 4);
        assert_eq!(state_2.scroll_of(Tool::Line), 2);
        assert_eq!(state_2.scroll_of(Tool::Fill), 0);
    }

    #[test]
    fn row_4_the_wheel_over_the_body_scrolls_rows_and_elsewhere_it_does_not() {
        for ppp in PPPS {
            let mut window = WindowBench::new(ppp, Tool::Pencil, 30);
            window.rest_2_2();
            let body = window.at(5 + 10, 39 + 10);
            window.frame(vec![move_pointer(in_points(ppp, body.0, body.1))]);
            window.frame(vec![wheel_line(false)]);
            assert_eq!(
                window.state.scroll_of(Tool::Pencil),
                3,
                "ppp {ppp}: una muesca = 3 filas"
            );
            let title = window.at(60, 6);
            window.frame(vec![move_pointer(in_points(ppp, title.0, title.1))]);
            window.frame(vec![wheel_line(false)]);
            assert_eq!(
                window.state.scroll_of(Tool::Pencil),
                3,
                "ppp {ppp}: fuera del cuerpo"
            );
        }
    }

    // -----------------------------------------------------------------------------------------
    // Fila 5: clic derecho en una herramienta
    // -----------------------------------------------------------------------------------------

    #[test]
    fn row_5_the_right_click_chooses_the_tool_and_asks_to_toggle_the_window_at_the_five_scales() {
        for ppp in PPPS {
            for (i, button) in TOOL_BUTTONS.iter().enumerate() {
                // Con cualquier herramienta activa, también con la propia.
                for active_2 in [Tool::Pencil, Tool::Eraser] {
                    let mut toolbox_2 =
                        Toolbox::new(ppp, state_2(active_2, x5(), Locks::default()));
                    let (x, y) = toolbox_2.center_of_tool(i as i64);
                    let outputs = toolbox_2.click(PointerButton::Secondary, x, y);
                    assert_eq!(
                        actions(&outputs),
                        vec![button.action],
                        "ppp {ppp}: {} con {active_2:?}",
                        button.label
                    );
                    assert!(
                        toggles(&outputs),
                        "ppp {ppp}: {} abre o cierra la ventana",
                        button.label
                    );
                }
            }
        }
    }

    #[test]
    fn row_5_blocked_there_is_neither_tool_nor_window_and_the_removal_does_not_block() {
        type Put = fn(&mut Locks);
        let locks_2: [(&str, Put); 4] = [
            ("gesto", |l| l.gesture = true),
            ("diálogo", |l| l.dialog = true),
            ("modal", |l| l.modal = true),
            ("campo", |l| l.text_focus = true),
        ];
        for ppp in PPPS {
            for (name_2, put) in locks_2 {
                let mut locks = Locks::default();
                put(&mut locks);
                let mut toolbox_2 = Toolbox::new(ppp, state_2(Tool::Pencil, x5(), locks));
                for i in 0..5 {
                    let (x, y) = toolbox_2.center_of_tool(i);
                    let outputs = toolbox_2.click(PointerButton::Secondary, x, y);
                    assert!(actions(&outputs).is_empty(), "ppp {ppp}: {name_2}");
                    assert!(!toggles(&outputs), "ppp {ppp}: {name_2}");
                }
            }
            let with_removes = Locks {
                removal: true,
                ..Locks::default()
            };
            let mut toolbox_2 = Toolbox::new(ppp, state_2(Tool::Pencil, x5(), with_removes));
            let (x, y) = toolbox_2.center_of_tool(1);
            let outputs = toolbox_2.click(PointerButton::Secondary, x, y);
            assert_eq!(actions(&outputs), vec![AppAction::SelectTool(Tool::Line)]);
            assert!(toggles(&outputs));
        }
    }

    #[test]
    fn row_5_the_left_click_does_not_toggle_the_window_and_the_middle_one_does_nothing() {
        for ppp in PPPS {
            let mut toolbox_2 = Toolbox::new(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
            let (x, y) = toolbox_2.center_of_tool(2);
            let left_2 = toolbox_2.click(PointerButton::Primary, x, y);
            assert_eq!(actions(&left_2), vec![AppAction::SelectTool(Tool::Fill)]);
            assert!(!toggles(&left_2), "ppp {ppp}");
            let central = toolbox_2.click(PointerButton::Middle, x, y);
            assert!(
                actions(&central).is_empty() && !toggles(&central),
                "ppp {ppp}"
            );
        }
    }

    // -----------------------------------------------------------------------------------------
    // Fila 6: clic derecho en el zoom (la tabla `Secondary` en los dos sentidos)
    // -----------------------------------------------------------------------------------------

    fn zoom_level(n: u32) -> Option<Zoom> {
        Zoom::from_int(n)
    }

    #[test]
    fn row_6_zoom_in_zooms_out_zoom_out_zooms_in_and_fit_and_one_to_one_do_nothing() {
        let state_2 = state_2(Tool::Pencil, x5(), Locks::default());
        assert_eq!(
            secondary_outcome(per_label("Acercar"), &state_2),
            SecondaryOutcome::Action(AppAction::ZoomOut)
        );
        assert_eq!(
            secondary_outcome(per_label("Alejar"), &state_2),
            SecondaryOutcome::Action(AppAction::ZoomIn)
        );
        assert_eq!(
            secondary_outcome(per_label("Ajustar"), &state_2),
            SecondaryOutcome::Nothing
        );
        assert_eq!(
            secondary_outcome(per_label("1:1"), &state_2),
            SecondaryOutcome::Nothing
        );
        for button in &TOOL_BUTTONS {
            assert_eq!(
                secondary_outcome(button, &state_2),
                SecondaryOutcome::SelectAndToggle(button.action)
            );
        }
    }

    #[test]
    fn row_6_it_is_enabled_by_the_effective_action_at_the_ends_of_the_ladder() {
        let at_max = state_2(Tool::Pencil, Some(Zoom::MAX), Locks::default());
        let at_min = state_2(Tool::Pencil, Some(Zoom::MIN), Locks::default());
        // En ×64 el botón `Acercar` está atenuado para el izquierdo y el derecho aleja.
        assert!(!px_ui::actions::enabled(AppAction::ZoomIn, &at_max));
        assert_eq!(
            secondary_outcome(per_label("Acercar"), &at_max),
            SecondaryOutcome::Action(AppAction::ZoomOut)
        );
        assert_eq!(
            secondary_outcome(per_label("Alejar"), &at_max),
            SecondaryOutcome::Nothing
        );
        // En 1/64, al revés.
        assert_eq!(
            secondary_outcome(per_label("Alejar"), &at_min),
            SecondaryOutcome::Action(AppAction::ZoomIn)
        );
        assert_eq!(
            secondary_outcome(per_label("Acercar"), &at_min),
            SecondaryOutcome::Nothing
        );
        // Sin imagen, nada.
        let without_image = state_2(Tool::Pencil, None, Locks::default());
        for label in ["Acercar", "Alejar", "Ajustar", "1:1"] {
            assert_eq!(
                secondary_outcome(per_label(label), &without_image),
                SecondaryOutcome::Nothing,
                "{label}"
            );
        }
    }

    #[test]
    fn row_6_the_zoom_locks_are_those_of_the_spec_17_with_the_button_source() {
        let with = |put: fn(&mut Locks)| {
            let mut l = Locks::default();
            put(&mut l);
            state_2(Tool::Pencil, zoom_level(5), l)
        };
        for (name_2, e, passes) in [
            ("gesto", with(|l| l.gesture = true), true),
            ("quita", with(|l| l.removal = true), true),
            ("campo", with(|l| l.text_focus = true), true),
            ("diálogo", with(|l| l.dialog = true), false),
            ("modal", with(|l| l.modal = true), false),
        ] {
            for (label, effective) in [
                ("Acercar", AppAction::ZoomOut),
                ("Alejar", AppAction::ZoomIn),
            ] {
                let expected_2 = if passes {
                    SecondaryOutcome::Action(effective)
                } else {
                    SecondaryOutcome::Nothing
                };
                assert_eq!(
                    secondary_outcome(per_label(label), &e),
                    expected_2,
                    "{label} con {name_2}"
                );
            }
        }
    }

    #[test]
    fn row_6_right_click_on_the_zoom_buttons_gives_the_opposite_action_at_the_five_scales() {
        for ppp in PPPS {
            for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
                let expected_2: Vec<AppAction> = match button.label {
                    "Acercar" => vec![AppAction::ZoomOut],
                    "Alejar" => vec![AppAction::ZoomIn],
                    _ => vec![],
                };
                let mut toolbox_2 =
                    Toolbox::new(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
                let (x, y) = toolbox_2.center_of_zoom(i as i64);
                let outputs = toolbox_2.click(PointerButton::Secondary, x, y);
                assert_eq!(actions(&outputs), expected_2, "ppp {ppp}: {}", button.label);
                assert!(!toggles(&outputs), "ppp {ppp}: el zoom no abre la ventana");
                // En ×64 el derecho de `Acercar` aleja aunque el izquierdo esté atenuado.
                if button.label == "Acercar" {
                    let mut toolbox_2 = Toolbox::new(
                        ppp,
                        state_2(Tool::Pencil, Some(Zoom::MAX), Locks::default()),
                    );
                    let right = toolbox_2.click(PointerButton::Secondary, x, y);
                    assert_eq!(actions(&right), vec![AppAction::ZoomOut], "ppp {ppp}");
                    let left_2 = toolbox_2.click(PointerButton::Primary, x, y);
                    assert!(actions(&left_2).is_empty(), "ppp {ppp}");
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Fila 7: tooltips
    // -----------------------------------------------------------------------------------------

    fn expected_tooltips(command: &str) -> Vec<(&'static str, String)> {
        vec![
            ("Lápiz", "Lápiz (d)\nclic derecho: opciones".to_string()),
            ("Línea", "Línea (v)\nclic derecho: opciones".to_string()),
            ("Relleno", "Relleno (f)\nclic derecho: opciones".to_string()),
            (
                "Cuentagotas",
                "Cuentagotas (,)\nclic derecho: opciones".to_string(),
            ),
            (
                "Borrador",
                "Borrador (k)\nclic derecho: opciones\nel clic derecho también borra con el fondo"
                    .to_string(),
            ),
            ("Acercar", "Acercar (>)\nclic derecho: alejar".to_string()),
            ("Alejar", "Alejar (<)\nclic derecho: acercar".to_string()),
            ("Ajustar", "Ajustar (Inicio)".to_string()),
            ("1:1", format!("1:1 ({command}+1)")),
        ]
    }

    #[test]
    fn row_7_the_tooltip_text_reads_the_table_on_the_three_platforms() {
        for (platform, command) in [
            (Platform::Linux, "Ctrl"),
            (Platform::Windows, "Ctrl"),
            (Platform::Mac, "Cmd"),
        ] {
            for (label, text_2) in expected_tooltips(command) {
                assert_eq!(
                    tooltip_text(per_label(label), default_keymap(), platform),
                    text_2,
                    "{platform:?}: {label}"
                );
            }
        }
        // Con otra tabla cambia (la `d` pasa a `x`).
        let rows_2 = default_keymap()
            .rows()
            .iter()
            .cloned()
            .map(|mut f| {
                if !f.command && f.key == KeyId::Char('d') {
                    f.key = KeyId::Char('x');
                }
                f
            })
            .collect();
        let other = Keymap::new(rows_2);
        assert_eq!(
            tooltip_text(per_label("Lápiz"), &other, LINUX),
            "Lápiz (x)\nclic derecho: opciones"
        );
    }

    #[test]
    fn row_7_all_the_tooltip_text_fits_in_the_interface_character_set() {
        for platform in [Platform::Linux, Platform::Windows, Platform::Mac] {
            for (label, _) in expected_tooltips("Ctrl") {
                let text_2 = tooltip_text(per_label(label), default_keymap(), platform);
                for c in text_2.chars().filter(|c| *c != '\n') {
                    assert!(INTERFACE_CHARSET.contains(c), "{label}: {c:?} no está");
                }
            }
        }
    }

    #[test]
    fn row_7_the_hover_shows_the_tooltip_with_the_pixel_font_and_without_hover_there_is_none() {
        for ppp in PPPS {
            let n = n_of(ppp);
            for (i, (label, text_2)) in expected_tooltips("Ctrl").iter().enumerate() {
                let mut toolbox_2 =
                    Toolbox::new(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
                // El primer renglón del tooltip es el que lleva el atajo; el segundo, el clic derecho.
                let first = text_2.lines().next().expect("hay una línea").to_string();
                let without_hover = toolbox_2.rest_2_2();
                let count = |p: &Painted, t: &str| p.texts.iter().filter(|x| x.text_2 == t).count();
                let before = count(&without_hover, &first);
                let (x, y) = if i < 5 {
                    toolbox_2.center_of_tool(i as i64)
                } else {
                    toolbox_2.center_of_zoom(i as i64 - 5)
                };
                let pos = in_points(ppp, x, y);
                let mut last_2 = toolbox_2.frame(vec![move_pointer(pos)]).0;
                for _ in 0..8 {
                    last_2 = toolbox_2.frame(vec![]).0;
                }
                assert_eq!(
                    count(&last_2, &first),
                    before + 1,
                    "ppp {ppp}: {label}: falta el tooltip {first:?}; hay {:?}",
                    last_2.all_the_texts()
                );
                let line_2 = last_2
                    .texts
                    .iter()
                    .rfind(|t| t.text_2 == first)
                    .expect("el tooltip");
                assert!(
                    (line_2.size_2.y * ppp - (14 * n) as f32).abs() <= 0.05,
                    "ppp {ppp}: {label}: el tooltip no usa la fuente de píxeles ({} px de alto)",
                    line_2.size_2.y * ppp
                );
                if let Some(second) = text_2.lines().nth(1) {
                    assert!(
                        last_2.paints(second),
                        "ppp {ppp}: {label}: falta {second:?}"
                    );
                }
                // Sin el puntero encima, desaparece.
                toolbox_2.frame(vec![move_pointer(in_points(ppp, 3000, 3000))]);
                let far = toolbox_2.frame(vec![]).0;
                assert_eq!(count(&far, &first), before, "ppp {ppp}: {label}");
            }
        }
    }

    #[test]
    fn row_7_a_dimmed_button_also_shows_its_tooltip() {
        let ppp = 1.0;
        let mut toolbox_2 = Toolbox::new(
            ppp,
            state_2(Tool::Pencil, Some(Zoom::MAX), Locks::default()),
        );
        let (x, y) = toolbox_2.center_of_zoom(0); // Acercar, atenuado en ×64.
        toolbox_2.frame(vec![move_pointer(in_points(ppp, x, y))]);
        let mut p = toolbox_2.frame(vec![]).0;
        for _ in 0..8 {
            p = toolbox_2.frame(vec![]).0;
        }
        assert!(p.paints("Acercar (>)"), "hay {:?}", p.all_the_texts());
    }

    // -----------------------------------------------------------------------------------------
    // Propiedad 3: el clic secundario sigue la tabla
    // -----------------------------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn property_3_the_right_click_follows_the_secondary_table_and_the_lock_of_the_button_source(
            tool in 0usize..5,
            level in prop::option::of(0usize..1000),
            gesture in any::<bool>(),
            dialog in any::<bool>(),
            text_focus in any::<bool>(),
            removal in any::<bool>(),
            modal in any::<bool>(),
        ) {
            let levels: Vec<Zoom> = Zoom::levels().collect();
            let zoom = level.map(|i| levels[i % levels.len()]);
            let locks = Locks { gesture, dialog, text_focus, removal, modal, ..Locks::default() };
            let e = state_2(TEST_TOOLS[tool], zoom, locks);
            for button in TOOL_BUTTONS.iter().chain(ZOOM_BUTTONS.iter()) {
                let effective = match button.label {
                    "Acercar" => Some(AppAction::ZoomOut),
                    "Alejar" => Some(AppAction::ZoomIn),
                    "Ajustar" | "1:1" => None,
                    _ => Some(button.action),
                };
                let expected_2 = match effective {
                    None => SecondaryOutcome::Nothing,
                    Some(action_2) => {
                        let there_is_step = match action_2 {
                            AppAction::ZoomOut => zoom.is_some_and(|z| z > Zoom::MIN),
                            AppAction::ZoomIn => zoom.is_some_and(|z| z < Zoom::MAX),
                            _ => true,
                        };
                        if is_blocked(action_2, locks, Origin::Button) || !there_is_step {
                            SecondaryOutcome::Nothing
                        } else if matches!(action_2, AppAction::SelectTool(_)) {
                            SecondaryOutcome::SelectAndToggle(action_2)
                        } else {
                            SecondaryOutcome::Action(action_2)
                        }
                    }
                };
                prop_assert_eq!(secondary_outcome(button, &e), expected_2, "{}", button.label);
            }
        }

    }
}

// ---------------------------------------------------------------------------------------------
// Unidad I.4 (specs/interfaz-iconos.md): filas 6 y 8 de la caja, con iconos
// ---------------------------------------------------------------------------------------------

mod icons {
    use super::*;
    use egui::output::OutputEvent;
    use egui::{Pos2, RawInput, Rect, Role};
    use px_ui::icon_button::{Look, look};
    use px_ui::theme::SURFACE_3;

    const PPPS_FINE: [f32; 9] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0];

    fn icon_of(i: usize) -> px_ui::icons::Icon {
        TOOL_BUTTONS
            .iter()
            .chain(ZOOM_BUTTONS.iter())
            .nth(i)
            .expect("botón")
            .icon
    }

    fn there_is(p: &Painted, ppp: f32, color: egui::Color32, expected_2: [i64; 4]) -> bool {
        p.fills.iter().any(|r| {
            r.fill == color
                && [r.rect.min.x, r.rect.min.y, r.rect.max.x, r.rect.max.y]
                    .iter()
                    .zip(expected_2)
                    .all(|(a, b)| (a * ppp - b as f32).abs() <= tolerance_of_egui(ppp))
        })
    }

    /// El botón `i` se pinta con la cara, el relieve y el icono del aspecto `l`.
    fn checks(p: &Painted, bench: &Bench, i: usize, l: &Look, context: &str) {
        let (ppp, n) = (bench.ppp, bench.n());
        let (ex, ey) = Bench::corner_u(i);
        let (x, y) = (bench.x0() + ex * n, ey * n);
        let side = TEST_BUTTON_U * n;
        assert!(
            there_is(p, ppp, l.face, [x, y, x + side, y + side]),
            "{context}: falta la cara"
        );
        for (band, color) in [
            ([x, y, x + side, y + n], l.top_left),
            ([x, y, x + n, y + side], l.top_left),
            ([x, y + side - n, x + side, y + side], l.bottom_right),
            ([x + side - n, y, x + side, y + side], l.bottom_right),
        ] {
            assert!(
                there_is(p, ppp, color, band),
                "{context}: falta la franja {band:?}"
            );
        }
        let id = bench.icons_2.set.get(icon_of(i)).expect("textura");
        let mesh = p
            .mesh_of(id)
            .unwrap_or_else(|| panic!("{context}: no se pinta el icono {i}"));
        let d = (4 + l.offset_u) * n;
        let expected_2 = [x + d, y + d, x + d + 16 * n, y + d + 16 * n];
        let real = [
            mesh.rect.min.x,
            mesh.rect.min.y,
            mesh.rect.max.x,
            mesh.rect.max.y,
        ];
        for (a, b) in real.iter().zip(expected_2) {
            assert!(
                (a * ppp - b as f32).abs() <= tolerance_of_egui(ppp),
                "{context}: icono {i} en {real:?} (pt) y no en {expected_2:?} (px)"
            );
            // Esquinas en píxeles físicos enteros.
            assert!(
                (a * ppp - (a * ppp).round()).abs() <= tolerance_of_egui(ppp),
                "{context}"
            );
        }
        assert_eq!(mesh.color, l.tint, "{context}: tinta");
        assert!(
            mesh.uv.min.distance(egui::pos2(0.0, 0.0)) < 1e-4
                && mesh.uv.max.distance(egui::pos2(1.0, 1.0)) < 1e-4,
            "{context}: UV completo"
        );
    }

    #[test]
    fn row_6_normal_pressed_and_disabled_are_painted_with_their_look_at_ppp_from_1_to_3() {
        for ppp in PPPS_FINE {
            // Normal (Borrador, Acercar) y hundido (el Lápiz activo).
            let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
            let p = bench.rest_2_2();
            let context = format!("ppp {ppp}");
            checks(
                &p,
                &bench,
                4,
                &look(false, true, false),
                &format!("{context} normal"),
            );
            checks(
                &p,
                &bench,
                5,
                &look(false, true, false),
                &format!("{context} zoom"),
            );
            checks(
                &p,
                &bench,
                0,
                &look(true, true, false),
                &format!("{context} hundido"),
            );
            // Con un gesto: las herramientas se atenúan (el activo, hundido y atenuado) y el zoom no.
            let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), lock(|l| l.gesture = true)));
            let p = bench.rest_2_2();
            checks(
                &p,
                &bench,
                1,
                &look(false, false, false),
                &format!("{context} atenuado"),
            );
            checks(
                &p,
                &bench,
                0,
                &look(true, false, false),
                &format!("{context} activo atenuado"),
            );
            checks(
                &p,
                &bench,
                6,
                &look(false, true, false),
                &format!("{context} zoom libre"),
            );
            // Ningún texto de botón: lo pintado como texto no cae sobre ninguna casilla.
            let n = bench.n();
            for text_2 in &p.texts {
                let r = text_2.rect();
                for i in 0..9 {
                    let (ex, ey) = Bench::corner_u(i);
                    let box_value = Rect::from_min_size(
                        egui::pos2((bench.x0() + ex * n) as f32 / ppp, (ey * n) as f32 / ppp),
                        egui::vec2(
                            (TEST_BUTTON_U * n) as f32 / ppp,
                            (TEST_BUTTON_U * n) as f32 / ppp,
                        ),
                    );
                    assert!(
                        !box_value.shrink(0.01).intersects(r),
                        "{context}: el texto {:?} cae sobre el botón {i}",
                        text_2.text_2
                    );
                }
            }
            // Todo lo pintado por la caja cae dentro de ella.
            for mesh in &p.meshes {
                assert!(
                    mesh.rect.max.x * ppp
                        <= (bench.x0() + COLUMN_U * n) as f32 + tolerance_of_egui(ppp)
                        && mesh.rect.min.x * ppp >= bench.x0() as f32 - tolerance_of_egui(ppp),
                    "{context}: icono fuera de la caja: {:?}",
                    mesh.rect
                );
            }
        }
    }

    #[test]
    fn row_6_with_the_mouse_over_it_the_face_lightens_and_a_dimmed_button_does_not_lighten() {
        for ppp in PPPS {
            let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
            let n = bench.n();
            let (cx, cy) = center_u(2);
            let pos = in_points(ppp, bench.x0() + cx * n, cy * n);
            bench.frame(vec![move_pointer(pos)]);
            let p = bench.frame(vec![]).0;
            checks(
                &p,
                &bench,
                2,
                &look(false, true, true),
                &format!("ppp {ppp}: encima"),
            );
            assert_eq!(look(false, true, true).face, SURFACE_3);
            // Atenuado: con el ratón encima sigue con la cara normal.
            let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), lock(|l| l.gesture = true)));
            bench.frame(vec![move_pointer(pos)]);
            let p = bench.frame(vec![]).0;
            checks(
                &p,
                &bench,
                2,
                &look(false, false, true),
                &format!("ppp {ppp}: atenuado"),
            );
        }
    }

    /// Los eventos de salida del fotograma (`widget_info`).
    fn events_2(bench: &Bench, events: Vec<Event>) -> Vec<OutputEvent> {
        let input_2 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, TEST_SCREEN)),
            events,
            ..RawInput::default()
        };
        let mut output_2 = bench.ctx.run_ui(input_2, |ui| {
            let view = ToolboxView {
                state: &bench.state_2,
                zoom_label: bench.label_2,
                icons: &bench.icons_2.set,
            };
            show_toolbox(ui, &view, default_keymap(), Platform::Linux);
        });
        output_2.textures_delta.clear();
        output_2.platform_output.events.clone()
    }

    #[test]
    fn row_8_each_enabled_button_emits_widget_info_of_type_button_with_its_label() {
        for ppp in [1.0, 2.0] {
            let bench = Bench::new_2(ppp, state_2(Tool::Pencil, x5(), Locks::default()));
            for (i, button) in TOOL_BUTTONS.iter().chain(ZOOM_BUTTONS.iter()).enumerate() {
                bench.rest_2_2();
                let n = bench.n();
                let (cx, cy) = center_u(i);
                let pos = in_points(ppp, bench.x0() + cx * n, cy * n);
                let mut all_2 = events_2(&bench, vec![move_pointer(pos)]);
                all_2.extend(events_2(&bench, vec![button_2(pos, true)]));
                all_2.extend(events_2(&bench, vec![button_2(pos, false)]));
                assert!(
                    all_2.iter().any(|e| matches!(e, OutputEvent::Clicked(info)
                        if info.role == Role::Button
                            && info.label.as_deref() == Some(button.label)
                            && info.enabled)),
                    "ppp {ppp}: {}: sin WidgetInfo de botón; hay {all_2:?}",
                    button.label
                );
                bench.frame(vec![move_pointer(in_points(ppp, 3000, 3000))]);
            }
        }
    }
}
