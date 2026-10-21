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
const ALTO_MINIMO_U: i64 = 341;
const GIB: usize = 1 << 30;
const MIB: usize = 1 << 20;

/// Todos los elementos con su rectángulo, para los barridos de recubrimiento.
fn elementos() -> Vec<(String, RectU)> {
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
    for fila in 0..VISIBLE_ROWS {
        v.push((format!("ojo {fila}"), eye_u(fila)));
        v.push((format!("candado {fila}"), lock_u(fila)));
        v.push((format!("nombre {fila}"), name_u(fila)));
    }
    for boton in PanelButton::ALL {
        v.push((format!("botón {boton:?}"), button_u(boton)));
    }
    v
}

fn solapan(a: RectU, b: RectU) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

fn ventana_minima(n: i64) -> (i64, i64) {
    (min_window_width_u() * n, ALTO_MINIMO_U * n)
}

// ---------------------------------------------------------------------------------------------
// Fila 5: geometría y colocación
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_5_las_medidas_son_188_por_277_con_seis_filas_de_24_u() {
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
    for fila in 0..VISIBLE_ROWS {
        let y = 21 + 25 * fila as i64;
        assert_eq!(
            eye_u(fila),
            RectU {
                x: 5,
                y,
                w: 24,
                h: 24
            },
            "ojo {fila}"
        );
        assert_eq!(
            lock_u(fila),
            RectU {
                x: 30,
                y,
                w: 24,
                h: 24
            },
            "candado {fila}"
        );
        assert_eq!(
            name_u(fila),
            RectU {
                x: 58,
                y,
                w: 114,
                h: 24
            },
            "nombre {fila}"
        );
        assert_eq!(
            row_u(fila),
            RectU {
                x: 5,
                y,
                w: 167,
                h: 24
            },
            "fila {fila}"
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
    for (i, boton) in PanelButton::ALL.into_iter().enumerate() {
        assert_eq!(
            button_u(boton),
            RectU {
                x: 5 + 25 * i as i64,
                y: 174,
                w: 24,
                h: 24
            },
            "{boton:?}"
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
fn fila_5_ningun_elemento_solapa_con_otro_y_todos_caben_en_el_panel() {
    let todos = elementos();
    for (nombre, r) in &todos {
        assert!(
            r.x >= 0 && r.y >= 0 && r.x + r.w <= PANEL_W_U && r.y + r.h <= PANEL_H_U,
            "{nombre}: {r:?}"
        );
        assert!(r.w > 0 && r.h > 0, "{nombre}");
    }
    for (i, (a, ra)) in todos.iter().enumerate() {
        for (b, rb) in &todos[i + 1..] {
            assert!(!solapan(*ra, *rb), "{a} solapa con {b}");
        }
    }
}

#[test]
fn fila_5_hit_golpea_el_elemento_en_su_primera_y_ultima_coordenada_a_todas_las_escalas() {
    for n in 1..=8u32 {
        let k = i64::from(n);
        let esperado = |nombre: &str| -> PanelHit {
            match nombre {
                "título" => PanelHit::Title,
                "cerrar" => PanelHit::Close,
                "barra" => PanelHit::Scrollbar,
                "contador" => PanelHit::Count,
                "nombre" => PanelHit::NameField,
                "deslizador" => PanelHit::OpacitySlider,
                "opacidad" => PanelHit::OpacityField,
                "mezcla" => PanelHit::BlendButton,
                "memoria" => PanelHit::Memory,
                otro if otro.starts_with("botón ") => {
                    let boton = PanelButton::ALL
                        .into_iter()
                        .find(|b| format!("botón {b:?}") == otro)
                        .expect("botón");
                    PanelHit::Button(boton)
                }
                otro => {
                    let fila: usize = otro.rsplit(' ').next().unwrap().parse().unwrap();
                    if otro.starts_with("ojo") {
                        PanelHit::Eye(fila)
                    } else if otro.starts_with("candado") {
                        PanelHit::Lock(fila)
                    } else {
                        PanelHit::Name(fila)
                    }
                }
            }
        };
        for (nombre, r) in elementos() {
            let quiero = esperado(&nombre);
            for (x, y) in [
                (r.x * k, r.y * k),
                ((r.x + r.w) * k - 1, (r.y + r.h) * k - 1),
            ] {
                assert_eq!(
                    hit(x, y, n),
                    Some(quiero),
                    "{nombre} en ({x}, {y}) a n = {n}"
                );
            }
            // Un píxel fuera por cada lado no es el mismo elemento.
            for (x, y) in [
                (r.x * k - 1, r.y * k),
                ((r.x + r.w) * k, r.y * k),
                (r.x * k, r.y * k - 1),
                (r.x * k, (r.y + r.h) * k),
            ] {
                assert_ne!(
                    hit(x, y, n),
                    Some(quiero),
                    "{nombre} en ({x}, {y}) a n = {n}"
                );
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
fn fila_5_place_recorta_a_la_franja_central_con_posiciones_dentro_fuera_y_negativas() {
    for n in [1u32, 2, 3] {
        let k = i64::from(n);
        let ventana = ventana_minima(k);
        let diseno = window_layout(ventana, n);
        let franja_y = diseno.menu.height;
        let franja_alto = diseno.status.y - diseno.menu.height;
        assert_eq!(
            franja_alto,
            305 * k,
            "la franja central mide 305 u a la ventana mínima"
        );
        for (pos, x_esperado, y_esperado) in [
            ((0, 0), 0, franja_y),
            ((10, 20), 10 * k, franja_y + 20 * k),
            ((-50, -50), 0, franja_y),
            (
                (10_000, 10_000),
                ventana.0 - PANEL_W_U * k,
                franja_y + franja_alto - PANEL_H_U * k,
            ),
        ] {
            let r = place(pos, ventana, n);
            assert_eq!(
                (r.x, r.y),
                (x_esperado, y_esperado),
                "pos {pos:?} a n = {n}"
            );
            assert_eq!((r.width, r.height), (PANEL_W_U * k, PANEL_H_U * k));
        }
    }
}

#[test]
fn fila_5_con_una_ventana_menor_que_el_panel_se_ancla_arriba_a_la_izquierda_sin_panico() {
    for ventana in [(100, 100), (0, 0), (187, 300)] {
        let r = place((30, 30), ventana, 1);
        let franja_y = window_layout(ventana, 1).menu.height;
        assert_eq!(r.x, 0, "{ventana:?}");
        assert_eq!(r.y, franja_y, "{ventana:?}");
        assert_eq!((r.width, r.height), (PANEL_W_U, PANEL_H_U));
    }
}

#[test]
fn fila_5_al_redimensionar_y_al_cambiar_de_escala_el_panel_se_recoloca_dentro() {
    // Condición 2. Un panel colocado a la derecha de una ventana grande, al achicarla hasta el
    // mínimo se recoloca dentro; al volver a agrandarla no se «acuerda» de la posición recortada
    // más allá de lo que se arrastró (la posición en `u` guardada es la recortada).
    let mut estado = PanelState {
        open: true,
        ..PanelState::default()
    };
    let grande = (1600, 900);
    estado.pos_u = Some((900, 80));
    let r_grande = estado.rect(grande, 1);
    assert!(r_grande.x + r_grande.width <= grande.0);
    let minima = ventana_minima(1);
    let r_min = estado.rect(minima, 1);
    assert!(
        r_min.x >= 0 && r_min.x + r_min.width <= minima.0,
        "{r_min:?}"
    );
    let franja = window_layout(minima, 1);
    assert!(r_min.y >= franja.menu.height && r_min.y + r_min.height <= franja.status.y);
    // Cambio de escala: la misma posición en `u` a otra escala.
    let mut otro = PanelState {
        open: true,
        pos_u: Some((40, 30)),
        ..PanelState::default()
    };
    let a1 = otro.rect((2000, 1200), 1);
    let a2 = otro.rect((4000, 2400), 2);
    assert_eq!((a2.x, a2.width), (a1.x * 2, a1.width * 2));
    assert_eq!(
        a2.y - window_layout((4000, 2400), 2).menu.height,
        (a1.y - window_layout((2000, 1200), 1).menu.height) * 2
    );
    // Y a una escala mayor con la ventana mínima también cabe.
    let r3 = otro.rect(ventana_minima(3), 3);
    assert!(r3.x + r3.width <= ventana_minima(3).0);
}

#[test]
fn fila_5_arrastrar_por_la_barra_mueve_a_pasos_de_u_y_la_posicion_inicial_pega_a_la_derecha_del_lienzo()
 {
    let ventana = (1280, 720);
    let k = 2i64;
    let ventana2 = (ventana.0 * k, ventana.1 * k);
    let (x, y) = default_pos_u(ventana2, 2);
    let canvas = window_layout(ventana2, 2).canvas;
    assert_eq!(x, (canvas.width / k) - PANEL_W_U - 8);
    assert_eq!(y, 8);
    let mut estado = PanelState {
        open: true,
        ..PanelState::default()
    };
    let antes = estado.rect(ventana2, 2);
    estado.drag(11, -7, 2); // 11 px = 5,5 u → 6 u y −7 px = −3,5 u → −4 u (redondeo)
    let despues = estado.rect(ventana2, 2);
    assert_eq!((despues.x - antes.x) % k, 0, "a pasos de u enteras");
    assert_eq!((despues.y - antes.y) % k, 0);
    assert!((despues.x - antes.x - 11).abs() <= 1 && (despues.y - antes.y + 7).abs() <= 1);
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2: el panel cabe siempre y se recoloca (barrido exhaustivo)
// ---------------------------------------------------------------------------------------------

#[test]
fn propiedad_2_el_panel_cabe_siempre_y_se_recoloca() {
    let posiciones = [(-500, -500), (0, 0), (37, 11), (120, 90), (5000, 5000)];
    for paso in 0..=60 {
        let ppp = 1.0 + 0.05 * paso as f32;
        let n = text_scale(ppp);
        let k = i64::from(n);
        for (ancho_extra, alto_extra) in [(0, 0), (1, 0), (0, 7), (333, 211)] {
            let ventana = (
                min_window_width_u() * k + ancho_extra,
                ALTO_MINIMO_U * k + alto_extra,
            );
            let diseno = window_layout(ventana, n);
            let (arriba, abajo) = (diseno.menu.height, diseno.status.y);
            for pos in posiciones {
                let r = place(pos, ventana, n);
                assert_eq!(
                    (r.width, r.height),
                    (PANEL_W_U * k, PANEL_H_U * k),
                    "ppp {ppp}"
                );
                assert!(
                    r.x >= 0 && r.x + r.width <= ventana.0,
                    "ppp {ppp} {pos:?}: {r:?}"
                );
                assert!(
                    r.y >= arriba && r.y + r.height <= abajo,
                    "ppp {ppp} {pos:?}: {r:?}"
                );
                // Idempotente: colocar el resultado (en `u` relativas a la franja) da lo mismo.
                let otra = place(((r.x) / k, (r.y - arriba) / k), ventana, n);
                assert_eq!(otra, r, "ppp {ppp} {pos:?}");
                // Un punto físico golpea como mucho un elemento y nunca fuera del panel.
                for (dx, dy) in [(0, 0), (r.width - 1, r.height - 1), (r.width, 0), (-1, -1)] {
                    let golpe = hit(dx, dy, n);
                    let dentro = (0..r.width).contains(&dx) && (0..r.height).contains(&dy);
                    assert_eq!(golpe.is_some(), dentro, "ppp {ppp}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6: la lista
// ---------------------------------------------------------------------------------------------

fn documento(capas: usize) -> Document {
    let imagen = Image::new(8, 8, Mode::Rgba).expect("imagen");
    let mut doc = Document::new(imagen);
    for i in 1..capas {
        doc.apply(Command::AddLayer {
            name: format!("Capa {}", i + 1),
            at: i,
        })
        .expect("añadir");
    }
    doc
}

fn id(doc: &Document, posicion: usize) -> LayerId {
    doc.layers()[posicion].id()
}

#[test]
fn fila_6_la_lista_empieza_por_la_capa_de_arriba_y_marca_la_activa_el_ojo_y_el_candado() {
    let mut doc = documento(3);
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
    let filas = visible_rows(&doc, b, 0);
    let nombres: Vec<&str> = filas.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(nombres, ["Capa 3", "Capa 2", "Capa 1"]);
    assert_eq!(
        filas
            .iter()
            .map(|f| (f.visible, f.locked, f.active))
            .collect::<Vec<_>>(),
        [
            (true, true, false),
            (false, false, true),
            (true, false, false)
        ]
    );
    assert_eq!(filas[0].id, c);
    // Con una sola capa, una sola fila.
    assert_eq!(
        visible_rows(&documento(1), id(&documento(1), 0), 0).len(),
        1
    );
}

#[test]
fn fila_6_seis_filas_visibles_con_desplazamiento_hasta_256_capas() {
    assert_eq!(max_scroll(1), 0);
    assert_eq!(max_scroll(6), 0);
    assert_eq!(max_scroll(7), 1);
    assert_eq!(max_scroll(256), 250);
    let doc = documento(7);
    let activa = id(&doc, 0);
    assert_eq!(visible_rows(&doc, activa, 0).len(), 6);
    let abajo = visible_rows(&doc, activa, 1);
    assert_eq!(abajo.len(), 6);
    assert_eq!(abajo[0].name, "Capa 6");
    assert_eq!(abajo[5].name, "Capa 1");
    assert!(abajo[5].active);
    // Un desplazamiento de más se recorta.
    assert_eq!(visible_rows(&doc, activa, 99).len(), 6);
    let grande = documento(256);
    assert_eq!(visible_rows(&grande, id(&grande, 0), 250).len(), 6);
    assert_eq!(visible_rows(&grande, id(&grande, 0), 250)[5].name, "Capa 1");
    // La rueda baja 3 filas por muesca y se recorta.
    let mut estado = PanelState::default();
    estado.scroll_by(1, 256);
    assert_eq!(estado.scroll, 3);
    estado.scroll_by(100, 256);
    assert_eq!(estado.scroll, 250);
    estado.scroll_by(-100, 256);
    assert_eq!(estado.scroll, 0);
    estado.scroll_by(1, 5);
    assert_eq!(estado.scroll, 0, "con 5 capas no hay nada que desplazar");
}

#[test]
fn fila_6_elegir_o_crear_una_capa_la_lleva_a_la_vista() {
    // `scroll_for(total, posición de la activa desde abajo, scroll actual)`.
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
fn fila_6_los_nombres_largos_se_recortan_con_puntos_suspensivos() {
    let justo = "a".repeat(NAME_CHARS);
    assert_eq!(truncate_name(&justo), justo);
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
fn fila_6_el_pie_de_memoria_formatea_kib_mib_y_gib_y_avisa_desde_1_gib() {
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
        for limite in [2 * GIB, GIB / 2, 3 * GIB / 2] {
            assert!(
                memory_line(bytes, limite).0.chars().count() <= 25,
                "{bytes} / {limite}"
            );
        }
    }
}
