//! Unidad I.2a, parte pura (specs/interfaz-dos-colores.md): fila 1 (fondo por defecto), fila 3
//! (bloqueos del color con un trazo), fila 5 (banderas y anillos), fila 6 (geometría del
//! indicador y del selector), fila 7 (teclas y menú) y la propiedad 3 (geometría de una sola
//! fuente). Sin ventana.

use egui::Color32;
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_ui::actions::{AppAction, Locks, Origin, Platform, is_blocked};
use px_ui::color::{
    ColorChoice, Role, RoleFlags, default_background, info_flags_roles, pixel_for_choice,
};
use px_ui::edit_layout::{block_height_px, min_window_height_pt, min_window_width_pt};
use px_ui::keymap::{Binding, Conflict, KeyId, Keymap, Platforms, Press};
use px_ui::menu::{MENUS, popup_width_u, shortcut_for, shortcut_text};
use px_ui::palette_grid::{
    self, CELL_U, GAP_U, GRID_X_U, GRID_Y_U, INDICATOR_H_U, LINE1_Y_U, LINE2_Y_U, SEPARATOR_Y_U,
    corner_rects, indicator_hit, indicator_rects, ring_rects, selector_hit, selector_rect,
};
use px_ui::text::text_scale;
use px_ui::theme::{RING_ACTIVE, RING_BACKGROUND, SURFACE_1, contrast_ratio};
use px_ui::view::ScreenRect;

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

// ---------------------------------------------------------------------------------------------
// Fila 1: fondo por defecto
// ---------------------------------------------------------------------------------------------

fn indexada(colores: Vec<Rgba>, transparente: Option<u8>) -> Image {
    let mut paleta = Palette::from_colors(colores).expect("paleta");
    paleta.set_transparent_index(transparente).expect("existe");
    Image::new_indexed(4, 4, paleta).expect("imagen")
}

fn opacos(n: usize) -> Vec<Rgba> {
    (0..n).map(|i| Rgba::opaque(i as u8 * 10, 1, 2)).collect()
}

#[test]
fn fila_1_rgba_gris_con_alfa_y_gris_usan_la_ranura_9() {
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        let image = Image::new(4, 4, mode).expect("imagen");
        assert_eq!(default_background(&image), ColorChoice::Slot(9), "{mode:?}");
    }
}

#[test]
fn fila_1_la_ranura_9_es_transparente_en_rgba_y_gris_con_alfa_y_blanca_en_gris() {
    let rgba = Image::new(4, 4, Mode::Rgba).expect("imagen");
    assert_eq!(
        pixel_for_choice(&rgba, default_background(&rgba)),
        Some(Pixel::Rgba(Rgba::TRANSPARENT))
    );
    let ga = Image::new(4, 4, Mode::GrayAlpha).expect("imagen");
    assert_eq!(
        pixel_for_choice(&ga, default_background(&ga)),
        Some(Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );
    let gray = Image::new(4, 4, Mode::Gray).expect("imagen");
    assert_eq!(
        pixel_for_choice(&gray, default_background(&gray)),
        Some(Pixel::Gray(255))
    );
}

#[test]
fn fila_1_indexada_con_indice_transparente_usa_esa_entrada() {
    let mut colores = opacos(5);
    colores[2] = Rgba::new(0, 255, 0, 0);
    let image = indexada(colores, Some(3));
    assert_eq!(default_background(&image), ColorChoice::Entry(3));
}

#[test]
fn fila_1_indexada_sin_indice_usa_la_primera_entrada_de_alfa_0() {
    let mut colores = opacos(5);
    colores[2] = Rgba::new(0, 255, 0, 0);
    colores[4] = Rgba::new(9, 9, 9, 0);
    assert_eq!(
        default_background(&indexada(colores.clone(), None)),
        ColorChoice::Entry(2)
    );
    colores[2] = Rgba::opaque(0, 255, 0);
    assert_eq!(
        default_background(&indexada(colores, None)),
        ColorChoice::Entry(4)
    );
}

#[test]
fn fila_1_indexada_sin_ninguna_y_paleta_de_una_entrada_usan_la_entrada_0() {
    assert_eq!(
        default_background(&indexada(opacos(5), None)),
        ColorChoice::Entry(0)
    );
    assert_eq!(
        default_background(&indexada(opacos(1), None)),
        ColorChoice::Entry(0)
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 3 (parte pura): bloqueos del color con un trazo
// ---------------------------------------------------------------------------------------------

const ORIGENES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];

const ACCIONES_DE_COLOR: [AppAction; 5] = [
    AppAction::ColorNext,
    AppAction::ColorPrev,
    AppAction::BackgroundNext,
    AppAction::BackgroundPrev,
    AppAction::SelectColor(3),
];

fn solo(f: impl FnOnce(&mut Locks)) -> Locks {
    let mut locks = Locks::default();
    f(&mut locks);
    locks
}

fn con_trazo() -> Locks {
    Locks {
        gesture: true,
        stroke: true,
        ..Locks::default()
    }
}

#[test]
fn fila_3_las_acciones_de_color_con_un_trazo_en_curso_siguen_permitidas() {
    for action in ACCIONES_DE_COLOR {
        for origin in ORIGENES {
            assert!(
                !is_blocked(action, con_trazo(), origin),
                "{action:?} con {origin:?}"
            );
        }
    }
}

#[test]
fn fila_3_el_color_con_el_deslizador_o_una_muestra_arrastrandose_sigue_bloqueado() {
    // `gesture` sin `stroke`: el deslizador del editor o el arrastre de una muestra.
    for action in ACCIONES_DE_COLOR {
        for origin in ORIGENES {
            assert!(is_blocked(action, solo(|l| l.gesture = true), origin));
        }
    }
}

#[test]
fn fila_3_el_color_se_bloquea_con_dialogo_modal_campo_y_quita_pendiente() {
    for action in ACCIONES_DE_COLOR {
        for origin in ORIGENES {
            for locks in [
                solo(|l| l.dialog = true),
                solo(|l| l.modal = true),
                solo(|l| l.text_focus = true),
                solo(|l| l.removal = true),
                Locks {
                    removal: true,
                    ..con_trazo()
                },
            ] {
                assert!(is_blocked(action, locks, origin), "{action:?} {locks:?}");
            }
        }
    }
}

#[test]
fn fila_3_el_resto_de_acciones_sigue_bloqueado_con_un_trazo() {
    for action in [
        AppAction::Undo,
        AppAction::Redo,
        AppAction::Open,
        AppAction::NewImage,
        AppAction::ClearPage,
        AppAction::SelectTool(px_ui::tool::Tool::Line),
    ] {
        for origin in ORIGENES {
            assert!(is_blocked(action, con_trazo(), origin), "{action:?}");
        }
    }
}

#[test]
fn fila_8_borrar_la_pagina_se_bloquea_con_gesto_dialogo_modal_y_campo_pero_no_con_quita() {
    for origin in ORIGENES {
        for locks in [
            solo(|l| l.gesture = true),
            con_trazo(),
            solo(|l| l.dialog = true),
            solo(|l| l.modal = true),
            solo(|l| l.text_focus = true),
        ] {
            assert!(is_blocked(AppAction::ClearPage, locks, origin), "{locks:?}");
        }
        assert!(!is_blocked(
            AppAction::ClearPage,
            solo(|l| l.removal = true),
            origin
        ));
        assert!(!is_blocked(AppAction::ClearPage, Locks::default(), origin));
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: banderas y anillos
// ---------------------------------------------------------------------------------------------

fn banderas(f: impl FnOnce(&mut RoleFlags)) -> RoleFlags {
    let mut flags = RoleFlags::default();
    f(&mut flags);
    flags
}

#[test]
fn fila_5_banderas_de_cada_rol_y_sus_combinaciones() {
    let casos = [
        (RoleFlags::default(), ""),
        (banderas(|f| f.foreground = true), "frente"),
        (banderas(|f| f.background = true), "fondo"),
        (
            banderas(|f| {
                f.foreground = true;
                f.background = true;
            }),
            "frente · fondo",
        ),
        (
            banderas(|f| {
                f.hover = true;
                f.foreground = true;
                f.background = true;
            }),
            "cursor · frente · fondo",
        ),
        (
            banderas(|f| {
                f.background = true;
                f.transparent = true;
            }),
            "fondo · transparente",
        ),
        (banderas(|f| f.alpha0 = true), "alfa 0"),
        (
            banderas(|f| {
                f.alpha0 = true;
                f.transparent = true;
            }),
            "transparente",
        ),
        (banderas(|f| f.in_range = true), "rango"),
        (
            banderas(|f| {
                f.in_range = true;
                f.foreground = true;
            }),
            "frente",
        ),
    ];
    for (flags, esperado) in casos {
        assert_eq!(info_flags_roles(flags, 32), esperado, "{flags:?}");
    }
}

#[test]
fn fila_5_si_no_cabe_en_32_columnas_se_omite_cursor() {
    let peor = banderas(|f| {
        f.hover = true;
        f.foreground = true;
        f.background = true;
        f.transparent = true;
    });
    assert_eq!(
        info_flags_roles(peor, 32),
        "frente · fondo · transparente",
        "sin cursor"
    );
    assert_eq!(
        info_flags_roles(peor, 40),
        "cursor · frente · fondo · transparente",
        "con sitio, cursor se queda"
    );
    for columns in 29..=64 {
        assert!(info_flags_roles(peor, columns).chars().count() <= columns.max(29));
    }
}

#[test]
fn fila_5_el_anillo_del_fondo_contrasta_3_a_1_y_no_es_el_del_primer_plano() {
    assert!(contrast_ratio(RING_BACKGROUND, SURFACE_1) >= 3.0);
    assert_ne!(RING_BACKGROUND, RING_ACTIVE);
    let _: Color32 = RING_BACKGROUND;
}

fn celda(n: i64) -> ScreenRect {
    ScreenRect {
        x: 0,
        y: 0,
        width: CELL_U * n,
        height: CELL_U * n,
    }
}

fn se_solapan(a: &ScreenRect, b: &ScreenRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

#[test]
fn fila_5_el_anillo_del_fondo_son_8_rectangulos_disjuntos_en_4_esquinas_fuera_de_la_muestra() {
    for n in 1..=3 {
        let cell = celda(n);
        let rects = corner_rects(cell, 0, 2 * n, 5 * n);
        assert_eq!(rects.len(), 8);
        for (i, a) in rects.iter().enumerate() {
            assert!(!se_solapan(a, &cell), "{a:?} tapa la muestra");
            for b in rects.iter().skip(i + 1) {
                assert!(!se_solapan(a, b), "{a:?} y {b:?}");
            }
            // Dentro del hueco de 4 u.
            assert!(a.x >= -GAP_U * n && a.x + a.width <= (CELL_U + GAP_U) * n);
            assert!(a.y >= -GAP_U * n && a.y + a.height <= (CELL_U + GAP_U) * n);
        }
        // Cada esquina de la muestra tiene su L (dos rectángulos); no hay anillo en el medio de
        // los lados (forma distinta a la del primer plano).
        let medio = ScreenRect {
            x: 4 * n,
            y: -2 * n,
            width: 2 * n,
            height: 2 * n,
        };
        assert!(rects.iter().all(|r| !se_solapan(r, &medio)));
    }
}

#[test]
fn fila_5_con_la_misma_muestra_el_del_primer_plano_va_dentro_y_el_del_fondo_fuera_sin_solaparse() {
    for n in 1..=3 {
        let cell = celda(n);
        let dentro = ring_rects(cell, 2 * n);
        let fuera = corner_rects(cell, 2 * n, 2 * n, 5 * n);
        for a in &dentro {
            for b in &fuera {
                assert!(!se_solapan(a, b), "{a:?} y {b:?}");
            }
        }
        for b in &fuera {
            assert!(b.x >= -4 * n && b.x + b.width <= (CELL_U + 4) * n);
            assert!(!se_solapan(b, &cell));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6: geometría del indicador y del selector
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_6_la_banda_mide_36_u_y_de_ella_cuelga_el_resto_del_panel() {
    assert_eq!(INDICATOR_H_U, 36);
    assert_eq!(LINE1_Y_U, INDICATOR_H_U + 4);
    assert_eq!(LINE2_Y_U, INDICATOR_H_U + 18);
    assert_eq!(SEPARATOR_Y_U, INDICATOR_H_U + 36);
    assert_eq!(GRID_Y_U, INDICATOR_H_U + 41);
    assert_eq!(GRID_Y_U, 77);
}

#[test]
fn fila_6_el_pequeno_primer_plano_14_por_14_va_encima_del_grande_fondo_20_por_20() {
    for n in 1..=3i64 {
        let (fg, bg) = indicator_rects(n as u32);
        assert_eq!((fg.width, fg.height), (14 * n, 14 * n));
        assert_eq!((bg.width, bg.height), (20 * n, 20 * n));
        assert!(se_solapan(&fg, &bg), "se superponen");
        // El conjunto cabe en la banda con su margen de 4 u y su borde de 1 u.
        for r in [fg, bg] {
            assert!(r.x >= 4 * n && r.y >= 4 * n);
            assert!(r.x + r.width <= (px_ui_panel_width() - 4) * n);
            assert!(r.y + r.height <= (INDICATOR_H_U - 4) * n);
        }
    }
}

fn px_ui_panel_width() -> i64 {
    palette_grid::PANEL_WIDTH_U
}

#[test]
fn fila_6_indicator_hit_distingue_primer_plano_fondo_y_huecos() {
    for n in 1..=3i64 {
        let (fg, bg) = indicator_rects(n as u32);
        let centro = |r: &ScreenRect| (r.x + r.width / 2, r.y + r.height / 2);
        let (x, y) = centro(&fg);
        assert_eq!(indicator_hit(x, y, n as u32), Some(Role::Foreground));
        // Donde se superponen manda el pequeño.
        let (x, y) = (bg.x + 1, bg.y + 1);
        assert_eq!(indicator_hit(x, y, n as u32), Some(Role::Foreground));
        // La parte visible del grande.
        let (x, y) = (bg.x + bg.width - 1, bg.y + bg.height - 1);
        assert_eq!(indicator_hit(x, y, n as u32), Some(Role::Background));
        // Las esquinas vacías de la caja que contiene a los dos.
        assert_eq!(indicator_hit(bg.x + bg.width - 1, fg.y, n as u32), None);
        assert_eq!(indicator_hit(fg.x, bg.y + bg.height - 1, n as u32), None);
        // Fuera de la banda y los bordes exactos.
        assert_eq!(indicator_hit(-1, 0, n as u32), None);
        assert_eq!(indicator_hit(fg.x - 1, fg.y, n as u32), None);
        assert_eq!(indicator_hit(0, INDICATOR_H_U * n, n as u32), None);
    }
}

#[test]
fn fila_6_los_dos_selectores_son_filas_excluyentes_dentro_de_la_banda() {
    for n in 1..=3i64 {
        let a = selector_rect(Role::Foreground, n as u32);
        let b = selector_rect(Role::Background, n as u32);
        assert!(!se_solapan(&a, &b));
        assert!(a.y + a.height <= b.y);
        assert!(b.y + b.height <= INDICATOR_H_U * n);
        let (fg, bg) = indicator_rects(n as u32);
        for fila in [&a, &b] {
            assert!(!se_solapan(fila, &fg) && !se_solapan(fila, &bg));
        }
        assert_eq!(
            selector_hit(a.x + 1, a.y + 1, n as u32),
            Some(Role::Foreground)
        );
        assert_eq!(
            selector_hit(b.x + b.width - 1, b.y + b.height - 1, n as u32),
            Some(Role::Background)
        );
        assert_eq!(selector_hit(a.x - 1, a.y, n as u32), None);
        assert_eq!(selector_hit(a.x, b.y + b.height, n as u32), None);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: teclas y menú
// ---------------------------------------------------------------------------------------------

fn pulsa(key: KeyId) -> Press {
    Press {
        key: Some(key),
        command: false,
        shift: false,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }
}

const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

fn accion(press: &Press, platform: Platform) -> Option<AppAction> {
    Keymap::default_scheme().action_for(press, platform)
}

#[test]
fn fila_7_llaves_y_corchetes_del_fondo_con_cualquier_mayus_o_alt() {
    for platform in PLATAFORMAS {
        for (c, esperada) in [
            ('}', AppAction::BackgroundNext),
            ('{', AppAction::BackgroundPrev),
        ] {
            for shift in [false, true] {
                for alt in [false, true] {
                    let press = Press {
                        shift,
                        alt,
                        ..pulsa(KeyId::Char(c))
                    };
                    assert_eq!(
                        accion(&press, platform),
                        Some(esperada),
                        "{c} {shift} {alt}"
                    );
                }
            }
            let con_ctrl = Press {
                command: true,
                ..pulsa(KeyId::Char(c))
            };
            assert_eq!(accion(&con_ctrl, platform), None);
        }
    }
}

#[test]
fn fila_7_k_mayuscula_borra_la_pagina_y_k_sin_mayus_o_con_ctrl_o_alt_no_hace_nada() {
    for platform in PLATAFORMAS {
        let k = KeyId::Char('k');
        let con_mayus = Press {
            shift: true,
            ..pulsa(k)
        };
        assert_eq!(accion(&con_mayus, platform), Some(AppAction::ClearPage));
        assert_eq!(accion(&pulsa(k), platform), None);
        for press in [
            Press {
                command: true,
                shift: true,
                ..pulsa(k)
            },
            Press {
                command: true,
                ..pulsa(k)
            },
            Press {
                alt: true,
                shift: true,
                ..pulsa(k)
            },
            Press {
                other_ctrl: true,
                shift: true,
                ..pulsa(k)
            },
        ] {
            assert_eq!(accion(&press, platform), None, "{press:?}");
        }
    }
}

#[test]
fn fila_7_mayus_re_pag_y_av_pag_son_el_fondo_y_sin_mayus_siguen_siendo_el_primer_plano() {
    for platform in PLATAFORMAS {
        let con = |key, shift, alt| Press {
            shift,
            alt,
            ..pulsa(key)
        };
        assert_eq!(
            accion(&con(KeyId::PageDown, true, false), platform),
            Some(AppAction::BackgroundNext)
        );
        assert_eq!(
            accion(&con(KeyId::PageUp, true, true), platform),
            Some(AppAction::BackgroundPrev)
        );
        assert_eq!(
            accion(&con(KeyId::PageDown, false, false), platform),
            Some(AppAction::ColorNext)
        );
        assert_eq!(
            accion(&con(KeyId::PageUp, false, true), platform),
            Some(AppAction::ColorPrev)
        );
        // `Inicio` no cambia con `Mayús`.
        for shift in [false, true] {
            assert_eq!(
                accion(&con(KeyId::Home, shift, false), platform),
                Some(AppAction::ZoomFit)
            );
        }
    }
}

#[test]
fn fila_7_la_tabla_tiene_30_filas_sin_choques_y_solo_las_nueve_modernas_llevan_command() {
    let keymap = Keymap::default_scheme();
    assert_eq!(keymap.rows().len(), 30);
    assert_eq!(keymap.conflicts(), Vec::<Conflict>::new());
    let con_command = keymap.rows().iter().filter(|row| row.command).count();
    assert_eq!(con_command, 9);
}

#[test]
fn fila_7_dos_filas_de_re_pag_con_distinto_mayus_no_chocan_pero_iguales_si() {
    let fila = |shift, action| Binding {
        key: KeyId::PageUp,
        command: false,
        shift,
        action,
        active: Platforms::All,
        shown: Platforms::Never,
    };
    let bien = Keymap::new(vec![
        fila(false, AppAction::ColorPrev),
        fila(true, AppAction::BackgroundPrev),
    ]);
    assert_eq!(bien.conflicts(), Vec::<Conflict>::new());
    let mal = Keymap::new(vec![
        fila(true, AppAction::ColorPrev),
        fila(true, AppAction::BackgroundPrev),
    ]);
    assert_eq!(mal.conflicts(), vec![Conflict::Duplicate(0, 1)]);
}

#[test]
fn fila_7_editar_gana_borrar_pagina_y_muestra_k_leida_de_la_tabla() {
    let editar = MENUS
        .iter()
        .find(|menu| menu.title == "Editar")
        .expect("menú Editar");
    let entrada = editar
        .items
        .iter()
        .find(|item| item.action == AppAction::ClearPage)
        .expect("Borrar página");
    assert_eq!(entrada.label, "Borrar página");
    for platform in PLATAFORMAS {
        let atajo = shortcut_for(AppAction::ClearPage, platform).expect("tiene atajo");
        assert_eq!(shortcut_text(atajo, platform), "K");
        // El desplegable cabe: el rótulo más largo más su atajo.
        let ancho = popup_width_u(editar, platform);
        let mas_largo = editar
            .items
            .iter()
            .map(|item| item.label.chars().count())
            .max()
            .unwrap_or(0) as i64;
        assert!(ancho >= (mas_largo + 2 + 1) * i64::from(px_ui::status::ADVANCE_PX));
    }
    // Deshacer y Rehacer siguen donde estaban.
    assert_eq!(editar.items[0].action, AppAction::Undo);
    assert_eq!(editar.items[1].action, AppAction::Redo);
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3: geometría de una sola fuente
// ---------------------------------------------------------------------------------------------

fn ppp_multiplos_de_0_05() -> impl Iterator<Item = f32> {
    (0..=60).map(|i| (100 + 5 * i) as f32 / 100.0)
}

#[test]
fn propiedad_3_minimos_de_ventana_a_los_cinco_ppp_y_criterio_del_72_por_ciento() {
    for (ppp, alto) in PPPS.into_iter().zip([387.0, 309.6, 516.0, 387.0, 387.0]) {
        assert!(
            (min_window_height_pt(ppp) - alto).abs() < 0.01,
            "alto con {ppp}: {}",
            min_window_height_pt(ppp)
        );
    }
    for (ppp, ancho) in PPPS.into_iter().zip([468.0, 374.4, 624.0, 468.0, 468.0]) {
        assert!(
            (min_window_width_pt(ppp) - ancho).abs() < 0.01,
            "ancho con {ppp}"
        );
    }
}

#[test]
fn propiedad_3_el_bloque_de_indexadas_mide_351_n_y_el_alto_minimo_sigue_la_formula() {
    for ppp in ppp_multiplos_de_0_05() {
        let n = i64::from(text_scale(ppp));
        assert_eq!(block_height_px(Mode::Indexed, n as u32), 351 * n);
        let alto = ((20 + 16 + 351) * n) as f32 / ppp;
        assert!((min_window_height_pt(ppp) - alto).abs() < 0.01, "ppp {ppp}");
        assert!(min_window_height_pt(ppp) <= 518.4 + 0.01, "alto con {ppp}");
        assert!(min_window_width_pt(ppp) <= 683.0, "ancho con {ppp}");
    }
}

#[test]
fn propiedad_3_cada_medida_del_indicador_es_un_entero_de_pixeles_multiplo_de_n() {
    for ppp in ppp_multiplos_de_0_05() {
        let n = text_scale(ppp);
        let k = i64::from(n);
        let (fg, bg) = indicator_rects(n);
        let a = selector_rect(Role::Foreground, n);
        let b = selector_rect(Role::Background, n);
        for r in [fg, bg, a, b] {
            for v in [r.x, r.y, r.width, r.height] {
                assert_eq!(v % k, 0, "{r:?} con n = {n}");
            }
        }
    }
}

#[test]
fn propiedad_3_las_aristas_de_la_maqueta_incluyen_las_de_la_banda() {
    let aristas = palette_grid::edges_u();
    for v in [
        0,
        INDICATOR_H_U,
        LINE1_Y_U,
        LINE2_Y_U,
        SEPARATOR_Y_U,
        GRID_Y_U,
        GRID_X_U,
    ] {
        assert!(aristas.contains(&v), "falta {v}");
    }
}
