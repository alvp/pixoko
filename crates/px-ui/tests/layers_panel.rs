//! Geometría, colocación y lista del panel de capas (specs/hito-3-capas-interfaz.md, unidad 3.4:
//! filas 5 y 6 y propiedad 2). En ROJO hasta la lógica: `px_ui::layers_panel` no existe todavía.
//! Lógica pura, sin ventana: `layers_panel` no conoce egui.
//!
//! - Fila 5: las medidas en `u` (188 × 277), el recubrimiento sin solapes, `hit`, `place` con
//!   posiciones dentro, fuera y negativas, la ventana mínima, redimensionar, cambiar de escala y
//!   arrastrar por la barra (condición 2).
//! - Fila 6: la lista con 1, 6, 7 y 256 capas (activa, ojo, candado, nombre largo, desplazamiento)
//!   y los textos del pie de memoria.
//! - Propiedad 2: el panel cabe siempre y se recoloca (barrido de `ppp` de 1,0 a 4,0 en pasos de
//!   0,05, ventanas desde el mínimo y posiciones dentro, fuera y negativas).

use px_core::{Command, Document, Image, LayerId, Mode};
use px_ui::chrome::window_layout;
use px_ui::edit_layout::min_window_width_u;
use px_ui::layers_panel::{
    NAME_CHARS, PANEL_H_U, PANEL_W_U, PanelButton, PanelHit, PanelState, RectU, VISIBLE_ROWS,
    blend_u, button_u, close_u, count_u, default_pos_u, eye_u, format_bytes, hit, lock_u,
    max_scroll, memory_line, memory_u, name_field_u, name_u, opacity_field_u, opacity_slider_u,
    place, row_u, scroll_for, scrollbar_u, title_u, truncate_name, visible_rows,
};
use px_ui::text::text_scale;

/// Alto de la ventana mínima en `u` (barra de estado 20 + menú 16 + panel mínimo 305).
const HEIGHT_MINIMUM_U: i64 = 341;
const GIB: usize = 1 << 30;
const MIB: usize = 1 << 20;

/// Todos los elementos con su rectángulo, para los barridos de recubrimiento.
fn elements() -> Vec<(String, RectU)> {
    let mut v = vec![
        ("título".to_string(), title_u()),
        ("cerrar".to_string(), close_u()),
        ("barra".to_string(), scrollbar_u()),
        ("contador".to_string(), count_u()),
        ("nombre".to_string(), name_field_u()),
        ("deslizador".to_string(), opacity_slider_u()),
        ("opacidad".to_string(), opacity_field_u()),
        ("mezcla".to_string(), blend_u()),
        ("memoria".to_string(), memory_u()),
    ];
    for row_2 in 0..VISIBLE_ROWS {
        v.push((format!("ojo {row_2}"), eye_u(row_2)));
        v.push((format!("candado {row_2}"), lock_u(row_2)));
        v.push((format!("nombre {row_2}"), name_u(row_2)));
    }
    for button_2 in PanelButton::ALL {
        v.push((format!("botón {button_2:?}"), button_u(button_2)));
    }
    v
}

fn overlap(a: RectU, b: RectU) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

fn window_minimum(n: i64) -> (i64, i64) {
    (min_window_width_u() * n, HEIGHT_MINIMUM_U * n)
}

// ---------------------------------------------------------------------------------------------
// Fila 5: geometría y colocación
// ---------------------------------------------------------------------------------------------

#[test]
fn row_5_the_measures_are_188_by_277_with_six_rows_of_24_u() {
    assert_eq!((PANEL_W_U, PANEL_H_U, VISIBLE_ROWS), (188, 277, 6));
    assert_eq!(
        title_u(),
        RectU {
            x: 1,
            y: 1,
            w: 170,
            h: 16
        }
    );
    assert_eq!(
        close_u(),
        RectU {
            x: 171,
            y: 1,
            w: 16,
            h: 16
        }
    );
    for row_2 in 0..VISIBLE_ROWS {
        let y = 21 + 25 * row_2 as i64;
        assert_eq!(
            eye_u(row_2),
            RectU {
                x: 5,
                y,
                w: 24,
                h: 24
            },
            "ojo {row_2}"
        );
        assert_eq!(
            lock_u(row_2),
            RectU {
                x: 30,
                y,
                w: 24,
                h: 24
            },
            "candado {row_2}"
        );
        assert_eq!(
            name_u(row_2),
            RectU {
                x: 58,
                y,
                w: 114,
                h: 24
            },
            "nombre {row_2}"
        );
        assert_eq!(
            row_u(row_2),
            RectU {
                x: 5,
                y,
                w: 167,
                h: 24
            },
            "fila {row_2}"
        );
    }
    assert_eq!(
        scrollbar_u(),
        RectU {
            x: 175,
            y: 21,
            w: 8,
            h: 149
        }
    );
    for (i, button_2) in PanelButton::ALL.into_iter().enumerate() {
        assert_eq!(
            button_u(button_2),
            RectU {
                x: 5 + 25 * i as i64,
                y: 174,
                w: 24,
                h: 24
            },
            "{button_2:?}"
        );
    }
    assert_eq!(name_field_u().y, 202);
    assert_eq!(name_field_u().h, 16);
    assert_eq!(opacity_slider_u().y, 220);
    assert_eq!(opacity_field_u().y, 220);
    assert_eq!(blend_u().y, 238);
    assert_eq!(
        memory_u(),
        RectU {
            x: 5,
            y: 254,
            w: 178,
            h: 14
        }
    );
    // El nombre de una fila muestra 16 caracteres de 7 u.
    assert!(NAME_CHARS as i64 * 7 <= name_u(0).w);
    assert_eq!(NAME_CHARS, 16);
}

#[test]
fn row_5_no_element_overlaps_another_and_all_fit_in_the_panel() {
    let all_2 = elements();
    for (name_2, r) in &all_2 {
        assert!(
            r.x >= 0 && r.y >= 0 && r.x + r.w <= PANEL_W_U && r.y + r.h <= PANEL_H_U,
            "{name_2}: {r:?}"
        );
        assert!(r.w > 0 && r.h > 0, "{name_2}");
    }
    for (i, (a, ra)) in all_2.iter().enumerate() {
        for (b, rb) in &all_2[i + 1..] {
            assert!(!overlap(*ra, *rb), "{a} solapa con {b}");
        }
    }
}

#[test]
fn row_5_hit_hits_the_element_at_its_first_and_last_coordinate_at_all_scales() {
    for n in 1..=8u32 {
        let k = i64::from(n);
        let expected_2 = |name_2: &str| -> PanelHit {
            match name_2 {
                "título" => PanelHit::Title,
                "cerrar" => PanelHit::Close,
                "barra" => PanelHit::Scrollbar,
                "contador" => PanelHit::Count,
                "nombre" => PanelHit::NameField,
                "deslizador" => PanelHit::OpacitySlider,
                "opacidad" => PanelHit::OpacityField,
                "mezcla" => PanelHit::BlendButton,
                "memoria" => PanelHit::Memory,
                other_2 if other_2.starts_with("botón ") => {
                    let button_2 = PanelButton::ALL
                        .into_iter()
                        .find(|b| format!("botón {b:?}") == other_2)
                        .expect("botón");
                    PanelHit::Button(button_2)
                }
                other_2 => {
                    let row_2: usize = other_2.rsplit(' ').next().unwrap().parse().unwrap();
                    if other_2.starts_with("ojo") {
                        PanelHit::Eye(row_2)
                    } else if other_2.starts_with("candado") {
                        PanelHit::Lock(row_2)
                    } else {
                        PanelHit::Name(row_2)
                    }
                }
            }
        };
        for (name_2, r) in elements() {
            let want = expected_2(&name_2);
            for (x, y) in [
                (r.x * k, r.y * k),
                ((r.x + r.w) * k - 1, (r.y + r.h) * k - 1),
            ] {
                assert_eq!(hit(x, y, n), Some(want), "{name_2} en ({x}, {y}) a n = {n}");
            }
            // Un píxel fuera por cada lado no es el mismo elemento.
            for (x, y) in [
                (r.x * k - 1, r.y * k),
                ((r.x + r.w) * k, r.y * k),
                (r.x * k, r.y * k - 1),
                (r.x * k, (r.y + r.h) * k),
            ] {
                assert_ne!(hit(x, y, n), Some(want), "{name_2} en ({x}, {y}) a n = {n}");
            }
        }
        // Dentro del panel pero en un hueco: el fondo. Fuera del panel: nada.
        assert_eq!(hit(2 * k, 19 * k, n), Some(PanelHit::Body));
        assert_eq!(hit(-1, 0, n), None);
        assert_eq!(hit(0, -1, n), None);
        assert_eq!(hit(PANEL_W_U * k, 0, n), None);
        assert_eq!(hit(0, PANEL_H_U * k, n), None);
        assert_eq!(
            hit(PANEL_W_U * k - 1, PANEL_H_U * k - 1, n),
            Some(PanelHit::Body)
        );
    }
}

#[test]
fn row_5_place_clips_to_the_central_strip_with_positions_inside_outside_and_negative() {
    for n in [1u32, 2, 3] {
        let k = i64::from(n);
        let window = window_minimum(k);
        let design = window_layout(window, n);
        let band_y = design.menu.height;
        let band_height = design.status.y - design.menu.height;
        assert_eq!(
            band_height,
            305 * k,
            "la franja central mide 305 u a la ventana mínima"
        );
        for (pos, x_expected, y_expected) in [
            ((0, 0), 0, band_y),
            ((10, 20), 10 * k, band_y + 20 * k),
            ((-50, -50), 0, band_y),
            (
                (10_000, 10_000),
                window.0 - PANEL_W_U * k,
                band_y + band_height - PANEL_H_U * k,
            ),
        ] {
            let r = place(pos, window, n);
            assert_eq!(
                (r.x, r.y),
                (x_expected, y_expected),
                "pos {pos:?} a n = {n}"
            );
            assert_eq!((r.width, r.height), (PANEL_W_U * k, PANEL_H_U * k));
        }
    }
}

#[test]
fn row_5_with_a_window_smaller_than_the_panel_it_anchors_top_left_without_panic() {
    for window in [(100, 100), (0, 0), (187, 300)] {
        let r = place((30, 30), window, 1);
        let band_y = window_layout(window, 1).menu.height;
        assert_eq!(r.x, 0, "{window:?}");
        assert_eq!(r.y, band_y, "{window:?}");
        assert_eq!((r.width, r.height), (PANEL_W_U, PANEL_H_U));
    }
}

#[test]
fn row_5_when_resizing_and_when_changing_scale_the_panel_repositions_inside() {
    // Condición 2. Un panel colocado a la derecha de una ventana grande, al achicarla hasta el
    // mínimo se recoloca dentro; al volver a agrandarla no se «acuerda» de la posición recortada
    // más allá de lo que se arrastró (la posición en `u` guardada es la recortada).
    let mut state_2 = PanelState {
        open: true,
        ..PanelState::default()
    };
    let large = (1600, 900);
    state_2.pos_u = Some((900, 80));
    let r_large = state_2.rect(large, 1);
    assert!(r_large.x + r_large.width <= large.0);
    let minimum = window_minimum(1);
    let r_min = state_2.rect(minimum, 1);
    assert!(
        r_min.x >= 0 && r_min.x + r_min.width <= minimum.0,
        "{r_min:?}"
    );
    let band = window_layout(minimum, 1);
    assert!(r_min.y >= band.menu.height && r_min.y + r_min.height <= band.status.y);
    // Cambio de escala: la misma posición en `u` a otra escala.
    let mut other_2 = PanelState {
        open: true,
        pos_u: Some((40, 30)),
        ..PanelState::default()
    };
    let a1 = other_2.rect((2000, 1200), 1);
    let a2 = other_2.rect((4000, 2400), 2);
    assert_eq!((a2.x, a2.width), (a1.x * 2, a1.width * 2));
    assert_eq!(
        a2.y - window_layout((4000, 2400), 2).menu.height,
        (a1.y - window_layout((2000, 1200), 1).menu.height) * 2
    );
    // Y a una escala mayor con la ventana mínima también cabe.
    let r3 = other_2.rect(window_minimum(3), 3);
    assert!(r3.x + r3.width <= window_minimum(3).0);
}

#[test]
fn row_5_dragging_by_the_bar_moves_in_steps_of_u_and_the_initial_position_sticks_to_the_right_of_the_canvas()
 {
    let window = (1280, 720);
    let k = 2i64;
    let window_2 = (window.0 * k, window.1 * k);
    let (x, y) = default_pos_u(window_2, 2);
    let canvas = window_layout(window_2, 2).canvas;
    assert_eq!(x, (canvas.width / k) - PANEL_W_U - 8);
    assert_eq!(y, 8);
    let mut state_2 = PanelState {
        open: true,
        ..PanelState::default()
    };
    let antes = state_2.rect(window_2, 2);
    state_2.drag(11, -7, 2); // 11 px = 5,5 u → 6 u y −7 px = −3,5 u → −4 u (redondeo)
    let after_2 = state_2.rect(window_2, 2);
    assert_eq!((after_2.x - antes.x) % k, 0, "a pasos de u enteras");
    assert_eq!((after_2.y - antes.y) % k, 0);
    assert!((after_2.x - antes.x - 11).abs() <= 1 && (after_2.y - antes.y + 7).abs() <= 1);
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2: el panel cabe siempre y se recoloca (barrido exhaustivo)
// ---------------------------------------------------------------------------------------------

#[test]
fn property_2_the_panel_always_fits_and_repositions() {
    let positions = [(-500, -500), (0, 0), (37, 11), (120, 90), (5000, 5000)];
    for step in 0..=60 {
        let ppp = 1.0 + 0.05 * step as f32;
        let n = text_scale(ppp);
        let k = i64::from(n);
        for (width_extra, height_extra) in [(0, 0), (1, 0), (0, 7), (333, 211)] {
            let window = (
                min_window_width_u() * k + width_extra,
                HEIGHT_MINIMUM_U * k + height_extra,
            );
            let design = window_layout(window, n);
            let (top, below) = (design.menu.height, design.status.y);
            for pos in positions {
                let r = place(pos, window, n);
                assert_eq!(
                    (r.width, r.height),
                    (PANEL_W_U * k, PANEL_H_U * k),
                    "ppp {ppp}"
                );
                assert!(
                    r.x >= 0 && r.x + r.width <= window.0,
                    "ppp {ppp} {pos:?}: {r:?}"
                );
                assert!(
                    r.y >= top && r.y + r.height <= below,
                    "ppp {ppp} {pos:?}: {r:?}"
                );
                // Idempotente: colocar el resultado (en `u` relativas a la franja) da lo mismo.
                let other = place(((r.x) / k, (r.y - top) / k), window, n);
                assert_eq!(other, r, "ppp {ppp} {pos:?}");
                // Un punto físico golpea como mucho un elemento y nunca fuera del panel.
                for (dx, dy) in [(0, 0), (r.width - 1, r.height - 1), (r.width, 0), (-1, -1)] {
                    let hit_2 = hit(dx, dy, n);
                    let inside = (0..r.width).contains(&dx) && (0..r.height).contains(&dy);
                    assert_eq!(hit_2.is_some(), inside, "ppp {ppp}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6: la lista
// ---------------------------------------------------------------------------------------------

fn document(layers: usize) -> Document {
    let image_2 = Image::new(8, 8, Mode::Rgba).expect("imagen");
    let mut doc = Document::new(image_2);
    for i in 1..layers {
        doc.apply(Command::AddLayer {
            name: format!("Capa {}", i + 1),
            at: i,
        })
        .expect("añadir");
    }
    doc
}

fn id(doc: &Document, position: usize) -> LayerId {
    doc.layers()[position].id()
}

#[test]
fn row_6_the_list_starts_with_the_top_layer_and_marks_the_active_one_the_eye_and_the_lock() {
    let mut doc = document(3);
    let (b, c) = (id(&doc, 1), id(&doc, 2));
    doc.apply(Command::SetLayerVisible {
        layer: b,
        visible: false,
    })
    .expect("ocultar");
    doc.apply(Command::SetLayerLocked {
        layer: c,
        locked: true,
    })
    .expect("bloquear");
    let rows_2 = visible_rows(&doc, b, 0);
    let names: Vec<&str> = rows_2.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Capa 3", "Capa 2", "Capa 1"]);
    assert_eq!(
        rows_2
            .iter()
            .map(|f| (f.visible, f.locked, f.active))
            .collect::<Vec<_>>(),
        [
            (true, true, false),
            (false, false, true),
            (true, false, false)
        ]
    );
    assert_eq!(rows_2[0].id, c);
    // Con una sola capa, una sola fila.
    assert_eq!(visible_rows(&document(1), id(&document(1), 0), 0).len(), 1);
}

#[test]
fn row_6_six_visible_rows_with_scroll_up_to_256_layers() {
    assert_eq!(max_scroll(1), 0);
    assert_eq!(max_scroll(6), 0);
    assert_eq!(max_scroll(7), 1);
    assert_eq!(max_scroll(256), 250);
    let doc = document(7);
    let active_2 = id(&doc, 0);
    assert_eq!(visible_rows(&doc, active_2, 0).len(), 6);
    let below = visible_rows(&doc, active_2, 1);
    assert_eq!(below.len(), 6);
    assert_eq!(below[0].name, "Capa 6");
    assert_eq!(below[5].name, "Capa 1");
    assert!(below[5].active);
    // Un desplazamiento de más se recorta.
    assert_eq!(visible_rows(&doc, active_2, 99).len(), 6);
    let large = document(256);
    assert_eq!(visible_rows(&large, id(&large, 0), 250).len(), 6);
    assert_eq!(visible_rows(&large, id(&large, 0), 250)[5].name, "Capa 1");
    // La rueda baja 3 filas por muesca y se recorta.
    let mut state_2 = PanelState::default();
    state_2.scroll_by(1, 256);
    assert_eq!(state_2.scroll, 3);
    state_2.scroll_by(100, 256);
    assert_eq!(state_2.scroll, 250);
    state_2.scroll_by(-100, 256);
    assert_eq!(state_2.scroll, 0);
    state_2.scroll_by(1, 5);
    assert_eq!(state_2.scroll, 0, "con 5 capas no hay nada que desplazar");
}

#[test]
fn row_6_choosing_or_creating_a_layer_brings_it_into_view() {
    // `scroll_for(total, posición de la active_2 from_2 below, scroll actual)`.
    assert_eq!(scroll_for(20, 19, 5), 0, "la de arriba es la fila 0");
    assert_eq!(
        scroll_for(20, 0, 0),
        14,
        "la de abajo es la fila 19: scroll 14"
    );
    assert_eq!(scroll_for(20, 10, 4), 4, "ya visible: no cambia");
    assert_eq!(scroll_for(20, 14, 6), 5, "fila 5 con scroll 6: sube a 5");
    assert_eq!(scroll_for(20, 8, 0), 6, "fila 11 con scroll 0: baja a 6");
    assert_eq!(scroll_for(3, 0, 0), 0);
}

#[test]
fn row_6_long_names_are_clipped_with_an_ellipsis() {
    let just = "a".repeat(NAME_CHARS);
    assert_eq!(truncate_name(&just), just);
    assert_eq!(
        truncate_name(&"a".repeat(NAME_CHARS + 1)),
        format!("{}…", "a".repeat(15))
    );
    assert_eq!(
        truncate_name("Ñandú ñoño con acentos largos"),
        "Ñandú ñoño con …"
    );
    assert_eq!(truncate_name(""), "");
    // Un nombre con 4 bytes por carácter no parte un carácter.
    assert_eq!(truncate_name(&"é".repeat(40)).chars().count(), NAME_CHARS);
}

#[test]
fn row_6_the_memory_footer_formats_kib_mib_and_gib_and_warns_from_1_gib() {
    assert_eq!(format_bytes(0), "0 KiB");
    assert_eq!(format_bytes(16 * 1024), "16 KiB");
    assert_eq!(format_bytes(768 * MIB), "768 MiB");
    assert_eq!(format_bytes(1023 * MIB), "1023 MiB");
    assert_eq!(format_bytes(GIB), "1 GiB");
    assert_eq!(format_bytes(1_400_000_000), "1,3 GiB");
    assert_eq!(
        format_bytes(5 * 256 * MIB),
        "1,2 GiB",
        "se redondea hacia abajo"
    );
    assert_eq!(format_bytes(2 * GIB), "2 GiB");
    assert_eq!(
        memory_line(768 * MIB, 2 * GIB),
        ("Memoria: 768 MiB / 2 GiB".to_string(), false)
    );
    assert_eq!(
        memory_line(1023 * MIB, 2 * GIB),
        ("Memoria: 1023 MiB / 2 GiB".to_string(), false)
    );
    assert_eq!(
        memory_line(GIB, 2 * GIB),
        ("Memoria: 1 GiB / 2 GiB".to_string(), true)
    );
    assert!(memory_line(1_400_000_000, 2 * GIB).1);
    // Nunca pasa de 25 caracteres (el ancho del pie).
    for bytes in [
        0,
        1,
        1023,
        MIB,
        1023 * MIB,
        GIB,
        3 * GIB / 2,
        2 * GIB,
        usize::MAX,
    ] {
        for limit in [2 * GIB, GIB / 2, 3 * GIB / 2] {
            assert!(
                memory_line(bytes, limit).0.chars().count() <= 25,
                "{bytes} / {limit}"
            );
        }
    }
}
