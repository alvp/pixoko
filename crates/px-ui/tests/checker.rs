//! Tests de la lógica pura del damero de transparencia y del fondo elegible (specs/hito-2-damero-
//! lienzo.md, «Casos» filas 1 a 4, 9, 10 y la parte pura de la 15, y propiedad 1): `px_ui::checker`
//! y `px_ui::background` no usan tipos de egui, así que aquí no hay ventana ni banco. Las filas de
//! lienzo (5 a 8, 14 y la parte de lienzo de la 15) están en `checker_canvas.rs`; el menú y los
//! bloqueos (12 y 13), en `background_menu.rs`.
//!
//! - Fila 1: `CELL_U`, `cell_px`, `Shade::color`, `texels` y `BACKGROUND`.
//! - Fila 2: `shade_at` con la tabla de la spec, con negativos, con origen desplazado y con i64
//!   enormes contra la definición por `rem_euclid` con período `2·cell`.
//! - Fila 3: `quad` (a, b, c, d) y `clip` (e).
//! - Fila 4: `cells` (a a e).
//! - Fila 9: `CanvasBackground`, `Fill`, `BLACK`, `WHITE` y `background_fill`.
//! - Fila 10: coherencia con `swatch_parts` y con las constantes `CHECK_A` y `CHECK_B`.
//! - Fila 15 (parte pura): `ENV_MESH` y `mesh_fallback_enabled`.
//! - Propiedad 1: `quad`, `shade_at` y `cells` son la misma función (512 casos).
//!
//! Suposiciones sobre las firmas de «Piezas»: `Cell` y `CheckerQuad` comparan con `==` (derives
//! `Debug` y `PartialEq`), `Shade`, `CanvasBackground` y `Fill` son `Clone, Copy, Debug, PartialEq,
//! Eq` y `Rgba` es `Debug` y `PartialEq`. Los uv de `CheckerQuad` se comparan **exactos**: todos los
//! valores de la spec son múltiplos de 1/16 y se representan sin error en `f32`.
//!
//! Unidad 19b (specs/hito-2-fondo-color.md): la fila 1 (`CanvasBackground::Custom`, `custom`,
//! `solid_color`, `background_value_text` y `background_fill` de un color propio) y la parte pura de
//! la propiedad 1 (`custom(c)` es `Some` si y solo si el alfa es 255, `background_value_text` se lee
//! de vuelta con `parse_hex` y `background_fill` nunca lleva alfa < 255) están al final del fichero.
//!
//! Los oráculos (paridad de las casillas, fase, número de casillas por eje) están escritos aquí con
//! `rem_euclid` y `div_euclid`, sin llamar a las funciones nuevas, salvo donde la propiedad 1 las
//! compara entre sí.

use proptest::prelude::*;
use px_core::{Mode, Rgba};
use px_ui::background::{
    BLACK, CanvasBackground, Fill, WHITE, background_fill, background_value_text,
};
use px_ui::canvas::BACKGROUND;
use px_ui::checker::{
    CELL_U, Cell, CheckerQuad, ENV_MESH, Shade, cell_px, cells, clip, mesh_fallback_enabled, quad,
    shade_at, texels,
};
use px_ui::color_edit::parse_hex;
use px_ui::palette_grid::{CHECK_A, CHECK_B, swatch_parts};
use px_ui::text::text_scale;
use px_ui::view::{ScreenRect, Size};

mod common;
use common::PPPS;

fn r(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

fn assert_plano<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}

const GRIS_A: Rgba = Rgba::opaque(0x4a, 0x4a, 0x4a);
const GRIS_B: Rgba = Rgba::opaque(0x6e, 0x6e, 0x6e);

/// Tono según la definición por fase, escrita aparte de `shade_at`: la casilla `(i, j)` es `A` si
/// `i + j` es par; `floor(d / c)` es impar justo cuando `d mod 2c >= c`. Con `i128` no desborda con
/// ningún `i64`.
fn tono_oraculo(x: i64, y: i64, origen: (i64, i64), cell: i64) -> Shade {
    let periodo = 2 * i128::from(cell);
    let medio = i128::from(cell);
    let impar_x = (i128::from(x) - i128::from(origen.0)).rem_euclid(periodo) >= medio;
    let impar_y = (i128::from(y) - i128::from(origen.1)).rem_euclid(periodo) >= medio;
    if impar_x == impar_y {
        Shade::A
    } else {
        Shade::B
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 1: constantes y tamaño
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_1_cell_u_vale_8() {
    assert_eq!(CELL_U, 8);
}

#[test]
fn fila_1_cell_px_es_8_por_n_de_1_a_8() {
    for n in 1..=8u32 {
        assert_eq!(cell_px(n), 8 * i64::from(n), "n = {n}");
    }
}

#[test]
fn fila_1_cell_px_con_n_cero_se_trata_como_1() {
    assert_eq!(cell_px(0), 8);
}

#[test]
fn fila_1_la_casilla_a_cada_escala_de_pantalla_es_8_8_16_16_y_24() {
    let esperado = [8, 8, 16, 16, 24];
    for (ppp, cell) in PPPS.into_iter().zip(esperado) {
        assert_eq!(cell_px(text_scale(ppp)), cell, "ppp {ppp}");
    }
}

#[test]
fn fila_1_los_dos_tonos_son_check_a_y_check_b_con_sus_valores() {
    assert_eq!(Shade::A.color(), CHECK_A);
    assert_eq!(Shade::B.color(), CHECK_B);
    // Los mismos valores que prueba `palette_grid.rs`: `#4a4a4a` y `#6e6e6e`.
    assert_eq!(Shade::A.color(), GRIS_A);
    assert_eq!(Shade::B.color(), GRIS_B);
    assert_eq!(CHECK_A, GRIS_A);
    assert_eq!(CHECK_B, GRIS_B);
    assert_ne!(Shade::A, Shade::B);
    assert_ne!(Shade::A.color(), Shade::B.color());
}

#[test]
fn fila_1_texels_son_a_b_b_a_por_filas() {
    assert_eq!(texels(), [GRIS_A, GRIS_B, GRIS_B, GRIS_A]);
    assert_eq!(
        texels(),
        [
            Shade::A.color(),
            Shade::B.color(),
            Shade::B.color(),
            Shade::A.color()
        ]
    );
}

#[test]
fn fila_1_el_fondo_sigue_siendo_gris_40_y_distinto_de_los_dos_tonos() {
    assert_eq!(BACKGROUND, egui::Color32::from_gray(40));
    let fondo = (BACKGROUND.r(), BACKGROUND.g(), BACKGROUND.b());
    assert_eq!(fondo, (40, 40, 40));
    for tono in [CHECK_A, CHECK_B] {
        assert_ne!(fondo, (tono.r, tono.g, tono.b));
    }
}

#[test]
fn fila_1_shade_es_un_tipo_plano() {
    assert_plano::<Shade>();
}

// ---------------------------------------------------------------------------------------------
// Fila 2: shade_at
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_2_origen_cero_cell_8_tabla_de_la_spec() {
    let casos = [
        ((0, 0), Shade::A),
        ((7, 7), Shade::A),
        ((8, 0), Shade::B),
        ((0, 8), Shade::B),
        ((8, 8), Shade::A),
        ((15, 15), Shade::A),
        ((16, 0), Shade::A),
    ];
    for ((x, y), tono) in casos {
        assert_eq!(shade_at(x, y, (0, 0), 8), tono, "({x}, {y})");
    }
}

#[test]
fn fila_2_con_coordenadas_negativas_vale_el_floor() {
    let casos = [
        ((-1, 0), Shade::B),
        ((-1, -1), Shade::A),
        ((-8, -8), Shade::A),
        ((-9, 0), Shade::A),
    ];
    for ((x, y), tono) in casos {
        assert_eq!(shade_at(x, y, (0, 0), 8), tono, "({x}, {y})");
    }
}

#[test]
fn fila_2_con_origen_10_20() {
    let casos = [
        ((10, 20), Shade::A),
        ((18, 20), Shade::B),
        ((9, 20), Shade::B),
    ];
    for ((x, y), tono) in casos {
        assert_eq!(shade_at(x, y, (10, 20), 8), tono, "({x}, {y})");
    }
}

#[test]
fn fila_2_con_origen_y_puntos_a_mas_menos_2_elevado_a_40_coincide_con_rem_euclid() {
    let g = 1_i64 << 40;
    let origen = (-g + 3, g);
    let valores = [-g, -g + 3, -g + 4, -1, 0, 1, g - 1, g, g + 1];
    for cell in [8, 16, 24, 40, 64] {
        for &x in &valores {
            for &y in &valores {
                assert_eq!(
                    shade_at(x, y, origen, cell),
                    tono_oraculo(x, y, origen, cell),
                    "({x}, {y}) cell {cell}"
                );
            }
        }
    }
    // Y la esquina del origen es siempre la casilla (0, 0): `A`.
    assert_eq!(shade_at(origen.0, origen.1, origen, 8), Shade::A);
}

#[test]
fn fila_2_con_cualquier_i64_no_desborda_ni_entra_en_panico() {
    // «Vale con cualquier i64»: la resta `x − ox` desbordaría un i64 (se hace en i128).
    let extremos = [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX];
    for cell in [1, 8, 24, 64] {
        for &ox in &extremos {
            for &oy in &extremos {
                for &x in &extremos {
                    for &y in &extremos {
                        assert_eq!(
                            shade_at(x, y, (ox, oy), cell),
                            tono_oraculo(x, y, (ox, oy), cell),
                            "({x}, {y}) origen ({ox}, {oy}) cell {cell}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn fila_2_alterna_casilla_a_casilla_en_las_dos_direcciones() {
    // Vecinas por la derecha y por abajo siempre de tono distinto, con cualquier origen.
    for origen in [(0, 0), (5, -3), (-100, 77)] {
        for cell in [8, 16, 24] {
            for i in -3..=3 {
                for j in -3..=3 {
                    let (x, y) = (origen.0 + i * cell, origen.1 + j * cell);
                    let aqui = shade_at(x, y, origen, cell);
                    assert_ne!(aqui, shade_at(x + cell, y, origen, cell));
                    assert_ne!(aqui, shade_at(x, y + cell, origen, cell));
                    assert_eq!(aqui, shade_at(x + cell, y + cell, origen, cell));
                    // Dentro de la casilla, el tono no cambia.
                    assert_eq!(aqui, shade_at(x + cell - 1, y + cell - 1, origen, cell));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3: quad y clip
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_3a_imagen_entera_visible() {
    let area = r(10, 20, 100, 50);
    assert_eq!(
        quad(area, (10, 20), 8),
        Some(CheckerQuad {
            rect: area,
            uv: [0.0, 0.0, 6.25, 3.125]
        })
    );
}

#[test]
fn fila_3b_imagen_con_origen_negativo_y_area_la_parte_visible() {
    let area = r(0, 0, 200, 150);
    let u0 = 5.0_f32 / 16.0; // (21 mod 16) / 16
    let v0 = 3.0_f32 / 16.0; // (35 mod 16) / 16
    let q = quad(area, (-21, -35), 8).expect("área no vacía");
    assert_eq!(q.rect, area);
    assert_eq!(q.uv, [u0, v0, u0 + 200.0 / 16.0, v0 + 150.0 / 16.0]);
}

#[test]
fn fila_3c_un_pixel_de_zoom_x1() {
    let area = r(37, 11, 1, 1);
    let u0 = 5.0_f32 / 16.0; // (37 mod 16) / 16
    let v0 = 11.0_f32 / 16.0; // (11 mod 16) / 16
    let q = quad(area, (0, 0), 8).expect("área de 1×1");
    assert_eq!(q.rect, area);
    assert_eq!(q.uv, [u0, v0, u0 + 1.0 / 16.0, v0 + 1.0 / 16.0]);
}

#[test]
fn fila_3d_area_de_ancho_0_de_alto_negativo_y_cell_0_dan_none() {
    assert_eq!(quad(r(5, 5, 0, 10), (0, 0), 8), None, "ancho 0");
    assert_eq!(quad(r(5, 5, 10, -3), (0, 0), 8), None, "alto negativo");
    assert_eq!(quad(r(5, 5, 10, 10), (0, 0), 0), None, "cell 0");
}

#[test]
fn fila_3d_tambien_ancho_negativo_alto_0_y_cell_negativa_dan_none() {
    // «None si area está vacía (ancho o alto <= 0) o cell < 1» (sección «Piezas»).
    assert_eq!(quad(r(5, 5, -2, 10), (0, 0), 8), None, "ancho negativo");
    assert_eq!(quad(r(5, 5, 10, 0), (0, 0), 8), None, "alto 0");
    assert_eq!(quad(r(5, 5, 0, 0), (0, 0), 8), None, "vacía del todo");
    assert_eq!(quad(r(5, 5, 10, 10), (0, 0), -8), None, "cell negativa");
}

#[test]
fn fila_3e_clip_de_una_imagen_que_sobresale_de_un_lado_y_de_otro() {
    let lienzo = Size::new(100, 80);
    assert_eq!(
        clip(r(-50, -40, 300, 200), lienzo),
        Some(r(0, 0, 100, 80)),
        "la imagen cubre el lienzo y sobresale"
    );
}

#[test]
fn fila_3e_clip_de_una_imagen_fuera_del_lienzo_es_none() {
    let lienzo = Size::new(100, 80);
    assert_eq!(clip(r(200, 10, 50, 50), lienzo), None, "a la derecha");
    assert_eq!(clip(r(10, 200, 50, 50), lienzo), None, "debajo");
    assert_eq!(
        clip(r(-100, -100, 50, 50), lienzo),
        None,
        "arriba a la izquierda"
    );
    assert_eq!(
        clip(r(100, 0, 50, 50), lienzo),
        None,
        "pegada al borde derecho"
    );
}

#[test]
fn fila_3e_clip_de_una_imagen_que_cubre_el_lienzo() {
    let lienzo = Size::new(100, 80);
    assert_eq!(clip(r(-10, -10, 500, 500), lienzo), Some(r(0, 0, 100, 80)));
    assert_eq!(clip(r(0, 0, 100, 80), lienzo), Some(r(0, 0, 100, 80)));
}

#[test]
fn fila_3e_clip_de_una_imagen_que_solo_en_parte_esta_dentro() {
    // `image ∩ (0, 0, ancho, alto)`.
    let lienzo = Size::new(100, 80);
    assert_eq!(clip(r(30, 20, 200, 200), lienzo), Some(r(30, 20, 70, 60)));
    assert_eq!(clip(r(-5, 10, 20, 20), lienzo), Some(r(0, 10, 15, 20)));
}

#[test]
fn fila_3_clip_con_un_lienzo_vacio_es_none() {
    assert_eq!(clip(r(0, 0, 10, 10), Size::new(0, 0)), None);
    assert_eq!(clip(r(0, 0, 10, 10), Size::new(0, 5)), None);
}

#[test]
fn fila_3_clip_con_lienzo_de_1x1_da_none_o_1x1() {
    let lienzo = Size::new(1, 1);
    for imagen in [
        r(-30, -20, 64, 48),
        r(0, 0, 64, 48),
        r(-63, -47, 64, 48),
        r(1, 0, 64, 48),
    ] {
        let recorte = clip(imagen, lienzo);
        assert!(
            matches!(
                recorte,
                None | Some(ScreenRect {
                    width: 1,
                    height: 1,
                    ..
                })
            ),
            "{imagen:?} → {recorte:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 4: cells
// ---------------------------------------------------------------------------------------------

fn celda(x: i64, y: i64, w: i64, h: i64, shade: Shade) -> Cell {
    Cell {
        rect: r(x, y, w, h),
        shade,
    }
}

fn area_total(lista: &[Cell]) -> i64 {
    lista.iter().map(|c| c.rect.width * c.rect.height).sum()
}

/// Dos rectángulos se solapan si comparten algún píxel.
fn se_solapan(a: ScreenRect, b: ScreenRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

fn sin_solapes(lista: &[Cell]) -> bool {
    for (i, a) in lista.iter().enumerate() {
        for b in &lista[i + 1..] {
            if se_solapan(a.rect, b.rect) {
                return false;
            }
        }
    }
    true
}

#[test]
fn fila_4a_area_de_20_por_12_con_origen_cero() {
    let lista: Vec<Cell> = cells(r(0, 0, 20, 12), (0, 0), 8).collect();
    assert_eq!(
        lista,
        [
            celda(0, 0, 8, 8, Shade::A),
            celda(8, 0, 8, 8, Shade::B),
            celda(16, 0, 4, 8, Shade::A),
            celda(0, 8, 8, 4, Shade::B),
            celda(8, 8, 8, 4, Shade::A),
            celda(16, 8, 4, 4, Shade::B),
        ]
    );
    assert_eq!(lista.len(), 6);
    assert_eq!(area_total(&lista), 240);
    assert!(sin_solapes(&lista));
}

#[test]
fn fila_4b_area_desplazada_5_3() {
    let lista: Vec<Cell> = cells(r(5, 3, 20, 12), (0, 0), 8).collect();
    assert_eq!(lista.len(), 8);
    assert_eq!(lista[0], celda(5, 3, 3, 5, Shade::A));
    // Columnas de ancho 3, 8, 8 y 1; filas de alto 5 y 7; por filas.
    let anchos: Vec<i64> = lista.iter().map(|c| c.rect.width).collect();
    let altos: Vec<i64> = lista.iter().map(|c| c.rect.height).collect();
    assert_eq!(anchos, [3, 8, 8, 1, 3, 8, 8, 1]);
    assert_eq!(altos, [5, 5, 5, 5, 7, 7, 7, 7]);
    let tonos: Vec<Shade> = lista.iter().map(|c| c.shade).collect();
    let (ta, tb) = (Shade::A, Shade::B);
    assert_eq!(tonos, [ta, tb, ta, tb, tb, ta, tb, ta]);
    assert_eq!(
        lista,
        [
            celda(5, 3, 3, 5, Shade::A),
            celda(8, 3, 8, 5, Shade::B),
            celda(16, 3, 8, 5, Shade::A),
            celda(24, 3, 1, 5, Shade::B),
            celda(5, 8, 3, 7, Shade::B),
            celda(8, 8, 8, 7, Shade::A),
            celda(16, 8, 8, 7, Shade::B),
            celda(24, 8, 1, 7, Shade::A),
        ]
    );
    assert_eq!(area_total(&lista), 240);
    assert!(sin_solapes(&lista));
}

#[test]
fn fila_4c_area_menor_que_una_casilla_es_una_casilla_igual_al_area() {
    let lista: Vec<Cell> = cells(r(3, 3, 2, 2), (0, 0), 8).collect();
    assert_eq!(lista, [celda(3, 3, 2, 2, Shade::A)]);
}

#[test]
fn fila_4d_area_de_1x1_una_casilla_y_area_vacia_ninguna() {
    let una: Vec<Cell> = cells(r(9, 0, 1, 1), (0, 0), 8).collect();
    assert_eq!(una, [celda(9, 0, 1, 1, Shade::B)]);
    assert_eq!(cells(r(0, 0, 0, 5), (0, 0), 8).count(), 0, "ancho 0");
    assert_eq!(cells(r(0, 0, 5, 0), (0, 0), 8).count(), 0, "alto 0");
    assert_eq!(
        cells(r(0, 0, -5, 5), (0, 0), 8).count(),
        0,
        "ancho negativo"
    );
    assert_eq!(cells(r(0, 0, 0, 0), (0, 0), 8).count(), 0, "vacía del todo");
}

#[test]
fn fila_4d_con_cell_menor_que_1_no_hay_casillas() {
    // «Vacío si area o cell lo son».
    assert_eq!(cells(r(0, 0, 20, 12), (0, 0), 0).count(), 0);
    assert_eq!(cells(r(0, 0, 20, 12), (0, 0), -8).count(), 0);
}

#[test]
fn fila_4e_un_lienzo_de_4k_tiene_129600_casillas_sin_reservar_la_lista() {
    // Solo se cuenta: `cells` es un iterador, no una lista.
    assert_eq!(cells(r(0, 0, 3840, 2160), (0, 0), 8).count(), 129_600);
    // Con un origen cualquiera el número depende de la fase, pero el iterador sigue siendo
    // perezoso: un área de 2³⁰ píxeles por lado da sus primeras casillas sin recorrerla entera.
    let enorme = r(0, 0, 1 << 30, 1 << 30);
    let primeras: Vec<Cell> = cells(enorme, (0, 0), 8).take(3).collect();
    assert_eq!(
        primeras,
        [
            celda(0, 0, 8, 8, Shade::A),
            celda(8, 0, 8, 8, Shade::B),
            celda(16, 0, 8, 8, Shade::A),
        ]
    );
}

#[test]
fn fila_4_el_tono_de_cada_casilla_es_el_de_shade_at() {
    let origen = (-7, 13);
    for c in cells(r(-20, -20, 70, 60), origen, 8) {
        assert_eq!(
            c.shade,
            shade_at(c.rect.x, c.rect.y, origen, 8),
            "{:?}",
            c.rect
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9: fondo elegible
// ---------------------------------------------------------------------------------------------

const FONDOS: [CanvasBackground; 4] = [
    CanvasBackground::Checker,
    CanvasBackground::Black,
    CanvasBackground::Gray,
    CanvasBackground::White,
];

#[test]
fn fila_9_por_defecto_es_el_damero() {
    assert_eq!(CanvasBackground::default(), CanvasBackground::Checker);
}

#[test]
fn fila_9_background_fill_de_las_cuatro_opciones() {
    assert_eq!(background_fill(CanvasBackground::Checker), Fill::Checker);
    assert_eq!(
        background_fill(CanvasBackground::Black),
        Fill::Solid(Rgba::opaque(0, 0, 0))
    );
    assert_eq!(
        background_fill(CanvasBackground::White),
        Fill::Solid(Rgba::opaque(255, 255, 255))
    );
    // Gris: los canales de `BACKGROUND` (gris 40).
    assert_eq!(
        background_fill(CanvasBackground::Gray),
        Fill::Solid(Rgba::opaque(BACKGROUND.r(), BACKGROUND.g(), BACKGROUND.b()))
    );
    assert_eq!(
        background_fill(CanvasBackground::Gray),
        Fill::Solid(Rgba::opaque(40, 40, 40))
    );
}

#[test]
fn fila_9_black_y_white_son_opacos_y_distintos_de_el_fondo_y_de_los_dos_tonos() {
    assert_eq!(BLACK, Rgba::opaque(0, 0, 0));
    assert_eq!(WHITE, Rgba::opaque(255, 255, 255));
    assert_eq!(BLACK.a, 255);
    assert_eq!(WHITE.a, 255);
    let fondo = Rgba::opaque(BACKGROUND.r(), BACKGROUND.g(), BACKGROUND.b());
    for color in [BLACK, WHITE] {
        assert_ne!(color, fondo);
        assert_ne!(color, CHECK_A);
        assert_ne!(color, CHECK_B);
    }
    assert_ne!(BLACK, WHITE);
}

#[test]
fn fila_9_las_cuatro_opciones_son_distintas_entre_si_y_sus_rellenos_tambien() {
    for (i, a) in FONDOS.iter().enumerate() {
        for b in &FONDOS[i + 1..] {
            assert_ne!(a, b);
            assert_ne!(background_fill(*a), background_fill(*b), "{a:?} y {b:?}");
        }
    }
}

#[test]
fn fila_9_los_tipos_del_fondo_son_planos_y_con_defecto() {
    assert_plano::<CanvasBackground>();
    assert_plano::<Fill>();
    let _: CanvasBackground = Default::default();
}

// ---------------------------------------------------------------------------------------------
// Fila 10: coherencia con el panel de paletas
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_10_las_dos_casillas_del_lienzo_son_las_constantes_del_panel() {
    assert_eq!(Shade::A.color(), CHECK_A);
    assert_eq!(Shade::B.color(), CHECK_B);
    assert_eq!(texels(), [CHECK_A, CHECK_B, CHECK_B, CHECK_A]);
}

#[test]
fn fila_10_la_muestra_transparente_del_panel_usa_la_misma_pareja_en_el_mismo_orden() {
    for n in 1..=3u32 {
        let partes = swatch_parts(n, Rgba::new(0, 0, 0, 0));
        assert_eq!(partes.len(), 4, "n = {n}: cuatro cuadrados y nada más");
        let tonos: Vec<Rgba> = partes.iter().map(|(_, tono)| *tono).collect();
        // Los cuatro cuadrados, por filas, son los cuatro texels de la textura del lienzo.
        assert_eq!(tonos, texels(), "n = {n}");
        // Y no se pinta con ningún otro gris.
        for tono in &tonos {
            assert!(
                *tono == Shade::A.color() || *tono == Shade::B.color(),
                "n = {n}: {tono:?}"
            );
        }
    }
}

#[test]
fn fila_10_en_la_muestra_los_cuadrados_siguen_el_mismo_patron_que_shade_at() {
    // Los cuatro cuadrados de la muestra (origen en su esquina, casilla de `5·n`) alternan igual
    // que las casillas del lienzo: la casilla (0, 0) es `A`.
    for n in 1..=3u32 {
        let partes = swatch_parts(n, Rgba::new(0, 0, 0, 0));
        for (rect, tono) in partes {
            let cell = rect.width;
            assert_eq!(
                tono,
                shade_at(rect.x, rect.y, (0, 0), cell).color(),
                "n = {n}: {rect:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 15, parte pura: la variable de entorno
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_15_la_variable_de_entorno_se_llama_pixoko_damero_malla() {
    assert_eq!(ENV_MESH, "PIXOKO_CHECKER_MESH");
}

#[test]
fn fila_15_solo_el_valor_1_activa_el_respaldo_por_malla() {
    assert!(!mesh_fallback_enabled(None));
    assert!(mesh_fallback_enabled(Some("1")));
    assert!(!mesh_fallback_enabled(Some("0")));
    assert!(!mesh_fallback_enabled(Some("")));
    assert!(!mesh_fallback_enabled(Some("true")));
}

#[test]
fn fila_15_ningun_otro_valor_parecido_a_1_la_activa() {
    for valor in [" 1", "1 ", "11", "01", "yes", "on", "TRUE", "-1"] {
        assert!(!mesh_fallback_enabled(Some(valor)), "{valor:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 1: quad, shade_at y cells son la misma función
// ---------------------------------------------------------------------------------------------

/// Muestra la textura 2×2 con `NEAREST` y envoltura en el centro del píxel `(i, j)` (de 0) del
/// cuadrilátero, interpolando los uv `f32` en `f64`.
fn muestra(q: &CheckerQuad, tex: &[Rgba; 4], i: i64, j: i64) -> Rgba {
    let t = (i as f64 + 0.5) / q.rect.width as f64;
    let s = (j as f64 + 0.5) / q.rect.height as f64;
    let (u0, v0) = (f64::from(q.uv[0]), f64::from(q.uv[1]));
    let (u1, v1) = (f64::from(q.uv[2]), f64::from(q.uv[3]));
    let u = u0 + (u1 - u0) * t;
    let v = v0 + (v1 - v0) * s;
    let tx = ((u * 2.0).floor() as i64).rem_euclid(2);
    let ty = ((v * 2.0).floor() as i64).rem_euclid(2);
    tex[(ty * 2 + tx) as usize]
}

/// Tono por fase en `i64` (los datos de la propiedad caben de sobra): más barato que `i128` y
/// escrito aparte de `shade_at`.
fn color_por_fase(x: i64, y: i64, origen: (i64, i64), cell: i64) -> Rgba {
    let impar_x = (x - origen.0).rem_euclid(2 * cell) >= cell;
    let impar_y = (y - origen.1).rem_euclid(2 * cell) >= cell;
    if impar_x == impar_y { GRIS_A } else { GRIS_B }
}

/// Bordes de las casillas del eje: el principio del área, cada borde de casilla estrictamente
/// dentro y el final.
fn bordes(inicio: i64, largo: i64, origen: i64, cell: i64) -> Vec<i64> {
    let fin = inicio + largo;
    let mut v = vec![inicio];
    let mut k = (inicio - origen).div_euclid(cell) + 1;
    while origen + k * cell < fin {
        v.push(origen + k * cell);
        k += 1;
    }
    v.push(fin);
    v
}

/// Número de casillas que corta el eje: `⌈(inicio + largo − origen) / cell⌉ − ⌊(inicio − origen) /
/// cell⌋`.
fn casillas_del_eje(inicio: i64, largo: i64, origen: i64, cell: i64) -> i64 {
    let techo = -(-(inicio + largo - origen)).div_euclid(cell);
    techo - (inicio - origen).div_euclid(cell)
}

const GRANDE: i64 = 1 << 40;

/// Datos de un caso: origen, `cell`, área y una subárea (como porcentajes sobre el área).
#[derive(Clone, Debug)]
struct Caso {
    origen: (i64, i64),
    cell: i64,
    area: ScreenRect,
    sub: ScreenRect,
}

fn caso() -> impl Strategy<Value = Caso> {
    (
        (-GRANDE..=GRANDE, -GRANDE..=GRANDE),
        1i64..=8,
        any::<bool>(),
        (-GRANDE..=GRANDE, -GRANDE..=GRANDE),
        (-3000i64..=3000, -3000i64..=3000),
        (1i64..=600, 1i64..=600),
        (any::<u32>(), any::<u32>(), any::<u32>(), any::<u32>()),
    )
        .prop_map(
            |(origen, k, cerca, lejos, desplazo, (w, h), (a, b, c, d))| {
                let cell = 8 * k;
                // Mitad de los casos con el área junto al origen (casillas a ambos lados del
                // cero de la fase) y mitad en cualquier posición, también negativa.
                let (x, y) = if cerca {
                    (origen.0 + desplazo.0, origen.1 + desplazo.1)
                } else {
                    lejos
                };
                let sx = i64::from(a) % w;
                let sy = i64::from(b) % h;
                let sw = 1 + i64::from(c) % (w - sx);
                let sh = 1 + i64::from(d) % (h - sy);
                Caso {
                    origen,
                    cell,
                    area: r(x, y, w, h),
                    sub: r(x + sx, y + sy, sw, sh),
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// (a) El cuadrilátero del área, muestreado como la GPU (`NEAREST` con envoltura) en el centro
    /// de cada píxel físico, da el tono de `shade_at`, que a su vez coincide con la fase calculada
    /// aparte.
    #[test]
    fn propiedad_1a_el_muestreo_del_cuadrilatero_es_shade_at(c in caso()) {
        let q = quad(c.area, c.origen, c.cell).expect("área no vacía");
        prop_assert_eq!(q.rect, c.area);
        let tex = texels();
        for j in 0..c.area.height {
            for i in 0..c.area.width {
                let (x, y) = (c.area.x + i, c.area.y + j);
                let esperado = color_por_fase(x, y, c.origen, c.cell);
                let visto = muestra(&q, &tex, i, j);
                prop_assert_eq!(visto, esperado, "el muestreo en ({}, {})", x, y);
                prop_assert_eq!(
                    shade_at(x, y, c.origen, c.cell).color(),
                    esperado,
                    "shade_at en ({}, {})", x, y
                );
            }
        }
    }

    /// (b) Para cualquier subárea, `quad(subárea)` muestrea lo mismo que `quad(área)` en esos
    /// píxeles: el recorte no cambia el color.
    #[test]
    fn propiedad_1b_el_recorte_no_cambia_el_color(c in caso()) {
        let grande = quad(c.area, c.origen, c.cell).expect("área no vacía");
        let pequeno = quad(c.sub, c.origen, c.cell).expect("subárea no vacía");
        prop_assert_eq!(pequeno.rect, c.sub);
        let tex = texels();
        for j in 0..c.sub.height {
            for i in 0..c.sub.width {
                let (x, y) = (c.sub.x + i, c.sub.y + j);
                let con_el_pequeno = muestra(&pequeno, &tex, i, j);
                let con_el_grande = muestra(&grande, &tex, x - c.area.x, y - c.area.y);
                prop_assert_eq!(con_el_pequeno, con_el_grande, "en ({}, {})", x, y);
                prop_assert_eq!(con_el_pequeno, color_por_fase(x, y, c.origen, c.cell));
            }
        }
    }

    /// (c) `cells(área)` cubre el área exactamente: sin solapes ni huecos, cada casilla dentro de
    /// una sola casilla del damero, con el tono de `shade_at` de cualquiera de sus píxeles,
    /// alternado con sus vecinas, y su número es el calculado aparte por eje.
    #[test]
    fn propiedad_1c_las_casillas_cubren_el_area_exactamente(c in caso()) {
        let lista: Vec<Cell> = cells(c.area, c.origen, c.cell).collect();
        let xs = bordes(c.area.x, c.area.width, c.origen.0, c.cell);
        let ys = bordes(c.area.y, c.area.height, c.origen.1, c.cell);
        let columnas = casillas_del_eje(c.area.x, c.area.width, c.origen.0, c.cell);
        let filas = casillas_del_eje(c.area.y, c.area.height, c.origen.1, c.cell);
        prop_assert_eq!(xs.len() as i64 - 1, columnas);
        prop_assert_eq!(ys.len() as i64 - 1, filas);
        prop_assert_eq!(lista.len() as i64, columnas * filas, "número de casillas");
        // Suma de áreas: sin huecos ni solapes (junto con que van contiguas, por filas).
        prop_assert_eq!(area_total(&lista), c.area.width * c.area.height);
        let columnas = columnas as usize;
        for (k, casilla) in lista.iter().enumerate() {
            let (fila, columna) = (k / columnas, k % columnas);
            // Por filas y contiguas: la que toca por posición, recortada al área.
            let rect = r(
                xs[columna],
                ys[fila],
                xs[columna + 1] - xs[columna],
                ys[fila + 1] - ys[fila],
            );
            prop_assert_eq!(casilla.rect, rect, "casilla {}", k);
            // Dentro del área.
            prop_assert!(casilla.rect.x >= c.area.x && casilla.rect.y >= c.area.y);
            prop_assert!(casilla.rect.x + casilla.rect.width <= c.area.x + c.area.width);
            prop_assert!(casilla.rect.y + casilla.rect.height <= c.area.y + c.area.height);
            // Dentro de una sola casilla del damero: el primer y el último píxel caen en la misma.
            let (x1, y1) = (rect.x + rect.width - 1, rect.y + rect.height - 1);
            prop_assert_eq!(
                (rect.x - c.origen.0).div_euclid(c.cell),
                (x1 - c.origen.0).div_euclid(c.cell)
            );
            prop_assert_eq!(
                (rect.y - c.origen.1).div_euclid(c.cell),
                (y1 - c.origen.1).div_euclid(c.cell)
            );
            // Tono: el de `shade_at` de sus esquinas y el de la fase calculada aparte.
            prop_assert_eq!(casilla.shade, shade_at(rect.x, rect.y, c.origen, c.cell));
            prop_assert_eq!(casilla.shade, shade_at(x1, y1, c.origen, c.cell));
            prop_assert_eq!(
                casilla.shade.color(),
                color_por_fase(rect.x, rect.y, c.origen, c.cell)
            );
            // Alternado con la vecina de la derecha y con la de abajo.
            if columna + 1 < columnas {
                prop_assert_ne!(casilla.shade, lista[k + 1].shade);
            }
            if k + columnas < lista.len() {
                prop_assert_ne!(casilla.shade, lista[k + columnas].shade);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19b, fila 1: tipo y relleno de `CanvasBackground::Custom`
// ---------------------------------------------------------------------------------------------
//
// Suposiciones: `CanvasBackground::custom(Rgba) -> Option<CanvasBackground>`, `solid_color(self) ->
// Option<Rgba>` y `background_value_text(CanvasBackground) -> Option<String>` son las de «Piezas».
// Con un `Custom` construido a mano con alfa < 255 (la variante es pública) la spec solo fija
// `background_fill` (alfa 255 forzado): su `solid_color` y su valor escrito no se prueban.
// NO PROBADO: `solid_color` y `background_value_text` de un `Custom(c)` con `c.a < 255` (la spec no
// dice si devuelven `c` tal cual, con el alfa forzado o `#rrggbbaa`).

const AZUL_ACERO: Rgba = Rgba::opaque(0x33, 0x66, 0x99);
const GRIS_40: Rgba = Rgba::opaque(40, 40, 40);

fn propio(c: Rgba) -> CanvasBackground {
    CanvasBackground::Custom(c)
}

/// Los cinco fondos, con un color propio de canales distintos.
fn los_cinco() -> [CanvasBackground; 5] {
    [
        CanvasBackground::Checker,
        CanvasBackground::Black,
        CanvasBackground::Gray,
        CanvasBackground::White,
        propio(AZUL_ACERO),
    ]
}

#[test]
fn fila_1_custom_da_some_solo_con_alfa_255_y_conserva_el_color() {
    assert_eq!(
        CanvasBackground::custom(AZUL_ACERO),
        Some(CanvasBackground::Custom(AZUL_ACERO))
    );
    assert_eq!(
        CanvasBackground::custom(Rgba::new(0x33, 0x66, 0x99, 0xfe)),
        None
    );
    assert_eq!(CanvasBackground::custom(Rgba::new(0, 0, 0, 0)), None);
    // `#ffffffff` es el blanco opaco: vale y no se confunde con `White`.
    assert_eq!(
        CanvasBackground::custom(Rgba::new(0xff, 0xff, 0xff, 0xff)),
        Some(CanvasBackground::Custom(WHITE))
    );
    assert_ne!(CanvasBackground::Custom(WHITE), CanvasBackground::White);
    // Los 256 alfas: solo el último vale.
    for a in 0..=255u8 {
        let c = Rgba::new(0x33, 0x66, 0x99, a);
        assert_eq!(
            CanvasBackground::custom(c).is_some(),
            a == 255,
            "alfa {a:#04x}"
        );
    }
}

#[test]
fn fila_1_background_fill_de_custom_es_siempre_solido_con_alfa_255() {
    assert_eq!(
        background_fill(propio(AZUL_ACERO)),
        Fill::Solid(Rgba::new(0x33, 0x66, 0x99, 255))
    );
    // Construido a mano con alfa < 255: el relleno fuerza el alfa a 255 y conserva r, g, b.
    for a in [0u8, 1, 0x7f, 0x80, 0xfe] {
        assert_eq!(
            background_fill(propio(Rgba::new(0x33, 0x66, 0x99, a))),
            Fill::Solid(Rgba::new(0x33, 0x66, 0x99, 255)),
            "alfa {a:#04x}"
        );
    }
    for c in [
        Rgba::opaque(0xff, 0x80, 0x00),
        Rgba::opaque(0x01, 0x02, 0x03),
        Rgba::new(0xff, 0x80, 0x00, 0),
    ] {
        assert_eq!(
            background_fill(propio(c)),
            Fill::Solid(Rgba::opaque(c.r, c.g, c.b))
        );
    }
}

#[test]
fn fila_1_el_relleno_de_las_otras_cuatro_es_el_de_la_17b() {
    assert_eq!(background_fill(CanvasBackground::Checker), Fill::Checker);
    assert_eq!(background_fill(CanvasBackground::Black), Fill::Solid(BLACK));
    assert_eq!(background_fill(CanvasBackground::White), Fill::Solid(WHITE));
    assert_eq!(
        background_fill(CanvasBackground::Gray),
        Fill::Solid(GRIS_40)
    );
    assert_eq!(
        background_fill(CanvasBackground::Gray),
        Fill::Solid(Rgba::opaque(BACKGROUND.r(), BACKGROUND.g(), BACKGROUND.b()))
    );
}

#[test]
fn fila_1_el_fondo_por_defecto_sigue_siendo_el_damero() {
    assert_eq!(CanvasBackground::default(), CanvasBackground::Checker);
    assert_eq!(background_fill(CanvasBackground::default()), Fill::Checker);
}

#[test]
fn fila_1_solid_color_es_some_en_las_cuatro_solidas_y_none_con_el_damero() {
    assert_eq!(CanvasBackground::Checker.solid_color(), None);
    assert_eq!(CanvasBackground::Black.solid_color(), Some(BLACK));
    assert_eq!(CanvasBackground::White.solid_color(), Some(WHITE));
    assert_eq!(CanvasBackground::Gray.solid_color(), Some(GRIS_40));
    assert_eq!(propio(AZUL_ACERO).solid_color(), Some(AZUL_ACERO));
    assert_eq!(
        propio(Rgba::opaque(0xff, 0x80, 0x00)).solid_color(),
        Some(Rgba::opaque(0xff, 0x80, 0x00))
    );
    // `Custom(#000000)` no pasa a `Black` ni a otro valor: su color es el elegido.
    assert_eq!(propio(BLACK).solid_color(), Some(BLACK));
}

#[test]
fn fila_1_background_value_text_es_some_solo_con_custom_y_mide_7_caracteres() {
    assert_eq!(
        background_value_text(propio(AZUL_ACERO)),
        Some("#336699".to_string())
    );
    assert_eq!(
        background_value_text(propio(Rgba::opaque(0xff, 0x80, 0x00))),
        Some("#ff8000".to_string())
    );
    assert_eq!(
        background_value_text(propio(Rgba::opaque(0x01, 0x02, 0x03))),
        Some("#010203".to_string())
    );
    assert_eq!(
        background_value_text(propio(BLACK)),
        Some("#000000".to_string())
    );
    assert_eq!(
        background_value_text(propio(AZUL_ACERO)).map(|t| t.chars().count()),
        Some(7)
    );
    for otro in [
        CanvasBackground::Checker,
        CanvasBackground::Black,
        CanvasBackground::Gray,
        CanvasBackground::White,
    ] {
        assert_eq!(background_value_text(otro), None, "{otro:?}");
    }
}

#[test]
fn fila_1_custom_no_se_normaliza_a_otra_variante_aunque_su_color_coincida() {
    // El usuario eligió «Color…»: la marca y el valor no saltan a otra entrada.
    assert_ne!(propio(BLACK), CanvasBackground::Black);
    assert_ne!(propio(WHITE), CanvasBackground::White);
    assert_ne!(propio(GRIS_40), CanvasBackground::Gray);
    assert_eq!(CanvasBackground::custom(BLACK), Some(propio(BLACK)));
    // Pero se ve igual: el mismo relleno.
    assert_eq!(
        background_fill(propio(BLACK)),
        background_fill(CanvasBackground::Black)
    );
    assert_eq!(
        background_fill(propio(WHITE)),
        background_fill(CanvasBackground::White)
    );
    assert_eq!(
        background_fill(propio(GRIS_40)),
        background_fill(CanvasBackground::Gray)
    );
}

#[test]
fn fila_1_las_cinco_variantes_son_distintas_entre_si_y_el_tipo_sigue_siendo_plano() {
    let cinco = los_cinco();
    for (i, a) in cinco.iter().enumerate() {
        for b in &cinco[i + 1..] {
            assert_ne!(a, b);
        }
    }
    // Dos colores propios distintos son fondos distintos y con rellenos distintos.
    assert_ne!(propio(AZUL_ACERO), propio(Rgba::opaque(0xff, 0x80, 0)));
    assert_ne!(
        background_fill(propio(AZUL_ACERO)),
        background_fill(propio(Rgba::opaque(0xff, 0x80, 0)))
    );
    assert_eq!(propio(AZUL_ACERO), propio(AZUL_ACERO));
    assert_plano::<CanvasBackground>();
    assert_plano::<Fill>();
}

// ---------------------------------------------------------------------------------------------
// Unidad 19b, propiedad 1 (parte pura): `custom`, `background_value_text` y `background_fill`
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// `custom(c)` es `Some` si y solo si `c.a == 255`; entonces es `Custom(c)`. El valor escrito de
    /// un color propio opaco se lee de vuelta (`parse_hex`) como el mismo color y mide 7
    /// caracteres. `background_fill(Custom(c))`, con cualquier alfa, es un sólido opaco con los
    /// mismos r, g, b y nunca lleva alfa < 255.
    #[test]
    fn propiedad_1_custom_solo_opaco_valor_de_ida_y_vuelta_y_relleno_siempre_opaco(
        r in any::<u8>(),
        g in any::<u8>(),
        b in any::<u8>(),
        a in any::<u8>(),
    ) {
        let c = Rgba::new(r, g, b, a);
        let hecho = CanvasBackground::custom(c);
        prop_assert_eq!(hecho.is_some(), a == 255);
        if let Some(fondo) = hecho {
            prop_assert_eq!(fondo, CanvasBackground::Custom(c));
            prop_assert_eq!(fondo.solid_color(), Some(c));
            let texto = background_value_text(fondo).expect("un color propio tiene valor");
            prop_assert_eq!(texto.chars().count(), 7);
            // Se lee de vuelta con `parse_hex`: con seis dígitos el alfa es el del color actual
            // (aquí, opaco).
            prop_assert_eq!(parse_hex(&texto, Mode::Rgba, Rgba::opaque(0, 0, 0)), Ok(c));
        }
        // El relleno de un `Custom` construido a mano, con cualquier alfa.
        match background_fill(CanvasBackground::Custom(c)) {
            Fill::Solid(s) => {
                prop_assert_eq!(s.a, 255);
                prop_assert_eq!((s.r, s.g, s.b), (r, g, b));
            }
            Fill::Checker => prop_assert!(false, "un color propio nunca es el damero"),
        }
    }
}
