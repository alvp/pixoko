//! Tests de la columna de herramientas `px_ui::toolbar` (specs/hito-2-menu-herramientas.md, parte
//! de la fila 7 «Geometría a las cinco escalas»; la pila completa está en `window_stack.rs` y la
//! aritmética pura en `chrome.rs`).
//!
//! Banco de egui sin ventana (`Context::run_ui` con `RawInput` sintético): `show_toolbar` se pinta
//! sola en una ventana vacía, así que la columna empieza en `(0, 0)`. Se comprueba a las cinco
//! escalas (`ppp` 1,0, 1,25, 1,5, 2,0 y 3,0), en píxeles físicos:
//!
//! - las tablas `TOOL_BUTTONS` y `ZOOM_BUTTONS`;
//! - los 9 botones golpean en su sitio: herramientas en `y = 4 + 18·i`, zoom en `y = 119 + 18·i`,
//!   de 99 × 16 u con 4 u de margen (primera y última coordenada física de cada botón dentro); el
//!   nivel de zoom y lo que queda fuera de la columna no golpean nada;
//! - el activo se resalta con relleno `ACCENT` y texto `SURFACE_0` (contraste ≥ 4,5 con
//!   `contrast_ratio`), la letra va en `TEXT_DIM` en los demás, y los deshabilitados se atenúan y
//!   no responden (con las reglas de bloqueo del zoom: gesto, quita y campo de texto no lo apagan);
//! - el nivel de zoom es un texto de 14 u en `y = 101`.
//!
//! Las posiciones pintadas se comparan con la tolerancia del redondeo de egui
//! (`common::tolerancia_de_egui`: 1/32 pt, porque `Panel` y `allocate_rect` redondean a múltiplos pares de 1/32 pt).
//!
//! Supuestos sobre lo que la spec no fija (cualquier otro reparto cambiaría esta página):
//! - `show_toolbar` crea su propio panel a la izquierda de `ui` (como `status::show` y
//!   `palette_panel::show`), de `toolbar_width_u()·n` píxeles de ancho;
//! - el rótulo de zoom que recibe es el texto del nivel (`×12`, `1/8`, `—`) con o sin el prefijo
//!   `Zoom `: el banco solo exige que el texto pintado lo contenga.

use egui::{Context, Event, Vec2, vec2};
use px_ui::actions::{AppAction, Locks, UiState};
use px_ui::theme::{ACCENT, SURFACE_0, TEXT_DIM, contrast_ratio};
use px_ui::tool::Tool;
use px_ui::toolbar::{TOOL_BUTTONS, ZOOM_BUTTONS, show_toolbar};
use px_ui::view::Zoom;

mod common;
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
/// Ancho de la columna y de sus botones, en `u`.
const COLUMNA_U: i64 = 107;
const BOTON_W_U: i64 = 99;
const BOTON_H_U: i64 = 16;
const MARGEN_U: i64 = 4;

fn y_herramienta_u(i: i64) -> i64 {
    4 + 18 * i
}

fn y_zoom_u(i: i64) -> i64 {
    119 + 18 * i
}

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
}

impl Banco {
    fn nuevo(ppp: f32, estado: UiState) -> Banco {
        Banco {
            ctx: contexto(ppp),
            ppp,
            estado,
            etiqueta: "×5",
        }
    }

    fn n(&self) -> i64 {
        n_de(self.ppp)
    }

    /// Un fotograma: lo pintado y lo que devolvió `show_toolbar`.
    fn fotograma(&self, events: Vec<Event>) -> (Pintado, Option<AppAction>) {
        let mut accion = None;
        let formas = fotograma(&self.ctx, PANTALLA, events, |ui| {
            accion = show_toolbar(ui, &self.estado, self.etiqueta);
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

    /// Clic en el centro del botón de la fila `y_u`.
    fn clic_en_boton(&self, y_u: i64) -> Vec<AppAction> {
        let n = self.n();
        self.clic((MARGEN_U + BOTON_W_U / 2) * n, y_u * n + BOTON_H_U * n / 2)
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

#[test]
fn tool_buttons_son_los_cinco_botones_con_su_letra() {
    let tabla: Vec<(&str, AppAction, Option<char>)> = TOOL_BUTTONS
        .iter()
        .map(|b| (b.label, b.action, b.key))
        .collect();
    assert_eq!(
        tabla,
        [
            ("Lápiz", AppAction::SelectTool(Tool::Pencil), Some('P')),
            ("Línea", AppAction::SelectTool(Tool::Line), Some('L')),
            ("Relleno", AppAction::SelectTool(Tool::Fill), Some('F')),
            (
                "Cuentagotas",
                AppAction::SelectTool(Tool::Dropper),
                Some('I')
            ),
            ("Borrador", AppAction::SelectTool(Tool::Eraser), Some('E')),
        ]
    );
}

#[test]
fn zoom_buttons_son_los_cuatro_botones_sin_letra() {
    let tabla: Vec<(&str, AppAction, Option<char>)> = ZOOM_BUTTONS
        .iter()
        .map(|b| (b.label, b.action, b.key))
        .collect();
    assert_eq!(
        tabla,
        [
            ("Acercar", AppAction::ZoomIn, None),
            ("Alejar", AppAction::ZoomOut, None),
            ("Ajustar", AppAction::ZoomFit, None),
            ("1:1", AppAction::ZoomActual, None),
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
                banco.clic_en_boton(y_herramienta_u(i as i64)),
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
                banco.clic_en_boton(y_zoom_u(i as i64)),
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
        let filas: Vec<(i64, AppAction)> = TOOL_BUTTONS
            .iter()
            .enumerate()
            .map(|(i, b)| (y_herramienta_u(i as i64), b.action))
            .chain(
                ZOOM_BUTTONS
                    .iter()
                    .enumerate()
                    .map(|(i, b)| (y_zoom_u(i as i64), b.action)),
            )
            .collect();
        for (y_u, accion) in filas {
            let (x0, y0) = (MARGEN_U * n, y_u * n);
            let (x1, y1) = ((MARGEN_U + BOTON_W_U) * n - 1, (y_u + BOTON_H_U) * n - 1);
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
        let centro_x = (MARGEN_U + BOTON_W_U / 2) * n;
        // El nivel de zoom (y = 101 a 115): a 16 u del último botón de herramienta y a 11 u del
        // primero de zoom.
        assert!(
            banco.clic(centro_x, 108 * n).is_empty(),
            "ppp {ppp}: el nivel de zoom no es un botón"
        );
        // Fuera de la columna (a su derecha) no hay nada.
        for y_u in [10, 50, 130] {
            assert!(
                banco.clic(150 * n, y_u * n).is_empty(),
                "ppp {ppp}: x = 150 u está fuera de la columna (y = {y_u} u)"
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
                r.min.x * p >= (MARGEN_U * n) as f32 - tolerancia_de_egui(ppp)
                    && r.max.x * p <= ((MARGEN_U + BOTON_W_U) * n) as f32 + tolerancia_de_egui(ppp),
                "ppp {ppp}: {:?} se sale de la columna: {r:?}",
                texto.texto
            );
        }
        for relleno in &pintado.rellenos {
            let r = relleno.rect;
            assert!(
                r.max.x * p <= (COLUMNA_U * n) as f32 + tolerancia_de_egui(ppp),
                "ppp {ppp}: relleno fuera de la columna: {r:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// El activo y la letra
// ---------------------------------------------------------------------------------------------

#[test]
fn el_activo_se_resalta_con_relleno_accent_en_su_boton_y_texto_surface_0() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for (i, tool) in HERRAMIENTAS.iter().enumerate() {
            let banco = Banco::nuevo(ppp, estado(*tool, x5(), Locks::default()));
            let pintado = banco.reposo();
            let acentos: Vec<_> = pintado
                .rellenos
                .iter()
                .filter(|r| r.fill == ACCENT)
                .collect();
            assert_eq!(
                acentos.len(),
                1,
                "ppp {ppp}: solo el activo ({tool:?}) lleva relleno ACCENT"
            );
            let r = acentos[0].rect;
            let esperado = [
                MARGEN_U * n,
                y_herramienta_u(i as i64) * n,
                (MARGEN_U + BOTON_W_U) * n,
                (y_herramienta_u(i as i64) + BOTON_H_U) * n,
            ];
            let real = [r.min.x * ppp, r.min.y * ppp, r.max.x * ppp, r.max.y * ppp];
            for (a, b) in real.iter().zip(esperado) {
                assert!(
                    (a - b as f32).abs() <= tolerancia_de_egui(ppp),
                    "ppp {ppp}: {tool:?} {real:?} y no {esperado:?}"
                );
            }
            let rotulo = TOOL_BUTTONS[i].label;
            let texto = pintado
                .texto_con(rotulo)
                .unwrap_or_else(|| panic!("ppp {ppp}: no se pinta {rotulo}"));
            assert_eq!(
                texto.color(),
                SURFACE_0,
                "ppp {ppp}: el rótulo del activo ({rotulo}) va en SURFACE_0"
            );
        }
    }
}

#[test]
fn las_letras_de_los_botones_no_activos_van_en_text_dim() {
    for ppp in PPPS {
        let banco = Banco::nuevo(ppp, estado(Tool::Pencil, x5(), Locks::default()));
        let pintado = banco.reposo();
        for button in TOOL_BUTTONS.iter().skip(1) {
            let letra = button.key.expect("lleva letra").to_string();
            let tramo = pintado
                .tramo(&letra)
                .unwrap_or_else(|| panic!("ppp {ppp}: no se pinta la letra {letra}"));
            assert_eq!(tramo.color, TEXT_DIM, "ppp {ppp}: letra {letra}");
        }
    }
}

#[test]
fn se_pintan_todos_los_rotulos_de_los_nueve_botones() {
    let banco = Banco::nuevo(1.0, estado(Tool::Fill, x5(), Locks::default()));
    let pintado = banco.reposo();
    for button in TOOL_BUTTONS.iter().chain(ZOOM_BUTTONS.iter()) {
        assert!(
            pintado.texto_con(button.label).is_some(),
            "no se pinta {:?}; hay {:?}",
            button.label,
            pintado.todos_los_textos()
        );
    }
}

#[test]
fn los_botones_de_zoom_no_llevan_letra_de_atajo() {
    let banco = Banco::nuevo(1.0, estado(Tool::Pencil, x5(), Locks::default()));
    let pintado = banco.reposo();
    for button in ZOOM_BUTTONS.iter() {
        let texto = pintado.texto_con(button.label).expect("rótulo de zoom");
        assert_eq!(
            texto.texto.trim(),
            button.label,
            "solo el rótulo, sin letra"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Nivel de zoom
// ---------------------------------------------------------------------------------------------

#[test]
fn el_nivel_de_zoom_es_un_texto_de_14_u_en_y_101() {
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
            (texto.pos.y * ppp - (101 * n) as f32).abs() <= tolerancia_de_egui(ppp),
            "ppp {ppp}: el nivel empieza en y = {} px y no en {}",
            texto.pos.y * ppp,
            101 * n
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
        herramientas[i] = banco.clic_en_boton(y_herramienta_u(i as i64)) == vec![button.action];
    }
    for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
        zoom[i] = banco.clic_en_boton(y_zoom_u(i as i64)) == vec![button.action];
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
fn sin_imagen_el_zoom_no_responde_y_en_x1_solo_acercar_y_ajustar() {
    let banco = Banco::nuevo(1.0, estado(Tool::Pencil, None, Locks::default()));
    assert_eq!(responden(&banco), ([true; 5], [false; 4]));
    let banco = Banco::nuevo(1.0, estado(Tool::Pencil, Some(Zoom::MIN), Locks::default()));
    // Acercar, Alejar, Ajustar y 1:1: `Alejar` y `1:1` están deshabilitados en el mínimo.
    assert_eq!(responden(&banco), ([true; 5], [true, false, true, false]));
}

#[test]
fn en_zoom_max_acercar_no_responde() {
    let banco = Banco::nuevo(1.0, estado(Tool::Pencil, Some(Zoom::MAX), Locks::default()));
    assert_eq!(responden(&banco), ([true; 5], [false, true, true, true]));
}

#[test]
fn un_boton_deshabilitado_se_pinta_atenuado() {
    // El rótulo de `Línea` cambia de color al deshabilitarlo (no es el activo en ninguno de los dos).
    let libre = Banco::nuevo(1.0, estado(Tool::Pencil, x5(), Locks::default()));
    let con_gesto = Banco::nuevo(
        1.0,
        estado(Tool::Pencil, x5(), bloqueo(|l| l.gesture = true)),
    );
    let color_libre = libre.reposo().texto_con("Línea").expect("rótulo").aspecto();
    let color_gris = con_gesto
        .reposo()
        .texto_con("Línea")
        .expect("rótulo")
        .aspecto();
    assert_ne!(color_libre, color_gris, "el deshabilitado va atenuado");
    // El zoom sigue habilitado con el gesto: su rótulo no cambia.
    let zoom_libre = libre
        .reposo()
        .texto_con("Acercar")
        .expect("rótulo")
        .aspecto();
    let zoom_gesto = con_gesto
        .reposo()
        .texto_con("Acercar")
        .expect("rótulo")
        .aspecto();
    assert_eq!(zoom_libre, zoom_gesto);
}
