//! Tests de la lógica pura de cuentagotas y borrador y de sus teclas (specs/hito-2-cuentagotas-
//! borrador.md, «Casos límite» fila 1 en lo que toca a `px-ui` y fila 2, y propiedad 3).
//!
//! `px_ui::pick` no usa egui: `eraser_pixel`, `eraser_label`, `is_erasable`, `picked_at` y
//! `pick_notice` se prueban con imágenes pequeñas hechas a mano, una por modo. Las filas 3 a 8 y
//! las propiedades 1, 2 y 4 están en `tests/eraser.rs` y `tests/dropper.rs`; la fila 10, en
//! `tests/eraser_canvas.rs`.
//!
//! La fila 14 (regla de la entrada de alfa 0 del valor de borrado en indexado) está al final.
//!
//! La propiedad 3 recorre todo `Key::ALL` por las 32 combinaciones de modificadores: se escribe
//! como `proptest!` (muestreo) y, además, como recorrido exhaustivo, que es más fuerte. Sustituye
//! a la propiedad 5 de la unidad 7 (`tests/tool.rs`), cuyo oráculo no conocía `I` ni `E`.

use egui::{Key, Modifiers};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Command, Document, Image, Mode, Palette, Pixel, Rgba};
use px_ui::color::{ColorChoice, SlotColors, choice_label_with};
use px_ui::pick::{
    EraseError, Picked, eraser_label, eraser_pixel, is_erasable, pick_notice, picked_at,
};
use px_ui::shortcuts::{action_for_key, file_action_for_key};
use px_ui::tool::{Tool, ToolAction, status_text, tool_action_for_key, tool_color_label};

const TODAS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

// Apoyo común.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// Imagen RGBA de 8×8 vacía (todo `(0, 0, 0, 0)`).
fn imagen_rgba() -> Image {
    Image::new(8, 8, Mode::Rgba).expect("imagen válida")
}

fn imagen_gris() -> Image {
    Image::new(8, 8, Mode::Gray).expect("imagen válida")
}

/// Imagen de gris con alfa de 8×8 vacía (todo `{ gray: 0, alpha: 0 }`).
fn imagen_gris_alfa() -> Image {
    Image::new(8, 8, Mode::GrayAlpha).expect("imagen válida")
}

fn paleta_de_5() -> Palette {
    Palette::from_colors((1..=5).map(|i| Rgba::opaque(10 * i, 0, 0))).expect("paleta válida")
}

/// Indexada de 5 entradas con el índice 3 transparente; todo el lienzo vale `Indexed(0)`.
fn indexada_con_transparente() -> Image {
    let mut paleta = paleta_de_5();
    paleta.set_transparent_index(Some(3)).expect("existe");
    Image::new_indexed(8, 8, paleta).expect("imagen válida")
}

fn indexada_sin_transparente() -> Image {
    Image::new_indexed(8, 8, paleta_de_5()).expect("imagen válida")
}

/// Indexada de 16 entradas, con la 12 en `#ff8800`.
fn indexada_de_16() -> Image {
    let mut colores: Vec<Rgba> = (0..16).map(|i| Rgba::opaque(i * 8, 0, 0)).collect();
    colores[12] = Rgba::opaque(255, 136, 0);
    let paleta = Palette::from_colors(colores).expect("paleta válida");
    Image::new_indexed(8, 8, paleta).expect("imagen válida")
}

fn con_pixel(mut image: Image, x: u32, y: u32, pixel: Pixel) -> Image {
    image.set_pixel(x, y, pixel).expect("dentro");
    image
}

/// Las cinco combinaciones de modificadores de la fila 1 más `ctrl` y `command` a la vez (lo que
/// da egui en Linux) y `mac_cmd` y `command` a la vez (macOS).
fn modificadores_de_la_fila_1() -> Vec<Modifiers> {
    vec![
        Modifiers::SHIFT,
        Modifiers::ALT,
        Modifiers::CTRL,
        Modifiers::COMMAND,
        Modifiers::MAC_CMD,
        Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
        Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        },
    ]
}

// Fila 1: teclas y rótulos.

#[test]
fn fila_1_coma_es_cuentagotas_y_e_ya_no_es_borrador_sin_modificadores() {
    assert_eq!(
        tool_action_for_key(Key::Comma, Modifiers::NONE),
        Some(ToolAction::Select(Tool::Dropper))
    );
    assert_eq!(tool_action_for_key(Key::E, Modifiers::NONE), None);
}

#[test]
fn fila_1_con_cualquier_modificador_coma_no_hace_nada() {
    let key = Key::Comma;
    for modifiers in modificadores_de_la_fila_1() {
        assert_eq!(
            tool_action_for_key(key, modifiers),
            None,
            "{key:?} con {modifiers:?}"
        );
    }
}

#[test]
fn fila_1_d_v_f_y_escape_con_las_teclas_nuevas() {
    for (key, accion) in [
        (Key::D, ToolAction::Select(Tool::Pencil)),
        (Key::V, ToolAction::Select(Tool::Line)),
        (Key::F, ToolAction::Select(Tool::Fill)),
        (Key::Escape, ToolAction::Cancel),
    ] {
        assert_eq!(tool_action_for_key(key, Modifiers::NONE), Some(accion));
    }
}

#[test]
fn fila_1_action_for_key_y_file_action_for_key_dan_none_para_coma() {
    let mut combinaciones = modificadores_de_la_fila_1();
    combinaciones.push(Modifiers::NONE);
    combinaciones.push(Modifiers::COMMAND | Modifiers::SHIFT);
    let key = Key::Comma;
    for modifiers in combinaciones {
        assert_eq!(
            action_for_key(key, modifiers),
            None,
            "action_for_key {key:?} con {modifiers:?}"
        );
        assert_eq!(
            file_action_for_key(key, modifiers),
            None,
            "file_action_for_key {key:?} con {modifiers:?}"
        );
    }
}

#[test]
fn fila_1_los_nombres_de_las_herramientas_nuevas() {
    assert_eq!(Tool::Dropper.label(), "cuentagotas");
    assert_eq!(Tool::Eraser.label(), "borrador");
}

#[test]
fn fila_1_texto_de_estado_del_borrador_y_del_cuentagotas() {
    assert_eq!(
        status_text(Tool::Eraser, Some("transparente")),
        "borrador · transparente"
    );
    assert_eq!(status_text(Tool::Eraser, None), "borrador");
    assert_eq!(
        status_text(Tool::Dropper, Some("color 3 #ff0000")),
        "cuentagotas · color 3 #ff0000"
    );
    assert_eq!(status_text(Tool::Dropper, None), "cuentagotas");
}

#[test]
fn fila_1_lapiz_linea_relleno_y_cuentagotas_dan_el_color_activo_en_rgba() {
    let image = imagen_rgba();
    let slots = SlotColors::defaults(Mode::Rgba);
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        assert_eq!(
            tool_color_label(tool, &image, ColorChoice::Slot(3), &slots),
            Some("color 3 #ff0000".to_string()),
            "{tool:?}"
        );
    }
}

#[test]
fn fila_1_el_color_activo_de_una_indexada_es_la_entrada_elegida() {
    let image = indexada_de_16();
    let slots = SlotColors::defaults(Mode::Indexed);
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        assert_eq!(
            tool_color_label(tool, &image, ColorChoice::Entry(12), &slots),
            Some("índice 12 #ff8800".to_string()),
            "{tool:?}"
        );
    }
}

#[test]
fn fila_1_el_color_activo_usa_las_ranuras_que_se_le_pasan() {
    let image = imagen_rgba();
    let mut slots = SlotColors::defaults(Mode::Rgba);
    assert!(slots.set(3, rgba(10, 20, 30, 128)));
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        let etiqueta = tool_color_label(tool, &image, ColorChoice::Slot(3), &slots);
        assert_eq!(
            etiqueta,
            choice_label_with(&image, ColorChoice::Slot(3), &slots),
            "{tool:?}"
        );
        assert_eq!(etiqueta, Some("color 3 #0a141e80".to_string()), "{tool:?}");
    }
}

#[test]
fn fila_1_con_una_eleccion_que_no_vale_las_otras_herramientas_dan_none() {
    // `choice_label_with` da `None` para una ranura 0 y para una entrada en una imagen sin paleta.
    let image = imagen_rgba();
    let slots = SlotColors::defaults(Mode::Rgba);
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        for choice in [ColorChoice::Slot(0), ColorChoice::Entry(0)] {
            assert_eq!(
                tool_color_label(tool, &image, choice, &slots),
                choice_label_with(&image, choice, &slots),
                "{tool:?} con {choice:?}"
            );
            assert_eq!(
                tool_color_label(tool, &image, choice, &slots),
                None,
                "{tool:?} con {choice:?}"
            );
        }
    }
}

#[test]
fn fila_1_el_borrador_da_lo_que_escribe_y_no_el_color_activo() {
    let casos = [
        (imagen_rgba(), "transparente"),
        (imagen_gris_alfa(), "transparente"),
        (indexada_con_transparente(), "transparente"),
        (indexada_sin_transparente(), "sin índice transparente"),
        (imagen_gris(), "sin transparencia"),
    ];
    for (image, esperado) in casos {
        let slots = SlotColors::defaults(image.mode());
        for choice in [
            ColorChoice::Slot(3),
            ColorChoice::Slot(9),
            ColorChoice::Entry(1),
        ] {
            assert_eq!(
                tool_color_label(Tool::Eraser, &image, choice, &slots),
                Some(esperado.to_string()),
                "{:?} con {choice:?}",
                image.mode()
            );
        }
    }
}

#[test]
fn fila_1_el_borrador_no_depende_de_que_la_eleccion_valga() {
    // Spec: el borrador da `Some(eraser_label(image))` siempre, valga o no la elección.
    let image = imagen_rgba();
    let slots = SlotColors::defaults(Mode::Rgba);
    for choice in [ColorChoice::Slot(0), ColorChoice::Entry(200)] {
        assert_eq!(
            tool_color_label(Tool::Eraser, &image, choice, &slots),
            Some("transparente".to_string()),
            "{choice:?}"
        );
    }
}

#[test]
fn fila_1_la_linea_completa_del_borrador() {
    let image = imagen_rgba();
    let slots = SlotColors::defaults(Mode::Rgba);
    let etiqueta = tool_color_label(Tool::Eraser, &image, ColorChoice::Slot(3), &slots);
    assert_eq!(
        status_text(Tool::Eraser, etiqueta.as_deref()),
        "borrador · transparente"
    );
}

// Fila 2: `eraser_pixel` y sus textos.

#[test]
fn fila_2_eraser_pixel_en_rgba_y_en_gris_con_alfa() {
    assert_eq!(
        eraser_pixel(&imagen_rgba()),
        Ok(Pixel::Rgba(Rgba::TRANSPARENT))
    );
    assert_eq!(
        eraser_pixel(&imagen_gris_alfa()),
        Ok(Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );
}

#[test]
fn fila_2_eraser_pixel_en_indexada_con_transparente_3() {
    assert_eq!(
        eraser_pixel(&indexada_con_transparente()),
        Ok(Pixel::Indexed(3))
    );
}

#[test]
fn fila_2_eraser_pixel_en_los_modos_que_no_pueden_borrar() {
    assert_eq!(
        eraser_pixel(&indexada_sin_transparente()),
        Err(EraseError::NoTransparentIndex)
    );
    assert_eq!(
        eraser_pixel(&imagen_gris()),
        Err(EraseError::NoTransparency)
    );
}

#[test]
fn fila_2_eraser_pixel_sigue_el_indice_transparente_de_la_paleta() {
    for t in 0..5u8 {
        let mut paleta = paleta_de_5();
        paleta.set_transparent_index(Some(t)).expect("existe");
        let image = Image::new_indexed(4, 4, paleta).expect("imagen válida");
        assert_eq!(eraser_pixel(&image), Ok(Pixel::Indexed(t)));
    }
}

#[test]
fn fila_2_los_textos_de_los_errores() {
    assert_eq!(
        EraseError::NoTransparentIndex.to_string(),
        "esta imagen no tiene índice transparente"
    );
    assert_eq!(
        EraseError::NoTransparency.to_string(),
        "el modo gris no tiene transparencia"
    );
}

#[test]
fn fila_2_los_tipos_derivan_lo_que_pide_la_spec() {
    fn copia<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
    copia::<EraseError>();
    copia::<Picked>();
    assert_ne!(EraseError::NoTransparentIndex, EraseError::NoTransparency);
    assert_ne!(Picked::Entry(1), Picked::Entry(2));
    assert_ne!(Picked::Entry(1), Picked::Color(Rgba::TRANSPARENT));
    let _ = format!("{:?} {:?}", EraseError::NoTransparency, Picked::Entry(0));
}

#[test]
fn eraser_label_por_modo() {
    assert_eq!(eraser_label(&imagen_rgba()), "transparente");
    assert_eq!(eraser_label(&imagen_gris_alfa()), "transparente");
    assert_eq!(eraser_label(&indexada_con_transparente()), "transparente");
    assert_eq!(
        eraser_label(&indexada_sin_transparente()),
        "sin índice transparente"
    );
    assert_eq!(eraser_label(&imagen_gris()), "sin transparencia");
}

// `is_erasable`.

#[test]
fn is_erasable_en_rgba_omite_el_alfa_0_aunque_el_rgb_oculto_no_sea_0() {
    let image = imagen_rgba();
    let image = con_pixel(image, 1, 1, Pixel::Rgba(rgba(255, 0, 0, 0)));
    let image = con_pixel(image, 2, 1, Pixel::Rgba(rgba(0, 0, 0, 1)));
    let image = con_pixel(image, 3, 1, Pixel::Rgba(rgba(0, 0, 255, 255)));
    assert!(!is_erasable(&image, 0, 0), "(0, 0, 0, 0)");
    assert!(!is_erasable(&image, 1, 1), "(255, 0, 0, 0)");
    assert!(is_erasable(&image, 2, 1), "alfa 1");
    assert!(is_erasable(&image, 3, 1), "opaco");
}

#[test]
fn is_erasable_en_gris_con_alfa_mira_solo_el_alfa() {
    let image = imagen_gris_alfa();
    let image = con_pixel(image, 1, 0, Pixel::GrayAlpha { gray: 7, alpha: 0 });
    let image = con_pixel(image, 2, 0, Pixel::GrayAlpha { gray: 0, alpha: 1 });
    let image = con_pixel(
        image,
        3,
        0,
        Pixel::GrayAlpha {
            gray: 0,
            alpha: 255,
        },
    );
    assert!(!is_erasable(&image, 0, 0), "{{ 0, 0 }}");
    assert!(!is_erasable(&image, 1, 0), "gris 7 con alfa 0");
    assert!(is_erasable(&image, 2, 0));
    assert!(is_erasable(&image, 3, 0));
}

#[test]
fn is_erasable_en_indexada_con_transparente_omite_solo_ese_indice() {
    let image = indexada_con_transparente();
    let image = con_pixel(image, 1, 0, Pixel::Indexed(3));
    let image = con_pixel(image, 2, 0, Pixel::Indexed(4));
    assert!(is_erasable(&image, 0, 0), "Indexed(0)");
    assert!(!is_erasable(&image, 1, 0), "Indexed(3) es el transparente");
    assert!(is_erasable(&image, 2, 0), "Indexed(4)");
}

#[test]
fn is_erasable_es_falso_fuera_de_la_imagen_en_todos_los_modos() {
    let opaca = |image: Image, pixel| {
        let (w, h) = (image.width(), image.height());
        let mut image = image;
        for y in 0..h {
            for x in 0..w {
                image.set_pixel(x, y, pixel).expect("dentro");
            }
        }
        image
    };
    let imagenes = [
        opaca(imagen_rgba(), Pixel::Rgba(rgba(1, 2, 3, 255))),
        opaca(
            imagen_gris_alfa(),
            Pixel::GrayAlpha {
                gray: 9,
                alpha: 255,
            },
        ),
        opaca(indexada_con_transparente(), Pixel::Indexed(0)),
    ];
    for image in &imagenes {
        assert!(is_erasable(image, 7, 7), "{:?}: dentro", image.mode());
        for (x, y) in [(8, 0), (0, 8), (8, 8), (u32::MAX, 0), (0, u32::MAX)] {
            assert!(
                !is_erasable(image, x, y),
                "{:?}: ({x}, {y}) fuera",
                image.mode()
            );
        }
    }
}

// Spec: «el borrador cambiaría ese píxel». En gris y en indexada sin transparente no escribe nada,
// así que no cambia ninguno. La página no lo dice con estas palabras.
#[test]
fn is_erasable_es_falso_donde_el_borrador_no_puede_escribir() {
    let gris = con_pixel(imagen_gris(), 1, 1, Pixel::Gray(200));
    let sin = con_pixel(indexada_sin_transparente(), 1, 1, Pixel::Indexed(2));
    for (x, y) in [(0, 0), (1, 1), (7, 7)] {
        assert!(!is_erasable(&gris, x, y), "gris ({x}, {y})");
        assert!(
            !is_erasable(&sin, x, y),
            "indexada sin transparente ({x}, {y})"
        );
    }
}

// `picked_at` y `pick_notice`.

#[test]
fn picked_at_en_indexada_da_la_entrada_tambien_la_transparente() {
    let image = indexada_con_transparente();
    let image = con_pixel(image, 1, 0, Pixel::Indexed(3));
    let image = con_pixel(image, 2, 0, Pixel::Indexed(4));
    assert_eq!(picked_at(&image, 0, 0), Some(Picked::Entry(0)));
    assert_eq!(picked_at(&image, 1, 0), Some(Picked::Entry(3)));
    assert_eq!(picked_at(&image, 2, 0), Some(Picked::Entry(4)));
}

#[test]
fn picked_at_en_rgba_da_el_valor_exacto_con_su_alfa() {
    let image = imagen_rgba();
    let image = con_pixel(image, 1, 1, Pixel::Rgba(rgba(10, 20, 30, 128)));
    let image = con_pixel(image, 2, 1, Pixel::Rgba(rgba(10, 20, 30, 0)));
    let image = con_pixel(image, 3, 1, Pixel::Rgba(rgba(10, 20, 30, 255)));
    assert_eq!(
        picked_at(&image, 1, 1),
        Some(Picked::Color(rgba(10, 20, 30, 128)))
    );
    assert_eq!(
        picked_at(&image, 2, 1),
        Some(Picked::Color(rgba(10, 20, 30, 0)))
    );
    assert_eq!(
        picked_at(&image, 3, 1),
        Some(Picked::Color(rgba(10, 20, 30, 255)))
    );
}

#[test]
fn picked_at_en_gris_da_r_igual_g_igual_b_y_alfa_255() {
    let image = con_pixel(imagen_gris(), 4, 4, Pixel::Gray(96));
    assert_eq!(
        picked_at(&image, 4, 4),
        Some(Picked::Color(rgba(96, 96, 96, 255)))
    );
    assert_eq!(
        picked_at(&image, 0, 0),
        Some(Picked::Color(rgba(0, 0, 0, 255)))
    );
}

#[test]
fn picked_at_en_gris_con_alfa_conserva_el_alfa_incluido_el_0() {
    let image = imagen_gris_alfa();
    let image = con_pixel(
        image,
        1,
        0,
        Pixel::GrayAlpha {
            gray: 7,
            alpha: 200,
        },
    );
    let image = con_pixel(image, 2, 0, Pixel::GrayAlpha { gray: 7, alpha: 0 });
    assert_eq!(
        picked_at(&image, 1, 0),
        Some(Picked::Color(rgba(7, 7, 7, 200)))
    );
    assert_eq!(
        picked_at(&image, 2, 0),
        Some(Picked::Color(rgba(7, 7, 7, 0)))
    );
}

#[test]
fn picked_at_fuera_de_la_imagen_es_none_en_todos_los_modos() {
    let imagenes = [
        imagen_rgba(),
        imagen_gris(),
        imagen_gris_alfa(),
        indexada_con_transparente(),
        indexada_sin_transparente(),
    ];
    for image in &imagenes {
        for (x, y) in [(8, 0), (0, 8), (8, 8), (u32::MAX, 0), (0, u32::MAX)] {
            assert_eq!(
                picked_at(image, x, y),
                None,
                "{:?}: ({x}, {y})",
                image.mode()
            );
        }
        assert!(picked_at(image, 7, 7).is_some(), "{:?}", image.mode());
    }
}

#[test]
fn pick_notice_solo_avisa_de_un_color_con_alfa_0() {
    assert_eq!(
        pick_notice(&Picked::Color(rgba(10, 20, 30, 0))),
        Some("píxel transparente copiado tal cual (#0a141e00)".to_string())
    );
    assert_eq!(
        pick_notice(&Picked::Color(rgba(7, 7, 7, 0))),
        Some("píxel transparente copiado tal cual (#07070700)".to_string())
    );
    assert_eq!(pick_notice(&Picked::Color(rgba(10, 20, 30, 128))), None);
    assert_eq!(pick_notice(&Picked::Color(rgba(10, 20, 30, 1))), None);
    assert_eq!(pick_notice(&Picked::Color(rgba(10, 20, 30, 255))), None);
}

#[test]
fn pick_notice_no_avisa_de_una_entrada_ni_de_la_transparente() {
    for i in [0, 1, 3, 255] {
        assert_eq!(pick_notice(&Picked::Entry(i)), None, "Entry({i})");
    }
}

// Propiedad 3: teclas.

fn modificadores(bits: u8) -> Modifiers {
    Modifiers {
        alt: bits & 1 != 0,
        ctrl: bits & 2 != 0,
        shift: bits & 4 != 0,
        mac_cmd: bits & 8 != 0,
        command: bits & 16 != 0,
    }
}

/// Lo que debe dar `tool_action_for_key`, escrito aparte de la implementación.
fn accion_esperada(key: Key, modifiers: Modifiers) -> Option<ToolAction> {
    let sin_modificadores = !(modifiers.alt
        || modifiers.ctrl
        || modifiers.shift
        || modifiers.mac_cmd
        || modifiers.command);
    if !sin_modificadores {
        return None;
    }
    match key {
        Key::D => Some(ToolAction::Select(Tool::Pencil)),
        Key::V => Some(ToolAction::Select(Tool::Line)),
        Key::F => Some(ToolAction::Select(Tool::Fill)),
        Key::Comma => Some(ToolAction::Select(Tool::Dropper)),
        Key::K => Some(ToolAction::Select(Tool::Eraser)),
        Key::Escape => Some(ToolAction::Cancel),
        _ => None,
    }
}

fn comprueba_teclas(key: Key, modifiers: Modifiers) -> Result<(), TestCaseError> {
    let accion = tool_action_for_key(key, modifiers);
    prop_assert_eq!(
        accion,
        accion_esperada(key, modifiers),
        "{:?} con {:?}",
        key,
        modifiers
    );
    if accion.is_some() {
        prop_assert!(
            matches!(
                key,
                Key::D | Key::V | Key::F | Key::Comma | Key::K | Key::Escape
            ),
            "{:?} no debería dar acción",
            key
        );
        prop_assert!(modifiers.is_none(), "{:?} con {:?}", key, modifiers);
    }
    if matches!(key, Key::Comma) {
        prop_assert_eq!(
            action_for_key(key, modifiers),
            None,
            "action_for_key {:?} con {:?}",
            key,
            modifiers
        );
        prop_assert_eq!(
            file_action_for_key(key, modifiers),
            None,
            "file_action_for_key {:?} con {:?}",
            key,
            modifiers
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn propiedad_3_teclas_por_muestreo(
        key in prop::sample::select(Key::ALL.to_vec()),
        bits in 0u8..32,
    ) {
        comprueba_teclas(key, modificadores(bits))?;
    }
}

#[test]
fn propiedad_3_teclas_exhaustiva_key_all_por_las_32_combinaciones() {
    assert!(!Key::ALL.is_empty());
    for &key in Key::ALL {
        for bits in 0..32 {
            let modifiers = modificadores(bits);
            if let Err(e) = comprueba_teclas(key, modifiers) {
                panic!("{e}");
            }
        }
    }
}

#[test]
fn propiedad_3_cada_herramienta_tiene_su_tecla_sin_modificadores() {
    // Las cinco herramientas se alcanzan, cada una con una sola tecla distinta (el Borrador, con `k`).
    let mut alcanzadas = Vec::new();
    for &key in Key::ALL {
        if let Some(ToolAction::Select(tool)) = tool_action_for_key(key, Modifiers::NONE) {
            alcanzadas.push((tool, key));
        }
    }
    assert_eq!(alcanzadas.len(), TODAS.len(), "{alcanzadas:?}");
    for tool in TODAS {
        assert_eq!(
            alcanzadas.iter().filter(|(t, _)| *t == tool).count(),
            1,
            "{tool:?}"
        );
    }
}

// Fila 14: regla de la entrada de alfa 0.
//
// Paleta de 5: `[azul, rojo, (0,255,0,α0), azul claro, (9,9,9,α0)]`. El valor de borrado en
// indexado es, por orden: `transparent_index` si está fijado; si no, la primera entrada cuyo alfa
// guardado es 0; si no, `Err(NoTransparentIndex)`. Se recalcula siempre, no se guarda nada.

const ENTRADA_2_ALFA_0: Rgba = Rgba::new(0, 255, 0, 0);
const ENTRADA_4_ALFA_0: Rgba = Rgba::new(9, 9, 9, 0);

/// La paleta de la fila 14, sin `transparent_index`: las entradas 2 y 4 con alfa 0.
fn paleta_fila_14() -> Palette {
    Palette::from_colors([
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(255, 0, 0),
        ENTRADA_2_ALFA_0,
        Rgba::opaque(100, 150, 255),
        ENTRADA_4_ALFA_0,
    ])
    .expect("paleta válida")
}

fn imagen_fila_14(paleta: Palette) -> Image {
    Image::new_indexed(8, 8, paleta).expect("imagen válida")
}

/// Caso (a): sin `transparent_index`, con las entradas 2 y 4 de alfa 0.
fn caso_14a() -> Image {
    imagen_fila_14(paleta_fila_14())
}

/// Caso (b): `transparent_index = Some(3)` y las entradas 2 y 4 con alfa 0.
fn caso_14b() -> Image {
    let mut paleta = paleta_fila_14();
    paleta.set_transparent_index(Some(3)).expect("existe");
    imagen_fila_14(paleta)
}

/// Caso (c): `transparent_index = Some(4)` y la entrada 2 con alfa 0 (la 4, con el alfa que se pida).
fn caso_14c(alfa_de_la_4: u8) -> Image {
    let mut paleta = paleta_fila_14();
    paleta
        .set(4, Rgba::new(9, 9, 9, alfa_de_la_4))
        .expect("existe");
    paleta.set_transparent_index(Some(4)).expect("existe");
    imagen_fila_14(paleta)
}

/// Caso (d): sin `transparent_index` y con solo la entrada 4 de alfa 0 (la 2 es opaca).
fn caso_14d() -> Image {
    let mut paleta = paleta_fila_14();
    paleta.set(2, Rgba::opaque(0, 255, 0)).expect("existe");
    imagen_fila_14(paleta)
}

#[test]
fn fila_14a_sin_transparent_index_gana_la_primera_entrada_de_alfa_0_no_la_ultima() {
    let image = caso_14a();
    assert_eq!(image.palette().unwrap().transparent_index(), None);
    assert_eq!(eraser_pixel(&image), Ok(Pixel::Indexed(2)));
    assert_eq!(eraser_label(&image), "transparente (entrada 2)");
}

#[test]
fn fila_14a_tool_color_label_y_status_text_del_borrador() {
    let image = caso_14a();
    let slots = SlotColors::defaults(Mode::Indexed);
    // El borrador no usa la elección: valga o no, el texto es el mismo.
    for choice in [
        ColorChoice::Entry(0),
        ColorChoice::Entry(3),
        ColorChoice::Slot(9),
    ] {
        let etiqueta = tool_color_label(Tool::Eraser, &image, choice, &slots);
        assert_eq!(
            etiqueta,
            Some("transparente (entrada 2)".to_string()),
            "{choice:?}"
        );
        assert_eq!(
            status_text(Tool::Eraser, etiqueta.as_deref()),
            "borrador · transparente (entrada 2)",
            "{choice:?}"
        );
    }
}

#[test]
fn fila_14a_las_otras_herramientas_siguen_dando_el_color_activo() {
    // La regla nueva solo toca al borrador.
    let image = caso_14a();
    let slots = SlotColors::defaults(Mode::Indexed);
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        assert_eq!(
            tool_color_label(tool, &image, ColorChoice::Entry(3), &slots),
            choice_label_with(&image, ColorChoice::Entry(3), &slots),
            "{tool:?}"
        );
    }
}

#[test]
fn fila_14b_con_transparent_index_gana_aunque_una_entrada_anterior_tenga_alfa_0() {
    let image = caso_14b();
    let paleta = image.palette().unwrap();
    assert_eq!(paleta.get(2).unwrap().a, 0, "la 2 es anterior y de alfa 0");
    assert_eq!(paleta.get(4).unwrap().a, 0);
    assert_eq!(eraser_pixel(&image), Ok(Pixel::Indexed(3)));
    // Sin `(entrada N)`: solo se usa la regla 2 cuando no hay `transparent_index`.
    assert_eq!(eraser_label(&image), "transparente");
    let slots = SlotColors::defaults(Mode::Indexed);
    assert_eq!(
        tool_color_label(Tool::Eraser, &image, ColorChoice::Entry(0), &slots),
        Some("transparente".to_string())
    );
    assert_eq!(
        status_text(
            Tool::Eraser,
            Some(eraser_label(&image).to_string().as_str())
        ),
        "borrador · transparente"
    );
}

#[test]
fn fila_14b_con_transparent_index_en_una_entrada_opaca_tambien_gana() {
    // El índice fijado manda aunque su alfa guardado no sea 0 (`resolved` lo pone a 0).
    let mut paleta = paleta_fila_14();
    paleta.set_transparent_index(Some(0)).expect("existe");
    let image = imagen_fila_14(paleta);
    assert_eq!(eraser_pixel(&image), Ok(Pixel::Indexed(0)));
    assert_eq!(eraser_label(&image), "transparente");
}

#[test]
fn fila_14c_con_transparent_index_4_gana_la_4_no_la_2() {
    for alfa_de_la_4 in [0, 255] {
        let image = caso_14c(alfa_de_la_4);
        assert_eq!(image.palette().unwrap().get(2).unwrap().a, 0);
        assert_eq!(
            eraser_pixel(&image),
            Ok(Pixel::Indexed(4)),
            "alfa guardado de la 4 = {alfa_de_la_4}"
        );
        assert_eq!(eraser_label(&image), "transparente");
    }
}

#[test]
fn fila_14d_sin_transparent_index_y_solo_la_entrada_4_de_alfa_0() {
    let image = caso_14d();
    assert_eq!(image.palette().unwrap().transparent_index(), None);
    assert_eq!(eraser_pixel(&image), Ok(Pixel::Indexed(4)));
    assert_eq!(eraser_label(&image), "transparente (entrada 4)");
    let slots = SlotColors::defaults(Mode::Indexed);
    let etiqueta = tool_color_label(Tool::Eraser, &image, ColorChoice::Entry(1), &slots);
    assert_eq!(etiqueta.as_deref(), Some("transparente (entrada 4)"));
    assert_eq!(
        status_text(Tool::Eraser, etiqueta.as_deref()),
        "borrador · transparente (entrada 4)"
    );
}

/// Cambia el color guardado de una entrada de la imagen principal con un comando.
fn edita_entrada(doc: &mut Document, index: u8, color: Rgba) {
    let image = doc.main_image();
    let comando = Command::SetPaletteColor {
        image,
        index,
        color,
    };
    assert_eq!(doc.apply(comando), Ok(true));
}

#[test]
fn fila_14e_tras_editar_la_paleta_el_valor_de_borrado_se_calcula_de_nuevo() {
    let mut doc = Document::new(caso_14a());
    let id = doc.main_image();
    assert_eq!(eraser_pixel(doc.image(id).unwrap()), Ok(Pixel::Indexed(2)));

    // La entrada 2 pasa a alfa 255: queda la 4.
    edita_entrada(&mut doc, 2, Rgba::opaque(0, 255, 0));
    let image = doc.image(id).unwrap();
    assert_eq!(eraser_pixel(image), Ok(Pixel::Indexed(4)));
    assert_eq!(eraser_label(image), "transparente (entrada 4)");

    // Y si ya no queda ninguna: el aviso de siempre.
    edita_entrada(&mut doc, 4, Rgba::opaque(9, 9, 9));
    let image = doc.image(id).unwrap();
    assert_eq!(eraser_pixel(image), Err(EraseError::NoTransparentIndex));
    assert_eq!(eraser_label(image), "sin índice transparente");
    let slots = SlotColors::defaults(Mode::Indexed);
    assert_eq!(
        tool_color_label(Tool::Eraser, image, ColorChoice::Entry(0), &slots),
        Some("sin índice transparente".to_string())
    );
    assert_eq!(
        status_text(Tool::Eraser, Some(eraser_label(image).to_string().as_str())),
        "borrador · sin índice transparente"
    );

    // Con undo vuelve la regla 2 con la entrada que se recupera, sin nada guardado.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(eraser_pixel(doc.image(id).unwrap()), Ok(Pixel::Indexed(4)));
}

#[test]
fn fila_14e_una_sola_entrada_de_alfa_0_que_se_vuelve_opaca_no_deja_ninguna() {
    let mut doc = Document::new(caso_14d());
    let id = doc.main_image();
    assert_eq!(eraser_pixel(doc.image(id).unwrap()), Ok(Pixel::Indexed(4)));
    edita_entrada(&mut doc, 4, Rgba::opaque(9, 9, 9));
    let image = doc.image(id).unwrap();
    assert_eq!(eraser_pixel(image), Err(EraseError::NoTransparentIndex));
    assert_eq!(eraser_label(image), "sin índice transparente");
}

#[test]
fn fila_14e_fijar_transparent_index_cambia_el_valor_en_el_siguiente_calculo() {
    let mut doc = Document::new(caso_14a());
    let id = doc.main_image();
    assert_eq!(eraser_pixel(doc.image(id).unwrap()), Ok(Pixel::Indexed(2)));
    let comando = Command::SetTransparentIndex {
        image: id,
        index: Some(3),
    };
    assert_eq!(doc.apply(comando), Ok(true));
    let image = doc.image(id).unwrap();
    assert_eq!(eraser_pixel(image), Ok(Pixel::Indexed(3)));
    assert_eq!(eraser_label(image), "transparente");
}

#[test]
fn fila_14_una_paleta_con_alfa_parcial_no_cuenta_como_alfa_0() {
    // Solo el alfa guardado exactamente 0 vale: alfa 1 no.
    let paleta = Palette::from_colors([
        Rgba::opaque(0, 0, 255),
        Rgba::new(1, 2, 3, 1),
        Rgba::new(1, 2, 3, 128),
    ])
    .expect("paleta válida");
    let image = Image::new_indexed(4, 4, paleta).expect("imagen válida");
    assert_eq!(eraser_pixel(&image), Err(EraseError::NoTransparentIndex));
    assert_eq!(eraser_label(&image), "sin índice transparente");
}

#[test]
fn fila_14_los_demas_modos_no_cambian_con_la_regla_nueva() {
    assert_eq!(eraser_label(&imagen_rgba()), "transparente");
    assert_eq!(eraser_label(&imagen_gris_alfa()), "transparente");
    assert_eq!(eraser_label(&imagen_gris()), "sin transparencia");
}

// `is_erasable` con la regla nueva (decisión 2: «ya transparente» es el valor resuelto con alfa 0).

#[test]
fn is_erasable_en_indexada_sin_transparent_index_omite_las_entradas_de_alfa_0() {
    let image = caso_14a();
    let image = con_pixel(image, 1, 0, Pixel::Indexed(2));
    let image = con_pixel(image, 2, 0, Pixel::Indexed(4));
    let image = con_pixel(image, 3, 0, Pixel::Indexed(1));
    assert!(is_erasable(&image, 0, 0), "Indexed(0), azul");
    assert!(!is_erasable(&image, 1, 0), "Indexed(2), alfa 0");
    assert!(!is_erasable(&image, 2, 0), "Indexed(4), alfa 0");
    assert!(is_erasable(&image, 3, 0), "Indexed(1), rojo");
}

#[test]
fn is_erasable_con_transparent_index_omite_tambien_otras_entradas_de_alfa_0() {
    // Con `transparent_index = 3`: el índice 3 y las entradas 2 y 4 (alfa 0 guardado) ya son
    // transparentes; no pasan a `Indexed(3)`.
    let image = caso_14b();
    let image = con_pixel(image, 1, 0, Pixel::Indexed(2));
    let image = con_pixel(image, 2, 0, Pixel::Indexed(4));
    let image = con_pixel(image, 3, 0, Pixel::Indexed(3));
    let image = con_pixel(image, 4, 0, Pixel::Indexed(1));
    assert!(is_erasable(&image, 0, 0), "Indexed(0)");
    assert!(!is_erasable(&image, 1, 0), "Indexed(2)");
    assert!(!is_erasable(&image, 2, 0), "Indexed(4)");
    assert!(!is_erasable(&image, 3, 0), "Indexed(3), el transparente");
    assert!(is_erasable(&image, 4, 0), "Indexed(1)");
}

#[test]
fn is_erasable_con_transparent_index_fijado_a_una_entrada_opaca_omite_su_indice() {
    // El valor resuelto del índice transparente tiene alfa 0 aunque el guardado no.
    let image = caso_14c(255);
    let image = con_pixel(image, 1, 0, Pixel::Indexed(4));
    let image = con_pixel(image, 2, 0, Pixel::Indexed(2));
    assert!(
        !is_erasable(&image, 1, 0),
        "Indexed(4), resuelto con alfa 0"
    );
    assert!(!is_erasable(&image, 2, 0), "Indexed(2), alfa 0 guardado");
    assert!(is_erasable(&image, 0, 0), "Indexed(0)");
}

#[test]
fn is_erasable_sin_ninguna_entrada_de_alfa_0_ni_indice_sigue_siendo_falso() {
    let image = caso_14d();
    let mut paleta = image.palette().unwrap().clone();
    paleta.set(4, Rgba::opaque(9, 9, 9)).expect("existe");
    let image = imagen_fila_14(paleta);
    for (x, y) in [(0, 0), (3, 3)] {
        assert!(!is_erasable(&image, x, y), "({x}, {y})");
    }
}
