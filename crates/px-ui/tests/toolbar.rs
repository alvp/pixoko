//! Tests de la columna de herramientas `px_ui::toolbar` (specs/hito-2-menu-herramientas.md, parte
//! de la fila 7 «Geometría a las cinco escalas»; la pila completa está en `window_stack.rs` y la
//! aritmética pura en `chrome.rs`).
//!
//! Banco de egui sin ventana (`Context::run_ui` con `RawInput` sintético): `show_toolbox` se pinta
//! sola en una ventana vacía, así que la columna queda pegada al borde derecho (`common::zonas::caja_sola`). Se comprueba a las cinco
//! escalas (`ppp` 1,0, 1,25, 1,5, 2,0 y 3,0), en píxeles físicos:
//!
//! - las tablas `TOOL_BUTTONS` y `ZOOM_BUTTONS`;
//! - los 9 botones golpean en su sitio: casillas de 24 × 24 u en una rejilla de cuatro columnas
//!   (specs/interfaz-iconos.md; primera y última coordenada física de cada botón dentro); el
//!   nivel de zoom y lo que queda fuera de la columna no golpean nada;
//! - el activo se hunde (cara `SURFACE_0`, icono `ACCENT`; contraste ≥ 4,5 con `contrast_ratio`),
//!   los demás llevan el icono en `TEXT`, y los deshabilitados se atenúan y no responden (con las
//!   reglas de bloqueo del zoom: gesto, quita y campo de texto no lo apagan);
//! - el nivel de zoom es un texto de 14 u en `y = 62`.
//!
//! Las posiciones pintadas se comparan con la tolerancia del redondeo de egui
//! (`common::tolerancia_de_egui`: 1/32 pt, porque `Panel` y `allocate_rect` redondean a múltiplos pares de 1/32 pt).
//!
//! Supuestos sobre lo que la spec no fija (cualquier otro reparto cambiaría esta página):
//! - `show_toolbox` crea su propio panel a la derecha de `ui` (como `status::show` y
//!   `palette_panel::show`), de `toolbar_width_u()·n` píxeles de ancho;
//! - el rótulo de zoom que recibe es el texto del nivel (`×12`, `1/8`, `—`) con o sin el prefijo
//!   `Zoom `: el banco solo exige que el texto pintado lo contenga.

use egui::{Context, Event, Vec2, vec2};
use px_ui::actions::{AppAction, Locks, Platform, UiState};
use px_ui::keymap::{Keymap, default_keymap};
use px_ui::options::OptionsState;
use px_ui::theme::{ACCENT, SURFACE_0, TEXT, TEXT_DIM, TEXT_DISABLED, contrast_ratio};
use px_ui::tool::Tool;
use px_ui::toolbar::{TOOL_BUTTONS, ToolboxView, ZOOM_BUTTONS, show_toolbox};
use px_ui::view::Zoom;

mod common;
use common::caja::centro_u;
use common::iconos::IconosDePrueba;
use common::zonas::caja_sola;
use common::{
    PPPS, Pintado, boton, contexto, en_puntos, fotograma, mover, n_de, pintado, tolerancia_de_egui,
};

const HERRAMIENTAS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];
const PANTALLA: Vec2 = vec2(400.0, 600.0);
/// Ancho de la columna, en `u`, y lado de un botón (casilla de 24 × 24 u).
const COLUMNA_U: i64 = 107;
const BOTON_U: i64 = 24;
const MARGEN_U: i64 = 4;

fn estado(tool: Tool, zoom: Option<Zoom>, locks: Locks) -> UiState {
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

struct Banco {
    ctx: Context,
    ppp: f32,
    estado: UiState,
    etiqueta: &'static str,
    iconos: IconosDePrueba,
}

impl Banco {
    fn nuevo(ppp: f32, estado: UiState) -> Banco {
        let ctx = contexto(ppp);
        let iconos = IconosDePrueba::nuevos(&ctx);
        Banco {
            ctx,
            ppp,
            estado,
            etiqueta: "×5",
            iconos,
        }
    }

    fn n(&self) -> i64 {
        n_de(self.ppp)
    }

    /// `x` de la columna en píxeles físicos (pegada al borde derecho de la pantalla del banco).
    fn x0(&self) -> i64 {
        caja_sola(
            (
                (PANTALLA.x * self.ppp).round() as i64,
                (PANTALLA.y * self.ppp).round() as i64,
            ),
            self.n() as u32,
        )
        .x
    }

    /// Un fotograma: lo pintado y lo que devolvió `show_toolbox`.
    fn fotograma(&self, events: Vec<Event>) -> (Pintado, Option<AppAction>) {
        let mut accion = None;
        let formas = fotograma(&self.ctx, PANTALLA, events, |ui| {
            let vista = ToolboxView {
                state: &self.estado,
                zoom_label: self.etiqueta,
                options: &OptionsState::default(),
                icons: &self.iconos.set,
            };
            accion = show_toolbox(ui, &vista, default_keymap(), Platform::Linux).action;
        });
        (pintado(&formas), accion)
    }

    fn reposo(&self) -> Pintado {
        // Dos fotogramas: el primero reparte el espacio y el segundo es el estable.
        self.fotograma(vec![]);
        self.fotograma(vec![]).0
    }

    /// Mueve, pulsa y suelta en `(x_px, y_px)`; devuelve las acciones que salieron.
    fn clic(&self, x_px: i64, y_px: i64) -> Vec<AppAction> {
        self.reposo();
        let pos = en_puntos(self.ppp, x_px, y_px);
        let mut acciones = Vec::new();
        for events in [
            vec![mover(pos)],
            vec![boton(pos, true)],
            vec![boton(pos, false)],
        ] {
            acciones.extend(self.fotograma(events).1);
        }
        // Aparta el puntero para no dejar nada pulsado ni bajo el cursor.
        self.fotograma(vec![mover(en_puntos(self.ppp, 3000, 3000))]);
        acciones
    }

    /// Clic en el centro del botón `i` (0 a 4 las herramientas, 5 a 8 el zoom).
    fn clic_en_boton(&self, i: usize) -> Vec<AppAction> {
        let n = self.n();
        let (cx, cy) = centro_u(i);
        self.clic(self.x0() + cx * n, cy * n)
    }

    /// Rectángulo `(x, y)` de la esquina del botón `i`, en `u` desde la esquina de la caja.
    fn esquina_u(i: usize) -> (i64, i64) {
        let (cx, cy) = centro_u(i);
        (cx - BOTON_U / 2, cy - BOTON_U / 2)
    }
}

// ---------------------------------------------------------------------------------------------
// Tablas y colores
// ---------------------------------------------------------------------------------------------

#[test]
fn el_activo_tiene_contraste_suficiente_accent_sobre_surface_0() {
    assert!(
        contrast_ratio(ACCENT, SURFACE_0) >= 4.5,
        "contraste {}",
        contrast_ratio(ACCENT, SURFACE_0)
    );
}

/// La letra que la columna enseña para `action`: sale de la tabla de teclas (`Button.key` ya no
/// existe) y no hay ninguna si la acción no tiene fila.
fn letra_de(action: AppAction) -> Option<char> {
    Keymap::default_scheme()
        .shown_text(action, Platform::Linux)
        .and_then(|texto| texto.chars().next())
}

#[test]
fn tool_buttons_son_los_cinco_botones_con_su_letra() {
    let tabla: Vec<(&str, AppAction, Option<char>)> = TOOL_BUTTONS
        .iter()
        .map(|b| (b.label, b.action, letra_de(b.action)))
        .collect();
    assert_eq!(
        tabla,
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
fn zoom_buttons_son_los_cuatro_botones_sin_letra() {
    let tabla: Vec<(&str, AppAction)> = ZOOM_BUTTONS.iter().map(|b| (b.label, b.action)).collect();
    assert_eq!(
        tabla,
        [
            ("Acercar", AppAction::ZoomIn),
            ("Alejar", AppAction::ZoomOut),
            ("Ajustar", AppAction::ZoomFit),
            ("1:1", AppAction::ZoomActual),
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 7: los botones golpean en su sitio
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_7_el_centro_de_cada_boton_de_herramienta_da_su_accion_a_las_cinco_escalas() {
    for ppp in PPPS {
        let banco = Banco::nuevo(ppp, estado(Tool::Pencil, x5(), Locks::default()));
        for (i, button) in TOOL_BUTTONS.iter().enumerate() {
            assert_eq!(
                banco.clic_en_boton(i),
                vec![button.action],
                "ppp {ppp}: {}",
                button.label
            );
        }
    }
}

#[test]
fn fila_7_el_centro_de_cada_boton_de_zoom_da_su_accion_a_las_cinco_escalas() {
    for ppp in PPPS {
        let banco = Banco::nuevo(ppp, estado(Tool::Pencil, x5(), Locks::default()));
        for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
            assert_eq!(
                banco.clic_en_boton(TOOL_BUTTONS.len() + i),
                vec![button.action],
                "ppp {ppp}: {}",
                button.label
            );
        }
    }
}

#[test]
fn fila_7_la_primera_y_la_ultima_coordenada_fisica_de_cada_boton_golpean() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let banco = Banco::nuevo(ppp, estado(Tool::Pencil, x5(), Locks::default()));
        let casillas: Vec<(usize, AppAction)> = TOOL_BUTTONS
            .iter()
            .chain(ZOOM_BUTTONS.iter())
            .enumerate()
            .map(|(i, b)| (i, b.action))
            .collect();
        for (i, accion) in casillas {
            let (cx_u, cy_u) = Banco::esquina_u(i);
            let (x0, y0) = (banco.x0() + cx_u * n, cy_u * n);
            let (x1, y1) = (
                banco.x0() + (cx_u + BOTON_U) * n - 1,
                (cy_u + BOTON_U) * n - 1,
            );
            for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                assert_eq!(
                    banco.clic(x, y),
                    vec![accion],
                    "ppp {ppp}: ({x}, {y}) debería golpear {accion:?}"
                );
            }
        }
    }
}

/// Los negativos solo se piden **lejos** de los botones: egui da el clic al widget más cercano
/// dentro de `style.interaction.interact_radius` (5 pt), así que un píxel fuera de un botón
/// golpea a ese botón salvo que la columna ponga el radio a 0 (la spec no lo exige).
#[test]
fn fila_7_el_nivel_de_zoom_y_lo_que_queda_lejos_de_los_botones_no_golpean_nada() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let banco = Banco::nuevo(ppp, estado(Tool::Pencil, x5(), Locks::default()));
        let centro_x = banco.x0() + centro_u(1).0 * n;
        // El nivel de zoom (y = 62 a 76): a 9 u de la última fila de herramientas y a 4 u de la
        // de zoom; se pulsa a 16 u y 11 u de ellas, fuera del radio de interacción de egui.
        assert!(
            banco.clic(centro_x, 69 * n).is_empty(),
            "ppp {ppp}: el nivel de zoom no es un botón"
        );
        // Fuera de la columna (a 40 u de su borde, en el lado del lienzo) no hay nada.
        for y_u in [10, 50, 130] {
            assert!(
                banco.clic(banco.x0() - 40 * n, y_u * n).is_empty(),
                "ppp {ppp}: 40 u fuera de la columna no golpean nada (y = {y_u} u)"
            );
        }
    }
}

#[test]
fn fila_7_todo_lo_pintado_cae_dentro_de_la_columna_y_de_sus_botones() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let p = ppp;
        let banco = Banco::nuevo(ppp, estado(Tool::Line, x5(), Locks::default()));
        let pintado = banco.reposo();
        for texto in &pintado.textos {
            let r = texto.rect();
            assert!(
                r.min.x * p >= (banco.x0() + MARGEN_U * n) as f32 - tolerancia_de_egui(ppp)
                    && r.max.x * p
                        <= (banco.x0() + (COLUMNA_U - MARGEN_U) * n) as f32
                            + tolerancia_de_egui(ppp),
                "ppp {ppp}: {:?} se sale de la columna: {r:?}",
                texto.texto
            );
        }
        for relleno in &pintado.rellenos {
            let r = relleno.rect;
            assert!(
                r.max.x * p <= (banco.x0() + COLUMNA_U * n) as f32 + tolerancia_de_egui(ppp),
                "ppp {ppp}: relleno fuera de la columna: {r:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// El activo y la letra
// ---------------------------------------------------------------------------------------------

#[test]
fn el_activo_se_hunde_con_cara_surface_0_e_icono_accent_en_su_boton() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for (i, tool) in HERRAMIENTAS.iter().enumerate() {
            let banco = Banco::nuevo(ppp, estado(*tool, x5(), Locks::default()));
            let pintado = banco.reposo();
            // Solo el icono del activo lleva `ACCENT`, y va 1 u abajo y a la derecha en su botón.
            let acentos: Vec<_> = pintado
                .mallas
                .iter()
                .filter(|m| m.color == ACCENT)
                .collect();
            assert_eq!(
                acentos.len(),
                1,
                "ppp {ppp}: solo el activo ({tool:?}) lleva el icono ACCENT"
            );
            let (ex, ey) = Banco::esquina_u(i);
            let esperado = [
                banco.x0() + (ex + 4 + 1) * n,
                (ey + 4 + 1) * n,
                banco.x0() + (ex + 4 + 1 + 16) * n,
                (ey + 4 + 1 + 16) * n,
            ];
            let r = acentos[0].rect;
            let real = [r.min.x * ppp, r.min.y * ppp, r.max.x * ppp, r.max.y * ppp];
            for (a, b) in real.iter().zip(esperado) {
                assert!(
                    (a - b as f32).abs() <= tolerancia_de_egui(ppp),
                    "ppp {ppp}: {tool:?} {real:?} y no {esperado:?}"
                );
            }
            // La cara del activo es `SURFACE_0` en el rectángulo del botón.
            let cara = [
                banco.x0() + ex * n,
                ey * n,
                banco.x0() + (ex + BOTON_U) * n,
                (ey + BOTON_U) * n,
            ];
            assert!(
                pintado.rellenos.iter().any(|r| {
                    r.fill == SURFACE_0
                        && [r.rect.min.x, r.rect.min.y, r.rect.max.x, r.rect.max.y]
                            .iter()
                            .zip(cara)
                            .all(|(a, b)| (a * ppp - b as f32).abs() <= tolerancia_de_egui(ppp))
                }),
                "ppp {ppp}: {tool:?}: falta la cara hundida en {cara:?}"
            );
        }
    }
}

#[test]
fn los_atajos_ya_no_se_pintan_en_los_botones_y_salen_en_el_tooltip() {
    // El atajo se dice en el tooltip (`tooltip_text`); en el botón solo queda el icono.
    for ppp in PPPS {
        let banco = Banco::nuevo(ppp, estado(Tool::Pencil, x5(), Locks::default()));
        let pintado = banco.reposo();
        for button in TOOL_BUTTONS.iter().skip(1) {
            let Some(letra) = letra_de(button.action).map(|c| c.to_string()) else {
                continue;
            };
            assert!(
                pintado.tramo(&letra).is_none(),
                "ppp {ppp}: la letra {letra} no se pinta en el botón"
            );
        }
    }
}

#[test]
fn se_pintan_los_nueve_iconos_y_ningun_rotulo_de_boton() {
    let banco = Banco::nuevo(1.0, estado(Tool::Fill, x5(), Locks::default()));
    let pintado = banco.reposo();
    let texturas: Vec<_> = pintado.mallas.iter().map(|m| m.textura).collect();
    assert_eq!(texturas.len(), 9, "nueve iconos, uno por botón");
    for (i, a) in texturas.iter().enumerate() {
        assert!(!texturas[i + 1..].contains(a), "iconos repetidos");
    }
    for button in ZOOM_BUTTONS.iter() {
        assert!(
            pintado.texto_con(button.label).is_none(),
            "el rótulo {:?} no se pinta en el botón; hay {:?}",
            button.label,
            pintado.todos_los_textos()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Nivel de zoom
// ---------------------------------------------------------------------------------------------

#[test]
fn el_nivel_de_zoom_es_un_texto_de_14_u_en_y_62() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let banco = Banco::nuevo(ppp, estado(Tool::Pencil, x5(), Locks::default()));
        let pintado = banco.reposo();
        let texto = pintado.texto_con("×5").unwrap_or_else(|| {
            panic!(
                "ppp {ppp}: no se pinta el nivel; hay {:?}",
                pintado.todos_los_textos()
            )
        });
        assert!(
            (texto.pos.y * ppp - (62 * n) as f32).abs() <= tolerancia_de_egui(ppp),
            "ppp {ppp}: el nivel empieza en y = {} px y no en {}",
            texto.pos.y * ppp,
            62 * n
        );
        assert!(
            (texto.tamano.y * ppp - (14 * n) as f32).abs() <= 0.05,
            "ppp {ppp}: el nivel mide {} px de alto y no {}",
            texto.tamano.y * ppp,
            14 * n
        );
    }
}

#[test]
fn sin_imagen_el_nivel_de_zoom_se_pinta_igualmente() {
    let mut banco = Banco::nuevo(1.0, estado(Tool::Pencil, None, Locks::default()));
    banco.etiqueta = "—";
    let pintado = banco.reposo();
    assert!(
        pintado.texto_con("—").is_some(),
        "{:?}",
        pintado.todos_los_textos()
    );
}

// ---------------------------------------------------------------------------------------------
// Deshabilitados
// ---------------------------------------------------------------------------------------------

fn bloqueo(f: impl FnOnce(&mut Locks)) -> Locks {
    let mut locks = Locks::default();
    f(&mut locks);
    locks
}

/// Qué botones responden a un clic: herramientas (5) y zoom (4).
fn responden(banco: &Banco) -> ([bool; 5], [bool; 4]) {
    let mut herramientas = [false; 5];
    let mut zoom = [false; 4];
    for (i, button) in TOOL_BUTTONS.iter().enumerate() {
        herramientas[i] = banco.clic_en_boton(i) == vec![button.action];
    }
    for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
        zoom[i] = banco.clic_en_boton(TOOL_BUTTONS.len() + i) == vec![button.action];
    }
    (herramientas, zoom)
}

#[test]
fn con_un_gesto_o_un_campo_enfocado_las_herramientas_no_responden_pero_el_zoom_si() {
    for ppp in PPPS {
        for locks in [
            bloqueo(|l| l.gesture = true),
            bloqueo(|l| l.text_focus = true),
        ] {
            let banco = Banco::nuevo(ppp, estado(Tool::Pencil, x5(), locks));
            assert_eq!(
                responden(&banco),
                ([false; 5], [true; 4]),
                "ppp {ppp} con {locks:?}"
            );
        }
    }
}

#[test]
fn con_una_quita_pendiente_responden_las_herramientas_y_el_zoom() {
    let banco = Banco::nuevo(
        1.0,
        estado(Tool::Pencil, x5(), bloqueo(|l| l.removal = true)),
    );
    assert_eq!(responden(&banco), ([true; 5], [true; 4]));
}

#[test]
fn con_un_dialogo_abierto_o_el_panel_de_licencias_nada_responde() {
    for locks in [bloqueo(|l| l.dialog = true), bloqueo(|l| l.modal = true)] {
        let banco = Banco::nuevo(1.0, estado(Tool::Pencil, x5(), locks));
        assert_eq!(responden(&banco), ([false; 5], [false; 4]), "{locks:?}");
    }
}

#[test]
fn sin_imagen_el_zoom_no_responde_y_en_el_minimo_solo_alejar_esta_apagado() {
    let banco = Banco::nuevo(1.0, estado(Tool::Pencil, None, Locks::default()));
    assert_eq!(responden(&banco), ([true; 5], [false; 4]));
    let banco = Banco::nuevo(1.0, estado(Tool::Pencil, Some(Zoom::MIN), Locks::default()));
    // Acercar, Alejar, Ajustar y 1:1: en el mínimo (1/64) solo `Alejar` está deshabilitado.
    assert_eq!(responden(&banco), ([true; 5], [true, false, true, true]));
    // En ×1 (el mínimo de antes) `Alejar` ya vale (baja a 1/2) y `1:1` no.
    let banco = Banco::nuevo(
        1.0,
        estado(Tool::Pencil, Zoom::from_int(1), Locks::default()),
    );
    assert_eq!(responden(&banco), ([true; 5], [true, true, true, false]));
}

#[test]
fn en_zoom_max_acercar_no_responde() {
    let banco = Banco::nuevo(1.0, estado(Tool::Pencil, Some(Zoom::MAX), Locks::default()));
    assert_eq!(responden(&banco), ([true; 5], [false, true, true, true]));
}

#[test]
fn un_boton_deshabilitado_se_pinta_atenuado() {
    // El icono de `Línea` cambia de tinta al deshabilitarlo (no es el activo en ninguno de los dos).
    let libre = Banco::nuevo(1.0, estado(Tool::Pencil, x5(), Locks::default()));
    let con_gesto = Banco::nuevo(
        1.0,
        estado(Tool::Pencil, x5(), bloqueo(|l| l.gesture = true)),
    );
    let tinta = |banco: &Banco, indice: usize| {
        let id = banco.iconos.set.get(
            TOOL_BUTTONS
                .iter()
                .chain(ZOOM_BUTTONS.iter())
                .nth(indice)
                .expect("botón")
                .icon,
        );
        banco
            .reposo()
            .malla_de(id.expect("textura"))
            .expect("icono pintado")
            .color
    };
    assert_eq!(tinta(&libre, 1), TEXT);
    assert_eq!(
        tinta(&con_gesto, 1),
        TEXT_DISABLED,
        "el deshabilitado va atenuado"
    );
    // El zoom sigue habilitado con el gesto: su icono no cambia.
    assert_eq!(tinta(&libre, 5), tinta(&con_gesto, 5));
}

// ---------------------------------------------------------------------------------------------
// Unidad I.3 (specs/interfaz-caja-derecha.md): recuadro de opciones, clic derecho y tooltips
// ---------------------------------------------------------------------------------------------

mod caja_derecha {
    use super::*;
    use egui::{Color32, PointerButton, RawInput, Rect};
    use proptest::prelude::*;
    use px_ui::actions::{Origin, is_blocked};
    use px_ui::chrome::{
        OPTIONS_BODY_ROWS, OPTIONS_BODY_X_U, OPTIONS_BODY_Y_U, OPTIONS_BORDER_U,
        OPTIONS_FOCUS_BORDER_U, OPTIONS_H_U, OPTIONS_KEY_Y_U, OPTIONS_TITLE_X_U, OPTIONS_TITLE_Y_U,
        OPTIONS_W_U, OPTIONS_X_U, OPTIONS_Y_U,
    };
    use px_ui::keymap::{KeyId, default_keymap};
    use px_ui::options::{
        NO_OPTIONS, OptionsState, key_line, options_max_scroll, options_row_count,
    };
    use px_ui::palette_grid::ring_rects;
    use px_ui::text::INTERFACE_CHARSET;
    use px_ui::theme::{SURFACE_1, SURFACE_3};
    use px_ui::toolbar::{
        SecondaryOutcome, ToolboxOutput, ToolboxView, secondary_outcome, show_toolbox, tooltip_text,
    };
    use px_ui::view::ScreenRect;

    use super::common::{Texto, boton_de, rueda_linea};

    const LINUX: Platform = Platform::Linux;

    /// Banco de la caja entera (`show_toolbox`) con el tiempo del fotograma (los tooltips lo usan).
    struct Caja {
        ctx: Context,
        ppp: f32,
        estado: UiState,
        opciones: OptionsState,
        keymap: Keymap,
        tiempo: f64,
        iconos: IconosDePrueba,
    }

    impl Caja {
        fn nueva(ppp: f32, estado: UiState) -> Caja {
            let ctx = contexto(ppp);
            let iconos = IconosDePrueba::nuevos(&ctx);
            Caja {
                ctx,
                ppp,
                estado,
                opciones: OptionsState::default(),
                keymap: default_keymap().clone(),
                tiempo: 0.0,
                iconos,
            }
        }

        fn n(&self) -> i64 {
            n_de(self.ppp)
        }

        /// `x` de la caja en píxeles físicos (pegada al borde derecho de la pantalla del banco).
        fn x0(&self) -> i64 {
            caja_sola(
                (
                    (PANTALLA.x * self.ppp).round() as i64,
                    (PANTALLA.y * self.ppp).round() as i64,
                ),
                self.n() as u32,
            )
            .x
        }

        fn fotograma(&mut self, events: Vec<Event>) -> (Pintado, ToolboxOutput) {
            self.tiempo += 0.1;
            let entrada = RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, PANTALLA)),
                events,
                time: Some(self.tiempo),
                ..RawInput::default()
            };
            let mut salida = None;
            let mut fuera = self.ctx.run_ui(entrada, |ui| {
                let vista = ToolboxView {
                    state: &self.estado,
                    zoom_label: "×5",
                    options: &self.opciones,
                    icons: &self.iconos.set,
                };
                salida = Some(show_toolbox(ui, &vista, &self.keymap, LINUX));
            });
            fuera.textures_delta.clear();
            let formas = std::mem::take(&mut fuera.shapes);
            (pintado(&formas), salida.expect("la caja se pinta"))
        }

        fn reposo(&mut self) -> Pintado {
            self.fotograma(vec![]);
            self.fotograma(vec![]).0
        }

        /// Mueve, pulsa y suelta `boton` en `(x, y)`; devuelve la salida de cada uno de los tres
        /// fotogramas.
        fn clic(&mut self, boton: PointerButton, x: i64, y: i64) -> Vec<ToolboxOutput> {
            self.reposo();
            let pos = en_puntos(self.ppp, x, y);
            let mut salidas = Vec::new();
            for events in [
                vec![mover(pos)],
                vec![boton_de(pos, boton, true)],
                vec![boton_de(pos, boton, false)],
            ] {
                salidas.push(self.fotograma(events).1);
            }
            self.fotograma(vec![mover(en_puntos(self.ppp, 3000, 3000))]);
            salidas
        }

        fn centro_de_herramienta(&self, i: i64) -> (i64, i64) {
            let n = self.n();
            let (cx, cy) = centro_u(i as usize);
            (self.x0() + cx * n, cy * n)
        }

        fn centro_de_zoom(&self, i: i64) -> (i64, i64) {
            let n = self.n();
            let (cx, cy) = centro_u(5 + i as usize);
            (self.x0() + cx * n, cy * n)
        }

        /// Rectángulo del recuadro de opciones en píxeles físicos.
        fn recuadro(&self) -> ScreenRect {
            let n = self.n();
            ScreenRect {
                x: self.x0() + OPTIONS_X_U * n,
                y: OPTIONS_Y_U * n,
                width: OPTIONS_W_U * n,
                height: OPTIONS_H_U * n,
            }
        }
    }

    fn cerca_rect(real: Rect, ppp: f32, esperado: ScreenRect) -> bool {
        let tol = tolerancia_de_egui(ppp);
        let r = [
            real.min.x * ppp,
            real.min.y * ppp,
            real.max.x * ppp,
            real.max.y * ppp,
        ];
        let e = [
            esperado.x,
            esperado.y,
            esperado.x + esperado.width,
            esperado.y + esperado.height,
        ];
        r.iter().zip(e).all(|(a, b)| (a - b as f32).abs() <= tol)
    }

    fn hay_relleno(p: &Pintado, ppp: f32, color: Color32, esperado: ScreenRect) -> bool {
        p.rellenos
            .iter()
            .any(|r| r.fill == color && cerca_rect(r.rect, ppp, esperado))
    }

    /// Borde de `grosor` px por dentro de `caja`: las cuatro franjas de un anillo sobre el interior.
    fn borde(caja: ScreenRect, grosor: i64) -> [ScreenRect; 4] {
        ring_rects(
            ScreenRect {
                x: caja.x + grosor,
                y: caja.y + grosor,
                width: caja.width - 2 * grosor,
                height: caja.height - 2 * grosor,
            },
            grosor,
        )
    }

    /// El texto pintado cuya esquina cae en `(x_px, y_px)`.
    fn texto_en(p: &Pintado, ppp: f32, x_px: i64, y_px: i64) -> Option<&Texto> {
        let tol = tolerancia_de_egui(ppp);
        p.textos.iter().find(|t| {
            (t.pos.x * ppp - x_px as f32).abs() <= tol && (t.pos.y * ppp - y_px as f32).abs() <= tol
        })
    }

    fn acciones(salidas: &[ToolboxOutput]) -> Vec<AppAction> {
        salidas.iter().filter_map(|s| s.action).collect()
    }

    fn foco(salidas: &[ToolboxOutput]) -> bool {
        salidas.iter().any(|s| s.focus_options)
    }

    fn por_rotulo(rotulo: &str) -> &'static px_ui::toolbar::Button {
        TOOL_BUTTONS
            .iter()
            .chain(ZOOM_BUTTONS.iter())
            .find(|b| b.label == rotulo)
            .expect("el botón existe")
    }

    // -----------------------------------------------------------------------------------------
    // Fila 3: el recuadro se pinta donde dice la spec
    // -----------------------------------------------------------------------------------------

    #[test]
    fn fila_3_el_recuadro_se_pinta_con_relleno_y_borde_de_1_u_a_las_cinco_escalas() {
        for ppp in PPPS {
            let mut caja = Caja::nueva(ppp, estado(Tool::Pencil, x5(), Locks::default()));
            let p = caja.reposo();
            let n = caja.n();
            let recuadro = caja.recuadro();
            assert!(
                hay_relleno(&p, ppp, SURFACE_0, recuadro),
                "ppp {ppp}: falta el relleno del recuadro {recuadro:?}"
            );
            for franja in borde(recuadro, OPTIONS_BORDER_U * n) {
                assert!(
                    hay_relleno(&p, ppp, SURFACE_3, franja),
                    "ppp {ppp}: falta una franja del borde de 1 u: {franja:?}"
                );
            }
            // Sin el foco no hay borde de 2 u en `ACCENT`.
            for franja in borde(recuadro, OPTIONS_FOCUS_BORDER_U * n) {
                assert!(
                    !hay_relleno(&p, ppp, ACCENT, franja),
                    "ppp {ppp}: el borde de foco no debería pintarse"
                );
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Fila 4: qué trae el panel hoy y cómo se prepara
    // -----------------------------------------------------------------------------------------

    #[test]
    fn fila_4_cada_herramienta_pinta_su_nombre_su_atajo_y_sin_opciones_en_un_recuadro_fijo() {
        let esperado = [
            (Tool::Pencil, "Lápiz", "tecla d"),
            (Tool::Line, "Línea", "tecla v"),
            (Tool::Fill, "Relleno", "tecla f"),
            (Tool::Dropper, "Cuentagotas", "tecla ,"),
            (Tool::Eraser, "Borrador", "tecla k"),
        ];
        for ppp in PPPS {
            for (tool, nombre, atajo) in esperado {
                let mut caja = Caja::nueva(ppp, estado(tool, x5(), Locks::default()));
                let p = caja.reposo();
                let n = caja.n();
                let x = caja.x0() + OPTIONS_TITLE_X_U * n;
                let titulo = texto_en(&p, ppp, x, OPTIONS_TITLE_Y_U * n)
                    .unwrap_or_else(|| panic!("ppp {ppp} {tool:?}: falta el título"));
                assert_eq!(titulo.texto, nombre, "ppp {ppp}");
                let linea = texto_en(&p, ppp, x, OPTIONS_KEY_Y_U * n)
                    .unwrap_or_else(|| panic!("ppp {ppp} {tool:?}: falta la línea del atajo"));
                assert_eq!(linea.texto, atajo, "ppp {ppp}");
                assert_eq!(linea.color(), TEXT_DIM, "ppp {ppp}");
                let cuerpo = texto_en(
                    &p,
                    ppp,
                    caja.x0() + OPTIONS_BODY_X_U * n,
                    OPTIONS_BODY_Y_U * n,
                )
                .unwrap_or_else(|| panic!("ppp {ppp} {tool:?}: falta `sin opciones`"));
                assert_eq!(cuerpo.texto, NO_OPTIONS, "ppp {ppp}");
                assert_eq!(NO_OPTIONS, "sin opciones");
                assert_eq!(cuerpo.color(), TEXT_DIM, "ppp {ppp}");
                // El recuadro es el mismo con cualquier herramienta.
                assert!(hay_relleno(&p, ppp, SURFACE_0, caja.recuadro()));
            }
        }
    }

    #[test]
    fn fila_4_la_linea_del_atajo_lee_la_tabla_y_sin_fila_dice_sin_tecla() {
        for tool in HERRAMIENTAS {
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
        let filas = default_keymap()
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
        let otra = Keymap::new(filas);
        assert_eq!(key_line(Tool::Pencil, &otra, LINUX), "tecla x");
        // Sin fila en la tabla, la herramienta sigue diciendo `sin tecla`.
        let vacia = Keymap::new(vec![]);
        assert_eq!(key_line(Tool::Eraser, &vacia, LINUX), "sin tecla");
        let mut caja = Caja::nueva(1.0, estado(Tool::Pencil, x5(), Locks::default()));
        caja.keymap = otra;
        let p = caja.reposo();
        assert!(p.pinta("tecla x"));
    }

    #[test]
    fn fila_4_las_filas_del_cuerpo_y_su_desplazamiento_estan_preparados_pero_vacios() {
        for tool in HERRAMIENTAS {
            assert_eq!(options_row_count(tool), 0, "{tool:?}");
        }
        assert_eq!(OPTIONS_BODY_ROWS, 7);
        for (filas, maximo) in [(0, 0), (7, 0), (8, 1), (30, 23)] {
            assert_eq!(options_max_scroll(filas), maximo, "{filas} filas");
        }
        let mut estado = OptionsState::default();
        assert_eq!((estado.focused, estado.scroll_rows), (false, 0));
        estado.scroll_by(3, 0);
        assert_eq!(estado.scroll_rows, 0, "sin filas no hay nada que desplazar");
        estado.scroll_by(5, 30);
        assert_eq!(estado.scroll_rows, 5);
        estado.scroll_by(100, 30);
        assert_eq!(estado.scroll_rows, 23, "se recorta a `filas − 7`");
        estado.scroll_by(-100, 30);
        assert_eq!(estado.scroll_rows, 0);
        // Cambiar de herramienta lo devuelve a 0.
        estado.scroll_by(4, 30);
        estado.sync_tool(Tool::Line);
        assert_eq!(estado.scroll_rows, 0);
    }

    #[test]
    fn fila_4_la_rueda_sobre_el_cuerpo_pide_desplazar_filas_y_en_otro_sitio_no() {
        for ppp in PPPS {
            let mut caja = Caja::nueva(ppp, estado(Tool::Pencil, x5(), Locks::default()));
            let n = caja.n();
            let cuerpo = (
                caja.x0() + (OPTIONS_BODY_X_U + 10) * n,
                (OPTIONS_BODY_Y_U + 10) * n,
            );
            caja.reposo();
            caja.fotograma(vec![mover(en_puntos(ppp, cuerpo.0, cuerpo.1))]);
            let (_, salida) = caja.fotograma(vec![rueda_linea(false)]);
            assert_eq!(
                salida.options_scroll_rows, 3,
                "ppp {ppp}: una muesca = 3 filas"
            );
            assert_eq!(salida.action, None);
            let (x, y) = caja.centro_de_herramienta(0);
            caja.fotograma(vec![mover(en_puntos(ppp, x, y))]);
            let (_, salida) = caja.fotograma(vec![rueda_linea(false)]);
            assert_eq!(salida.options_scroll_rows, 0, "ppp {ppp}: fuera del cuerpo");
        }
    }

    // -----------------------------------------------------------------------------------------
    // Fila 5: clic derecho en una herramienta
    // -----------------------------------------------------------------------------------------

    #[test]
    fn fila_5_el_clic_derecho_elige_la_herramienta_y_da_el_foco_a_las_cinco_escalas() {
        for ppp in PPPS {
            for (i, button) in TOOL_BUTTONS.iter().enumerate() {
                // Con cualquier herramienta activa, también con la propia.
                for activa in [Tool::Pencil, Tool::Eraser] {
                    let mut caja = Caja::nueva(ppp, estado(activa, x5(), Locks::default()));
                    let (x, y) = caja.centro_de_herramienta(i as i64);
                    let salidas = caja.clic(PointerButton::Secondary, x, y);
                    assert_eq!(
                        acciones(&salidas),
                        vec![button.action],
                        "ppp {ppp}: {} con {activa:?}",
                        button.label
                    );
                    assert!(foco(&salidas), "ppp {ppp}: {} da el foco", button.label);
                    assert!(
                        salidas.iter().any(|s| s.pressed_outside_options),
                        "ppp {ppp}: el clic cae fuera del recuadro"
                    );
                }
            }
        }
    }

    #[test]
    fn fila_5_bloqueado_no_hay_ni_herramienta_ni_foco_y_la_quita_no_bloquea() {
        type Poner = fn(&mut Locks);
        let bloqueos: [(&str, Poner); 4] = [
            ("gesto", |l| l.gesture = true),
            ("diálogo", |l| l.dialog = true),
            ("modal", |l| l.modal = true),
            ("campo", |l| l.text_focus = true),
        ];
        for ppp in PPPS {
            for (nombre, poner) in bloqueos {
                let mut locks = Locks::default();
                poner(&mut locks);
                let mut caja = Caja::nueva(ppp, estado(Tool::Pencil, x5(), locks));
                for i in 0..5 {
                    let (x, y) = caja.centro_de_herramienta(i);
                    let salidas = caja.clic(PointerButton::Secondary, x, y);
                    assert!(acciones(&salidas).is_empty(), "ppp {ppp}: {nombre}");
                    assert!(!foco(&salidas), "ppp {ppp}: {nombre}");
                }
            }
            let con_quita = Locks {
                removal: true,
                ..Locks::default()
            };
            let mut caja = Caja::nueva(ppp, estado(Tool::Pencil, x5(), con_quita));
            let (x, y) = caja.centro_de_herramienta(1);
            let salidas = caja.clic(PointerButton::Secondary, x, y);
            assert_eq!(acciones(&salidas), vec![AppAction::SelectTool(Tool::Line)]);
            assert!(foco(&salidas));
        }
    }

    #[test]
    fn fila_5_el_clic_izquierdo_no_da_foco_y_el_central_no_hace_nada() {
        for ppp in PPPS {
            let mut caja = Caja::nueva(ppp, estado(Tool::Pencil, x5(), Locks::default()));
            let (x, y) = caja.centro_de_herramienta(2);
            let izquierdo = caja.clic(PointerButton::Primary, x, y);
            assert_eq!(
                acciones(&izquierdo),
                vec![AppAction::SelectTool(Tool::Fill)]
            );
            assert!(!foco(&izquierdo), "ppp {ppp}");
            let central = caja.clic(PointerButton::Middle, x, y);
            assert!(
                acciones(&central).is_empty() && !foco(&central),
                "ppp {ppp}"
            );
        }
    }

    #[test]
    fn fila_5_una_pulsacion_dentro_del_recuadro_no_es_fuera_y_en_cualquier_otro_sitio_si() {
        for ppp in PPPS {
            let mut caja = Caja::nueva(ppp, estado(Tool::Pencil, x5(), Locks::default()));
            let n = caja.n();
            let dentro = (
                caja.x0() + (OPTIONS_BODY_X_U + 20) * n,
                (OPTIONS_BODY_Y_U + 20) * n,
            );
            for boton in [PointerButton::Primary, PointerButton::Secondary] {
                let s = caja.clic(boton, dentro.0, dentro.1);
                assert!(
                    s.iter().all(|s| !s.pressed_outside_options),
                    "ppp {ppp}: dentro del recuadro"
                );
                // Fuera de la caja (hacia el lienzo) y sobre un botón de la propia caja.
                let s = caja.clic(boton, caja.x0() - 40 * n, 50 * n);
                assert!(
                    s.iter().any(|s| s.pressed_outside_options),
                    "ppp {ppp}: fuera"
                );
                let (x, y) = caja.centro_de_zoom(3);
                let s = caja.clic(boton, x, y);
                assert!(
                    s.iter().any(|s| s.pressed_outside_options),
                    "ppp {ppp}: un botón no es el recuadro"
                );
            }
        }
    }

    #[test]
    fn fila_5_el_foco_se_pinta_con_un_borde_de_2_u_accent_con_contraste_3_a_1() {
        // Sobre el fondo de la caja.
        assert!(contrast_ratio(ACCENT, SURFACE_1) >= 3.0);
        for ppp in PPPS {
            let mut caja = Caja::nueva(ppp, estado(Tool::Line, x5(), Locks::default()));
            caja.opciones.focus();
            let p = caja.reposo();
            let n = caja.n();
            for franja in borde(caja.recuadro(), OPTIONS_FOCUS_BORDER_U * n) {
                assert!(
                    hay_relleno(&p, ppp, ACCENT, franja),
                    "ppp {ppp}: falta una franja del foco: {franja:?}"
                );
            }
            // El borde de 1 u desaparece: no depende solo del color, cambia el grosor.
            for franja in borde(caja.recuadro(), OPTIONS_BORDER_U * n) {
                assert!(!hay_relleno(&p, ppp, SURFACE_3, franja), "ppp {ppp}");
            }
        }
    }

    #[test]
    fn fila_5_el_foco_se_pierde_con_una_pulsacion_fuera_otra_herramienta_o_un_dialogo() {
        let mut e = OptionsState::default();
        e.focus();
        assert!(e.focused);
        e.sync_tool(Tool::Pencil);
        assert!(e.focused, "la misma herramienta no lo quita");
        e.sync_tool(Tool::Line);
        assert!(!e.focused, "otra herramienta por otra vía lo quita");
        e.focus();
        e.press_outside();
        assert!(!e.focused);
        e.focus();
        e.blur();
        assert!(!e.focused, "abrir un diálogo lo quita");
        // El orden del fotograma: pulsación fuera y luego el foco del clic derecho que la causó.
        e.press_outside();
        e.focus();
        assert!(e.focused);
    }

    // -----------------------------------------------------------------------------------------
    // Fila 6: clic derecho en el zoom (la tabla `Secondary` en los dos sentidos)
    // -----------------------------------------------------------------------------------------

    fn zoom_nivel(n: u32) -> Option<Zoom> {
        Zoom::from_int(n)
    }

    #[test]
    fn fila_6_acercar_aleja_alejar_acerca_y_ajustar_y_uno_a_uno_no_hacen_nada() {
        let estado = estado(Tool::Pencil, x5(), Locks::default());
        assert_eq!(
            secondary_outcome(por_rotulo("Acercar"), &estado),
            SecondaryOutcome::Action(AppAction::ZoomOut)
        );
        assert_eq!(
            secondary_outcome(por_rotulo("Alejar"), &estado),
            SecondaryOutcome::Action(AppAction::ZoomIn)
        );
        assert_eq!(
            secondary_outcome(por_rotulo("Ajustar"), &estado),
            SecondaryOutcome::Nothing
        );
        assert_eq!(
            secondary_outcome(por_rotulo("1:1"), &estado),
            SecondaryOutcome::Nothing
        );
        for button in &TOOL_BUTTONS {
            assert_eq!(
                secondary_outcome(button, &estado),
                SecondaryOutcome::SelectAndFocus(button.action)
            );
        }
    }

    #[test]
    fn fila_6_se_habilita_por_la_accion_efectiva_en_los_extremos_de_la_escalera() {
        let en_max = estado(Tool::Pencil, Some(Zoom::MAX), Locks::default());
        let en_min = estado(Tool::Pencil, Some(Zoom::MIN), Locks::default());
        // En ×64 el botón `Acercar` está atenuado para el izquierdo y el derecho aleja.
        assert!(!px_ui::actions::enabled(AppAction::ZoomIn, &en_max));
        assert_eq!(
            secondary_outcome(por_rotulo("Acercar"), &en_max),
            SecondaryOutcome::Action(AppAction::ZoomOut)
        );
        assert_eq!(
            secondary_outcome(por_rotulo("Alejar"), &en_max),
            SecondaryOutcome::Nothing
        );
        // En 1/64, al revés.
        assert_eq!(
            secondary_outcome(por_rotulo("Alejar"), &en_min),
            SecondaryOutcome::Action(AppAction::ZoomIn)
        );
        assert_eq!(
            secondary_outcome(por_rotulo("Acercar"), &en_min),
            SecondaryOutcome::Nothing
        );
        // Sin imagen, nada.
        let sin_imagen = estado(Tool::Pencil, None, Locks::default());
        for rotulo in ["Acercar", "Alejar", "Ajustar", "1:1"] {
            assert_eq!(
                secondary_outcome(por_rotulo(rotulo), &sin_imagen),
                SecondaryOutcome::Nothing,
                "{rotulo}"
            );
        }
    }

    #[test]
    fn fila_6_los_bloqueos_del_zoom_son_los_de_la_spec_17_con_el_origen_boton() {
        let con = |poner: fn(&mut Locks)| {
            let mut l = Locks::default();
            poner(&mut l);
            estado(Tool::Pencil, zoom_nivel(5), l)
        };
        for (nombre, e, pasa) in [
            ("gesto", con(|l| l.gesture = true), true),
            ("quita", con(|l| l.removal = true), true),
            ("campo", con(|l| l.text_focus = true), true),
            ("diálogo", con(|l| l.dialog = true), false),
            ("modal", con(|l| l.modal = true), false),
        ] {
            for (rotulo, efectiva) in [
                ("Acercar", AppAction::ZoomOut),
                ("Alejar", AppAction::ZoomIn),
            ] {
                let esperado = if pasa {
                    SecondaryOutcome::Action(efectiva)
                } else {
                    SecondaryOutcome::Nothing
                };
                assert_eq!(
                    secondary_outcome(por_rotulo(rotulo), &e),
                    esperado,
                    "{rotulo} con {nombre}"
                );
            }
        }
    }

    #[test]
    fn fila_6_el_clic_derecho_en_los_botones_de_zoom_da_la_accion_contraria_a_las_cinco_escalas() {
        for ppp in PPPS {
            for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
                let esperado: Vec<AppAction> = match button.label {
                    "Acercar" => vec![AppAction::ZoomOut],
                    "Alejar" => vec![AppAction::ZoomIn],
                    _ => vec![],
                };
                let mut caja = Caja::nueva(ppp, estado(Tool::Pencil, x5(), Locks::default()));
                let (x, y) = caja.centro_de_zoom(i as i64);
                let salidas = caja.clic(PointerButton::Secondary, x, y);
                assert_eq!(acciones(&salidas), esperado, "ppp {ppp}: {}", button.label);
                assert!(!foco(&salidas), "ppp {ppp}: el zoom no da foco");
                // En ×64 el derecho de `Acercar` aleja aunque el izquierdo esté atenuado.
                if button.label == "Acercar" {
                    let mut caja =
                        Caja::nueva(ppp, estado(Tool::Pencil, Some(Zoom::MAX), Locks::default()));
                    let derecho = caja.clic(PointerButton::Secondary, x, y);
                    assert_eq!(acciones(&derecho), vec![AppAction::ZoomOut], "ppp {ppp}");
                    let izquierdo = caja.clic(PointerButton::Primary, x, y);
                    assert!(acciones(&izquierdo).is_empty(), "ppp {ppp}");
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Fila 7: tooltips
    // -----------------------------------------------------------------------------------------

    fn esperados_tooltips(command: &str) -> Vec<(&'static str, String)> {
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
    fn fila_7_el_texto_del_tooltip_lee_la_tabla_en_las_tres_plataformas() {
        for (platform, command) in [
            (Platform::Linux, "Ctrl"),
            (Platform::Windows, "Ctrl"),
            (Platform::Mac, "Cmd"),
        ] {
            for (rotulo, texto) in esperados_tooltips(command) {
                assert_eq!(
                    tooltip_text(por_rotulo(rotulo), default_keymap(), platform),
                    texto,
                    "{platform:?}: {rotulo}"
                );
            }
        }
        // Con otra tabla cambia (la `d` pasa a `x`).
        let filas = default_keymap()
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
        let otra = Keymap::new(filas);
        assert_eq!(
            tooltip_text(por_rotulo("Lápiz"), &otra, LINUX),
            "Lápiz (x)\nclic derecho: opciones"
        );
    }

    #[test]
    fn fila_7_todo_el_texto_de_los_tooltips_cabe_en_el_juego_de_caracteres_de_la_interfaz() {
        for platform in [Platform::Linux, Platform::Windows, Platform::Mac] {
            for (rotulo, _) in esperados_tooltips("Ctrl") {
                let texto = tooltip_text(por_rotulo(rotulo), default_keymap(), platform);
                for c in texto.chars().filter(|c| *c != '\n') {
                    assert!(INTERFACE_CHARSET.contains(c), "{rotulo}: {c:?} no está");
                }
            }
        }
    }

    #[test]
    fn fila_7_el_hover_enseña_el_tooltip_con_la_fuente_de_pixeles_y_sin_hover_no_hay_ninguno() {
        for ppp in PPPS {
            let n = n_de(ppp);
            for (i, (rotulo, texto)) in esperados_tooltips("Ctrl").iter().enumerate() {
                let mut caja = Caja::nueva(ppp, estado(Tool::Pencil, x5(), Locks::default()));
                // El primer renglón del tooltip es el que lleva el atajo; el segundo, el clic derecho.
                let primero = texto.lines().next().expect("hay una línea").to_string();
                let sin_hover = caja.reposo();
                let cuenta =
                    |p: &Pintado, t: &str| p.textos.iter().filter(|x| x.texto == t).count();
                let antes = cuenta(&sin_hover, &primero);
                let (x, y) = if i < 5 {
                    caja.centro_de_herramienta(i as i64)
                } else {
                    caja.centro_de_zoom(i as i64 - 5)
                };
                let pos = en_puntos(ppp, x, y);
                let mut ultimo = caja.fotograma(vec![mover(pos)]).0;
                for _ in 0..8 {
                    ultimo = caja.fotograma(vec![]).0;
                }
                assert_eq!(
                    cuenta(&ultimo, &primero),
                    antes + 1,
                    "ppp {ppp}: {rotulo}: falta el tooltip {primero:?}; hay {:?}",
                    ultimo.todos_los_textos()
                );
                let renglon = ultimo
                    .textos
                    .iter()
                    .rfind(|t| t.texto == primero)
                    .expect("el tooltip");
                assert!(
                    (renglon.tamano.y * ppp - (14 * n) as f32).abs() <= 0.05,
                    "ppp {ppp}: {rotulo}: el tooltip no usa la fuente de píxeles ({} px de alto)",
                    renglon.tamano.y * ppp
                );
                if let Some(segundo) = texto.lines().nth(1) {
                    assert!(
                        ultimo.pinta(segundo),
                        "ppp {ppp}: {rotulo}: falta {segundo:?}"
                    );
                }
                // Sin el puntero encima, desaparece.
                caja.fotograma(vec![mover(en_puntos(ppp, 3000, 3000))]);
                let lejos = caja.fotograma(vec![]).0;
                assert_eq!(cuenta(&lejos, &primero), antes, "ppp {ppp}: {rotulo}");
            }
        }
    }

    #[test]
    fn fila_7_un_boton_atenuado_tambien_enseña_su_tooltip() {
        let ppp = 1.0;
        let mut caja = Caja::nueva(ppp, estado(Tool::Pencil, Some(Zoom::MAX), Locks::default()));
        let (x, y) = caja.centro_de_zoom(0); // Acercar, atenuado en ×64.
        caja.fotograma(vec![mover(en_puntos(ppp, x, y))]);
        let mut p = caja.fotograma(vec![]).0;
        for _ in 0..8 {
            p = caja.fotograma(vec![]).0;
        }
        assert!(p.pinta("Acercar (>)"), "hay {:?}", p.todos_los_textos());
    }

    // -----------------------------------------------------------------------------------------
    // Propiedad 3: el clic secundario sigue la tabla
    // -----------------------------------------------------------------------------------------

    #[derive(Clone, Copy, Debug)]
    enum Ev {
        ClicDerechoEnHerramienta { permitido: bool },
        PulsacionFuera,
        CambioDeHerramienta,
        Dialogo,
        PulsacionDentro,
    }

    fn evento() -> impl Strategy<Value = Ev> {
        prop_oneof![
            any::<bool>().prop_map(|permitido| Ev::ClicDerechoEnHerramienta { permitido }),
            Just(Ev::PulsacionFuera),
            Just(Ev::CambioDeHerramienta),
            Just(Ev::Dialogo),
            Just(Ev::PulsacionDentro),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn propiedad_3_el_clic_derecho_sigue_la_tabla_secondary_y_el_bloqueo_del_origen_boton(
            tool in 0usize..5,
            nivel in prop::option::of(0usize..1000),
            gesture in any::<bool>(),
            dialog in any::<bool>(),
            text_focus in any::<bool>(),
            removal in any::<bool>(),
            modal in any::<bool>(),
        ) {
            let niveles: Vec<Zoom> = Zoom::levels().collect();
            let zoom = nivel.map(|i| niveles[i % niveles.len()]);
            let locks = Locks { gesture, dialog, text_focus, removal, modal, ..Locks::default() };
            let e = estado(HERRAMIENTAS[tool], zoom, locks);
            for button in TOOL_BUTTONS.iter().chain(ZOOM_BUTTONS.iter()) {
                let efectiva = match button.label {
                    "Acercar" => Some(AppAction::ZoomOut),
                    "Alejar" => Some(AppAction::ZoomIn),
                    "Ajustar" | "1:1" => None,
                    _ => Some(button.action),
                };
                let esperado = match efectiva {
                    None => SecondaryOutcome::Nothing,
                    Some(accion) => {
                        let hay_escalon = match accion {
                            AppAction::ZoomOut => zoom.is_some_and(|z| z > Zoom::MIN),
                            AppAction::ZoomIn => zoom.is_some_and(|z| z < Zoom::MAX),
                            _ => true,
                        };
                        if is_blocked(accion, locks, Origin::Button) || !hay_escalon {
                            SecondaryOutcome::Nothing
                        } else if matches!(accion, AppAction::SelectTool(_)) {
                            SecondaryOutcome::SelectAndFocus(accion)
                        } else {
                            SecondaryOutcome::Action(accion)
                        }
                    }
                };
                prop_assert_eq!(secondary_outcome(button, &e), esperado, "{}", button.label);
            }
        }

        #[test]
        fn propiedad_3_el_foco_solo_dura_tras_un_clic_derecho_permitido(
            eventos in prop::collection::vec(evento(), 0..24),
        ) {
            let mut e = OptionsState::default();
            let mut herramienta = Tool::Pencil;
            let mut antes;
            for ev in eventos {
                antes = e.focused;
                match ev {
                    Ev::ClicDerechoEnHerramienta { permitido } => {
                        // Orden del fotograma: primero la pulsación fuera, luego el foco.
                        e.press_outside();
                        if permitido {
                            e.focus();
                        }
                        prop_assert_eq!(e.focused, permitido);
                    }
                    Ev::PulsacionFuera => {
                        e.press_outside();
                        prop_assert!(!e.focused);
                    }
                    Ev::CambioDeHerramienta => {
                        herramienta = if herramienta == Tool::Line { Tool::Fill } else { Tool::Line };
                        e.sync_tool(herramienta);
                        prop_assert!(!e.focused);
                    }
                    Ev::Dialogo => {
                        e.blur();
                        prop_assert!(!e.focused);
                    }
                    Ev::PulsacionDentro => prop_assert_eq!(e.focused, antes),
                }
                prop_assert!(e.scroll_rows <= options_max_scroll(options_row_count(herramienta)));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad I.4 (specs/interfaz-iconos.md): filas 6 y 8 de la caja, con iconos
// ---------------------------------------------------------------------------------------------

mod iconos {
    use super::*;
    use egui::output::OutputEvent;
    use egui::{Pos2, RawInput, Rect, Role};
    use px_ui::icon_button::{Look, look};
    use px_ui::theme::SURFACE_3;

    const PPPS_FINOS: [f32; 9] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0];

    fn icono_de(i: usize) -> px_ui::icons::Icon {
        TOOL_BUTTONS
            .iter()
            .chain(ZOOM_BUTTONS.iter())
            .nth(i)
            .expect("botón")
            .icon
    }

    fn hay(p: &Pintado, ppp: f32, color: egui::Color32, esperado: [i64; 4]) -> bool {
        p.rellenos.iter().any(|r| {
            r.fill == color
                && [r.rect.min.x, r.rect.min.y, r.rect.max.x, r.rect.max.y]
                    .iter()
                    .zip(esperado)
                    .all(|(a, b)| (a * ppp - b as f32).abs() <= tolerancia_de_egui(ppp))
        })
    }

    /// El botón `i` se pinta con la cara, el relieve y el icono del aspecto `l`.
    fn comprueba(p: &Pintado, banco: &Banco, i: usize, l: &Look, contexto: &str) {
        let (ppp, n) = (banco.ppp, banco.n());
        let (ex, ey) = Banco::esquina_u(i);
        let (x, y) = (banco.x0() + ex * n, ey * n);
        let lado = BOTON_U * n;
        assert!(
            hay(p, ppp, l.face, [x, y, x + lado, y + lado]),
            "{contexto}: falta la cara"
        );
        for (franja, color) in [
            ([x, y, x + lado, y + n], l.top_left),
            ([x, y, x + n, y + lado], l.top_left),
            ([x, y + lado - n, x + lado, y + lado], l.bottom_right),
            ([x + lado - n, y, x + lado, y + lado], l.bottom_right),
        ] {
            assert!(
                hay(p, ppp, color, franja),
                "{contexto}: falta la franja {franja:?}"
            );
        }
        let id = banco.iconos.set.get(icono_de(i)).expect("textura");
        let malla = p
            .malla_de(id)
            .unwrap_or_else(|| panic!("{contexto}: no se pinta el icono {i}"));
        let d = (4 + l.offset_u) * n;
        let esperado = [x + d, y + d, x + d + 16 * n, y + d + 16 * n];
        let real = [
            malla.rect.min.x,
            malla.rect.min.y,
            malla.rect.max.x,
            malla.rect.max.y,
        ];
        for (a, b) in real.iter().zip(esperado) {
            assert!(
                (a * ppp - b as f32).abs() <= tolerancia_de_egui(ppp),
                "{contexto}: icono {i} en {real:?} (pt) y no en {esperado:?} (px)"
            );
            // Esquinas en píxeles físicos enteros.
            assert!(
                (a * ppp - (a * ppp).round()).abs() <= tolerancia_de_egui(ppp),
                "{contexto}"
            );
        }
        assert_eq!(malla.color, l.tint, "{contexto}: tinta");
        assert!(
            malla.uv.min.distance(egui::pos2(0.0, 0.0)) < 1e-4
                && malla.uv.max.distance(egui::pos2(1.0, 1.0)) < 1e-4,
            "{contexto}: UV completo"
        );
    }

    #[test]
    fn fila_6_normal_hundido_y_desactivado_se_pintan_con_su_aspecto_a_ppp_de_1_a_3() {
        for ppp in PPPS_FINOS {
            // Normal (Borrador, Acercar) y hundido (el Lápiz activo).
            let banco = Banco::nuevo(ppp, estado(Tool::Pencil, x5(), Locks::default()));
            let p = banco.reposo();
            let contexto = format!("ppp {ppp}");
            comprueba(
                &p,
                &banco,
                4,
                &look(false, true, false),
                &format!("{contexto} normal"),
            );
            comprueba(
                &p,
                &banco,
                5,
                &look(false, true, false),
                &format!("{contexto} zoom"),
            );
            comprueba(
                &p,
                &banco,
                0,
                &look(true, true, false),
                &format!("{contexto} hundido"),
            );
            // Con un gesto: las herramientas se atenúan (el activo, hundido y atenuado) y el zoom no.
            let banco = Banco::nuevo(
                ppp,
                estado(Tool::Pencil, x5(), bloqueo(|l| l.gesture = true)),
            );
            let p = banco.reposo();
            comprueba(
                &p,
                &banco,
                1,
                &look(false, false, false),
                &format!("{contexto} atenuado"),
            );
            comprueba(
                &p,
                &banco,
                0,
                &look(true, false, false),
                &format!("{contexto} activo atenuado"),
            );
            comprueba(
                &p,
                &banco,
                6,
                &look(false, true, false),
                &format!("{contexto} zoom libre"),
            );
            // Ningún texto de botón: lo pintado como texto no cae sobre ninguna casilla.
            let n = banco.n();
            for texto in &p.textos {
                let r = texto.rect();
                for i in 0..9 {
                    let (ex, ey) = Banco::esquina_u(i);
                    let casilla = Rect::from_min_size(
                        egui::pos2((banco.x0() + ex * n) as f32 / ppp, (ey * n) as f32 / ppp),
                        egui::vec2((BOTON_U * n) as f32 / ppp, (BOTON_U * n) as f32 / ppp),
                    );
                    assert!(
                        !casilla.shrink(0.01).intersects(r),
                        "{contexto}: el texto {:?} cae sobre el botón {i}",
                        texto.texto
                    );
                }
            }
            // Todo lo pintado por la caja cae dentro de ella.
            for malla in &p.mallas {
                assert!(
                    malla.rect.max.x * ppp
                        <= (banco.x0() + COLUMNA_U * n) as f32 + tolerancia_de_egui(ppp)
                        && malla.rect.min.x * ppp >= banco.x0() as f32 - tolerancia_de_egui(ppp),
                    "{contexto}: icono fuera de la caja: {:?}",
                    malla.rect
                );
            }
        }
    }

    #[test]
    fn fila_6_con_el_raton_encima_la_cara_se_aclara_y_un_boton_atenuado_no_se_aclara() {
        for ppp in PPPS {
            let banco = Banco::nuevo(ppp, estado(Tool::Pencil, x5(), Locks::default()));
            let n = banco.n();
            let (cx, cy) = centro_u(2);
            let pos = en_puntos(ppp, banco.x0() + cx * n, cy * n);
            banco.fotograma(vec![mover(pos)]);
            let p = banco.fotograma(vec![]).0;
            comprueba(
                &p,
                &banco,
                2,
                &look(false, true, true),
                &format!("ppp {ppp}: encima"),
            );
            assert_eq!(look(false, true, true).face, SURFACE_3);
            // Atenuado: con el ratón encima sigue con la cara normal.
            let banco = Banco::nuevo(
                ppp,
                estado(Tool::Pencil, x5(), bloqueo(|l| l.gesture = true)),
            );
            banco.fotograma(vec![mover(pos)]);
            let p = banco.fotograma(vec![]).0;
            comprueba(
                &p,
                &banco,
                2,
                &look(false, false, true),
                &format!("ppp {ppp}: atenuado"),
            );
        }
    }

    /// Los eventos de salida del fotograma (`widget_info`).
    fn eventos(banco: &Banco, events: Vec<Event>) -> Vec<OutputEvent> {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, PANTALLA)),
            events,
            ..RawInput::default()
        };
        let mut salida = banco.ctx.run_ui(entrada, |ui| {
            let vista = ToolboxView {
                state: &banco.estado,
                zoom_label: banco.etiqueta,
                options: &OptionsState::default(),
                icons: &banco.iconos.set,
            };
            show_toolbox(ui, &vista, default_keymap(), Platform::Linux);
        });
        salida.textures_delta.clear();
        salida.platform_output.events.clone()
    }

    #[test]
    fn fila_8_cada_boton_habilitado_emite_widget_info_de_tipo_boton_con_su_rotulo() {
        for ppp in [1.0, 2.0] {
            let banco = Banco::nuevo(ppp, estado(Tool::Pencil, x5(), Locks::default()));
            for (i, button) in TOOL_BUTTONS.iter().chain(ZOOM_BUTTONS.iter()).enumerate() {
                banco.reposo();
                let n = banco.n();
                let (cx, cy) = centro_u(i);
                let pos = en_puntos(ppp, banco.x0() + cx * n, cy * n);
                let mut todos = eventos(&banco, vec![mover(pos)]);
                todos.extend(eventos(&banco, vec![boton(pos, true)]));
                todos.extend(eventos(&banco, vec![boton(pos, false)]));
                assert!(
                    todos.iter().any(|e| matches!(e, OutputEvent::Clicked(info)
                        if info.role == Role::Button
                            && info.label.as_deref() == Some(button.label)
                            && info.enabled)),
                    "ppp {ppp}: {}: sin WidgetInfo de botón; hay {todos:?}",
                    button.label
                );
                banco.fotograma(vec![mover(en_puntos(ppp, 3000, 3000))]);
            }
        }
    }
}
