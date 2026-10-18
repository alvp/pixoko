//! Tests de la parte pura de la unidad 11d (specs/hito-2-paleta-operaciones.md): añadir, quitar,
//! mover y rotar entradas de la paleta, sin `egui` ni ventana.
//!
//! Cubre las filas 3 a 8, 10 y 11 en su parte pura, la parte de longitudes de la fila 12 y las
//! propiedades 1 a 3. La fila 8 es solo `clamp_choice` (lo de `Session` es de `px-app`) y la
//! propiedad 4 es de `px-app`.
//!
//! Los números esperados salen de la spec, escritos como literales. Las entradas se numeran
//! desde 0. `n` = 1, 2 y 3 salvo que se diga. Paletas de la spec: `P5` = `[R, G, B, X, Y]` con
//! píxeles `[0, 1, 2, 3, 3]` y `F` = `[R, G, B, K]` con píxeles `[0, 1, 2, 3]`.
//!
//! Los modelos de las propiedades (destino del arrastre con `i128`, máquina del arrastre y
//! vector de identificadores de la propiedad 3) están escritos en el propio test y no llaman a
//! `entry_fate` ni a `choice_after`.

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Command, Document, Image, ImageId, Mode, Palette, Pixel, Rgba};
use px_ui::color::{ColorChoice, clamp_choice, info_flags, info_flags_range};
use px_ui::color_edit::{ColorEditor, EditTarget, StaleReason, stale_notice};
use px_ui::edit_layout::PaletteButton;
use px_ui::palette_grid::{
    CELL_U, COLUMNS, DRAG_START_U, DROP_HALF_GAP_U, DROP_MARK_W_U, GAP_U, GRID_X_U, GRID_Y_U, Grid,
    PITCH_U, RING_DEST_U, RING_RANGE_U, drop_mark_rect, edges_u, max_first_row, total_rows,
};
use px_ui::palette_ops::{
    DragEnd, Fate, MoveDrag, NOTHING_TO_ROTATE_NOTICE, PaletteOp, PaletteOps,
    REMOVAL_CANCELLED_NOTICE, Removal, RemovalWarning, blocked_reason, choice_after,
    default_destination, drag_labels, entry_fate, removal_prompt, removal_warning, rotation_range,
    summary, uses_entry,
};
use px_ui::status::columns;
use px_ui::text::{INTERFACE_CHARSET, fit_text, text_scale};
use px_ui::theme::{
    ACCENT, DROP_MARK, NOTICE_PREFIX, RING_DEST, RING_RANGE, SURFACE_1, TEXT, WARNING,
    contrast_ratio,
};
use px_ui::view::ScreenRect;

const ESCALAS: [u32; 3] = [1, 2, 3];
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const VERDE: Rgba = Rgba::opaque(0, 255, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);
const NEGRO: Rgba = Rgba::opaque(0, 0, 0);
const X: Rgba = Rgba::opaque(200, 30, 30);
const Y: Rgba = Rgba::opaque(10, 10, 240);

// Apoyo común.

fn rect(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

fn paleta(colores: &[Rgba], transparente: Option<u8>) -> Palette {
    let mut paleta = Palette::from_colors(colores.iter().copied()).expect("paleta válida");
    paleta
        .set_transparent_index(transparente)
        .expect("índice transparente válido");
    paleta
}

/// Imagen indexada de una fila con esos píxeles.
fn imagen_con(colores: &[Rgba], transparente: Option<u8>, pixeles: &[u8]) -> Image {
    let ancho = u32::try_from(pixeles.len()).expect("pocos píxeles");
    let mut imagen =
        Image::new_indexed(ancho, 1, paleta(colores, transparente)).expect("imagen válida");
    for (x, &indice) in pixeles.iter().enumerate() {
        imagen
            .set_pixel(x as u32, 0, Pixel::Indexed(indice))
            .expect("píxel válido");
    }
    imagen
}

/// `P5`: paleta `[R, G, B, X, Y]`, píxeles `[0, 1, 2, 3, 3]`.
fn imagen_p5(transparente: Option<u8>) -> Image {
    imagen_con(&[ROJO, VERDE, AZUL, X, Y], transparente, &[0, 1, 2, 3, 3])
}

/// `F`: paleta `[R, G, B, K]`, píxeles `[0, 1, 2, 3]`.
fn imagen_f(transparente: Option<u8>) -> Image {
    imagen_con(&[ROJO, VERDE, AZUL, NEGRO], transparente, &[0, 1, 2, 3])
}

fn pixeles(imagen: &Image) -> Vec<u8> {
    (0..imagen.width())
        .map(|x| match imagen.get_pixel(x, 0).expect("píxel dentro") {
            Pixel::Indexed(indice) => indice,
            otro => panic!("píxel no indexado: {otro:?}"),
        })
        .collect()
}

fn imagen_de(doc: &Document, id: ImageId) -> &Image {
    doc.image(id).expect("la imagen existe")
}

fn colores_de(doc: &Document, id: ImageId) -> Vec<Rgba> {
    imagen_de(doc, id)
        .palette()
        .expect("indexada")
        .colors()
        .to_vec()
}

fn en_el_juego(texto: &str) -> bool {
    texto.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

fn caracteres(texto: &str) -> usize {
    texto.chars().count()
}

const BOTONES: [PaletteButton; 6] = [
    PaletteButton::Add,
    PaletteButton::Remove,
    PaletteButton::Confirm,
    PaletteButton::Cancel,
    PaletteButton::RotateLeft,
    PaletteButton::RotateRight,
];

// Tipos y constantes.

#[test]
fn los_tipos_son_copy_debug_y_comparables() {
    fn comprueba<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
    comprueba::<PaletteOp>();
    comprueba::<Fate>();
    comprueba::<RemovalWarning>();
    comprueba::<DragEnd>();
    comprueba::<Removal>();
    assert_ne!(
        PaletteOp::Move { from: 1, to: 2 },
        PaletteOp::Move { from: 2, to: 1 }
    );
}

#[test]
fn las_constantes_de_la_rejilla_valen_lo_que_dice_la_spec() {
    assert_eq!(DRAG_START_U, 4);
    assert_eq!(DROP_HALF_GAP_U, 2);
    assert_eq!(RING_RANGE_U, 1);
    assert_eq!(RING_DEST_U, 2);
    assert_eq!(DROP_MARK_W_U, 2);
}

#[test]
fn las_constantes_de_la_rejilla_son_coherentes_con_la_separacion() {
    // La media separación de captura por cada lado cubre toda la separación de 4 u.
    assert_eq!(2 * DROP_HALF_GAP_U, GAP_U);
    // La barra cabe en la separación y no toca las muestras.
    const { assert!(DROP_MARK_W_U <= GAP_U) };
}

#[test]
fn los_colores_nuevos_del_tema_son_los_de_la_spec() {
    assert_eq!(RING_RANGE, ACCENT);
    assert_eq!(RING_DEST, WARNING);
    assert_eq!(DROP_MARK, TEXT);
}

#[test]
fn los_colores_nuevos_del_tema_tienen_contraste_de_3_a_1_con_surface_1() {
    for (nombre, color) in [
        ("RING_RANGE", RING_RANGE),
        ("RING_DEST", RING_DEST),
        ("DROP_MARK", DROP_MARK),
    ] {
        let contraste = contrast_ratio(color, SURFACE_1);
        assert!(contraste >= 3.0, "{nombre}: {contraste}");
    }
}

#[test]
fn edges_u_incluye_los_bordes_del_anillo_de_rango_del_destino_y_de_la_barra() {
    let bordes = edges_u();
    for paso in 0..COLUMNS as i64 {
        for (origen, nombre) in [(GRID_X_U, "x"), (GRID_Y_U, "y")] {
            let inicio = origen + paso * PITCH_U;
            let mut esperados = vec![
                // Anillo de rango (1 u) y de destino (2 u).
                inicio - RING_RANGE_U,
                inicio + CELL_U + RING_RANGE_U,
                inicio - RING_DEST_U,
                inicio + CELL_U + RING_DEST_U,
            ];
            if nombre == "x" {
                // Barra de 2 u: a 3 u a la izquierda o a 11 u a la derecha del origen.
                esperados.extend([inicio - 3, inicio - 1, inicio + 11, inicio + 13]);
            } else {
                // La barra empieza 2 u por encima de la muestra y mide 14 u.
                esperados.extend([inicio - 2, inicio + 12]);
            }
            for borde in esperados {
                assert!(
                    bordes.contains(&borde),
                    "falta el borde {borde} ({nombre}, paso {paso})"
                );
            }
        }
    }
}

// Fila 3: Añadir.

#[test]
fn fila_3_blocked_reason_de_anadir_con_256_255_y_1_entradas() {
    assert_eq!(
        blocked_reason(PaletteButton::Add, 256, false),
        Some("paleta llena: 256 entradas")
    );
    assert_eq!(
        blocked_reason(PaletteButton::Add, 256, true),
        Some("paleta llena: 256 entradas")
    );
    assert_eq!(blocked_reason(PaletteButton::Add, 255, false), None);
    assert_eq!(blocked_reason(PaletteButton::Add, 1, false), None);
}

#[test]
fn fila_3_resumen_de_anadir() {
    assert_eq!(
        summary(&PaletteOp::Add, 1, 4, false, false),
        "entrada 4 añadida (copia de la 1)"
    );
    assert_eq!(
        summary(&PaletteOp::Add, 3, 5, false, false),
        "entrada 5 añadida (copia de la 3)"
    );
    // La spec de Quitar: «entrada 5 añadida (copia de la 4)».
    assert_eq!(
        summary(&PaletteOp::Add, 4, 5, false, false),
        "entrada 5 añadida (copia de la 4)"
    );
}

#[test]
fn fila_3_la_seleccion_tras_anadir_es_la_nueva() {
    for (activa, largo) in [(0u8, 1usize), (1, 4), (3, 4), (254, 255)] {
        assert_eq!(
            choice_after(&PaletteOp::Add, activa, largo),
            largo as u8,
            "activa {activa}, largo {largo}"
        );
    }
}

#[test]
fn fila_3_anadir_sobre_f_copia_la_activa_y_deshacer_recorta_la_seleccion() {
    let mut doc = Document::new(imagen_f(None));
    let id = doc.main_image();
    let activa = 1u8;
    let color = imagen_de(&doc, id)
        .palette()
        .unwrap()
        .resolved(activa)
        .unwrap();
    assert!(
        doc.apply(Command::AddPaletteEntry { image: id, color })
            .unwrap()
    );
    let colores = colores_de(&doc, id);
    assert_eq!(colores.len(), 5);
    assert_eq!(colores[4], VERDE);

    let eleccion = ColorChoice::Entry(choice_after(&PaletteOp::Add, activa, 4));
    assert_eq!(eleccion, ColorChoice::Entry(4));

    doc.undo().unwrap();
    assert_eq!(colores_de(&doc, id).len(), 4);
    let tras_deshacer = clamp_choice(imagen_de(&doc, id), eleccion);
    assert_eq!(
        tras_deshacer,
        ColorChoice::Entry(3),
        "se recorta a la última"
    );

    doc.redo().unwrap();
    assert_eq!(colores_de(&doc, id).len(), 5);
    let tras_rehacer = clamp_choice(imagen_de(&doc, id), tras_deshacer);
    assert_eq!(
        tras_rehacer,
        ColorChoice::Entry(3),
        "no persigue a la entrada"
    );
}

#[test]
fn fila_3_anadir_la_copia_de_la_transparente_es_alfa_0_y_no_es_la_transparente() {
    let mut doc = Document::new(imagen_f(Some(3)));
    let id = doc.main_image();
    let color = imagen_de(&doc, id).palette().unwrap().resolved(3).unwrap();
    assert_eq!(color, Rgba::new(0, 0, 0, 0));
    doc.apply(Command::AddPaletteEntry { image: id, color })
        .unwrap();
    let paleta = imagen_de(&doc, id).palette().unwrap();
    assert_eq!(paleta.len(), 5);
    assert_eq!(paleta.get(4).unwrap(), Rgba::new(0, 0, 0, 0));
    assert_eq!(paleta.transparent_index(), Some(3));
}

#[test]
fn fila_3_con_255_entradas_se_llega_a_256_y_el_siguiente_se_desactiva() {
    assert_eq!(blocked_reason(PaletteButton::Add, 255, true), None);
    // Tras añadir, la paleta tiene 256 y el botón queda desactivado.
    assert_eq!(
        choice_after(&PaletteOp::Add, 10, 255),
        255,
        "la nueva es la 255"
    );
    assert!(blocked_reason(PaletteButton::Add, 256, true).is_some());
}

#[test]
fn fila_11_con_una_entrada_anadir_da_la_1() {
    assert_eq!(blocked_reason(PaletteButton::Add, 1, false), None);
    assert_eq!(choice_after(&PaletteOp::Add, 0, 1), 1);
}

// Filas 3 y 4: avisos de los botones desactivados.

#[test]
fn fila_4_blocked_reason_de_quitar_y_confirmar() {
    assert_eq!(
        blocked_reason(PaletteButton::Remove, 1, false),
        Some("no se puede quitar la única entrada")
    );
    assert_eq!(
        blocked_reason(PaletteButton::Remove, 0, false),
        Some("no se puede quitar la única entrada")
    );
    assert_eq!(blocked_reason(PaletteButton::Remove, 2, false), None);
    assert_eq!(blocked_reason(PaletteButton::Remove, 256, false), None);
    assert_eq!(
        blocked_reason(PaletteButton::Confirm, 5, false),
        Some("elige un destino pulsando una muestra")
    );
    assert_eq!(blocked_reason(PaletteButton::Confirm, 5, true), None);
}

#[test]
fn rotar_y_cancelar_nunca_se_desactivan() {
    for largo in [0usize, 1, 2, 255, 256] {
        for destino in [false, true] {
            for boton in [
                PaletteButton::RotateLeft,
                PaletteButton::RotateRight,
                PaletteButton::Cancel,
            ] {
                assert_eq!(
                    blocked_reason(boton, largo, destino),
                    None,
                    "{boton:?}, largo {largo}, destino {destino}"
                );
            }
        }
    }
}

#[test]
fn blocked_reason_solo_depende_de_lo_que_dice_la_spec() {
    for largo in [1usize, 2, 100, 255, 256] {
        for destino in [false, true] {
            let anadir = blocked_reason(PaletteButton::Add, largo, destino).is_some();
            assert_eq!(anadir, largo >= 256, "Add, largo {largo}");
            let quitar = blocked_reason(PaletteButton::Remove, largo, destino).is_some();
            assert_eq!(quitar, largo <= 1, "Remove, largo {largo}");
            let confirmar = blocked_reason(PaletteButton::Confirm, largo, destino).is_some();
            assert_eq!(confirmar, !destino, "Confirm, destino {destino}");
        }
    }
}

#[test]
fn las_constantes_de_aviso_son_las_de_la_spec() {
    assert_eq!(
        REMOVAL_CANCELLED_NOTICE,
        "quitar cancelado: el documento cambió"
    );
    assert_eq!(NOTHING_TO_ROTATE_NOTICE, "nada que rotar: una sola entrada");
    assert_eq!(
        stale_notice(StaleReason::Moved),
        "texto descartado: la entrada se movió"
    );
}

// Fila 4: Quitar.

#[test]
fn fila_4_default_destination_de_p5() {
    let p5 = imagen_p5(None);
    let paleta = p5.palette().unwrap();
    assert_eq!(default_destination(paleta, 3), Some(0), "X → R");
    assert_eq!(default_destination(paleta, 4), Some(2), "Y → B");
    assert_eq!(
        default_destination(paleta, 0),
        Some(3),
        "R → X, en el resto [G, B, X, Y] gana X (índice 3 antes de quitar)"
    );
}

#[test]
fn fila_4_default_destination_de_rgb_quitando_la_1_gana_el_menor_indice_en_el_empate() {
    let rgb = paleta(&[ROJO, VERDE, AZUL], None);
    assert_eq!(default_destination(&rgb, 1), Some(0));
}

#[test]
fn fila_4_uses_entry_es_falso_solo_para_la_4_en_p5() {
    let p5 = imagen_p5(None);
    for (indice, esperado) in [(0u8, true), (1, true), (2, true), (3, true), (4, false)] {
        assert_eq!(uses_entry(&p5, indice), esperado, "entrada {indice}");
    }
    assert!(!uses_entry(&p5, 200), "una entrada que no existe no se usa");
}

#[test]
fn fila_4_uses_entry_encuentra_un_pixel_en_cualquier_posicion() {
    let paleta_de_3 = paleta(&[ROJO, VERDE, AZUL], None);
    let mut imagen = Image::new_indexed(7, 5, paleta_de_3).unwrap();
    assert!(uses_entry(&imagen, 0), "todos los píxeles empiezan en 0");
    assert!(!uses_entry(&imagen, 2));
    imagen.set_pixel(6, 4, Pixel::Indexed(2)).unwrap();
    assert!(uses_entry(&imagen, 2), "último píxel");
    imagen.set_pixel(6, 4, Pixel::Indexed(0)).unwrap();
    imagen.set_pixel(0, 0, Pixel::Indexed(1)).unwrap();
    assert!(uses_entry(&imagen, 1), "primer píxel");
    assert!(uses_entry(&imagen, 0));
    imagen.set_pixel(0, 0, Pixel::Indexed(0)).unwrap();
    assert!(!uses_entry(&imagen, 1));
}

#[test]
fn fila_4_quitar_la_3_de_p5_pasa_los_pixeles_a_la_2_y_un_undo_la_restaura() {
    let mut doc = Document::new(imagen_p5(None));
    let id = doc.main_image();
    let inicial = imagen_de(&doc, id).clone();
    let (indice, destino) = (3u8, 2u8);
    let op = PaletteOp::Remove {
        index: indice,
        into: destino,
    };

    assert!(
        doc.apply(Command::RemovePaletteEntry {
            image: id,
            index: indice,
            into: destino
        })
        .unwrap()
    );
    assert_eq!(colores_de(&doc, id), vec![ROJO, VERDE, AZUL, Y]);
    assert_eq!(pixeles(imagen_de(&doc, id)), vec![0, 1, 2, 2, 2]);
    assert_eq!(choice_after(&op, 3, 5), 3, "la selección es la 3 (Y)");
    assert_eq!(
        summary(&op, 3, 5, false, false),
        "entrada 3 quitada; píxeles pasan a la 2"
    );

    doc.undo().unwrap();
    assert_eq!(imagen_de(&doc, id), &inicial);
    assert!(!doc.can_undo(), "un solo paso");
}

#[test]
fn fila_4_quitar_la_4_sin_pixeles_se_aplica_con_el_destino_por_defecto() {
    let mut doc = Document::new(imagen_p5(None));
    let id = doc.main_image();
    let destino = default_destination(imagen_de(&doc, id).palette().unwrap(), 4).unwrap();
    assert_eq!(destino, 2);
    let op = PaletteOp::Remove {
        index: 4,
        into: destino,
    };
    doc.apply(Command::RemovePaletteEntry {
        image: id,
        index: 4,
        into: destino,
    })
    .unwrap();
    assert_eq!(colores_de(&doc, id), vec![ROJO, VERDE, AZUL, X]);
    assert_eq!(choice_after(&op, 4, 5), 3, "se recorta a la última");
    assert_eq!(
        summary(&op, 4, 5, false, true),
        "entrada 4 quitada (sin píxeles)",
        "con `unused` verdadero"
    );
}

#[test]
fn fila_4_resumen_de_quitar_con_pixeles_usa_la_numeracion_posterior() {
    // `into` por debajo de `index`: no cambia.
    assert_eq!(
        summary(&PaletteOp::Remove { index: 3, into: 0 }, 3, 5, false, false),
        "entrada 3 quitada; píxeles pasan a la 0"
    );
    assert_eq!(
        summary(&PaletteOp::Remove { index: 1, into: 0 }, 1, 5, false, false),
        "entrada 1 quitada; píxeles pasan a la 0"
    );
    // `into` por encima de `index`: baja un lugar.
    assert_eq!(
        summary(&PaletteOp::Remove { index: 3, into: 4 }, 3, 5, false, false),
        "entrada 3 quitada; píxeles pasan a la 3"
    );
    assert_eq!(
        summary(&PaletteOp::Remove { index: 0, into: 1 }, 0, 5, false, false),
        "entrada 0 quitada; píxeles pasan a la 0"
    );
    assert_eq!(
        summary(&PaletteOp::Remove { index: 0, into: 4 }, 0, 5, false, false),
        "entrada 0 quitada; píxeles pasan a la 3"
    );
}

#[test]
fn fila_4_resumen_de_quitar_sin_pixeles() {
    assert_eq!(
        summary(&PaletteOp::Remove { index: 4, into: 2 }, 4, 5, false, true),
        "entrada 4 quitada (sin píxeles)"
    );
    assert_eq!(
        summary(&PaletteOp::Remove { index: 0, into: 1 }, 0, 5, false, true),
        "entrada 0 quitada (sin píxeles)"
    );
}

#[test]
fn fila_4_las_lineas_del_editor_con_quita_pendiente() {
    assert_eq!(
        removal_prompt(3, Some(0), None),
        [
            "quitar 3; destino 0".to_string(),
            "elige otra muestra o confirma".to_string()
        ]
    );
    assert_eq!(
        removal_prompt(3, Some(2), None),
        [
            "quitar 3; destino 2".to_string(),
            "elige otra muestra o confirma".to_string()
        ]
    );
}

// Fila 5: Quitar con la transparente, la única y los duplicados.

#[test]
fn fila_5_la_transparente_no_tiene_destino_por_defecto() {
    let p5 = imagen_p5(Some(3));
    assert_eq!(default_destination(p5.palette().unwrap(), 3), None);
    // En 0 y en la última también.
    let en_0 = paleta(&[ROJO, VERDE, AZUL], Some(0));
    assert_eq!(default_destination(&en_0, 0), None);
    let en_la_ultima = paleta(&[ROJO, VERDE, AZUL], Some(2));
    assert_eq!(default_destination(&en_la_ultima, 2), None);
}

#[test]
fn fila_5_una_paleta_de_menos_de_dos_entradas_no_tiene_destino() {
    assert_eq!(default_destination(&paleta(&[ROJO], None), 0), None);
    assert_eq!(default_destination(&Palette::new(), 0), None);
}

#[test]
fn fila_5_quitar_la_transparente_avisa_de_que_sus_pixeles_quedan_opacos() {
    let p5 = imagen_p5(Some(3));
    let paleta = p5.palette().unwrap();
    for destino in [None, Some(0), Some(4)] {
        assert_eq!(
            removal_warning(paleta, 3, destino),
            Some(RemovalWarning::PixelsOpaque),
            "destino {destino:?}"
        );
    }
}

#[test]
fn fila_5_con_destino_la_transparente_el_aviso_es_destino_transparente() {
    let p5 = imagen_p5(Some(3));
    let paleta = p5.palette().unwrap();
    assert_eq!(
        removal_warning(paleta, 0, Some(3)),
        Some(RemovalWarning::DestinationTransparent)
    );
    assert_eq!(removal_warning(paleta, 0, Some(4)), None);
    assert_eq!(
        removal_warning(paleta, 0, None),
        None,
        "sin destino y sin ser la transparente no hay aviso"
    );
}

#[test]
fn fila_5_sin_transparente_nunca_hay_aviso() {
    let p5 = imagen_p5(None);
    let paleta = p5.palette().unwrap();
    for indice in 0..5u8 {
        for destino in [None, Some(0), Some(1), Some(4)] {
            assert_eq!(removal_warning(paleta, indice, destino), None);
        }
    }
}

#[test]
fn fila_5_las_lineas_del_editor_con_avisos() {
    assert_eq!(
        removal_prompt(3, None, Some(RemovalWarning::PixelsOpaque)),
        [
            "quitar 3: pulsa un destino".to_string(),
            "aviso: sus píxeles quedan opacos".to_string()
        ]
    );
    // Al pulsar la muestra 4 (con el aviso de que sus píxeles quedan opacos).
    let [linea1, linea2] = removal_prompt(3, Some(4), Some(RemovalWarning::PixelsOpaque));
    assert_eq!(linea1, "quitar 3; destino 4");
    assert_eq!(linea2, "aviso: sus píxeles quedan opacos");
    assert_eq!(
        removal_prompt(0, Some(3), Some(RemovalWarning::DestinationTransparent)),
        [
            "quitar 0; destino 3".to_string(),
            "aviso: destino transparente".to_string()
        ]
    );
}

#[test]
fn fila_5_quitar_la_primera_linea_sin_destino_dice_pulsa_un_destino() {
    for indice in [0u8, 3, 255] {
        let [linea1, _] = removal_prompt(indice, None, Some(RemovalWarning::PixelsOpaque));
        assert_eq!(linea1, format!("quitar {indice}: pulsa un destino"));
    }
}

#[test]
fn fila_5_quitar_la_transparente_con_destino_la_4_deja_la_y_opaca() {
    let mut doc = Document::new(imagen_p5(Some(3)));
    let id = doc.main_image();
    let inicial = imagen_de(&doc, id).clone();
    assert!(
        doc.apply(Command::RemovePaletteEntry {
            image: id,
            index: 3,
            into: 4
        })
        .unwrap()
    );
    let imagen = imagen_de(&doc, id);
    let paleta = imagen.palette().unwrap();
    assert_eq!(paleta.transparent_index(), None);
    assert_eq!(paleta.colors(), &[ROJO, VERDE, AZUL, Y]);
    assert_eq!(pixeles(imagen), vec![0, 1, 2, 3, 3], "ahora Y opaco");
    doc.undo().unwrap();
    assert_eq!(imagen_de(&doc, id), &inicial);
}

#[test]
fn fila_5_quitar_la_transparente_sin_pixeles_dice_que_era_la_transparente() {
    let op = PaletteOp::Remove { index: 5, into: 0 };
    assert_eq!(
        summary(&op, 5, 6, true, true),
        "entrada 5 quitada; era la transparente"
    );
    // Con píxeles que la usan, el resumen es el normal.
    assert_eq!(
        summary(&op, 5, 6, true, false),
        "entrada 5 quitada; píxeles pasan a la 0"
    );
    // Sin ser la transparente, `unused` da el de sin píxeles.
    assert_eq!(
        summary(&op, 5, 6, false, true),
        "entrada 5 quitada (sin píxeles)"
    );
}

#[test]
fn fila_5_con_duplicados_el_destino_es_el_primero_y_sin_aviso() {
    let aab = paleta(&[ROJO, ROJO, AZUL], None);
    assert_eq!(default_destination(&aab, 1), Some(0), "distancia 0");
    assert_eq!(default_destination(&aab, 0), Some(1));
    assert_eq!(removal_warning(&aab, 1, Some(0)), None);
    assert_eq!(removal_warning(&aab, 0, Some(1)), None);
}

#[test]
fn fila_5_con_tres_duplicados_el_destino_de_cualquiera_es_el_primero_de_los_otros() {
    let aaa = paleta(&[ROJO, ROJO, ROJO, AZUL], None);
    assert_eq!(default_destination(&aaa, 2), Some(0));
    assert_eq!(default_destination(&aaa, 1), Some(0));
    assert_eq!(default_destination(&aaa, 0), Some(1));
}

#[test]
fn fila_5_con_256_entradas_quitar_la_255_deja_la_254() {
    let op = PaletteOp::Remove {
        index: 255,
        into: 0,
    };
    assert_eq!(choice_after(&op, 255, 256), 254);
    assert_eq!(blocked_reason(PaletteButton::Remove, 256, false), None);
}

#[test]
fn fila_5_la_transparente_se_reajusta_en_el_resto_al_buscar_el_destino() {
    // [D, T (transparente, guardada blanca), W]: quitando la 0 el resto es [T, W] y la
    // transparente es la primera del resto. Gana T (índice 1 antes de quitar). Si no se
    // reajustara, la transparente sería W (índice 2).
    let oscura = Rgba::opaque(5, 5, 5);
    let blanca = Rgba::opaque(255, 255, 255);
    let tras = paleta(&[oscura, blanca, blanca], Some(1));
    assert_eq!(default_destination(&tras, 0), Some(1));

    // La transparente antes de la quitada: [T, D, W], quitando la 1.
    let antes = paleta(&[blanca, oscura, blanca], Some(0));
    assert_eq!(default_destination(&antes, 1), Some(0));
}

#[test]
fn fila_5_el_destino_por_defecto_puede_ser_una_transparente_y_entonces_hay_aviso() {
    // [D, W, T]: quitando D (50, 0, 0), la transparente (distancia 67525·255²) queda más cerca
    // que W (172075·255²).
    let oscura = Rgba::opaque(50, 0, 0);
    let blanca = Rgba::opaque(255, 255, 255);
    let con_transparente = paleta(&[oscura, blanca, blanca], Some(2));
    let destino = default_destination(&con_transparente, 0);
    assert_eq!(destino, Some(2));
    assert_eq!(
        removal_warning(&con_transparente, 0, destino),
        Some(RemovalWarning::DestinationTransparent)
    );
}

// Fila 6: Rotar.

#[test]
fn fila_6_rotation_range_de_la_spec() {
    assert_eq!(rotation_range(4, 1, Some(3)), (1, 3));
    assert_eq!(rotation_range(4, 3, Some(1)), (1, 3));
    assert_eq!(rotation_range(256, 0, None), (0, 256));
    assert_eq!(rotation_range(4, 2, Some(2)), (2, 1));
}

#[test]
fn fila_6_sin_rango_se_rota_toda_la_paleta() {
    for largo in [1usize, 2, 5, 255, 256] {
        assert_eq!(rotation_range(largo, 0, None), (0, largo as u16));
        assert_eq!(
            rotation_range(largo, (largo - 1) as u8, None),
            (0, largo as u16),
            "la activa no importa sin rango"
        );
    }
}

#[test]
fn fila_11_un_rango_de_toda_la_paleta_es_lo_mismo_que_sin_rango() {
    assert_eq!(rotation_range(4, 0, Some(3)), rotation_range(4, 0, None));
    assert_eq!(rotation_range(4, 3, Some(0)), rotation_range(4, 0, None));
    assert_eq!(rotation_range(256, 0, Some(255)), (0, 256));
}

#[test]
fn fila_11_un_rango_de_una_entrada_o_una_paleta_de_una_entrada_da_longitud_1() {
    assert_eq!(rotation_range(1, 0, None), (0, 1));
    assert_eq!(rotation_range(4, 2, Some(2)), (2, 1));
    assert_eq!(rotation_range(256, 255, Some(255)), (255, 1));
}

proptest! {
    #[test]
    fn rotation_range_es_el_minimo_y_la_longitud_del_tramo(
        largo in 1usize..=256,
        activa_cruda in any::<u8>(),
        otro_crudo in proptest::option::of(any::<u8>()),
    ) {
        let activa = (usize::from(activa_cruda) % largo) as u8;
        let otro = otro_crudo.map(|crudo| (usize::from(crudo) % largo) as u8);
        let (inicio, longitud) = rotation_range(largo, activa, otro);
        match otro {
            None => prop_assert_eq!((inicio, longitud), (0, largo as u16)),
            Some(otro) => {
                prop_assert_eq!(inicio, activa.min(otro));
                prop_assert_eq!(longitud, u16::from(activa.max(otro) - activa.min(otro)) + 1);
            }
        }
        prop_assert!(longitud >= 1);
        prop_assert!(usize::from(inicio) + usize::from(longitud) <= largo);
    }
}

#[test]
fn fila_6_el_signo_de_rotate_colors_derecha_es_mas_1_sobre_f() {
    let mut doc = Document::new(imagen_f(None));
    let id = doc.main_image();
    let inicial = imagen_de(&doc, id).clone();
    let [a, b, c, d] = [ROJO, VERDE, AZUL, NEGRO];

    assert!(
        doc.apply(Command::RotateColors {
            image: id,
            start: 0,
            len: 4,
            steps: 1
        })
        .unwrap()
    );
    assert_eq!(colores_de(&doc, id), vec![d, a, b, c], "+1: [D, A, B, C]");
    assert_eq!(
        pixeles(imagen_de(&doc, id)),
        vec![0, 1, 2, 3],
        "los píxeles no cambian"
    );
    doc.undo().unwrap();
    assert_eq!(imagen_de(&doc, id), &inicial);

    assert!(
        doc.apply(Command::RotateColors {
            image: id,
            start: 0,
            len: 4,
            steps: -1
        })
        .unwrap()
    );
    assert_eq!(colores_de(&doc, id), vec![b, c, d, a], "−1: [B, C, D, A]");
}

#[test]
fn fila_6_rotar_sin_rango_con_la_activa_2_da_lo_mismo_que_el_comando_directo() {
    for (pasos, esperado) in [
        (1, vec![NEGRO, ROJO, VERDE, AZUL]),
        (-1, vec![VERDE, AZUL, NEGRO, ROJO]),
    ] {
        let mut doc = Document::new(imagen_f(None));
        let id = doc.main_image();
        let (inicio, longitud) = rotation_range(4, 2, None);
        doc.apply(Command::RotateColors {
            image: id,
            start: inicio,
            len: longitud,
            steps: pasos,
        })
        .unwrap();
        assert_eq!(colores_de(&doc, id), esperado, "steps {pasos}");
        // La selección se queda en la 2.
        let op = PaletteOp::Rotate {
            start: inicio,
            len: longitud,
            steps: pasos,
        };
        assert_eq!(choice_after(&op, 2, 4), 2);
    }
}

#[test]
fn fila_6_con_rango_de_1_a_3_desde_la_1_y_la_activa_1() {
    // `Mayús`+clic en la 3 desde `Entry(1)`: rango (1, 3).
    let (inicio, longitud) = rotation_range(4, 1, Some(3));
    for (pasos, esperado) in [
        (1, vec![ROJO, NEGRO, VERDE, AZUL]),
        (-1, vec![ROJO, AZUL, NEGRO, VERDE]),
    ] {
        let mut doc = Document::new(imagen_f(None));
        let id = doc.main_image();
        doc.apply(Command::RotateColors {
            image: id,
            start: inicio,
            len: longitud,
            steps: pasos,
        })
        .unwrap();
        assert_eq!(colores_de(&doc, id), esperado, "steps {pasos}");
        assert_eq!(pixeles(imagen_de(&doc, id)), vec![0, 1, 2, 3]);
    }
}

#[test]
fn fila_6_con_la_transparente_dentro_del_rango_se_queda_en_su_ranura() {
    let mut doc = Document::new(imagen_f(Some(3)));
    let id = doc.main_image();
    doc.apply(Command::RotateColors {
        image: id,
        start: 0,
        len: 4,
        steps: 1,
    })
    .unwrap();
    let paleta = imagen_de(&doc, id).palette().unwrap();
    assert_eq!(paleta.transparent_index(), Some(3));
    assert_eq!(paleta.colors(), &[NEGRO, ROJO, VERDE, AZUL]);
}

#[test]
fn fila_6_resumenes_de_rotar() {
    let rango = PaletteOp::Rotate {
        start: 1,
        len: 3,
        steps: 1,
    };
    assert_eq!(
        summary(&rango, 1, 4, false, false),
        "rotadas entradas 1 a 3 a la derecha"
    );
    let rango_izquierda = PaletteOp::Rotate {
        start: 1,
        len: 3,
        steps: -1,
    };
    assert_eq!(
        summary(&rango_izquierda, 1, 4, false, false),
        "rotadas entradas 1 a 3 a la izquierda"
    );
    let toda = PaletteOp::Rotate {
        start: 0,
        len: 4,
        steps: 1,
    };
    assert_eq!(
        summary(&toda, 2, 4, false, false),
        "paleta rotada a la derecha"
    );
    let toda_izquierda = PaletteOp::Rotate {
        start: 0,
        len: 4,
        steps: -1,
    };
    assert_eq!(
        summary(&toda_izquierda, 2, 4, false, false),
        "paleta rotada a la izquierda"
    );
}

#[test]
fn fila_6_un_tramo_que_empieza_en_0_pero_no_llega_al_final_no_es_toda_la_paleta() {
    let op = PaletteOp::Rotate {
        start: 0,
        len: 3,
        steps: -1,
    };
    assert_eq!(
        summary(&op, 0, 4, false, false),
        "rotadas entradas 0 a 2 a la izquierda"
    );
    let op = PaletteOp::Rotate {
        start: 1,
        len: 3,
        steps: 1,
    };
    assert_eq!(
        summary(&op, 0, 4, false, false),
        "rotadas entradas 1 a 3 a la derecha",
        "empieza en 1: no es toda"
    );
    let toda = PaletteOp::Rotate {
        start: 0,
        len: 256,
        steps: 1,
    };
    assert_eq!(
        summary(&toda, 0, 256, false, false),
        "paleta rotada a la derecha"
    );
}

#[test]
fn fila_6_el_signo_del_resumen_sale_de_steps() {
    for (pasos, lado) in [
        (1, "derecha"),
        (5, "derecha"),
        (-1, "izquierda"),
        (-7, "izquierda"),
    ] {
        let op = PaletteOp::Rotate {
            start: 0,
            len: 4,
            steps: pasos,
        };
        assert_eq!(
            summary(&op, 0, 4, false, false),
            format!("paleta rotada a la {lado}"),
            "steps {pasos}"
        );
    }
}

#[test]
fn fila_6_la_seleccion_tras_rotar_es_la_misma() {
    for activa in [0u8, 2, 3] {
        let op = PaletteOp::Rotate {
            start: 1,
            len: 3,
            steps: 1,
        };
        assert_eq!(choice_after(&op, activa, 4), activa);
    }
}

#[test]
fn fila_13_rotar_un_rango_de_una_entrada_da_ok_false_sin_paso() {
    let mut doc = Document::new(imagen_f(None));
    let id = doc.main_image();
    let estado = doc.state_id();
    for pasos in [1, -1] {
        let (inicio, longitud) = rotation_range(4, 2, Some(2));
        assert_eq!(longitud, 1);
        assert!(
            !doc.apply(Command::RotateColors {
                image: id,
                start: inicio,
                len: longitud,
                steps: pasos
            })
            .unwrap()
        );
    }
    // Una paleta de una entrada, igual.
    let mut una = Document::new(imagen_con(&[ROJO], None, &[0, 0]));
    let id_una = una.main_image();
    let estado_una = una.state_id();
    let (inicio, longitud) = rotation_range(1, 0, None);
    assert!(
        !una.apply(Command::RotateColors {
            image: id_una,
            start: inicio,
            len: longitud,
            steps: 1
        })
        .unwrap()
    );
    assert_eq!(una.state_id(), estado_una);
    assert!(!una.can_undo());
    assert_eq!(doc.state_id(), estado);
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
}

// Fila 7: Mover.

#[test]
fn fila_7_drop_index_con_256_entradas_y_16_filas() {
    let casos: [((i64, i64), Option<usize>); 10] = [
        ((-3, 0), None),
        ((-2, 0), Some(0)),
        ((0, 0), Some(0)),
        ((11, 0), Some(0)),
        ((12, 0), Some(1)),
        ((221, 0), Some(15)),
        ((222, 0), None),
        ((0, 12), Some(16)),
        ((0, 221), Some(240)),
        ((0, 222), None),
    ];
    for n in ESCALAS {
        let k = i64::from(n);
        let rejilla = Grid::new(n, 256, 16, 0);
        for ((x, y), esperado) in casos {
            assert_eq!(
                rejilla.drop_index(x * k, y * k),
                esperado,
                "n={n}, punto ({x}, {y})"
            );
        }
    }
}

#[test]
fn fila_7_drop_index_con_9_entradas_la_ultima_fila_cuenta_como_el_final() {
    for n in ESCALAS {
        let k = i64::from(n);
        let rejilla = Grid::new(n, 9, 16, 0);
        assert_eq!(rejilla.drop_index(126 * k, 0), Some(8), "n={n}");
        assert_eq!(rejilla.drop_index(0, 14 * k), None, "n={n}: no hay fila 1");
        // Las muestras que existen se encuentran por su esquina.
        for i in 0..9i64 {
            assert_eq!(
                rejilla.drop_index(i * 14 * k, 0),
                Some(i as usize),
                "n={n}, i={i}"
            );
        }
    }
}

#[test]
fn fila_7_drop_index_con_first_row_3_y_5_filas_visibles() {
    for n in ESCALAS {
        let k = i64::from(n);
        let rejilla = Grid::new(n, 256, 5, 3);
        assert_eq!(rejilla.drop_index(0, 0), Some(48), "n={n}");
        assert_eq!(rejilla.drop_index(0, 67 * k), Some(112), "n={n}");
        assert_eq!(rejilla.drop_index(0, 68 * k), None, "n={n}");
    }
}

#[test]
fn fila_7_drop_index_con_20_entradas_la_columna_de_mas_es_la_ultima() {
    for n in ESCALAS {
        let k = i64::from(n);
        let rejilla = Grid::new(n, 20, 16, 0);
        assert_eq!(rejilla.drop_index(42 * k, 14 * k), Some(19), "n={n}");
        assert_eq!(rejilla.drop_index(56 * k, 14 * k), Some(19), "n={n}");
        assert_eq!(rejilla.drop_index(210 * k, 14 * k), Some(19), "n={n}");
        assert_eq!(rejilla.drop_index(0, 14 * k), Some(16), "n={n}");
    }
}

#[test]
fn fila_7_drop_index_sin_muestras_o_sin_filas_visibles_es_none() {
    for n in ESCALAS {
        let k = i64::from(n);
        for (x, y) in [(0, 0), (-2 * k, -2 * k), (5 * k, 5 * k)] {
            assert_eq!(
                Grid::new(n, 0, 16, 0).drop_index(x, y),
                None,
                "sin muestras"
            );
            assert_eq!(Grid::new(n, 256, 0, 0).drop_index(x, y), None, "sin filas");
        }
    }
}

#[test]
fn fila_7_drop_index_de_una_sola_entrada_siempre_cae_en_ella() {
    for n in ESCALAS {
        let k = i64::from(n);
        let rejilla = Grid::new(n, 1, 16, 0);
        for x in [-2 * k, 0, 11 * k, 12 * k, 100 * k, 221 * k] {
            assert_eq!(rejilla.drop_index(x, 0), Some(0), "n={n}, x={x}");
        }
        assert_eq!(rejilla.drop_index(222 * k, 0), None);
    }
}

#[test]
fn fila_7_drop_index_de_la_muestra_encontrada_por_index_at_es_la_misma() {
    for n in ESCALAS {
        let rejilla = Grid::new(n, 100, 8, 2);
        for i in 0..100usize {
            if let Some(celda) = rejilla.cell_rect(i) {
                for (x, y) in [
                    (celda.x, celda.y),
                    (celda.x + celda.width - 1, celda.y + celda.height - 1),
                ] {
                    assert_eq!(rejilla.drop_index(x, y), Some(i), "n={n}, i={i}");
                }
            }
        }
    }
}

#[test]
fn fila_7_drop_mark_rect_a_la_izquierda_y_a_la_derecha() {
    for n in ESCALAS {
        let k = i64::from(n);
        let rejilla = Grid::new(n, 256, 16, 0);
        for i in [0usize, 1, 5, 15, 16, 100, 255] {
            let celda = rejilla.cell_rect(i).unwrap();
            assert_eq!(
                drop_mark_rect(celda, false, n),
                rect(celda.x - 3 * k, celda.y - 2 * k, 2 * k, 14 * k),
                "izquierda, n={n}, i={i}"
            );
            assert_eq!(
                drop_mark_rect(celda, true, n),
                rect(celda.x + 11 * k, celda.y - 2 * k, 2 * k, 14 * k),
                "derecha, n={n}, i={i}"
            );
        }
    }
}

#[test]
fn fila_7_la_barra_a_la_derecha_de_una_muestra_es_la_de_la_izquierda_de_la_siguiente() {
    for n in ESCALAS {
        let rejilla = Grid::new(n, 256, 16, 0);
        for i in [0usize, 1, 7, 14, 16, 100, 238] {
            if i % COLUMNS == COLUMNS - 1 {
                continue;
            }
            let (celda, siguiente) = (
                rejilla.cell_rect(i).unwrap(),
                rejilla.cell_rect(i + 1).unwrap(),
            );
            assert_eq!(
                drop_mark_rect(celda, true, n),
                drop_mark_rect(siguiente, false, n),
                "n={n}, i={i}"
            );
        }
    }
}

#[test]
fn fila_7_la_barra_cabe_en_la_separacion_y_cubre_el_alto_de_la_muestra() {
    for n in ESCALAS {
        let rejilla = Grid::new(n, 256, 16, 0);
        let celda = rejilla.cell_rect(37).unwrap();
        for despues in [false, true] {
            let barra = drop_mark_rect(celda, despues, n);
            let (izquierda, derecha) = (barra.x, barra.x + barra.width);
            if despues {
                assert!(
                    izquierda >= celda.x + celda.width && derecha <= celda.x + 14 * i64::from(n)
                );
            } else {
                assert!(derecha <= celda.x && izquierda >= celda.x - 4 * i64::from(n));
            }
            assert!(barra.y <= celda.y && barra.y + barra.height >= celda.y + celda.height);
        }
    }
}

#[test]
fn fila_7_drag_labels() {
    assert_eq!(
        drag_labels(5, Some(9)),
        ["mover 5 a 9".to_string(), "Escape cancela".to_string()]
    );
    assert_eq!(
        drag_labels(5, None),
        [
            "mover 5: suelta en la rejilla".to_string(),
            "Escape cancela".to_string()
        ]
    );
    assert_eq!(drag_labels(0, Some(255))[0], "mover 0 a 255");
}

#[test]
fn fila_7_resumen_de_mover() {
    assert_eq!(
        summary(&PaletteOp::Move { from: 5, to: 9 }, 5, 12, false, false),
        "entrada 5 movida a la posición 9"
    );
    assert_eq!(
        summary(&PaletteOp::Move { from: 0, to: 2 }, 0, 4, false, false),
        "entrada 0 movida a la posición 2"
    );
}

#[test]
fn fila_7_la_seleccion_tras_mover_es_to() {
    assert_eq!(choice_after(&PaletteOp::Move { from: 0, to: 2 }, 0, 4), 2);
    assert_eq!(choice_after(&PaletteOp::Move { from: 9, to: 1 }, 9, 12), 1);
    assert_eq!(choice_after(&PaletteOp::Move { from: 4, to: 4 }, 4, 12), 4);
}

#[test]
fn fila_7_mover_de_0_a_2_en_f_da_g_b_r_k_y_un_solo_undo() {
    let mut doc = Document::new(imagen_f(None));
    let id = doc.main_image();
    let inicial = imagen_de(&doc, id).clone();
    assert!(
        doc.apply(Command::MovePaletteEntry {
            image: id,
            from: 0,
            to: 2
        })
        .unwrap()
    );
    assert_eq!(colores_de(&doc, id), vec![VERDE, AZUL, ROJO, NEGRO]);
    assert_eq!(pixeles(imagen_de(&doc, id)), vec![2, 0, 1, 3]);
    doc.undo().unwrap();
    assert_eq!(imagen_de(&doc, id), &inicial);
    assert!(!doc.can_undo(), "un solo paso");
    // A la misma posición no hay paso.
    assert!(
        !doc.apply(Command::MovePaletteEntry {
            image: id,
            from: 1,
            to: 1
        })
        .unwrap()
    );
    assert!(!doc.can_undo());
}

#[test]
fn fila_7_move_drag_un_clic_de_3_px_es_click_y_uno_de_4_px_empieza() {
    let mut clic = MoveDrag::press(2, (10, 10));
    assert!(!clic.dragging());
    clic.update((13, 10), 1, Some(5));
    assert!(!clic.dragging(), "3 px: bajo el umbral");
    assert_eq!(clic.preview(), None);
    assert_eq!(clic.release(), DragEnd::Click);

    let mut arrastre = MoveDrag::press(2, (10, 10));
    arrastre.update((14, 10), 1, Some(5));
    assert!(arrastre.dragging(), "4 px: umbral alcanzado");
    assert_eq!(arrastre.preview(), Some((2, 5)));
    assert_eq!(arrastre.release(), DragEnd::Move { from: 2, to: 5 });
}

#[test]
fn fila_7_move_drag_el_umbral_es_euclideo_y_escala_con_n() {
    // dx = dy = 3: 18 >= 16.
    let mut diagonal = MoveDrag::press(0, (0, 0));
    diagonal.update((3, 3), 1, Some(1));
    assert!(diagonal.dragging());
    // dx = 3, dy = 2: 13 < 16.
    let mut corto = MoveDrag::press(0, (0, 0));
    corto.update((3, 2), 1, Some(1));
    assert!(!corto.dragging());
    // Hacia atrás también cuenta.
    let mut atras = MoveDrag::press(0, (10, 10));
    atras.update((6, 10), 1, Some(1));
    assert!(atras.dragging());
    let mut atras_corto = MoveDrag::press(0, (10, 10));
    atras_corto.update((7, 10), 1, Some(1));
    assert!(!atras_corto.dragging());
    // Con n = 2 el umbral es de 8 px; con n = 3, de 12.
    for (n, justo_antes, justo) in [(2u32, 7i64, 8i64), (3, 11, 12)] {
        let mut antes = MoveDrag::press(0, (0, 0));
        antes.update((justo_antes, 0), n, Some(1));
        assert!(!antes.dragging(), "n={n}, {justo_antes} px");
        let mut ahora = MoveDrag::press(0, (0, 0));
        ahora.update((0, justo), n, Some(1));
        assert!(ahora.dragging(), "n={n}, {justo} px");
    }
}

#[test]
fn fila_7_move_drag_a_la_misma_sin_destino_o_fuera_es_nothing() {
    let mut misma = MoveDrag::press(4, (0, 0));
    misma.update((20, 0), 1, Some(4));
    assert!(misma.dragging());
    assert_eq!(misma.preview(), None, "to == from: sin vista previa");
    assert_eq!(misma.release(), DragEnd::Nothing);

    let mut fuera = MoveDrag::press(4, (0, 0));
    fuera.update((20, 0), 1, None);
    assert_eq!(fuera.preview(), None);
    assert_eq!(fuera.release(), DragEnd::Nothing);

    // Se pasó por un destino pero se soltó fuera: manda el último.
    let mut despues = MoveDrag::press(4, (0, 0));
    despues.update((20, 0), 1, Some(7));
    despues.update((40, 0), 1, None);
    assert_eq!(despues.release(), DragEnd::Nothing);
}

#[test]
fn fila_7_move_drag_guarda_el_ultimo_destino() {
    let mut arrastre = MoveDrag::press(1, (0, 0));
    arrastre.update((20, 0), 1, Some(3));
    arrastre.update((30, 0), 1, Some(8));
    assert_eq!(arrastre.preview(), Some((1, 8)));
    arrastre.update((40, 0), 1, Some(1));
    assert_eq!(arrastre.preview(), None);
    arrastre.update((50, 0), 1, Some(6));
    assert_eq!(arrastre.release(), DragEnd::Move { from: 1, to: 6 });
}

#[test]
fn fila_7_move_drag_no_vuelve_atras_al_volver_al_punto_de_pulsacion() {
    let mut arrastre = MoveDrag::press(1, (100, 100));
    arrastre.update((120, 100), 1, Some(3));
    assert!(arrastre.dragging());
    arrastre.update((100, 100), 1, Some(5));
    assert!(arrastre.dragging(), "volver al punto inicial no desarma");
    assert_eq!(arrastre.release(), DragEnd::Move { from: 1, to: 5 });
}

#[test]
fn fila_7_move_drag_antes_del_umbral_un_destino_no_vale_para_nada() {
    let mut clic = MoveDrag::press(1, (0, 0));
    clic.update((1, 1), 1, Some(9));
    clic.update((0, 2), 1, Some(4));
    assert_eq!(clic.preview(), None);
    assert_eq!(clic.release(), DragEnd::Click);
}

#[test]
fn fila_7_move_drag_sin_ningun_update_es_click() {
    assert_eq!(MoveDrag::press(7, (5, 5)).release(), DragEnd::Click);
}

#[test]
fn fila_7_move_drag_con_coordenadas_grandes_no_desborda() {
    let mut arrastre = MoveDrag::press(0, (-1_000_000_000, -1_000_000_000));
    arrastre.update((1_000_000_000, 1_000_000_000), 8, Some(1));
    assert!(arrastre.dragging());
    assert_eq!(arrastre.release(), DragEnd::Move { from: 0, to: 1 });
}

#[test]
fn fila_11_un_arrastre_con_una_sola_entrada_solo_puede_caer_en_ella() {
    let rejilla = Grid::new(1, 1, 16, 0);
    let destino = rejilla.drop_index(100, 0);
    assert_eq!(destino, Some(0));
    let mut arrastre = MoveDrag::press(0, (0, 0));
    arrastre.update((100, 0), 1, destino.map(|i| i as u8));
    assert_eq!(arrastre.release(), DragEnd::Nothing);
}

#[test]
fn fila_11_con_256_entradas_el_arrastre_de_0_a_255_es_move() {
    let rejilla = Grid::new(1, 256, 16, 0);
    let destino = rejilla.drop_index(210, 210).map(|i| i as u8);
    assert_eq!(destino, Some(255));
    let mut arrastre = MoveDrag::press(0, (0, 0));
    arrastre.update((210, 210), 1, destino);
    assert_eq!(arrastre.release(), DragEnd::Move { from: 0, to: 255 });
    assert!(blocked_reason(PaletteButton::Add, 256, false).is_some());
}

// Fila 8: clamp_choice.

#[test]
fn fila_8_clamp_choice_en_indexadas() {
    let cinco = imagen_con(&[ROJO, VERDE, AZUL, X, Y], None, &[0, 1, 2, 3, 4]);
    assert_eq!(
        clamp_choice(&cinco, ColorChoice::Entry(7)),
        ColorChoice::Entry(4)
    );
    assert_eq!(
        clamp_choice(&cinco, ColorChoice::Entry(2)),
        ColorChoice::Entry(2)
    );
    assert_eq!(
        clamp_choice(&cinco, ColorChoice::Slot(3)),
        ColorChoice::Entry(2)
    );
    let dos = imagen_con(&[ROJO, VERDE], None, &[0, 1]);
    assert_eq!(
        clamp_choice(&dos, ColorChoice::Slot(3)),
        ColorChoice::Entry(1)
    );
    assert_eq!(
        clamp_choice(&dos, ColorChoice::Entry(255)),
        ColorChoice::Entry(1)
    );
    let una = imagen_con(&[ROJO], None, &[0]);
    assert_eq!(
        clamp_choice(&una, ColorChoice::Entry(9)),
        ColorChoice::Entry(0)
    );
    assert_eq!(
        clamp_choice(&una, ColorChoice::Slot(9)),
        ColorChoice::Entry(0)
    );
}

#[test]
fn fila_8_clamp_choice_sin_paleta() {
    for modo in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let imagen = Image::new(4, 4, modo).unwrap();
        assert_eq!(
            clamp_choice(&imagen, ColorChoice::Slot(3)),
            ColorChoice::Slot(3),
            "{modo:?}"
        );
        assert_eq!(
            clamp_choice(&imagen, ColorChoice::Slot(9)),
            ColorChoice::Slot(9),
            "{modo:?}"
        );
        assert_eq!(
            clamp_choice(&imagen, ColorChoice::Entry(0)),
            ColorChoice::Slot(1),
            "{modo:?}: la inicial"
        );
        assert_eq!(
            clamp_choice(&imagen, ColorChoice::Entry(200)),
            ColorChoice::Slot(1),
            "{modo:?}: la inicial"
        );
    }
}

fn eleccion_cualquiera() -> impl Strategy<Value = ColorChoice> {
    prop_oneof![
        any::<u8>().prop_map(ColorChoice::Entry),
        any::<u8>().prop_map(ColorChoice::Slot),
    ]
}

proptest! {
    #[test]
    fn clamp_choice_en_indexadas_da_una_entrada_valida_idempotente_y_respeta_lo_que_vale(
        largo in 1usize..=256,
        eleccion in eleccion_cualquiera(),
    ) {
        let colores: Vec<Rgba> = (0..largo).map(|i| Rgba::opaque(i as u8, 0, 0)).collect();
        let imagen = imagen_con(&colores, None, &[0]);
        let resultado = clamp_choice(&imagen, eleccion);
        let ColorChoice::Entry(indice) = resultado else {
            return Err(TestCaseError::fail(format!("no es Entry: {resultado:?}")));
        };
        prop_assert!(usize::from(indice) < largo);
        prop_assert_eq!(clamp_choice(&imagen, resultado), resultado);
        if let ColorChoice::Entry(i) = eleccion
            && usize::from(i) < largo
        {
            prop_assert_eq!(resultado, eleccion);
        }
    }
}

// Fila 10: destino de la entrada con texto pendiente.

#[test]
fn fila_10_entry_fate_de_mover_con_la_entrada_3() {
    assert_eq!(
        entry_fate(&PaletteOp::Move { from: 3, to: 5 }, 3),
        Fate::Moved
    );
    assert_eq!(
        entry_fate(&PaletteOp::Move { from: 1, to: 4 }, 3),
        Fate::Moved
    );
    assert_eq!(
        entry_fate(&PaletteOp::Move { from: 5, to: 0 }, 3),
        Fate::Moved
    );
    assert_eq!(
        entry_fate(&PaletteOp::Move { from: 6, to: 7 }, 3),
        Fate::Same
    );
}

#[test]
fn fila_10_entry_fate_de_quitar_con_la_entrada_3() {
    assert_eq!(
        entry_fate(&PaletteOp::Remove { index: 3, into: 0 }, 3),
        Fate::Gone
    );
    assert_eq!(
        entry_fate(&PaletteOp::Remove { index: 1, into: 0 }, 3),
        Fate::Moved
    );
    assert_eq!(
        entry_fate(&PaletteOp::Remove { index: 5, into: 0 }, 3),
        Fate::Same
    );
}

#[test]
fn fila_10_entry_fate_de_rotar_con_la_entrada_3() {
    assert_eq!(
        entry_fate(
            &PaletteOp::Rotate {
                start: 2,
                len: 3,
                steps: 1
            },
            3
        ),
        Fate::Changed,
        "entradas 2 a 4"
    );
    assert_eq!(
        entry_fate(
            &PaletteOp::Rotate {
                start: 5,
                len: 2,
                steps: 1
            },
            3
        ),
        Fate::Same
    );
}

#[test]
fn fila_10_entry_fate_de_anadir_es_same_para_cualquier_entrada() {
    for entrada in [0u8, 3, 255] {
        assert_eq!(entry_fate(&PaletteOp::Add, entrada), Fate::Same);
    }
}

#[test]
fn fila_10_entry_fate_en_los_bordes() {
    // Mover una entrada a su sitio no la cambia.
    assert_eq!(
        entry_fate(&PaletteOp::Move { from: 3, to: 3 }, 3),
        Fate::Same
    );
    // Rotar un número de pasos múltiplo de la longitud no cambia nada.
    for pasos in [0, 3, -3, 6] {
        assert_eq!(
            entry_fate(
                &PaletteOp::Rotate {
                    start: 2,
                    len: 3,
                    steps: pasos
                },
                3
            ),
            Fate::Same,
            "steps {pasos}"
        );
    }
    // Rotar a la izquierda cambia lo mismo.
    assert_eq!(
        entry_fate(
            &PaletteOp::Rotate {
                start: 2,
                len: 3,
                steps: -1
            },
            3
        ),
        Fate::Changed
    );
    // Los extremos del rango están dentro; los vecinos, fuera.
    for (entrada, esperado) in [
        (1u8, Fate::Same),
        (2, Fate::Changed),
        (4, Fate::Changed),
        (5, Fate::Same),
    ] {
        assert_eq!(
            entry_fate(
                &PaletteOp::Rotate {
                    start: 2,
                    len: 3,
                    steps: 1
                },
                entrada
            ),
            esperado,
            "entrada {entrada}"
        );
    }
}

#[test]
fn fila_10_el_texto_pendiente_resuelto_antes_no_queda_y_discard_pending_lo_borra() {
    let color = Rgba::opaque(10, 20, 30);
    let mut editor = ColorEditor::default();
    editor.sync(Some((EditTarget::Entry(3), color)));
    assert_eq!(editor.pending_target(), None);
    assert!(!editor.discard_pending(), "nada pendiente");

    editor.type_hex(EditTarget::Entry(3), color, "ff0000".to_string());
    assert!(editor.has_pending());
    assert_eq!(editor.pending_target(), Some(EditTarget::Entry(3)));
    assert!(editor.discard_pending(), "había texto");
    assert!(!editor.has_pending());
    assert_eq!(editor.pending_target(), None);
    assert_eq!(editor.hex_display(), None);
    assert!(!editor.discard_pending(), "ya no hay");
}

#[test]
fn fila_10_discard_pending_no_da_commit_despues() {
    use px_ui::color_edit::HexOutcome;
    let color = Rgba::opaque(10, 20, 30);
    let mut editor = ColorEditor::default();
    editor.sync(Some((EditTarget::Entry(3), color)));
    editor.type_hex(EditTarget::Entry(3), color, "ff0000".to_string());
    editor.discard_pending();
    assert!(matches!(
        editor.confirm_hex(Mode::Indexed, true),
        HexOutcome::Nothing
    ));
}

// Fila 11: banderas de la línea 2 del panel.

#[test]
fn fila_11_info_flags_range_de_la_spec() {
    assert_eq!(info_flags_range(false, false, true, false), "rango");
    assert_eq!(info_flags_range(true, false, true, false), "cursor · rango");
    assert_eq!(
        info_flags_range(true, false, true, true),
        "cursor · rango · transparente"
    );
    assert_eq!(
        info_flags_range(true, true, false, true),
        "cursor · activa · transparente"
    );
    assert_eq!(info_flags_range(false, false, false, false), "");
}

#[test]
fn fila_11_la_bandera_rango_no_sale_en_la_activa() {
    assert_eq!(info_flags_range(false, true, true, false), "activa");
    assert_eq!(info_flags_range(true, true, true, false), "cursor · activa");
    assert_eq!(
        info_flags_range(true, true, true, true),
        "cursor · activa · transparente"
    );
}

#[test]
fn fila_11_info_flags_range_coincide_con_un_modelo_en_las_16_combinaciones() {
    for bits in 0..16u8 {
        let (cursor, activa, rango, transparente) =
            (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
        let mut esperado = Vec::new();
        if cursor {
            esperado.push("cursor");
        }
        if activa {
            esperado.push("activa");
        }
        if rango && !activa {
            esperado.push("rango");
        }
        if transparente {
            esperado.push("transparente");
        }
        let obtenido = info_flags_range(cursor, activa, rango, transparente);
        assert_eq!(obtenido, esperado.join(" · "), "bits {bits:04b}");
        // `info_flags` es el caso sin rango.
        if !rango {
            assert_eq!(obtenido, info_flags(cursor, activa, transparente));
        }
        assert_eq!(
            info_flags(cursor, activa, transparente),
            info_flags_range(cursor, activa, false, transparente)
        );
        assert!(caracteres(&obtenido) <= 30, "la más larga son 30 columnas");
        assert!(en_el_juego(&obtenido));
    }
    assert_eq!(
        caracteres("cursor · activa · transparente"),
        30,
        "es la línea más larga"
    );
}

// Fila 12: textos.

/// Todos los avisos de la unidad (barra de estado), con los números más largos.
fn avisos() -> Vec<String> {
    let mut avisos: Vec<String> = vec![
        REMOVAL_CANCELLED_NOTICE.to_string(),
        NOTHING_TO_ROTATE_NOTICE.to_string(),
        stale_notice(StaleReason::Moved).to_string(),
        stale_notice(StaleReason::Gone).to_string(),
        stale_notice(StaleReason::Changed).to_string(),
    ];
    for boton in BOTONES {
        for largo in [0usize, 1, 2, 255, 256] {
            for destino in [false, true] {
                if let Some(motivo) = blocked_reason(boton, largo, destino) {
                    avisos.push(motivo.to_string());
                }
            }
        }
    }
    // Añadir: todos los números de entrada.
    for largo in 1..=255usize {
        for activa in [0u8, (largo - 1) as u8] {
            avisos.push(summary(&PaletteOp::Add, activa, largo, false, false));
        }
    }
    // Quitar: todas las parejas, con las cuatro combinaciones de `unused` y `was_transparent`.
    for indice in 0..=255u8 {
        for destino in 0..=255u8 {
            if destino == indice {
                continue;
            }
            for (sin_pixeles, era_transparente) in
                [(false, false), (true, false), (true, true), (false, true)]
            {
                avisos.push(summary(
                    &PaletteOp::Remove {
                        index: indice,
                        into: destino,
                    },
                    indice,
                    256,
                    era_transparente,
                    sin_pixeles,
                ));
            }
        }
    }
    // Mover.
    for desde in 0..=255u8 {
        for hasta in 0..=255u8 {
            avisos.push(summary(
                &PaletteOp::Move {
                    from: desde,
                    to: hasta,
                },
                desde,
                256,
                false,
                false,
            ));
        }
    }
    // Rotar: todos los tramos posibles de una paleta de 256 y de una de 255.
    for largo_paleta in [255usize, 256] {
        for inicio in 0..largo_paleta {
            for longitud in 1..=(largo_paleta - inicio) {
                for pasos in [1, -1] {
                    avisos.push(summary(
                        &PaletteOp::Rotate {
                            start: inicio as u8,
                            len: longitud as u16,
                            steps: pasos,
                        },
                        0,
                        largo_paleta,
                        false,
                        false,
                    ));
                }
            }
        }
    }
    avisos
}

#[test]
fn fila_12_el_resumen_mas_largo_de_quitar_son_43_caracteres() {
    let texto = summary(
        &PaletteOp::Remove {
            index: 255,
            into: 254,
        },
        255,
        256,
        false,
        false,
    );
    assert_eq!(texto, "entrada 255 quitada; píxeles pasan a la 254");
    assert_eq!(caracteres(&texto), 43);
}

#[test]
fn fila_12_ningun_aviso_pasa_de_43_caracteres_ni_sale_del_juego_de_caracteres() {
    let todos = avisos();
    assert!(todos.len() > 100_000, "se probaron muchos: {}", todos.len());
    let mas_largo = todos.iter().map(|texto| caracteres(texto)).max().unwrap();
    assert_eq!(mas_largo, 43, "el más largo es el de tres cifras de Quitar");
    for texto in &todos {
        assert!(
            caracteres(texto) <= 43,
            "{texto:?}: {} caracteres",
            caracteres(texto)
        );
        assert!(en_el_juego(texto), "{texto:?} sale de INTERFACE_CHARSET");
        assert!(!texto.is_empty());
    }
}

#[test]
fn fila_12_con_el_prefijo_aviso_caben_en_las_50_columnas_de_361_u_y_fit_text_no_recorta() {
    assert_eq!(caracteres(NOTICE_PREFIX), 7);
    for ppp in PPPS {
        let n = text_scale(ppp);
        let ancho_px = (361 - 6) as f32 * n as f32;
        let maximo = columns(ancho_px, ppp);
        assert_eq!(maximo, 50, "ppp {ppp}");
        assert_eq!(maximo - caracteres(NOTICE_PREFIX), 43);
    }
    let todos = avisos();
    for texto in todos.iter().step_by(97) {
        let completo = format!("{NOTICE_PREFIX}{texto}");
        let cabe = fit_text(&completo, 50, |c| INTERFACE_CHARSET.contains(c));
        assert_eq!(cabe, completo, "se recortó {completo:?}");
    }
    let el_mas_largo = format!(
        "{NOTICE_PREFIX}{}",
        summary(
            &PaletteOp::Remove {
                index: 255,
                into: 254
            },
            255,
            256,
            false,
            false
        )
    );
    assert_eq!(caracteres(&el_mas_largo), 50);
    assert_eq!(
        fit_text(&el_mas_largo, 50, |c| INTERFACE_CHARSET.contains(c)),
        el_mas_largo
    );
}

#[test]
fn fila_12_las_lineas_del_editor_y_del_panel_caben_en_32_columnas() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        assert_eq!(columns(225.0 * n as f32, ppp), 32, "ppp {ppp}");
    }
    let mut lineas: Vec<String> = Vec::new();
    for indice in 0..=255u8 {
        for destino in std::iter::once(None).chain((0..=255u8).map(Some)) {
            for aviso in [
                None,
                Some(RemovalWarning::PixelsOpaque),
                Some(RemovalWarning::DestinationTransparent),
            ] {
                lineas.extend(removal_prompt(indice, destino, aviso));
            }
        }
    }
    for desde in 0..=255u8 {
        for hasta in std::iter::once(None).chain((0..=255u8).map(Some)) {
            lineas.extend(drag_labels(desde, hasta));
        }
    }
    for bits in 0..16u8 {
        lineas.push(info_flags_range(
            bits & 1 != 0,
            bits & 2 != 0,
            bits & 4 != 0,
            bits & 8 != 0,
        ));
    }
    let mas_larga = lineas.iter().map(|linea| caracteres(linea)).max().unwrap();
    assert!(mas_larga <= 32, "la más larga tiene {mas_larga}");
    for linea in &lineas {
        assert!(caracteres(linea) <= 32, "{linea:?}");
        assert!(en_el_juego(linea), "{linea:?} sale de INTERFACE_CHARSET");
    }
}

#[test]
fn fila_12_los_rotulos_de_los_botones_caben_y_estan_en_el_juego_de_caracteres() {
    // Texto en (6, 1) dentro del botón; 7 u por carácter.
    for (rotulo, ancho_u) in [
        ("Añadir", 110),
        ("Quitar", 110),
        ("Confirmar", 110),
        ("Cancelar", 110),
        ("Rotar a la izquierda", 225),
        ("Rotar a la derecha", 225),
    ] {
        assert!(en_el_juego(rotulo), "{rotulo}");
        let usado = 6 + 7 * caracteres(rotulo) as i64;
        assert!(usado <= ancho_u, "{rotulo}: {usado} u en {ancho_u} u");
    }
    assert_eq!(6 + 140, 146, "Rotar a la izquierda: 6 + 140 u");
}

// PaletteOps.

#[test]
fn palette_ops_nuevo_esta_vacio() {
    let mut ops = PaletteOps::default();
    assert!(ops.drag().is_none());
    assert_eq!(ops.range_end(), None);
    assert_eq!(ops.removal(), None);
    assert!(!ops.busy());
    assert_eq!(ops.end_drag(), None);
}

#[test]
fn palette_ops_sync_sin_quita_pendiente_no_avisa() {
    let mut ops = PaletteOps::default();
    assert!(!ops.sync(0, 5));
    assert!(!ops.sync(1, 5));
    assert!(!ops.sync(99, 1));
}

#[test]
fn palette_ops_sync_con_el_mismo_estado_conserva_la_quita() {
    let mut ops = PaletteOps::default();
    ops.sync(7, 5);
    ops.begin_removal(3, Some(0));
    assert_eq!(
        ops.removal(),
        Some(Removal {
            index: 3,
            into: Some(0)
        })
    );
    for _ in 0..3 {
        assert!(!ops.sync(7, 5));
    }
    assert_eq!(
        ops.removal(),
        Some(Removal {
            index: 3,
            into: Some(0)
        })
    );
}

#[test]
fn palette_ops_sync_cancela_la_quita_si_cambia_el_documento_y_avisa_una_vez() {
    for nuevo in [8u64, 6, 0, u64::MAX] {
        let mut ops = PaletteOps::default();
        ops.sync(7, 5);
        ops.begin_removal(3, Some(0));
        assert!(ops.sync(nuevo, 5), "estado {nuevo}: hay que avisar");
        assert_eq!(ops.removal(), None);
        assert!(!ops.sync(nuevo, 5), "solo avisa una vez");
        assert!(
            !ops.sync(nuevo.wrapping_add(1), 5),
            "sin quita no hay aviso"
        );
    }
}

#[test]
fn palette_ops_sync_cancela_la_quita_sin_destino_si_cambia_el_documento() {
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(3, None);
    assert!(ops.sync(2, 5));
    assert_eq!(ops.removal(), None);
}

#[test]
fn palette_ops_sync_cancela_la_quita_si_sus_indices_ya_no_valen() {
    // `index` fuera de la paleta.
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(4, Some(0));
    assert!(!ops.sync(1, 5), "4 < 5 vale");
    assert!(ops.sync(1, 4), "4 >= 4 ya no vale");
    assert_eq!(ops.removal(), None);

    // `into` fuera de la paleta.
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(1, Some(4));
    assert!(ops.sync(1, 4));
    assert_eq!(ops.removal(), None);

    // Sin destino solo importa `index`.
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(3, None);
    assert!(!ops.sync(1, 4), "3 < 4 vale");
    assert_eq!(ops.removal().map(|quita| quita.index), Some(3));
    assert!(ops.sync(1, 3));
    assert_eq!(ops.removal(), None);
}

#[test]
fn palette_ops_begin_removal_se_ata_al_estado_de_la_ultima_sync() {
    let mut ops = PaletteOps::default();
    ops.sync(10, 5);
    ops.sync(11, 5);
    ops.begin_removal(2, Some(1));
    assert!(!ops.sync(11, 5), "el de la última sync");
    ops.cancel_removal();
    assert_eq!(ops.removal(), None);

    ops.begin_removal(2, Some(1));
    assert!(
        ops.sync(10, 5),
        "el estado anterior ya no es el de la quita"
    );
}

#[test]
fn palette_ops_sync_recorta_el_otro_extremo_del_rango() {
    let mut ops = PaletteOps::default();
    ops.sync(1, 10);
    ops.set_range_end(8);
    assert_eq!(ops.range_end(), Some(8));
    assert!(!ops.sync(1, 10));
    assert_eq!(ops.range_end(), Some(8), "cabe");
    ops.sync(1, 6);
    assert_eq!(ops.range_end(), Some(5), "se recorta a la última");
    ops.sync(1, 5);
    assert_eq!(ops.range_end(), Some(4));
    ops.sync(1, 20);
    assert_eq!(ops.range_end(), Some(4), "no vuelve a crecer");
}

#[test]
fn palette_ops_rango_poner_y_borrar() {
    let mut ops = PaletteOps::default();
    ops.set_range_end(3);
    assert_eq!(ops.range_end(), Some(3));
    ops.set_range_end(7);
    assert_eq!(ops.range_end(), Some(7));
    ops.clear_range();
    assert_eq!(ops.range_end(), None);
    ops.clear_range();
    assert_eq!(ops.range_end(), None);
}

#[test]
fn palette_ops_set_destination() {
    let mut ops = PaletteOps::default();
    assert!(!ops.set_destination(2, 5), "sin quita pendiente");
    assert_eq!(ops.removal(), None);

    ops.sync(1, 5);
    ops.begin_removal(3, Some(0));
    assert!(!ops.set_destination(3, 5), "la propia se ignora");
    assert_eq!(ops.removal().map(|quita| quita.into), Some(Some(0)));
    assert!(!ops.set_destination(5, 5), "fuera de la paleta");
    assert!(!ops.set_destination(200, 5));
    assert_eq!(ops.removal().map(|quita| quita.into), Some(Some(0)));

    assert!(ops.set_destination(2, 5));
    assert_eq!(
        ops.removal(),
        Some(Removal {
            index: 3,
            into: Some(2)
        })
    );
    assert!(ops.set_destination(4, 5), "el último vale");
    assert_eq!(ops.removal().map(|quita| quita.into), Some(Some(4)));
}

#[test]
fn palette_ops_set_destination_da_destino_a_una_quita_sin_el() {
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(3, None);
    assert!(ops.set_destination(4, 5));
    assert_eq!(
        ops.removal(),
        Some(Removal {
            index: 3,
            into: Some(4)
        })
    );
    // El destino no cambia a qué estado está atada.
    assert!(!ops.sync(1, 5));
    assert!(ops.sync(2, 5));
}

#[test]
fn palette_ops_cancel_removal() {
    let mut ops = PaletteOps::default();
    ops.cancel_removal();
    ops.sync(1, 5);
    ops.begin_removal(3, Some(0));
    ops.cancel_removal();
    assert_eq!(ops.removal(), None);
    assert!(!ops.sync(2, 5), "cancelada a mano: no hay aviso");
}

#[test]
fn palette_ops_busy_solo_con_arrastre_real() {
    let mut ops = PaletteOps::default();
    assert!(!ops.busy());
    ops.begin_drag(2, (10, 10));
    assert!(ops.drag().is_some());
    assert!(!ops.busy(), "armado, antes del umbral");
    ops.drag().unwrap().update((13, 10), 1, Some(5));
    assert!(!ops.busy(), "3 px");
    ops.drag().unwrap().update((14, 10), 1, Some(5));
    assert!(ops.busy(), "4 px");
    assert!(ops.drag().is_some_and(|arrastre| arrastre.dragging()));
    ops.drag().unwrap().update((10, 10), 1, Some(5));
    assert!(ops.busy(), "no vuelve atrás");

    assert_eq!(ops.end_drag(), Some(DragEnd::Move { from: 2, to: 5 }));
    assert!(!ops.busy());
    assert!(ops.drag().is_none());
    assert_eq!(ops.end_drag(), None, "una sola salida");
}

#[test]
fn palette_ops_un_clic_armado_termina_en_click() {
    let mut ops = PaletteOps::default();
    ops.begin_drag(4, (0, 0));
    ops.drag().unwrap().update((2, 1), 1, Some(9));
    assert!(!ops.busy());
    assert_eq!(ops.end_drag(), Some(DragEnd::Click));
    assert!(ops.drag().is_none());
}

#[test]
fn palette_ops_cancel_drag_no_da_salida() {
    let mut ops = PaletteOps::default();
    ops.cancel_drag();
    ops.begin_drag(4, (0, 0));
    ops.drag().unwrap().update((40, 0), 1, Some(9));
    assert!(ops.busy());
    ops.cancel_drag();
    assert!(ops.drag().is_none());
    assert!(!ops.busy());
    assert_eq!(ops.end_drag(), None);
}

#[test]
fn palette_ops_reset_lo_borra_todo() {
    let mut ops = PaletteOps::default();
    ops.sync(1, 5);
    ops.begin_removal(3, Some(0));
    ops.set_range_end(2);
    ops.begin_drag(1, (0, 0));
    ops.drag().unwrap().update((40, 0), 1, Some(9));
    assert!(ops.busy());
    ops.reset();
    assert_eq!(ops.removal(), None);
    assert_eq!(ops.range_end(), None);
    assert!(ops.drag().is_none());
    assert!(!ops.busy());
    assert!(!ops.sync(2, 5), "reset no deja una quita a medias");
}

// Propiedad 1: destino del arrastre.

fn coordenada(selector: u8, cruda: i64, paso: i64, delta: i64, n: i64, extremo: usize) -> i64 {
    const EXTREMOS: [i64; 6] = [i64::MIN, i64::MIN + 1, i64::MAX - 1, i64::MAX, -1, 0];
    match selector % 8 {
        // Alrededor de los bordes de las zonas de captura.
        0..=3 => paso * 14 * n - 2 * n + delta,
        4 => EXTREMOS[extremo % EXTREMOS.len()],
        _ => cruda,
    }
}

/// Modelo de `drop_index` con `i128` y `div_euclid`.
fn modelo_drop_index(
    n: i128,
    count: usize,
    visibles: usize,
    primera: usize,
    x: i64,
    y: i64,
) -> Option<usize> {
    let (x, y) = (i128::from(x), i128::from(y));
    let total = count.div_ceil(COLUMNS);
    let con_muestras = visibles.min(total.saturating_sub(primera)) as i128;
    let paso = 14 * n;
    let en_x = x >= -2 * n && x < COLUMNS as i128 * paso - 2 * n;
    let en_y = y >= -2 * n && y < con_muestras * paso - 2 * n;
    if !(en_x && en_y) || count == 0 {
        return None;
    }
    let columna = (x + 2 * n).div_euclid(paso);
    let fila = (y + 2 * n).div_euclid(paso);
    let indice = (primera as i128 + fila) * COLUMNS as i128 + columna;
    let ultimo = count as i128 - 1;
    Some(usize::try_from(indice.min(ultimo)).expect("índice dentro"))
}

proptest! {
    #[test]
    fn propiedad_1_drop_index_es_total_y_coincide_con_el_modelo(
        n in 1u32..=8,
        count in 0usize..=256,
        visibles in 0usize..=20,
        fila_cruda in any::<usize>(),
        (sx, cx, px, dx, ex) in (any::<u8>(), -40i64..3600, -1i64..=17, -3i64..=3, any::<usize>()),
        (sy, cy, py, dy, ey) in (any::<u8>(), -40i64..3600, -1i64..=17, -3i64..=3, any::<usize>()),
        (sx2, cx2, px2, dx2, ex2) in (any::<u8>(), -40i64..3600, -1i64..=17, -3i64..=3, any::<usize>()),
    ) {
        let total = total_rows(count);
        let primera = fila_cruda % (max_first_row(total, visibles) + 1);
        let rejilla = Grid::new(n, count, visibles, primera);
        let k = i64::from(n);
        let x = coordenada(sx, cx, px, dx, k, ex);
        let y = coordenada(sy, cy, py, dy, k, ey);
        let x2 = coordenada(sx2, cx2, px2, dx2, k, ex2);

        let obtenido = rejilla.drop_index(x, y);
        prop_assert_eq!(obtenido, modelo_drop_index(i128::from(n), count, visibles, primera, x, y));
        if let Some(i) = obtenido {
            prop_assert!(i < count);
        }

        // Si `index_at` encuentra una muestra, `drop_index` da la misma.
        if let Some(i) = rejilla.index_at(x, y) {
            prop_assert_eq!(obtenido, Some(i));
        }

        // Monótona en la fila: con `y` fijo, `x` mayor no da un índice menor.
        let (menor, mayor) = if x <= x2 { (x, x2) } else { (x2, x) };
        if let (Some(a), Some(b)) = (rejilla.drop_index(menor, y), rejilla.drop_index(mayor, y)) {
            prop_assert!(a <= b, "x={} da {}, x={} da {}", menor, a, mayor, b);
        }
    }

    #[test]
    fn propiedad_1_el_centro_de_cada_muestra_visible_es_su_destino(
        n in 1u32..=8,
        count in 1usize..=256,
        visibles in 1usize..=20,
        fila_cruda in any::<usize>(),
    ) {
        let total = total_rows(count);
        let primera = fila_cruda % (max_first_row(total, visibles) + 1);
        let rejilla = Grid::new(n, count, visibles, primera);
        for i in 0..count {
            if let Some(celda) = rejilla.cell_rect(i) {
                let centro = (celda.x + celda.width / 2, celda.y + celda.height / 2);
                prop_assert_eq!(rejilla.drop_index(centro.0, centro.1), Some(i));
            }
        }
    }
}

// Propiedad 2: máquina del arrastre.

#[derive(Clone, Debug)]
enum Accion {
    Pulsar { from: u8, at: (i64, i64) },
    Mover { at: (i64, i64), target: Option<u8> },
    Soltar,
    Cancelar,
}

fn punto() -> impl Strategy<Value = (i64, i64)> {
    (-40i64..40, -40i64..40)
}

fn accion() -> impl Strategy<Value = Accion> {
    prop_oneof![
        2 => (0u8..6, punto()).prop_map(|(from, at)| Accion::Pulsar { from, at }),
        6 => (punto(), proptest::option::of(0u8..6))
            .prop_map(|(at, target)| Accion::Mover { at, target }),
        2 => Just(Accion::Soltar),
        1 => Just(Accion::Cancelar),
    ]
}

/// Modelo del arrastre: el punto de pulsación, si alguna vez se pasó el umbral y el último
/// destino dado con el arrastre vivo.
struct ModeloArrastre {
    from: u8,
    inicio: (i64, i64),
    cruzado: bool,
    destino: Option<u8>,
}

impl ModeloArrastre {
    fn mover(&mut self, at: (i64, i64), n: u32, target: Option<u8>) {
        let dx = i128::from(at.0) - i128::from(self.inicio.0);
        let dy = i128::from(at.1) - i128::from(self.inicio.1);
        let umbral = 4 * i128::from(n);
        if dx * dx + dy * dy >= umbral * umbral {
            self.cruzado = true;
        }
        if self.cruzado {
            self.destino = target;
        }
    }

    fn salida(&self) -> DragEnd {
        if !self.cruzado {
            return DragEnd::Click;
        }
        match self.destino {
            Some(to) if to != self.from => DragEnd::Move {
                from: self.from,
                to,
            },
            _ => DragEnd::Nothing,
        }
    }

    fn preview(&self) -> Option<(u8, u8)> {
        match self.destino {
            Some(to) if self.cruzado && to != self.from => Some((self.from, to)),
            _ => None,
        }
    }
}

proptest! {
    #[test]
    fn propiedad_2_move_drag_sigue_al_modelo(
        n in 1u32..=8,
        acciones in vec(accion(), 0..40),
    ) {
        let mut arrastre: Option<MoveDrag> = None;
        let mut modelo: Option<ModeloArrastre> = None;
        for accion in acciones {
            match accion {
                Accion::Pulsar { from, at } => {
                    if arrastre.is_none() {
                        arrastre = Some(MoveDrag::press(from, at));
                        modelo = Some(ModeloArrastre { from, inicio: at, cruzado: false, destino: None });
                    }
                }
                Accion::Mover { at, target } => {
                    if let (Some(real), Some(esperado)) = (arrastre.as_mut(), modelo.as_mut()) {
                        real.update(at, n, target);
                        esperado.mover(at, n, target);
                    }
                }
                Accion::Soltar => {
                    if let (Some(real), Some(esperado)) = (arrastre.take(), modelo.take()) {
                        prop_assert_eq!(real.release(), esperado.salida());
                    }
                }
                Accion::Cancelar => {
                    arrastre = None;
                    modelo = None;
                }
            }
            match (&arrastre, &modelo) {
                (Some(real), Some(esperado)) => {
                    prop_assert_eq!(real.dragging(), esperado.cruzado);
                    prop_assert_eq!(real.preview(), esperado.preview());
                }
                (None, None) => {}
                _ => prop_assert!(false, "real y modelo no coinciden en si hay arrastre"),
            }
        }
    }

    #[test]
    fn propiedad_2_palette_ops_sigue_al_modelo_y_no_da_salidas_de_mas(
        n in 1u32..=8,
        acciones in vec(accion(), 0..40),
    ) {
        let mut ops = PaletteOps::default();
        let mut modelo: Option<ModeloArrastre> = None;
        for accion in acciones {
            match accion {
                Accion::Pulsar { from, at } => {
                    if modelo.is_none() {
                        ops.begin_drag(from, at);
                        modelo = Some(ModeloArrastre { from, inicio: at, cruzado: false, destino: None });
                    }
                }
                Accion::Mover { at, target } => {
                    if let Some(esperado) = modelo.as_mut() {
                        ops.drag().expect("hay arrastre").update(at, n, target);
                        esperado.mover(at, n, target);
                    }
                }
                Accion::Soltar => {
                    let esperada = modelo.take().map(|m| m.salida());
                    prop_assert_eq!(ops.end_drag(), esperada);
                    // Después de soltar no hay más salidas.
                    prop_assert_eq!(ops.end_drag(), None);
                }
                Accion::Cancelar => {
                    ops.cancel_drag();
                    modelo = None;
                    prop_assert_eq!(ops.end_drag(), None);
                }
            }
            prop_assert_eq!(ops.drag().is_some(), modelo.is_some());
            let cruzado = modelo.as_ref().is_some_and(|m| m.cruzado);
            prop_assert_eq!(ops.busy(), cruzado);
            if let (Some(arrastre), Some(esperado)) = (ops.drag(), modelo.as_ref()) {
                prop_assert_eq!(arrastre.preview(), esperado.preview());
            }
        }
    }
}

// Propiedad 3: destino de la entrada y selección, con un modelo de identificadores.

/// Identificador de la entrada que añade `Add`.
const NUEVA: u32 = 1000;

fn estrategia_pasos() -> impl Strategy<Value = i32> {
    prop_oneof![
        4 => any::<i32>(),
        4 => -10i32..10,
        1 => Just(i32::MIN),
        1 => Just(i32::MAX),
    ]
}

/// Una operación válida para una paleta de `largo` entradas, o `None` si el tipo no se puede.
fn operacion_valida(largo: usize, tipo: u8, a: u8, b: u8, pasos: i32) -> Option<PaletteOp> {
    match tipo % 4 {
        0 => (largo < 256).then_some(PaletteOp::Add),
        1 => {
            if largo < 2 {
                return None;
            }
            let indice = usize::from(a) % largo;
            let destino = (indice + 1 + usize::from(b) % (largo - 1)) % largo;
            Some(PaletteOp::Remove {
                index: indice as u8,
                into: destino as u8,
            })
        }
        2 => Some(PaletteOp::Move {
            from: (usize::from(a) % largo) as u8,
            to: (usize::from(b) % largo) as u8,
        }),
        _ => {
            let inicio = usize::from(a) % largo;
            let longitud = 1 + usize::from(b) % (largo - inicio);
            Some(PaletteOp::Rotate {
                start: inicio as u8,
                len: longitud as u16,
                steps: pasos,
            })
        }
    }
}

/// Aplica la operación a un vector de identificadores.
fn modelo_aplicar(op: &PaletteOp, ids: &[u32]) -> Vec<u32> {
    let mut nuevo = ids.to_vec();
    match *op {
        PaletteOp::Add => nuevo.push(NUEVA),
        PaletteOp::Remove { index, .. } => {
            nuevo.remove(usize::from(index));
        }
        PaletteOp::Move { from, to } => {
            let id = nuevo.remove(usize::from(from));
            nuevo.insert(usize::from(to), id);
        }
        PaletteOp::Rotate { start, len, steps } => {
            let (inicio, fin) = (usize::from(start), usize::from(start) + usize::from(len));
            let k = i64::from(steps).rem_euclid(i64::from(len));
            nuevo[inicio..fin].rotate_right(usize::try_from(k).expect("k pequeño"));
        }
    }
    nuevo
}

fn destino_modelo(op: &PaletteOp, ids: &[u32], nuevo: &[u32], entrada: usize) -> Fate {
    let id = ids[entrada];
    if matches!(op, PaletteOp::Rotate { .. }) {
        // Mismo índice, otro color.
        return if nuevo[entrada] == id {
            Fate::Same
        } else {
            Fate::Changed
        };
    }
    match nuevo.iter().position(|&otro| otro == id) {
        None => Fate::Gone,
        Some(posicion) if posicion == entrada => Fate::Same,
        Some(_) => Fate::Moved,
    }
}

fn seleccion_modelo(op: &PaletteOp, activa: u8, nuevo: &[u32]) -> usize {
    match *op {
        PaletteOp::Add => nuevo.iter().position(|&id| id == NUEVA).expect("la nueva"),
        PaletteOp::Remove { index, .. } => usize::from(index).min(nuevo.len() - 1),
        PaletteOp::Move { from, .. } => nuevo
            .iter()
            .position(|&id| id == u32::from(from))
            .expect("la movida"),
        PaletteOp::Rotate { .. } => usize::from(activa),
    }
}

proptest! {
    #[test]
    fn propiedad_3_entry_fate_y_choice_after_siguen_al_modelo_de_identificadores(
        largo in 1usize..=256,
        tipo in any::<u8>(),
        a in any::<u8>(),
        b in any::<u8>(),
        pasos in estrategia_pasos(),
        activa_cruda in any::<u8>(),
    ) {
        let Some(op) = operacion_valida(largo, tipo, a, b, pasos) else {
            return Ok(());
        };
        let activa = (usize::from(activa_cruda) % largo) as u8;
        let ids: Vec<u32> = (0..largo as u32).collect();
        let nuevo = modelo_aplicar(&op, &ids);

        for entrada in 0..largo {
            prop_assert_eq!(
                entry_fate(&op, entrada as u8),
                destino_modelo(&op, &ids, &nuevo, entrada),
                "{:?}, entrada {}", op, entrada
            );
        }
        let elegida = choice_after(&op, activa, largo);
        prop_assert_eq!(usize::from(elegida), seleccion_modelo(&op, activa, &nuevo), "{:?}", op);
        prop_assert!(usize::from(elegida) < nuevo.len());
    }

    #[test]
    fn propiedad_3_el_modelo_coincide_con_lo_que_hace_px_core(
        largo in 1usize..=256,
        tipo in any::<u8>(),
        a in any::<u8>(),
        b in any::<u8>(),
        pasos in estrategia_pasos(),
    ) {
        let Some(op) = operacion_valida(largo, tipo, a, b, pasos) else {
            return Ok(());
        };
        let color_de = |id: u32| {
            if id == NUEVA {
                Rgba::new(0, 0, 0, 1)
            } else {
                Rgba::opaque(id as u8, 255 - id as u8, 7)
            }
        };
        let ids: Vec<u32> = (0..largo as u32).collect();
        let colores: Vec<Rgba> = ids.iter().map(|&id| color_de(id)).collect();
        let mut doc = Document::new(imagen_con(&colores, None, &[0]));
        let id = doc.main_image();
        let comando = match op {
            PaletteOp::Add => Command::AddPaletteEntry { image: id, color: color_de(NUEVA) },
            PaletteOp::Remove { index, into } => {
                Command::RemovePaletteEntry { image: id, index, into }
            }
            PaletteOp::Move { from, to } => Command::MovePaletteEntry { image: id, from, to },
            PaletteOp::Rotate { start, len, steps } => {
                Command::RotateColors { image: id, start, len, steps }
            }
        };
        doc.apply(comando).expect("operación válida");
        let esperado: Vec<Rgba> = modelo_aplicar(&op, &ids)
            .into_iter()
            .map(color_de)
            .collect();
        prop_assert_eq!(colores_de(&doc, id), esperado, "{:?}", op);
    }

    #[test]
    fn default_destination_coincide_con_un_modelo_de_fuerza_bruta(
        colores in vec((any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()), 2..=12),
        transparente_crudo in any::<usize>(),
        quitada_cruda in any::<usize>(),
    ) {
        let colores: Vec<Rgba> = colores.into_iter().map(|(r, g, b, a)| Rgba::new(r, g, b, a)).collect();
        let largo = colores.len();
        let transparente = {
            let t = transparente_crudo % (largo + 1);
            (t < largo).then_some(t as u8)
        };
        let indice = (quitada_cruda % largo) as u8;
        let paleta = paleta(&colores, transparente);

        let esperado = if transparente == Some(indice) {
            None
        } else {
            let premultiplicado = |c: Rgba| {
                let alfa = i64::from(c.a);
                [i64::from(c.r) * alfa, i64::from(c.g) * alfa, i64::from(c.b) * alfa, 255 * alfa]
            };
            let resuelto = |j: usize| {
                let color = colores[j];
                if transparente == Some(j as u8) { Rgba { a: 0, ..color } } else { color }
            };
            let objetivo = premultiplicado(resuelto(usize::from(indice)));
            let mut mejor: Option<(i64, usize)> = None;
            for j in (0..largo).filter(|&j| j != usize::from(indice)) {
                let otro = premultiplicado(resuelto(j));
                let distancia: i64 = objetivo.iter().zip(otro).map(|(&p, q)| (p - q) * (p - q)).sum();
                if mejor.is_none_or(|(d, _)| distancia < d) {
                    mejor = Some((distancia, j));
                }
            }
            mejor.map(|(_, j)| j as u8)
        };
        prop_assert_eq!(default_destination(&paleta, indice), esperado);
        if let Some(destino) = default_destination(&paleta, indice) {
            prop_assert_ne!(destino, indice);
            prop_assert!(usize::from(destino) < largo);
        }
    }

    #[test]
    fn uses_entry_coincide_con_contar_los_pixeles(
        ancho in 1u32..8,
        alto in 1u32..8,
        largo in 1usize..8,
        crudos in vec(any::<u8>(), 64),
    ) {
        let colores: Vec<Rgba> = (0..largo).map(|i| Rgba::opaque(i as u8, 0, 0)).collect();
        let mut imagen = Image::new_indexed(ancho, alto, paleta(&colores, None)).unwrap();
        let mut usados = vec![false; largo];
        for y in 0..alto {
            for x in 0..ancho {
                let indice = (usize::from(crudos[(y * 8 + x) as usize]) % largo) as u8;
                imagen.set_pixel(x, y, Pixel::Indexed(indice)).unwrap();
                usados[usize::from(indice)] = true;
            }
        }
        for (indice, &usado) in usados.iter().enumerate() {
            prop_assert_eq!(uses_entry(&imagen, indice as u8), usado, "entrada {}", indice);
        }
    }
}
