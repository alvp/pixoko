//! Tests de `ColorChoice` y las piezas de selección de `color` (specs/hito-2-panel-paletas.md,
//! unidad 11b): `pixel_for_choice`, `initial_choice`, `cell_count`, `choice_for_cell`,
//! `cell_of_choice`, `key_choice`, `cell_color`, `is_transparent_cell`, `cell_label`,
//! `info_flags` y `choice_label`.
//!
//! Cubre las filas 6 y 7 (la parte de `px-ui`: elegir y celdas inexistentes), la 8 (solo
//! `initial_choice`; lo de `Session` es de `px-app`), la 9 (textos), la parte de selección de la
//! 11 (transparente en 0, 17, 255 y ninguno; paletas con colores repetidos; las nueve ranuras de
//! los modos sin paleta) y la propiedad 4 en su parte pura.
//!
//! Las muestras («celdas») se numeran desde 0; las ranuras y las teclas, desde 1. Un clic en la
//! celda `k` y la tecla `k + 1` dan la misma `ColorChoice`. Los textos de las filas 9 y 11 están
//! escritos con las cadenas literales de la spec.

use proptest::collection::vec;
use proptest::prelude::*;
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_ui::color::{
    ColorChoice, SLOTS, cell_color, cell_count, cell_label, cell_of_choice, choice_for_cell,
    choice_label, info_flags, initial_choice, is_transparent_cell, key_choice, pixel_for_choice,
    pixel_for_slot,
};

fn imagen(mode: Mode) -> Image {
    Image::new(4, 4, mode).unwrap()
}

/// Indexada con los colores dados y el índice transparente indicado.
fn indexada_con(colores: Vec<Rgba>, transparente: Option<u8>) -> Image {
    let mut paleta = Palette::from_colors(colores).unwrap();
    paleta.set_transparent_index(transparente).unwrap();
    Image::new_indexed(4, 4, paleta).unwrap()
}

/// Indexada de `n` entradas opacas y distintas entre sí, sin índice transparente.
fn indexada(n: usize) -> Image {
    indexada_con(
        (0..n)
            .map(|i| Rgba::opaque(i as u8, 255 - i as u8, 7))
            .collect(),
        None,
    )
}

fn rgba(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::opaque(r, g, b)
}

/// Las cuatro formas de imagen que se prueban siempre: tres sin paleta, y indexadas de 3 y 256.
fn todas() -> Vec<(&'static str, Image)> {
    vec![
        ("rgba", imagen(Mode::Rgba)),
        ("gray", imagen(Mode::Gray)),
        ("gray alpha", imagen(Mode::GrayAlpha)),
        ("indexada de 1", indexada(1)),
        ("indexada de 3", indexada(3)),
        ("indexada de 9", indexada(9)),
        ("indexada de 256", indexada(256)),
    ]
}

fn sin_paleta() -> Vec<(&'static str, Image)> {
    vec![
        ("rgba", imagen(Mode::Rgba)),
        ("gray", imagen(Mode::Gray)),
        ("gray alpha", imagen(Mode::GrayAlpha)),
    ]
}

// `ColorChoice` y `pixel_for_choice`.

#[test]
fn color_choice_es_copy_debug_y_comparable() {
    fn comprueba<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
    comprueba::<ColorChoice>();
    assert_ne!(ColorChoice::Slot(3), ColorChoice::Entry(3));
    assert_eq!(ColorChoice::Entry(3), ColorChoice::Entry(3));
}

#[test]
fn from_u8_da_una_ranura() {
    assert_eq!(ColorChoice::from(3u8), ColorChoice::Slot(3));
    assert_eq!(ColorChoice::from(0u8), ColorChoice::Slot(0));
    assert_eq!(ColorChoice::from(255u8), ColorChoice::Slot(255));
    let eleccion: ColorChoice = 9u8.into();
    assert_eq!(eleccion, ColorChoice::Slot(9));
}

#[test]
fn pixel_for_choice_con_una_ranura_es_pixel_for_slot_en_los_cuatro_modos() {
    let imagenes = [
        imagen(Mode::Rgba),
        imagen(Mode::Gray),
        imagen(Mode::GrayAlpha),
        indexada(3),
        indexada(9),
        indexada(256),
    ];
    for image in &imagenes {
        for n in 0..=10u8 {
            assert_eq!(
                pixel_for_choice(image, ColorChoice::Slot(n)),
                pixel_for_slot(image, n),
                "{:?}, ranura {n}",
                image.mode()
            );
        }
    }
    // Con ejemplos concretos: la ranura vale también en las indexadas, como hoy.
    assert_eq!(
        pixel_for_choice(&indexada(3), ColorChoice::Slot(3)),
        Some(Pixel::Indexed(2))
    );
    assert_eq!(pixel_for_choice(&indexada(3), ColorChoice::Slot(4)), None);
}

#[test]
fn pixel_for_choice_con_una_entrada_en_una_indexada() {
    let image = indexada(256);
    for i in [0u8, 1, 8, 200, 255] {
        assert_eq!(
            pixel_for_choice(&image, ColorChoice::Entry(i)),
            Some(Pixel::Indexed(i))
        );
    }
    let tres = indexada(3);
    assert_eq!(
        pixel_for_choice(&tres, ColorChoice::Entry(2)),
        Some(Pixel::Indexed(2))
    );
    assert_eq!(pixel_for_choice(&tres, ColorChoice::Entry(3)), None);
    assert_eq!(pixel_for_choice(&tres, ColorChoice::Entry(255)), None);
}

#[test]
fn pixel_for_choice_con_una_entrada_en_una_imagen_no_indexada_es_none() {
    for (nombre, image) in sin_paleta() {
        for i in [0u8, 1, 8, 200, 255] {
            assert_eq!(
                pixel_for_choice(&image, ColorChoice::Entry(i)),
                None,
                "{nombre}, Entry({i})"
            );
        }
    }
}

// Fila 8 (parte de px-ui): `initial_choice`.

#[test]
fn fila_8_initial_choice() {
    assert_eq!(initial_choice(&imagen(Mode::Rgba)), ColorChoice::Slot(1));
    assert_eq!(initial_choice(&imagen(Mode::Gray)), ColorChoice::Slot(1));
    assert_eq!(
        initial_choice(&imagen(Mode::GrayAlpha)),
        ColorChoice::Slot(1)
    );
    for n in [1usize, 2, 3, 9, 256] {
        assert_eq!(
            initial_choice(&indexada(n)),
            ColorChoice::Entry(0),
            "indexada de {n}"
        );
    }
}

#[test]
fn la_eleccion_inicial_vale_para_su_imagen() {
    for (nombre, image) in todas() {
        let inicial = initial_choice(&image);
        assert!(
            pixel_for_choice(&image, inicial).is_some(),
            "{nombre}: {inicial:?}"
        );
        assert_eq!(cell_of_choice(&image, inicial), Some(0), "{nombre}");
        assert_eq!(choice_for_cell(&image, 0), Some(inicial), "{nombre}");
    }
}

// `cell_count`.

#[test]
fn cell_count_es_la_paleta_o_nueve() {
    assert_eq!(SLOTS, 9);
    for (nombre, image) in sin_paleta() {
        assert_eq!(cell_count(&image), 9, "{nombre}");
    }
    for n in [1usize, 2, 3, 9, 10, 255, 256] {
        assert_eq!(cell_count(&indexada(n)), n, "indexada de {n}");
    }
}

// Fila 6: elegir (parte de px-ui).

#[test]
fn fila_6_indexada_de_256_clic_en_la_200_y_en_la_8_y_teclas_1_y_9() {
    let image = indexada(256);
    assert_eq!(choice_for_cell(&image, 200), Some(ColorChoice::Entry(200)));
    assert_eq!(
        pixel_for_choice(&image, ColorChoice::Entry(200)),
        Some(Pixel::Indexed(200))
    );
    assert_eq!(choice_for_cell(&image, 8), Some(ColorChoice::Entry(8)));
    assert_eq!(key_choice(&image, 9), Some(ColorChoice::Entry(8)));
    assert_eq!(key_choice(&image, 1), Some(ColorChoice::Entry(0)));
    // `cell_of_choice` da la muestra pulsada (o la de la tecla menos 1).
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(200)), Some(200));
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(8)), Some(8));
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(0)), Some(0));
    // `choice_label`: `índice 200 #…` con el color de la entrada 200 (Rgba::opaque(200, 55, 7)).
    assert_eq!(
        choice_label(&image, ColorChoice::Entry(200)).as_deref(),
        Some("índice 200 #c83707")
    );
}

#[test]
fn fila_6_rgba_clic_en_la_2_y_tecla_3() {
    let image = imagen(Mode::Rgba);
    assert_eq!(choice_for_cell(&image, 2), Some(ColorChoice::Slot(3)));
    assert_eq!(key_choice(&image, 3), Some(ColorChoice::Slot(3)));
    assert_eq!(
        pixel_for_choice(&image, ColorChoice::Slot(3)),
        Some(Pixel::Rgba(rgba(0xff, 0, 0)))
    );
    assert_eq!(cell_of_choice(&image, ColorChoice::Slot(3)), Some(2));
    assert_eq!(
        choice_label(&image, ColorChoice::Slot(3)).as_deref(),
        Some("color 3 #ff0000")
    );
}

#[test]
fn fila_6_gray_clic_en_la_8() {
    let image = imagen(Mode::Gray);
    assert_eq!(choice_for_cell(&image, 8), Some(ColorChoice::Slot(9)));
    assert_eq!(key_choice(&image, 9), Some(ColorChoice::Slot(9)));
    assert_eq!(
        pixel_for_choice(&image, ColorChoice::Slot(9)),
        Some(Pixel::Gray(255))
    );
    assert_eq!(cell_of_choice(&image, ColorChoice::Slot(9)), Some(8));
}

#[test]
fn fila_6_indexada_de_3_la_tecla_9_no_existe() {
    let image = indexada(3);
    assert_eq!(key_choice(&image, 9), None);
    for n in 4..=9u8 {
        assert_eq!(key_choice(&image, n), None, "tecla {n}");
    }
    for n in 1..=3u8 {
        assert_eq!(
            key_choice(&image, n),
            Some(ColorChoice::Entry(n - 1)),
            "tecla {n}"
        );
    }
    // La selección no cambia: se modela como «si es `None`, se conserva».
    let mut seleccion = ColorChoice::Entry(1);
    if let Some(nueva) = key_choice(&image, 9) {
        seleccion = nueva;
    }
    assert_eq!(seleccion, ColorChoice::Entry(1));
}

#[test]
fn fila_6_un_clic_en_k_y_la_tecla_k_mas_1_dan_la_misma_eleccion_y_el_mismo_pixel() {
    for (nombre, image) in todas() {
        for k in 0..9usize {
            let por_clic = choice_for_cell(&image, k);
            let por_tecla = key_choice(&image, (k + 1) as u8);
            assert_eq!(por_clic, por_tecla, "{nombre}, celda {k}");
            if let (Some(clic), Some(tecla)) = (por_clic, por_tecla) {
                assert_eq!(
                    pixel_for_choice(&image, clic),
                    pixel_for_choice(&image, tecla),
                    "{nombre}, celda {k}"
                );
            }
        }
    }
}

#[test]
fn la_forma_normal_es_entry_en_indexadas_y_slot_en_las_demas() {
    for (nombre, image) in sin_paleta() {
        for i in 0..9usize {
            assert_eq!(
                choice_for_cell(&image, i),
                Some(ColorChoice::Slot(i as u8 + 1)),
                "{nombre}, celda {i}"
            );
        }
    }
    let image = indexada(20);
    for i in 0..20usize {
        assert_eq!(
            choice_for_cell(&image, i),
            Some(ColorChoice::Entry(i as u8)),
            "celda {i}"
        );
    }
}

// Fila 7 (parte de px-ui): celdas inexistentes.

#[test]
fn fila_7_las_celdas_inexistentes_no_eligen() {
    for (nombre, image) in todas() {
        let cuantas = cell_count(&image);
        assert!(choice_for_cell(&image, cuantas - 1).is_some(), "{nombre}");
        for cell in [cuantas, cuantas + 1, cuantas + 100, 256, 1000, usize::MAX] {
            assert_eq!(
                choice_for_cell(&image, cell),
                None,
                "{nombre}, celda {cell}"
            );
            assert_eq!(cell_color(&image, cell), None, "{nombre}, celda {cell}");
            assert_eq!(cell_label(&image, cell), None, "{nombre}, celda {cell}");
        }
    }
}

#[test]
fn fila_7_las_teclas_sin_muestra_se_ignoran() {
    // Una sola entrada: las teclas 2 a 9 se ignoran (fila 11).
    let una = indexada(1);
    assert_eq!(key_choice(&una, 1), Some(ColorChoice::Entry(0)));
    for n in 2..=9u8 {
        assert_eq!(key_choice(&una, n), None, "tecla {n}");
    }
    // Paletas de 5 entradas: de la 6 a la 9 no existen.
    let cinco = indexada(5);
    for n in 6..=9u8 {
        assert_eq!(key_choice(&cinco, n), None, "tecla {n}");
    }
    // La tecla 0 no es de ninguna muestra.
    for (nombre, image) in todas() {
        assert_eq!(key_choice(&image, 0), None, "{nombre}, tecla 0");
    }
}

#[test]
fn fila_7_cell_of_choice_es_none_si_no_vale_para_la_imagen() {
    // Entrada en una imagen sin paleta.
    for (nombre, image) in sin_paleta() {
        assert_eq!(
            cell_of_choice(&image, ColorChoice::Entry(0)),
            None,
            "{nombre}"
        );
        assert_eq!(
            cell_of_choice(&image, ColorChoice::Entry(200)),
            None,
            "{nombre}"
        );
        // Ranuras fuera de 1 a 9.
        assert_eq!(
            cell_of_choice(&image, ColorChoice::Slot(0)),
            None,
            "{nombre}"
        );
        assert_eq!(
            cell_of_choice(&image, ColorChoice::Slot(10)),
            None,
            "{nombre}"
        );
        // Las nueve ranuras dan su muestra.
        for n in 1..=9u8 {
            assert_eq!(
                cell_of_choice(&image, ColorChoice::Slot(n)),
                Some(usize::from(n) - 1),
                "{nombre}, ranura {n}"
            );
        }
    }
    // Entrada fuera de la paleta.
    let tres = indexada(3);
    assert_eq!(cell_of_choice(&tres, ColorChoice::Entry(2)), Some(2));
    assert_eq!(cell_of_choice(&tres, ColorChoice::Entry(3)), None);
    assert_eq!(cell_of_choice(&tres, ColorChoice::Entry(255)), None);
}

// Fila 9: textos.

/// `[#000000, #ff0000, #00ff00 (índice transparente), #0000ff80, …, #ffffff (255)]`.
fn paleta_de_la_fila_9() -> Image {
    let mut colores = vec![Rgba::opaque(10, 20, 30); 256];
    colores[0] = rgba(0, 0, 0);
    colores[1] = rgba(0xff, 0, 0);
    colores[2] = rgba(0, 0xff, 0);
    colores[3] = Rgba::new(0, 0, 0xff, 0x80);
    colores[255] = rgba(0xff, 0xff, 0xff);
    indexada_con(colores, Some(2))
}

#[test]
fn fila_9_cell_label_indexada() {
    let image = paleta_de_la_fila_9();
    assert_eq!(cell_label(&image, 1).as_deref(), Some("índice 1 #ff0000"));
    // Resuelto: la transparente da alfa 0, aunque se guardó opaca.
    assert_eq!(cell_label(&image, 2).as_deref(), Some("índice 2 #00ff0000"));
    assert_eq!(cell_label(&image, 3).as_deref(), Some("índice 3 #0000ff80"));
    assert_eq!(
        cell_label(&image, 255).as_deref(),
        Some("índice 255 #ffffff")
    );
    assert_eq!(cell_label(&image, 0).as_deref(), Some("índice 0 #000000"));
    assert_eq!(cell_label(&image, 256), None);
}

#[test]
fn fila_9_cell_label_indexada_con_el_ejemplo_del_indice_12() {
    let mut colores = vec![rgba(1, 2, 3); 20];
    colores[12] = rgba(0xff, 0x88, 0x00);
    let image = indexada_con(colores, None);
    assert_eq!(cell_label(&image, 12).as_deref(), Some("índice 12 #ff8800"));
}

#[test]
fn fila_9_cell_label_rgba() {
    // Las muestras 3 y 9 de la spec son las ranuras `color 3` y `color 9` (celdas 2 y 8).
    let image = imagen(Mode::Rgba);
    assert_eq!(cell_label(&image, 2).as_deref(), Some("color 3 #ff0000"));
    assert_eq!(cell_label(&image, 8).as_deref(), Some("color 9 #00000000"));
    assert_eq!(cell_label(&image, 0).as_deref(), Some("color 1 #000000"));
    assert_eq!(cell_label(&image, 7).as_deref(), Some("color 8 #00ffff"));
    assert_eq!(cell_label(&image, 9), None);
}

#[test]
fn fila_9_cell_label_gray_alpha() {
    let image = imagen(Mode::GrayAlpha);
    assert_eq!(cell_label(&image, 2).as_deref(), Some("color 3 gris 64"));
    assert_eq!(
        cell_label(&image, 8).as_deref(),
        Some("color 9 gris 0 alfa 0")
    );
    assert_eq!(cell_label(&image, 9), None);
}

#[test]
fn fila_9_cell_label_gray() {
    let image = imagen(Mode::Gray);
    assert_eq!(cell_label(&image, 3).as_deref(), Some("color 4 gris 96"));
    assert_eq!(cell_label(&image, 9), None);
}

#[test]
fn fila_9_info_flags_en_las_ocho_combinaciones() {
    // (hover, activa, transparente) -> texto.
    let casos = [
        (false, false, false, ""),
        (false, true, false, "activa"),
        (false, false, true, "transparente"),
        (false, true, true, "activa · transparente"),
        (true, false, false, "cursor"),
        (true, true, false, "cursor · activa"),
        (true, false, true, "cursor · transparente"),
        (true, true, true, "cursor · activa · transparente"),
    ];
    for (hover, activa, transparente, esperado) in casos {
        assert_eq!(
            info_flags(hover, activa, transparente),
            esperado,
            "hover {hover}, activa {activa}, transparente {transparente}"
        );
    }
    // El más largo, de 30 caracteres (fila 12).
    assert_eq!(
        info_flags(true, true, true).chars().count(),
        30,
        "cursor · activa · transparente"
    );
}

#[test]
fn fila_9_choice_label() {
    let indexada = paleta_de_la_fila_9();
    assert_eq!(
        choice_label(&indexada, ColorChoice::Entry(2)).as_deref(),
        Some("índice 2 #00ff0000 transparente")
    );
    assert_eq!(
        choice_label(&indexada, ColorChoice::Entry(1)).as_deref(),
        Some("índice 1 #ff0000")
    );
    assert_eq!(
        choice_label(&indexada, ColorChoice::Entry(3)).as_deref(),
        Some("índice 3 #0000ff80")
    );
    // Entrada que no existe: no hay etiqueta.
    assert_eq!(choice_label(&indexada, ColorChoice::Slot(0)), None);

    let color = imagen(Mode::Rgba);
    assert_eq!(
        choice_label(&color, ColorChoice::Slot(9)).as_deref(),
        Some("color 9 transparente")
    );
    assert_eq!(
        choice_label(&color, ColorChoice::Slot(3)).as_deref(),
        Some("color 3 #ff0000")
    );
    // Una entrada en una imagen sin paleta no vale.
    assert_eq!(choice_label(&color, ColorChoice::Entry(2)), None);
}

#[test]
fn choice_label_con_una_ranura_es_slot_label_como_hoy() {
    for (nombre, image) in sin_paleta() {
        for n in 1..=9u8 {
            assert_eq!(
                choice_label(&image, ColorChoice::Slot(n)),
                px_ui::color::slot_label(&image, n),
                "{nombre}, ranura {n}"
            );
        }
    }
    // En una indexada, `Slot` mantiene el texto de hoy (`color 2 índice 1`).
    assert_eq!(
        choice_label(&indexada(3), ColorChoice::Slot(2)).as_deref(),
        Some("color 2 índice 1")
    );
}

#[test]
fn choice_label_existe_exactamente_cuando_hay_color() {
    for (nombre, image) in todas() {
        for choice in [
            ColorChoice::Slot(0),
            ColorChoice::Slot(1),
            ColorChoice::Slot(9),
            ColorChoice::Slot(10),
            ColorChoice::Entry(0),
            ColorChoice::Entry(2),
            ColorChoice::Entry(8),
            ColorChoice::Entry(255),
        ] {
            assert_eq!(
                choice_label(&image, choice).is_some(),
                pixel_for_choice(&image, choice).is_some(),
                "{nombre}, {choice:?}"
            );
        }
    }
}

// Fila 11 (parte de selección).

fn indexada_de_256_con_transparente(t: Option<u8>) -> Image {
    indexada_con(
        (0..256usize)
            .map(|i| Rgba::opaque(i as u8, 100, 200))
            .collect(),
        t,
    )
}

#[test]
fn fila_11_el_indice_transparente_en_0_17_y_255() {
    for t in [0u8, 17, 255] {
        let image = indexada_de_256_con_transparente(Some(t));
        for cell in 0..256usize {
            let es_la_transparente = cell == usize::from(t);
            assert_eq!(
                is_transparent_cell(&image, cell),
                es_la_transparente,
                "t={t}, celda {cell}"
            );
            let color = cell_color(&image, cell).unwrap();
            assert_eq!(color.a == 0, es_la_transparente, "t={t}, celda {cell}");
        }
        // La transparente se ve con alfa 0 (su RGB guardado sigue ahí) y se elige como las otras.
        let guardado = Rgba::opaque(t, 100, 200);
        assert_eq!(
            cell_color(&image, usize::from(t)),
            Some(Rgba { a: 0, ..guardado })
        );
        assert_eq!(
            pixel_for_choice(&image, ColorChoice::Entry(t)),
            Some(Pixel::Indexed(t))
        );
        assert_eq!(
            choice_for_cell(&image, usize::from(t)),
            Some(ColorChoice::Entry(t))
        );
        assert_eq!(
            cell_of_choice(&image, ColorChoice::Entry(t)),
            Some(usize::from(t))
        );
        // El resto conserva su color opaco.
        let otra = usize::from(t ^ 1);
        assert_eq!(
            cell_color(&image, otra),
            Some(Rgba::opaque(otra as u8, 100, 200))
        );
        // El texto de la entrada transparente dice alfa 0.
        assert_eq!(
            cell_label(&image, usize::from(t)),
            Some(format!("índice {t} #{t:02x}64c800"))
        );
    }
}

#[test]
fn fila_11_sin_indice_transparente_ninguna_celda_es_transparente() {
    let image = indexada_de_256_con_transparente(None);
    for cell in 0..256usize {
        assert!(!is_transparent_cell(&image, cell), "celda {cell}");
        assert_eq!(
            cell_color(&image, cell),
            Some(Rgba::opaque(cell as u8, 100, 200)),
            "celda {cell}"
        );
    }
}

#[test]
fn fila_11_colores_alfa_parcial_no_son_transparentes() {
    // Un alfa de 128 guardado no es el índice transparente: solo cuenta `transparent_index`.
    let a = rgba(0x12, 0x34, 0x56);
    let image = indexada_con(vec![a, Rgba { a: 128, ..a }], None);
    assert!(!is_transparent_cell(&image, 0));
    assert!(!is_transparent_cell(&image, 1));
    assert_eq!(cell_color(&image, 0), Some(a));
    assert_eq!(cell_color(&image, 1), Some(Rgba { a: 128, ..a }));
    assert_eq!(cell_label(&image, 1).as_deref(), Some("índice 1 #12345680"));
}

#[test]
fn fila_11_colores_duplicados_se_pueden_elegir_por_separado() {
    let a = rgba(0x12, 0x34, 0x56);
    for colores in [vec![a, a, a], vec![a, Rgba { a: 128, ..a }]] {
        let cuantos = colores.len();
        let image = indexada_con(colores, None);
        assert_eq!(cell_count(&image), cuantos);
        let mut elecciones = Vec::new();
        for i in 0..cuantos {
            let eleccion = choice_for_cell(&image, i).unwrap();
            assert_eq!(eleccion, ColorChoice::Entry(i as u8));
            assert_eq!(
                pixel_for_choice(&image, eleccion),
                Some(Pixel::Indexed(i as u8))
            );
            // `cell_of_choice` devuelve la celda, no la primera con ese color.
            assert_eq!(cell_of_choice(&image, eleccion), Some(i), "celda {i}");
            elecciones.push(eleccion);
        }
        for (i, a) in elecciones.iter().enumerate() {
            for b in &elecciones[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
    // En concreto, el clic en la 1 da `Entry(1)` y `cell_of_choice` da 1.
    let image = indexada_con(vec![a, a, a], None);
    assert_eq!(choice_for_cell(&image, 1), Some(ColorChoice::Entry(1)));
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(1)), Some(1));
}

#[test]
fn fila_11_las_nueve_ranuras_de_rgba() {
    let image = imagen(Mode::Rgba);
    let esperado = [
        Rgba::opaque(0x00, 0x00, 0x00),
        Rgba::opaque(0xff, 0xff, 0xff),
        Rgba::opaque(0xff, 0x00, 0x00),
        Rgba::opaque(0x00, 0xff, 0x00),
        Rgba::opaque(0x00, 0x00, 0xff),
        Rgba::opaque(0xff, 0xff, 0x00),
        Rgba::opaque(0xff, 0x00, 0xff),
        Rgba::opaque(0x00, 0xff, 0xff),
        Rgba::new(0, 0, 0, 0),
    ];
    assert_eq!(cell_count(&image), 9);
    for (i, esperado) in esperado.into_iter().enumerate() {
        assert_eq!(cell_color(&image, i), Some(esperado), "celda {i}");
        assert_eq!(is_transparent_cell(&image, i), i == 8, "celda {i}");
    }
    assert_eq!(cell_color(&image, 9), None);
    assert_eq!(choice_for_cell(&image, 9), None);
}

#[test]
fn fila_11_las_nueve_ranuras_de_gray() {
    let image = imagen(Mode::Gray);
    assert_eq!(cell_count(&image), 9);
    for i in 0..8usize {
        let gris = (i as u8) * 32;
        assert_eq!(
            cell_color(&image, i),
            Some(Rgba::opaque(gris, gris, gris)),
            "celda {i}"
        );
        assert!(!is_transparent_cell(&image, i), "celda {i}");
    }
    // La novena es gris 255 opaco: Gray no tiene transparente.
    assert_eq!(cell_color(&image, 8), Some(Rgba::opaque(255, 255, 255)));
    assert!(!is_transparent_cell(&image, 8));
    assert_eq!(cell_color(&image, 9), None);
    assert_eq!(choice_for_cell(&image, 9), None);
}

#[test]
fn fila_11_las_nueve_ranuras_de_gray_alpha() {
    let image = imagen(Mode::GrayAlpha);
    assert_eq!(cell_count(&image), 9);
    for i in 0..8usize {
        let gris = (i as u8) * 32;
        assert_eq!(
            cell_color(&image, i),
            Some(Rgba::new(gris, gris, gris, 255)),
            "celda {i}"
        );
        assert!(!is_transparent_cell(&image, i), "celda {i}");
    }
    assert_eq!(cell_color(&image, 8), Some(Rgba::new(0, 0, 0, 0)));
    assert!(is_transparent_cell(&image, 8));
    assert_eq!(cell_color(&image, 9), None);
    assert_eq!(choice_for_cell(&image, 9), None);
}

#[test]
fn fila_11_paleta_de_una_entrada() {
    let image = indexada(1);
    assert_eq!(cell_count(&image), 1);
    assert_eq!(choice_for_cell(&image, 0), Some(ColorChoice::Entry(0)));
    assert_eq!(choice_for_cell(&image, 1), None);
    assert_eq!(cell_color(&image, 0), Some(Rgba::opaque(0, 255, 7)));
    assert_eq!(cell_color(&image, 1), None);
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(0)), Some(0));
}

#[test]
fn fila_11_paleta_de_256_entradas() {
    let image = indexada(256);
    assert_eq!(cell_count(&image), 256);
    assert_eq!(choice_for_cell(&image, 255), Some(ColorChoice::Entry(255)));
    assert_eq!(choice_for_cell(&image, 256), None);
    assert_eq!(
        pixel_for_choice(&image, ColorChoice::Entry(255)),
        Some(Pixel::Indexed(255))
    );
    assert_eq!(cell_of_choice(&image, ColorChoice::Entry(255)), Some(255));
}

#[test]
fn cell_color_es_lo_que_pinta_pixel_rgba_para_cada_eleccion() {
    // El color con el que se ve la celda es el de su `Pixel`.
    for (nombre, image) in todas() {
        for cell in 0..cell_count(&image) {
            let eleccion = choice_for_cell(&image, cell).unwrap();
            let pixel = pixel_for_choice(&image, eleccion).unwrap();
            assert_eq!(
                cell_color(&image, cell),
                px_ui::texture::pixel_rgba(&image, pixel),
                "{nombre}, celda {cell}"
            );
        }
    }
}

// Propiedad 4 (parte pura): la selección que se mantiene siempre vale.

#[derive(Clone, Copy, Debug)]
enum Accion {
    /// Clic en una celda cualquiera. Con `cerca`, relativa al número de celdas (incluye las que
    /// no existen justo después de la última).
    Clic { celda: usize, cerca: bool },
    /// Tecla 0 a 9.
    Tecla(u8),
}

fn accion() -> impl Strategy<Value = Accion> {
    prop_oneof![
        (0usize..300, any::<bool>()).prop_map(|(celda, cerca)| Accion::Clic { celda, cerca }),
        (0u8..=9).prop_map(Accion::Tecla),
    ]
}

fn imagen_para(modo: usize, colores: usize) -> Image {
    match modo {
        0 => imagen(Mode::Rgba),
        1 => imagen(Mode::Gray),
        2 => imagen(Mode::GrayAlpha),
        _ => indexada(colores),
    }
}

proptest! {
    #[test]
    fn propiedad_4_la_seleccion_que_se_mantiene_siempre_tiene_pixel(
        modo in 0usize..4,
        colores in 1usize..=256,
        acciones in vec(accion(), 0..60),
    ) {
        let image = imagen_para(modo, colores);
        let cuantas = cell_count(&image);
        let mut seleccion = initial_choice(&image);
        prop_assert!(pixel_for_choice(&image, seleccion).is_some());

        for accion in acciones {
            match accion {
                Accion::Clic { celda, cerca } => {
                    let celda = if cerca { celda % (cuantas + 4) } else { celda };
                    let eleccion = choice_for_cell(&image, celda);
                    // Solo hay elección en las celdas que existen.
                    prop_assert_eq!(eleccion.is_some(), celda < cuantas);
                    if let Some(nueva) = eleccion {
                        seleccion = nueva;
                        // Un clic en una celda que existe deja `cell_of_choice == Some(celda)`.
                        prop_assert_eq!(cell_of_choice(&image, seleccion), Some(celda));
                    }
                }
                Accion::Tecla(n) => {
                    let eleccion = key_choice(&image, n);
                    prop_assert_eq!(
                        eleccion.is_some(),
                        (1..=9).contains(&n) && usize::from(n) <= cuantas.min(9)
                    );
                    if let Some(nueva) = eleccion {
                        seleccion = nueva;
                        prop_assert_eq!(
                            cell_of_choice(&image, seleccion),
                            Some(usize::from(n) - 1)
                        );
                        // La tecla y el clic en la celda `n - 1` coinciden.
                        prop_assert_eq!(
                            choice_for_cell(&image, usize::from(n) - 1),
                            Some(seleccion)
                        );
                    }
                }
            }
            prop_assert!(
                pixel_for_choice(&image, seleccion).is_some(),
                "la selección {:?} no tiene color en {:?} de {} celdas",
                seleccion, image.mode(), cuantas
            );
            prop_assert!(cell_of_choice(&image, seleccion).is_some());
            prop_assert!(choice_label(&image, seleccion).is_some());
        }
    }
}
