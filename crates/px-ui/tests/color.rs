//! Tests de `color` (specs/hito-2-lapiz-deshacer.md, sección 4; fila 11 y propiedad 5):
//! `SLOTS`, `pixel_for_slot(image, slot)` y `slot_label(image, slot)`.
//!
//! Las tablas de la spec se comprueban a mano, ranura por ranura; la propiedad 5 comprueba que
//! el color nunca es de otro modo ni de fuera de la paleta, para cualquier ranura de 0 a 255.

use proptest::prelude::*;
use px_core::{Document, Image, Mode, Palette, Pixel, Rgba};
use px_ui::color::{SLOTS, pixel_for_slot, slot_label};
use px_ui::stroke::Stroke;

fn imagen(mode: Mode) -> Image {
    Image::new(4, 4, mode).unwrap()
}

fn indexada(colores: usize) -> Image {
    let paleta =
        Palette::from_colors((0..colores).map(|i| Rgba::opaque((i % 20) as u8 * 10, 0, 0)))
            .unwrap();
    Image::new_indexed(4, 4, paleta).unwrap()
}

fn rgba(r: u8, g: u8, b: u8) -> Pixel {
    Pixel::Rgba(Rgba::opaque(r, g, b))
}

#[test]
fn hay_nueve_ranuras() {
    assert_eq!(SLOTS, 9);
}

#[test]
fn fila_11_ejemplos_de_la_spec() {
    let color = imagen(Mode::Rgba);
    assert_eq!(pixel_for_slot(&color, 1), Some(rgba(0, 0, 0)));
    assert_eq!(
        pixel_for_slot(&color, 9),
        Some(Pixel::Rgba(Rgba::new(0, 0, 0, 0)))
    );

    let gris = imagen(Mode::Gray);
    assert_eq!(pixel_for_slot(&gris, 8), Some(Pixel::Gray(224)));
    assert_eq!(pixel_for_slot(&gris, 9), Some(Pixel::Gray(255)));

    let gris_alfa = imagen(Mode::GrayAlpha);
    assert_eq!(
        pixel_for_slot(&gris_alfa, 9),
        Some(Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );

    // Paleta de 3: las ranuras sobrantes se ignoran sin error.
    let tres = indexada(3);
    assert_eq!(pixel_for_slot(&tres, 3), Some(Pixel::Indexed(2)));
    assert_eq!(pixel_for_slot(&tres, 4), None);

    for image in [
        imagen(Mode::Rgba),
        imagen(Mode::Gray),
        imagen(Mode::GrayAlpha),
        indexada(9),
    ] {
        assert_eq!(pixel_for_slot(&image, 0), None, "{:?}", image.mode());
        assert_eq!(pixel_for_slot(&image, 10), None, "{:?}", image.mode());
    }
}

#[test]
fn rgba_tabla_completa() {
    let image = imagen(Mode::Rgba);
    let esperado = [
        rgba(0x00, 0x00, 0x00),
        rgba(0xff, 0xff, 0xff),
        rgba(0xff, 0x00, 0x00),
        rgba(0x00, 0xff, 0x00),
        rgba(0x00, 0x00, 0xff),
        rgba(0xff, 0xff, 0x00),
        rgba(0xff, 0x00, 0xff),
        rgba(0x00, 0xff, 0xff),
        Pixel::Rgba(Rgba::new(0, 0, 0, 0)),
    ];
    for (i, esperado) in esperado.into_iter().enumerate() {
        let slot = i as u8 + 1;
        assert_eq!(
            pixel_for_slot(&image, slot),
            Some(esperado),
            "ranura {slot}"
        );
    }
}

#[test]
fn gray_tabla_completa_y_la_nueve_es_un_gris_distinto() {
    let image = imagen(Mode::Gray);
    let mut vistos = Vec::new();
    for slot in 1..=9u8 {
        let esperado = Pixel::Gray(((u32::from(slot) - 1) * 32).min(255) as u8);
        assert_eq!(
            pixel_for_slot(&image, slot),
            Some(esperado),
            "ranura {slot}"
        );
        vistos.push(esperado);
    }
    assert_eq!(vistos[0], Pixel::Gray(0));
    assert_eq!(vistos[7], Pixel::Gray(224));
    assert_eq!(vistos[8], Pixel::Gray(255));
    for (i, a) in vistos.iter().enumerate() {
        for b in &vistos[i + 1..] {
            assert_ne!(a, b, "las ranuras de Gray son todas distintas");
        }
    }
}

#[test]
fn gray_alpha_tabla_completa() {
    let image = imagen(Mode::GrayAlpha);
    for slot in 1..=8u8 {
        let gray = ((u32::from(slot) - 1) * 32).min(255) as u8;
        assert_eq!(
            pixel_for_slot(&image, slot),
            Some(Pixel::GrayAlpha { gray, alpha: 255 }),
            "ranura {slot}"
        );
    }
    assert_eq!(
        pixel_for_slot(&image, 9),
        Some(Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );
}

#[test]
fn indexed_usa_el_indice_n_menos_uno_mientras_haya_paleta() {
    for colores in [1usize, 2, 5, 9, 10, 256] {
        let image = indexada(colores);
        for slot in 1..=9u8 {
            let esperado = if usize::from(slot) <= colores {
                Some(Pixel::Indexed(slot - 1))
            } else {
                None
            };
            assert_eq!(
                pixel_for_slot(&image, slot),
                esperado,
                "paleta de {colores}, ranura {slot}"
            );
        }
    }
}

#[test]
fn slot_label_con_los_formatos_de_la_spec() {
    let color = imagen(Mode::Rgba);
    assert_eq!(slot_label(&color, 3).as_deref(), Some("color 3 #ff0000"));
    assert_eq!(slot_label(&color, 1).as_deref(), Some("color 1 #000000"));
    assert_eq!(slot_label(&color, 6).as_deref(), Some("color 6 #ffff00"));
    assert_eq!(slot_label(&color, 8).as_deref(), Some("color 8 #00ffff"));
    assert_eq!(
        slot_label(&color, 9).as_deref(),
        Some("color 9 transparente")
    );

    let paleta = indexada(3);
    assert_eq!(slot_label(&paleta, 2).as_deref(), Some("color 2 índice 1"));
    assert_eq!(slot_label(&paleta, 1).as_deref(), Some("color 1 índice 0"));

    let gris = imagen(Mode::Gray);
    assert_eq!(slot_label(&gris, 4).as_deref(), Some("color 4 gris 96"));
    assert_eq!(slot_label(&gris, 9).as_deref(), Some("color 9 gris 255"));

    // GrayAlpha: gris con alfa 255 en 1 a 8 y transparente en la 9.
    let gris_alfa = imagen(Mode::GrayAlpha);
    assert_eq!(
        slot_label(&gris_alfa, 4).as_deref(),
        Some("color 4 gris 96")
    );
    assert_eq!(
        slot_label(&gris_alfa, 9).as_deref(),
        Some("color 9 transparente")
    );
}

#[test]
fn slot_label_es_none_cuando_la_ranura_es_none() {
    let tres = indexada(3);
    assert_eq!(slot_label(&tres, 4), None);
    assert_eq!(slot_label(&tres, 9), None);
    for image in [
        imagen(Mode::Rgba),
        imagen(Mode::Gray),
        imagen(Mode::GrayAlpha),
        indexada(9),
    ] {
        assert_eq!(slot_label(&image, 0), None, "{:?}", image.mode());
        assert_eq!(slot_label(&image, 10), None, "{:?}", image.mode());
        assert_eq!(slot_label(&image, 255), None, "{:?}", image.mode());
    }
}

// Propiedad 5: el color nunca es del modo equivocado.

fn imagen_para(modo: usize, colores: usize, width: u32, height: u32) -> Image {
    match modo {
        0 => Image::new(width, height, Mode::Rgba).unwrap(),
        1 => Image::new(width, height, Mode::Gray).unwrap(),
        2 => Image::new(width, height, Mode::GrayAlpha).unwrap(),
        _ => {
            let paleta =
                Palette::from_colors((0..colores).map(|i| Rgba::opaque(i as u8 * 20, 1, 2)))
                    .unwrap();
            Image::new_indexed(width, height, paleta).unwrap()
        }
    }
}

proptest! {
    #[test]
    fn el_color_de_una_ranura_es_siempre_del_modo_de_la_imagen(
        modo in 0usize..4,
        colores in 1usize..=12,
        width in 1u32..=16,
        height in 1u32..=16,
        slot in any::<u8>(),
    ) {
        let image = imagen_para(modo, colores, width, height);
        let pixel = pixel_for_slot(&image, slot);

        // `None` solo con la ranura fuera de 1 a 9 o con indexado y `n > paleta.len()`.
        let sobrante = image.mode() == Mode::Indexed && usize::from(slot) > colores;
        prop_assert_eq!(pixel.is_none(), !(1..=9).contains(&slot) || sobrante);
        // La etiqueta existe exactamente cuando hay color.
        prop_assert_eq!(slot_label(&image, slot).is_some(), pixel.is_some());

        if let Some(pixel) = pixel {
            prop_assert_eq!(image.check_pixel(pixel), Ok(()));

            // Un trazo con ese color nunca da `Err(Pixel { .. })`.
            let mut doc = Document::new(image);
            let id = doc.main_image();
            let mut stroke = Stroke::new(width, height);
            stroke.move_to((0, 0), pixel);
            stroke.move_to((width as i32 - 1, height as i32 - 1), pixel);
            let resultado = stroke.commit(&mut doc, id);
            prop_assert!(
                resultado.is_ok(),
                "el trazo con la ranura {} dio {:?}",
                slot,
                resultado
            );
        }
    }
}

// Unidad 21 (specs/hito-2-fijar-transparente.md, fila 5): la bandera `alfa 0` de la línea 2 de
// la muestra y `is_alpha0_cell_with`.

mod alfa_0 {
    use super::*;
    use px_ui::color::{
        SlotColors, info_flags, info_flags_alpha0, info_flags_range, is_alpha0_cell_with,
    };
    use px_ui::text::INTERFACE_CHARSET;

    /// Oráculo propio: las banderas que apliquen, en orden, unidas con ` · `.
    fn modelo(
        hover: bool,
        active: bool,
        in_range: bool,
        transparent: bool,
        alpha0: bool,
    ) -> String {
        let mut partes: Vec<&str> = Vec::new();
        if hover {
            partes.push("cursor");
        }
        if active {
            partes.push("activa");
        }
        if in_range && !active {
            partes.push("rango");
        }
        if transparent {
            partes.push("transparente");
        }
        // Excluyentes: si es la transparente, gana `transparente`.
        if alpha0 && !transparent {
            partes.push("alfa 0");
        }
        partes.join(" · ")
    }

    fn combinaciones() -> impl Iterator<Item = (bool, bool, bool, bool, bool)> {
        (0..32u8).map(|b| (b & 1 != 0, b & 2 != 0, b & 4 != 0, b & 8 != 0, b & 16 != 0))
    }

    #[test]
    fn info_flags_alpha0_de_la_spec() {
        assert_eq!(info_flags_alpha0(false, false, false, false, false), "");
        assert_eq!(
            info_flags_alpha0(false, false, false, true, false),
            "transparente"
        );
        assert_eq!(
            info_flags_alpha0(false, false, false, false, true),
            "alfa 0"
        );
        assert_eq!(
            info_flags_alpha0(true, true, false, true, false),
            "cursor · activa · transparente"
        );
        assert_eq!(
            info_flags_alpha0(true, true, false, false, true),
            "cursor · activa · alfa 0"
        );
        assert_eq!(
            info_flags_alpha0(true, false, true, false, true),
            "cursor · rango · alfa 0"
        );
        assert_eq!(
            info_flags_alpha0(false, false, true, true, false),
            "rango · transparente"
        );
    }

    #[test]
    fn info_flags_alpha0_sigue_al_modelo_en_las_32_combinaciones() {
        for (h, a, r, t, z) in combinaciones() {
            assert_eq!(
                info_flags_alpha0(h, a, r, t, z),
                modelo(h, a, r, t, z),
                "h={h} a={a} r={r} t={t} z={z}"
            );
        }
    }

    #[test]
    fn transparente_y_alfa_0_son_excluyentes_y_gana_transparente() {
        for (h, a, r, t, z) in combinaciones() {
            let linea = info_flags_alpha0(h, a, r, t, z);
            assert!(
                !(linea.contains("transparente") && linea.contains("alfa 0")),
                "h={h} a={a} r={r} t={t} z={z}: {linea:?}"
            );
            if t {
                assert!(linea.contains("transparente"), "{linea:?}");
                assert!(!linea.contains("alfa 0"), "{linea:?}");
            } else if z {
                assert!(linea.contains("alfa 0"), "{linea:?}");
                assert!(!linea.contains("transparente"), "{linea:?}");
            } else {
                assert!(!linea.contains("alfa 0") && !linea.contains("transparente"));
            }
        }
        assert_eq!(
            info_flags_alpha0(true, true, false, true, true),
            "cursor · activa · transparente"
        );
        assert_eq!(
            info_flags_alpha0(false, false, false, true, true),
            "transparente"
        );
        assert_eq!(
            info_flags_alpha0(false, false, false, false, true),
            "alfa 0"
        );
    }

    #[test]
    fn la_bandera_rango_solo_sale_si_esta_en_el_rango_y_no_es_la_activa() {
        for t in [false, true] {
            let con = info_flags_alpha0(false, false, true, t, !t);
            assert!(con.contains("rango"), "{con:?}");
            let activa = info_flags_alpha0(false, true, true, t, !t);
            assert!(!activa.contains("rango"), "{activa:?}");
            let fuera = info_flags_alpha0(false, false, false, t, !t);
            assert!(!fuera.contains("rango"), "{fuera:?}");
        }
    }

    #[test]
    fn info_flags_e_info_flags_range_no_cambian_y_coinciden_sin_alfa_0() {
        for (h, a, r, t, _) in combinaciones() {
            assert_eq!(
                info_flags_alpha0(h, a, r, t, false),
                info_flags_range(h, a, r, t)
            );
            assert_eq!(
                info_flags_alpha0(h, a, false, t, false),
                info_flags(h, a, t)
            );
        }
        assert_eq!(
            info_flags(true, true, true),
            "cursor · activa · transparente"
        );
        assert_eq!(
            info_flags_range(true, false, true, true),
            "cursor · rango · transparente"
        );
        assert_eq!(info_flags(false, false, false), "");
    }

    #[test]
    fn la_linea_mas_larga_son_30_columnas_y_todo_sale_del_juego_de_caracteres() {
        let mut mas_larga = 0;
        for (h, a, r, t, z) in combinaciones() {
            let linea = info_flags_alpha0(h, a, r, t, z);
            mas_larga = mas_larga.max(linea.chars().count());
            assert!(linea.chars().count() <= 32, "{linea:?}");
            assert!(
                linea.chars().all(|c| INTERFACE_CHARSET.contains(c)),
                "{linea:?}"
            );
        }
        assert_eq!(mas_larga, 30);
        assert_eq!("cursor · activa · transparente".chars().count(), 30);
    }

    fn paleta_con_alfa_0(transparente: Option<u8>) -> Image {
        // 0 opaca, 1 con alfa guardado 0, 2 opaca, 3 con alfa guardado 0.
        let mut paleta = Palette::from_colors([
            Rgba::opaque(255, 0, 0),
            Rgba::new(0, 255, 0, 0),
            Rgba::opaque(0, 0, 255),
            Rgba::new(1, 1, 1, 0),
        ])
        .unwrap();
        paleta.set_transparent_index(transparente).unwrap();
        Image::new_indexed(2, 2, paleta).unwrap()
    }

    #[test]
    fn is_alpha0_cell_with_marca_las_de_alfa_guardado_0_que_no_son_la_transparente() {
        let slots = SlotColors::defaults(Mode::Indexed);
        let sin = paleta_con_alfa_0(None);
        let esperado = [false, true, false, true];
        for (celda, esperado) in esperado.into_iter().enumerate() {
            assert_eq!(
                is_alpha0_cell_with(&sin, celda, &slots),
                esperado,
                "sin marca, {celda}"
            );
        }
        // Con la 3 marcada: la marcada ya dice `transparente`, no `alfa 0`.
        let marcada = paleta_con_alfa_0(Some(3));
        for (celda, esperado) in [false, true, false, false].into_iter().enumerate() {
            assert_eq!(
                is_alpha0_cell_with(&marcada, celda, &slots),
                esperado,
                "con la 3 marcada, {celda}"
            );
        }
        // Una opaca marcada tampoco es alfa 0 (resuelve 0, pero el guardado es 255).
        let opaca = paleta_con_alfa_0(Some(0));
        assert!(!is_alpha0_cell_with(&opaca, 0, &slots));
        assert!(is_alpha0_cell_with(&opaca, 1, &slots));
        assert!(is_alpha0_cell_with(&opaca, 3, &slots));
    }

    #[test]
    fn is_alpha0_cell_with_es_excluyente_con_is_transparent_cell_with() {
        use px_ui::color::is_transparent_cell_with;
        let slots = SlotColors::defaults(Mode::Indexed);
        for transparente in [None, Some(0), Some(1), Some(2), Some(3)] {
            let imagen = paleta_con_alfa_0(transparente);
            for celda in 0..4 {
                assert!(
                    !(is_alpha0_cell_with(&imagen, celda, &slots)
                        && is_transparent_cell_with(&imagen, celda, &slots)),
                    "{transparente:?}, celda {celda}"
                );
            }
        }
    }

    #[test]
    fn is_alpha0_cell_con_una_celda_fuera_de_la_paleta_es_falso() {
        let slots = SlotColors::defaults(Mode::Indexed);
        let imagen = paleta_con_alfa_0(None);
        for celda in [4usize, 5, 255, 256, 1000] {
            assert!(!is_alpha0_cell_with(&imagen, celda, &slots), "{celda}");
        }
    }

    #[test]
    fn is_alpha0_cell_with_en_los_demas_modos_es_siempre_falso() {
        for modo in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            let imagen = Image::new(2, 2, modo).unwrap();
            let slots = SlotColors::defaults(modo);
            for celda in 0..12 {
                assert!(
                    !is_alpha0_cell_with(&imagen, celda, &slots),
                    "{modo:?}, celda {celda}"
                );
            }
        }
    }
}
