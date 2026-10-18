//! Tests de la lógica pura de cuentagotas y borrador y de sus teclas (specs/hito-2-cuentagotas-
//! borrador.md, «Casos límite» fila 1 en lo que toca a `px-ui` y fila 2, y propiedad 3).
//!
//! `px_ui::pick` no usa egui: `eraser_pixel`, `eraser_label`, `is_erasable`, `picked_at` y
//! `pick_notice` se prueban con imágenes pequeñas hechas a mano, una por modo. Las filas 3 a 8 y
//! las propiedades 1, 2 y 4 están en `tests/eraser.rs` y `tests/dropper.rs`; la fila 10, en
//! `tests/eraser_canvas.rs`.
//!
//! La propiedad 3 recorre todo `Key::ALL` por las 32 combinaciones de modificadores: se escribe
//! como `proptest!` (muestreo) y, además, como recorrido exhaustivo, que es más fuerte. Sustituye
//! a la propiedad 5 de la unidad 7 (`tests/tool.rs`), cuyo oráculo no conocía `I` ni `E`.

use egui::{Key, Modifiers};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Image, Mode, Palette, Pixel, Rgba};
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
fn fila_1_i_es_cuentagotas_y_e_es_borrador_sin_modificadores() {
    assert_eq!(
        tool_action_for_key(Key::I, Modifiers::NONE),
        Some(ToolAction::Select(Tool::Dropper))
    );
    assert_eq!(
        tool_action_for_key(Key::E, Modifiers::NONE),
        Some(ToolAction::Select(Tool::Eraser))
    );
}

#[test]
fn fila_1_con_cualquier_modificador_i_y_e_no_hacen_nada() {
    for modifiers in modificadores_de_la_fila_1() {
        for key in [Key::I, Key::E] {
            assert_eq!(
                tool_action_for_key(key, modifiers),
                None,
                "{key:?} con {modifiers:?}"
            );
        }
    }
}

#[test]
fn fila_1_p_l_f_y_escape_siguen_igual() {
    for (key, accion) in [
        (Key::P, ToolAction::Select(Tool::Pencil)),
        (Key::L, ToolAction::Select(Tool::Line)),
        (Key::F, ToolAction::Select(Tool::Fill)),
        (Key::Escape, ToolAction::Cancel),
    ] {
        assert_eq!(tool_action_for_key(key, Modifiers::NONE), Some(accion));
    }
}

#[test]
fn fila_1_action_for_key_y_file_action_for_key_dan_none_para_i_y_e() {
    let mut combinaciones = modificadores_de_la_fila_1();
    combinaciones.push(Modifiers::NONE);
    combinaciones.push(Modifiers::COMMAND | Modifiers::SHIFT);
    for modifiers in combinaciones {
        for key in [Key::I, Key::E] {
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
        Key::P => Some(ToolAction::Select(Tool::Pencil)),
        Key::L => Some(ToolAction::Select(Tool::Line)),
        Key::F => Some(ToolAction::Select(Tool::Fill)),
        Key::I => Some(ToolAction::Select(Tool::Dropper)),
        Key::E => Some(ToolAction::Select(Tool::Eraser)),
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
                Key::P | Key::L | Key::F | Key::I | Key::E | Key::Escape
            ),
            "{:?} no debería dar acción",
            key
        );
        prop_assert!(modifiers.is_none(), "{:?} con {:?}", key, modifiers);
    }
    if matches!(key, Key::I | Key::E) {
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
    // Las cinco herramientas se alcanzan, cada una con una sola tecla distinta.
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
