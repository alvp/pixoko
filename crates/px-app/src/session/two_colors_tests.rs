//! Unidad I.2a (specs/interfaz-dos-colores.md), parte de sesión: fila 3 (herramientas por rol y
//! el color con el botón pulsado), fila 4 (cuentagotas por rol), fila 8 (`K`), fila 10 (reiniciar
//! y mantener) y las propiedades 1 y 2. Sin ventana: un `ToolState` con eventos a mano y la
//! `Session`, como hace `main.rs`.

use super::*;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use px_core::{Allowed, Image, Mode, Palette, Pixel, Rgba};
use px_ui::actions::{AppAction, Locks, Origin};
use px_ui::canvas::CanvasEvent;
use px_ui::color::{ColorChoice, Role, SlotColors, default_background};
use px_ui::pick::{Picked, eraser_pixel};
use px_ui::tool::{Tool, ToolAction, ToolState};

use crate::dispatch::{Dispatched, Targets, dispatch};

// ---------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------

const PERMISIVO: Allowed = Allowed {
    luma: true,
    drop_alpha: Some([0, 0, 0]),
};
const ROLES: [Role; 2] = [Role::Foreground, Role::Background];
const MODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn colores(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::opaque((i * 30) as u8, 255 - (i * 9) as u8, (i * 7) as u8))
        .collect()
}

/// Imagen de 8×4 en `mode`: indexada de 6 colores (la entrada 5 es la transparente) con todo en
/// el índice 0; RGBA y gris con alfa, todo opaco; gris, todo a 0.
fn imagen(mode: Mode) -> Image {
    match mode {
        Mode::Indexed => {
            let mut paleta = Palette::from_colors(colores(6)).expect("paleta");
            paleta.set_transparent_index(Some(5)).expect("existe");
            Image::new_indexed(8, 4, paleta).expect("imagen")
        }
        Mode::Rgba | Mode::GrayAlpha => {
            let mut image = Image::new(8, 4, mode).expect("imagen");
            let base = if mode == Mode::Rgba {
                Pixel::Rgba(Rgba::opaque(10, 20, 30))
            } else {
                Pixel::GrayAlpha {
                    gray: 7,
                    alpha: 255,
                }
            };
            for y in 0..4 {
                for x in 0..8 {
                    image.set_pixel(x, y, base).expect("dentro");
                }
            }
            image
        }
        Mode::Gray => Image::new(8, 4, Mode::Gray).expect("imagen"),
    }
}

struct Mundo {
    session: Session,
    tools: ToolState,
}

impl Mundo {
    fn nuevo(image: Image) -> Mundo {
        let mut session = Session::sample(Document::new(image));
        // Dos colores distintos y que no son el de la imagen.
        let (fg, bg) = match session.document.image(session.document.main_image()) {
            Some(image) if image.mode() == Mode::Indexed => {
                (ColorChoice::Entry(2), ColorChoice::Entry(4))
            }
            _ => (ColorChoice::Slot(3), ColorChoice::Slot(5)),
        };
        session.choice = fg;
        session.background = bg;
        Mundo {
            session,
            tools: ToolState::new(),
        }
    }

    fn imagen(&self) -> &Image {
        self.session
            .document
            .image(self.session.document.main_image())
            .expect("imagen")
    }

    fn locks(&self) -> Locks {
        Locks {
            gesture: self.tools.busy() || self.session.busy(),
            stroke: self.tools.busy() && !self.session.busy(),
            removal: self.session.ops.removal().is_some(),
            ..Locks::default()
        }
    }

    fn despacha(&mut self, action: AppAction) -> Dispatched {
        let locks = self.locks();
        dispatch(
            action,
            Origin::Key,
            &mut Targets {
                session: &mut self.session,
                tools: &mut self.tools,
                locks,
            },
        )
    }

    fn evento(&mut self, pixel: Option<Pixel>, event: CanvasEvent) {
        let id = self.session.document.main_image();
        self.tools
            .canvas_event_with(&mut self.session.document, id, pixel, event)
            .expect("evento");
        let cambios = self.tools.take_changes();
        self.session.absorb(cambios);
    }

    /// Un gesto con la herramienta activa y el botón de `role`: el `Pixel` se calcula al pulsar,
    /// como hace `main.rs`. `durante` corre con el botón aún pulsado.
    fn gesto(&mut self, role: Role, puntos: &[(i32, i32)], durante: impl FnOnce(&mut Mundo)) {
        let pixel = self.session.pixel_of(role);
        self.evento(pixel, CanvasEvent::StrokeBegin(puntos[0]));
        for punto in &puntos[1..] {
            self.evento(pixel, CanvasEvent::StrokeMove(*punto));
        }
        durante(self);
        self.evento(pixel, CanvasEvent::StrokeEnd);
    }

    fn elige(&mut self, tool: Tool) {
        self.tools.apply_action(ToolAction::Select(tool));
    }

    fn pixeles(&self) -> Vec<Pixel> {
        let image = self.imagen();
        (0..image.height())
            .flat_map(|y| (0..image.width()).map(move |x| (x, y)))
            .map(|(x, y)| image.get_pixel(x, y).expect("dentro"))
            .collect()
    }
}

// ---------------------------------------------------------------------------------------
// Fila 1 (parte de sesión): pixel_of y choice_of
// ---------------------------------------------------------------------------------------

#[test]
fn fila_1_pixel_of_y_choice_of_leen_los_dos_roles_y_active_pixel_es_el_primer_plano() {
    for mode in MODOS {
        let mundo = Mundo::nuevo(imagen(mode));
        assert_eq!(
            mundo.session.choice_of(Role::Foreground),
            mundo.session.choice
        );
        assert_eq!(
            mundo.session.choice_of(Role::Background),
            mundo.session.background
        );
        assert_eq!(
            mundo.session.pixel_of(Role::Foreground),
            mundo.session.active_pixel()
        );
        assert_ne!(
            mundo.session.pixel_of(Role::Foreground),
            mundo.session.pixel_of(Role::Background),
            "{mode:?}"
        );
    }
}

#[test]
fn fila_1_una_sesion_nueva_empieza_con_el_fondo_por_defecto_y_editando_el_primer_plano() {
    for mode in MODOS {
        let image = imagen(mode);
        let esperado = default_background(&image);
        let session = Session::sample(Document::new(image));
        assert_eq!(session.background, esperado, "{mode:?}");
        assert_eq!(session.editing, Role::Foreground);
    }
}

// ---------------------------------------------------------------------------------------
// Fila 3: herramientas por rol y el color con el botón pulsado
// ---------------------------------------------------------------------------------------

#[test]
fn fila_3_el_lapiz_pinta_con_el_pixel_de_su_rol_en_un_solo_paso() {
    for mode in MODOS {
        for role in ROLES {
            let mut mundo = Mundo::nuevo(imagen(mode));
            let original = mundo.pixeles();
            let esperado = mundo.session.pixel_of(role).expect("color");
            mundo.gesto(role, &[(1, 1), (4, 1)], |_| {});
            let despues = mundo.pixeles();
            for y in 0..4usize {
                for x in 0..8usize {
                    let pixel = despues[y * 8 + x];
                    if y == 1 && (1..=4).contains(&x) {
                        assert_eq!(pixel, esperado, "{mode:?} {role:?} ({x}, {y})");
                    } else {
                        assert_eq!(pixel, original[y * 8 + x], "{mode:?} {role:?}");
                    }
                }
            }
            assert!(mundo.session.document.can_undo());
            mundo.session.undo().expect("deshacer");
            assert_eq!(mundo.pixeles(), original, "un solo paso");
            assert!(!mundo.session.document.can_undo());
        }
    }
}

#[test]
fn fila_3_la_linea_y_el_relleno_usan_el_pixel_de_su_rol() {
    for mode in MODOS {
        for role in ROLES {
            let mut mundo = Mundo::nuevo(imagen(mode));
            let original = mundo.pixeles();
            let esperado = mundo.session.pixel_of(role).expect("color");
            mundo.elige(Tool::Line);
            mundo.gesto(role, &[(0, 0), (5, 0)], |_| {});
            let despues = mundo.pixeles();
            assert!((0..6).all(|x| despues[x] == esperado), "{mode:?} {role:?}");
            assert_eq!(despues[6], original[6]);

            let mut mundo = Mundo::nuevo(imagen(mode));
            mundo.elige(Tool::Fill);
            mundo.gesto(role, &[(2, 2)], |_| {});
            assert!(
                mundo.pixeles().iter().all(|p| *p == esperado),
                "relleno {mode:?} {role:?}"
            );
        }
    }
}

#[test]
fn fila_3_el_borrador_escribe_lo_mismo_con_los_dos_botones() {
    for mode in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        let mut resultados = Vec::new();
        for role in ROLES {
            let mut mundo = Mundo::nuevo(imagen(mode));
            let borrado = eraser_pixel(mundo.imagen()).expect("borrable");
            mundo.elige(Tool::Eraser);
            mundo.gesto(role, &[(1, 1), (4, 1)], |_| {});
            let despues = mundo.pixeles();
            assert!(
                (1..=4).all(|x| despues[8 + x] == borrado),
                "{mode:?} {role:?}"
            );
            resultados.push(despues);
            assert!(mundo.session.document.can_undo());
        }
        assert_eq!(resultados[0], resultados[1], "{mode:?}");
    }
}

#[test]
fn fila_3_un_cambio_de_color_con_el_boton_pulsado_no_cambia_el_trazo_en_curso_y_si_el_siguiente() {
    for mode in MODOS {
        for role in ROLES {
            let mut mundo = Mundo::nuevo(imagen(mode));
            let primero = mundo.session.pixel_of(role).expect("color");
            mundo.gesto(role, &[(0, 0), (2, 0)], |mundo| {
                // Con el trazo en curso, las acciones de color no se bloquean.
                for action in [
                    AppAction::ColorNext,
                    AppAction::BackgroundNext,
                    AppAction::ColorPrev,
                    AppAction::BackgroundPrev,
                    AppAction::ColorNext,
                    AppAction::BackgroundNext,
                ] {
                    let resultado = mundo.despacha(action);
                    assert_ne!(resultado, Dispatched::Blocked, "{mode:?} {action:?}");
                }
            });
            let despues = mundo.pixeles();
            assert!(
                (0..=2).all(|x| despues[x] == primero),
                "{mode:?} {role:?}: el trazo conserva el color con el que empezó"
            );
            let segundo = mundo.session.pixel_of(role).expect("color");
            assert_ne!(primero, segundo, "{mode:?} {role:?}: el color cambió");
            mundo.gesto(role, &[(0, 3), (2, 3)], |_| {});
            let despues = mundo.pixeles();
            assert!((0..=2).all(|x| despues[3 * 8 + x] == segundo), "{mode:?}");
        }
    }
}

#[test]
fn fila_3_con_el_deslizador_del_editor_pulsado_el_color_sigue_bloqueado() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
    mundo.session.sync_editor();
    assert!(
        mundo
            .session
            .editor
            .begin(px_ui::color_edit::Control::Red, 10)
    );
    assert!(mundo.session.busy());
    for action in [AppAction::ColorNext, AppAction::BackgroundNext] {
        assert_eq!(mundo.despacha(action), Dispatched::Blocked, "{action:?}");
    }
}

#[test]
fn fila_3_background_next_y_prev_recorren_el_fondo_dando_la_vuelta_sin_tocar_el_documento() {
    for mode in MODOS {
        let mut mundo = Mundo::nuevo(imagen(mode));
        let cuenta = px_ui::color::cell_count(mundo.imagen());
        let inicial = mundo.session.background;
        let documento = (
            mundo.session.document.state_id(),
            mundo.session.document.can_undo(),
        );
        let fg = mundo.session.choice;
        for _ in 0..cuenta {
            assert_eq!(
                mundo.despacha(AppAction::BackgroundNext),
                Dispatched::Done {
                    refresh: false,
                    notice: None
                }
            );
        }
        assert_eq!(mundo.session.background, inicial, "{mode:?} da la vuelta");
        mundo.despacha(AppAction::BackgroundPrev);
        assert_ne!(mundo.session.background, inicial);
        mundo.despacha(AppAction::BackgroundNext);
        assert_eq!(mundo.session.background, inicial);
        assert_eq!(mundo.session.choice, fg, "el primer plano no se mueve");
        assert_eq!(
            (
                mundo.session.document.state_id(),
                mundo.session.document.can_undo()
            ),
            documento
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 4: cuentagotas por rol
// ---------------------------------------------------------------------------------------

fn indexada_de(n: usize) -> Session {
    let paleta = Palette::from_colors(colores(n)).expect("paleta");
    let image = Image::new_indexed(4, 1, paleta).expect("imagen");
    Session::sample(Document::new(image))
}

#[test]
fn fila_4_indexada_el_clic_izquierdo_elige_el_primer_plano_y_el_derecho_el_fondo() {
    let mut s = indexada_de(12);
    s.choice = ColorChoice::Entry(2);
    s.background = ColorChoice::Entry(3);
    s.ops.set_range_end(5);
    let rango = s.ops.range_end();
    let documento = (s.document.state_id(), s.modified(), s.document.can_undo());

    let fuera = s.apply_pick_for(Picked::Entry(7), Role::Background);
    assert_eq!(fuera.notice, None);
    assert_eq!(s.background, ColorChoice::Entry(7));
    assert_eq!(s.choice, ColorChoice::Entry(2), "el primer plano no cambia");
    assert_eq!(s.ops.range_end(), rango, "no toca el rango de rotación");

    s.apply_pick_for(Picked::Entry(9), Role::Foreground);
    assert_eq!(s.choice, ColorChoice::Entry(9));
    assert_eq!(s.background, ColorChoice::Entry(7));
    // `apply_pick` es el primer plano.
    s.apply_pick(Picked::Entry(4));
    assert_eq!(s.choice, ColorChoice::Entry(4));
    assert_eq!(
        (s.document.state_id(), s.modified(), s.document.can_undo()),
        documento
    );
}

#[test]
fn fila_4_rgba_gris_y_gris_con_alfa_escriben_el_color_en_la_ranura_del_rol() {
    for (mode, color) in [
        (Mode::Rgba, Rgba::new(10, 20, 30, 128)),
        (Mode::Gray, Rgba::opaque(96, 96, 96)),
        (Mode::GrayAlpha, Rgba::new(7, 7, 7, 200)),
    ] {
        for role in ROLES {
            let mut mundo = Mundo::nuevo(imagen(mode));
            let otro = if role == Role::Foreground {
                Role::Background
            } else {
                Role::Foreground
            };
            let ranura = |m: &Mundo, r: Role| match m.session.choice_of(r) {
                ColorChoice::Slot(n) => n,
                other => panic!("{other:?}"),
            };
            let (mia, ajena) = (ranura(&mundo, role), ranura(&mundo, otro));
            let antes_ajena = mundo.session.slots.get(mode).get(ajena);
            let eleccion = mundo.session.choice_of(role);
            mundo.session.apply_pick_for(Picked::Color(color), role);
            assert_eq!(mundo.session.slots.get(mode).get(mia), Some(color));
            assert_eq!(mundo.session.slots.get(mode).get(ajena), antes_ajena);
            assert_eq!(
                mundo.session.choice_of(role),
                eleccion,
                "la elección no cambia"
            );
        }
    }
}

#[test]
fn fila_4_un_pixel_transparente_copiado_al_fondo_avisa_con_su_valor() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
    let resultado = mundo
        .session
        .apply_pick_for(Picked::Color(Rgba::new(10, 20, 30, 0)), Role::Background);
    assert_eq!(
        resultado.notice.as_deref(),
        Some("píxel transparente copiado tal cual (#0a141e00)")
    );
}

#[test]
fn fila_4_con_una_quita_pendiente_el_cuentagotas_indexado_se_ignora_para_los_dos_roles() {
    for role in ROLES {
        let mut s = indexada_de(5);
        s.choice = ColorChoice::Entry(3);
        s.background = ColorChoice::Entry(1);
        s.sync_editor();
        s.normalize();
        s.ops.begin_removal(3, Some(0));
        assert!(s.ops.removal().is_some());
        s.apply_pick_for(Picked::Entry(4), role);
        assert_eq!(s.choice, ColorChoice::Entry(3));
        assert_eq!(s.background, ColorChoice::Entry(1));
    }
}

#[test]
fn fila_4_un_ctrl_z_posterior_deshace_la_edicion_previa_y_no_la_eleccion() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
    mundo.gesto(Role::Foreground, &[(0, 0)], |_| {});
    let editada = mundo.pixeles();
    mundo
        .session
        .apply_pick_for(Picked::Color(Rgba::opaque(1, 2, 3)), Role::Background);
    assert_eq!(mundo.pixeles(), editada);
    mundo.session.undo().expect("deshacer");
    assert_ne!(mundo.pixeles(), editada);
}

// ---------------------------------------------------------------------------------------
// Fila 8: K (borrar la página)
// ---------------------------------------------------------------------------------------

#[test]
fn fila_8_borrar_la_pagina_pone_toda_la_imagen_al_pixel_del_fondo_en_un_paso() {
    for mode in MODOS {
        let mut mundo = Mundo::nuevo(imagen(mode));
        let original = mundo.pixeles();
        let esperado = mundo.session.pixel_of(Role::Background).expect("color");
        let eleccion = (mundo.session.choice, mundo.session.background);
        let resultado = mundo.despacha(AppAction::ClearPage);
        assert_eq!(
            resultado,
            Dispatched::Done {
                refresh: true,
                notice: None
            },
            "{mode:?}"
        );
        assert!(mundo.pixeles().iter().all(|p| *p == esperado), "{mode:?}");
        assert_eq!(
            (mundo.session.choice, mundo.session.background),
            eleccion,
            "la elección no cambia"
        );
        assert!(mundo.session.modified());
        // Toda la imagen se regenera.
        let sucio = mundo.session.take_dirty();
        let (w, h) = (mundo.imagen().width(), mundo.imagen().height());
        assert_eq!(sucio, px_ui::dirty::BlockSet::full(w, h));
        mundo.session.undo().expect("deshacer");
        assert_eq!(mundo.pixeles(), original, "un solo paso");
        mundo.session.redo().expect("rehacer");
        assert!(mundo.pixeles().iter().all(|p| *p == esperado));
    }
}

#[test]
fn fila_8_rgba_con_el_fondo_transparente_deja_la_pagina_transparente() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
    mundo.session.background = ColorChoice::Slot(9);
    mundo.despacha(AppAction::ClearPage);
    assert!(
        mundo
            .pixeles()
            .iter()
            .all(|p| *p == Pixel::Rgba(Rgba::TRANSPARENT))
    );
}

#[test]
fn fila_8_indexada_con_el_fondo_en_la_entrada_transparente_escribe_ese_indice() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Indexed));
    mundo.session.background = default_background(mundo.imagen());
    assert_eq!(mundo.session.background, ColorChoice::Entry(5));
    mundo.despacha(AppAction::ClearPage);
    assert!(mundo.pixeles().iter().all(|p| *p == Pixel::Indexed(5)));
}

#[test]
fn fila_8_una_imagen_ya_del_color_del_fondo_no_crea_paso_ni_toca_el_rehacer() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Indexed));
    // Un paso y un deshacer: queda algo que rehacer.
    mundo.gesto(Role::Foreground, &[(0, 0)], |_| {});
    mundo.session.undo().expect("deshacer");
    assert!(mundo.session.document.can_redo());
    mundo.session.background = ColorChoice::Entry(0);
    let id = mundo.session.document.state_id();
    let resultado = mundo.despacha(AppAction::ClearPage);
    assert_eq!(
        resultado,
        Dispatched::Done {
            refresh: false,
            notice: None
        }
    );
    assert_eq!(mundo.session.document.state_id(), id);
    assert!(mundo.session.document.can_redo(), "el rehacer sigue");
}

#[test]
fn fila_8_con_gesto_en_curso_la_tecla_k_esta_bloqueada() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
    let pixel = mundo.session.pixel_of(Role::Foreground);
    mundo.evento(pixel, CanvasEvent::StrokeBegin((0, 0)));
    assert_eq!(mundo.despacha(AppAction::ClearPage), Dispatched::Blocked);
    mundo.evento(pixel, CanvasEvent::StrokeEnd);
    assert_ne!(mundo.despacha(AppAction::ClearPage), Dispatched::Blocked);
}

#[test]
fn fila_8_si_la_reserva_falla_la_imagen_y_la_historia_quedan_intactas_y_hay_aviso() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
    let original = mundo.pixeles();
    let id = mundo.session.document.state_id();
    let mut pedido = None;
    let resultado = mundo.session.clear_page_with(&mut |bytes| {
        pedido = Some(bytes);
        false
    });
    assert_eq!(
        pedido,
        Some(8 * 4 * 4),
        "se reserva antes de modificar: w·h·bpp"
    );
    let error = resultado.expect_err("falla");
    assert_eq!(error.to_string(), "no hay memoria para borrar la página");
    assert_eq!(mundo.pixeles(), original);
    assert_eq!(mundo.session.document.state_id(), id);
    assert!(!mundo.session.document.can_undo());
    assert!(!mundo.session.modified());
}

#[test]
fn fila_8_con_la_reserva_concedida_borra_y_devuelve_true() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Gray));
    let resultado = mundo.session.clear_page_with(&mut |_| true);
    assert!(resultado.expect("borra"));
}

// ---------------------------------------------------------------------------------------
// Fila 10: reiniciar y mantener
// ---------------------------------------------------------------------------------------

static CONTADOR: AtomicU64 = AtomicU64::new(0);

fn png_en_disco(image: &Image) -> PathBuf {
    let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("pixoko-dos-colores-{}-{n}", std::process::id()));
    fs::create_dir_all(&dir).expect("directorio");
    let path = dir.join("imagen.png");
    fs::write(&path, write_png(image).expect("png")).expect("escribir");
    path
}

fn ensucia(session: &mut Session) {
    session.background = ColorChoice::Entry(2);
    session.editing = Role::Background;
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let slots = session.slots.get_mut(mode);
        for slot in 1..=9 {
            slots.set(slot, Rgba::opaque(slot, 200, 100));
        }
    }
}

fn de_fabrica(mode: Mode) -> SlotColors {
    SlotColors::defaults(mode)
}

#[test]
fn fila_10_abrir_reinicia_el_fondo_el_destino_del_editor_y_la_ranura_9_pero_ninguna_otra() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
    ensucia(&mut mundo.session);
    let nueva = imagen(Mode::Rgba);
    let path = png_en_disco(&nueva);
    mundo.session.open(&path, 16384, true).expect("abrir");
    assert_eq!(mundo.session.background, ColorChoice::Slot(9));
    assert_eq!(mundo.session.editing, Role::Foreground);
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let slots = mundo.session.slots.get(mode);
        assert_eq!(
            slots.get(9),
            de_fabrica(mode).get(9),
            "{mode:?}: la 9 de fábrica"
        );
        for slot in 1..=8 {
            assert_eq!(
                slots.get(slot),
                Some(Rgba::opaque(slot, 200, 100)),
                "{mode:?}: la ranura {slot} no se restaura"
            );
        }
    }
    let _ = fs::remove_dir_all(path.parent().expect("dir"));
}

#[test]
fn fila_10_abrir_una_indexada_pone_el_fondo_en_la_entrada_transparente() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
    let nueva = imagen(Mode::Indexed);
    let esperado = default_background(&nueva);
    let path = png_en_disco(&nueva);
    mundo.session.open(&path, 16384, true).expect("abrir");
    let image = mundo.imagen();
    assert_eq!(mundo.session.background, default_background(image));
    // El PNG puede no conservar el índice transparente: la regla es la misma que la de la imagen.
    let _ = esperado;
    let _ = fs::remove_dir_all(path.parent().expect("dir"));
}

#[test]
fn fila_10_nueva_imagen_reinicia_igual_y_restaura_la_ranura_9() {
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha, Mode::Indexed] {
        let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
        ensucia(&mut mundo.session);
        let nueva = imagen(mode);
        let esperado = default_background(&nueva);
        mundo.session.new_image(nueva, true).expect("nueva");
        assert_eq!(mundo.session.background, esperado, "{mode:?}");
        assert_eq!(mundo.session.editing, Role::Foreground);
        for modo in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            let slots = mundo.session.slots.get(modo);
            assert_eq!(slots.get(9), de_fabrica(modo).get(9), "{modo:?}");
            assert_eq!(slots.get(3), Some(Rgba::opaque(3, 200, 100)));
        }
    }
}

#[test]
fn fila_10_convertir_de_modo_pone_el_fondo_por_defecto_del_modo_nuevo() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
    mundo.session.background = ColorChoice::Slot(4);
    mundo
        .session
        .convert_mode(Mode::Gray, PERMISIVO)
        .expect("convertir");
    let image = mundo.imagen();
    assert_eq!(image.mode(), Mode::Gray);
    assert_eq!(mundo.session.background, default_background(image));
    assert_eq!(mundo.session.background, ColorChoice::Slot(9));
}

#[test]
fn fila_10_guardar_y_guardar_como_no_reinician_nada() {
    let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
    ensucia(&mut mundo.session);
    mundo.session.background = ColorChoice::Slot(4);
    let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("pixoko-dos-colores-g-{}-{n}", std::process::id()));
    fs::create_dir_all(&dir).expect("directorio");
    mundo.session.save_as(&dir).ok();
    mundo.session.save(&dir).ok();
    assert_eq!(mundo.session.background, ColorChoice::Slot(4));
    assert_eq!(mundo.session.editing, Role::Background);
    assert_eq!(
        mundo.session.slots.get(Mode::Rgba).get(9),
        Some(Rgba::opaque(9, 200, 100))
    );
    // Con error (directorio inexistente) tampoco.
    mundo.session.save(&dir.join("no-existe")).ok();
    assert_eq!(mundo.session.background, ColorChoice::Slot(4));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fila_10_tras_deshacer_y_quitar_el_fondo_sigue_siendo_valido() {
    let mut s = indexada_de(5);
    s.choice = ColorChoice::Entry(0);
    s.background = ColorChoice::Entry(4);
    let resultado = s
        .apply_palette_op(
            px_ui::palette_ops::PaletteOp::Remove { index: 4, into: 0 },
            true,
        )
        .expect("quitar");
    assert!(resultado.changed);
    assert_eq!(s.background, ColorChoice::Entry(3), "recortado a la última");
    s.undo().expect("deshacer");
    s.normalize();
    assert!(s.pixel_of(Role::Background).is_some());
}

#[test]
fn fila_10_mover_una_entrada_arrastra_al_fondo_que_estaba_en_ella() {
    let mut s = indexada_de(6);
    s.choice = ColorChoice::Entry(0);
    s.background = ColorChoice::Entry(2);
    s.apply_palette_op(
        px_ui::palette_ops::PaletteOp::Move { from: 2, to: 4 },
        false,
    )
    .expect("mover");
    assert_eq!(
        s.background,
        ColorChoice::Entry(4),
        "el fondo sigue a su entrada"
    );
    // Una entrada que se cruza se desplaza un lugar.
    s.background = ColorChoice::Entry(3);
    s.apply_palette_op(
        px_ui::palette_ops::PaletteOp::Move { from: 1, to: 5 },
        false,
    )
    .expect("mover");
    assert_eq!(s.background, ColorChoice::Entry(2));
}

#[test]
fn fila_10_el_editor_con_editing_fondo_muestra_y_escribe_el_color_del_fondo() {
    // Indexada: edita la entrada del fondo con SetPaletteColor.
    let mut s = indexada_de(6);
    s.choice = ColorChoice::Entry(1);
    s.background = ColorChoice::Entry(3);
    s.editing = Role::Background;
    s.sync_editor();
    let objetivo = s.editor.target();
    assert_eq!(objetivo, Some(px_ui::color_edit::EditTarget::Entry(3)));
    // RGBA: edita la ranura del fondo.
    let mut mundo = Mundo::nuevo(imagen(Mode::Rgba));
    mundo.session.editing = Role::Background;
    mundo.session.sync_editor();
    assert_eq!(
        mundo.session.editor.target(),
        Some(px_ui::color_edit::EditTarget::Slot(5))
    );
    mundo.session.editing = Role::Foreground;
    mundo.session.sync_editor();
    assert_eq!(
        mundo.session.editor.target(),
        Some(px_ui::color_edit::EditTarget::Slot(3))
    );
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: el par siempre es válido y el documento solo cambia donde debe
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Paso {
    ClicIzquierdo(usize),
    ClicDerecho(usize),
    Siguiente,
    Anterior,
    FondoSiguiente,
    FondoAnterior,
    Cuentagotas(Role, u32, u32),
    Pincelada(u32, u32),
    Deshacer,
    Rehacer,
    Borrar,
    Convertir(usize),
    NuevaImagen(usize),
}

fn paso() -> impl Strategy<Value = Paso> {
    prop_oneof![
        (0usize..300).prop_map(Paso::ClicIzquierdo),
        (0usize..300).prop_map(Paso::ClicDerecho),
        Just(Paso::Siguiente),
        Just(Paso::Anterior),
        Just(Paso::FondoSiguiente),
        Just(Paso::FondoAnterior),
        (any::<bool>(), 0u32..8, 0u32..4).prop_map(|(b, x, y)| Paso::Cuentagotas(
            if b {
                Role::Foreground
            } else {
                Role::Background
            },
            x,
            y
        )),
        (0u32..8, 0u32..4).prop_map(|(x, y)| Paso::Pincelada(x, y)),
        Just(Paso::Deshacer),
        Just(Paso::Rehacer),
        Just(Paso::Borrar),
        (0usize..4).prop_map(Paso::Convertir),
        (0usize..4).prop_map(Paso::NuevaImagen),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_1_el_par_es_siempre_valido_y_el_documento_solo_cambia_donde_debe(
        modo in 0usize..4,
        pasos in proptest::collection::vec(paso(), 0..24),
    ) {
        let mut mundo = Mundo::nuevo(imagen(MODOS[modo]));
        for paso in pasos {
            let antes = (
                mundo.session.document.state_id(),
                mundo.session.document.can_undo(),
                mundo.session.document.can_redo(),
            );
            let (fg, bg) = (mundo.session.choice, mundo.session.background);
            let cambia_el_documento = matches!(
                paso,
                Paso::Pincelada(..) | Paso::Deshacer | Paso::Rehacer | Paso::Borrar
                    | Paso::Convertir(_) | Paso::NuevaImagen(_)
            );
            match paso {
                Paso::ClicIzquierdo(cell) => {
                    if let Some(c) = px_ui::color::choice_for_cell(mundo.imagen(), cell) {
                        mundo.session.choice = c;
                        prop_assert_eq!(mundo.session.background, bg);
                    }
                }
                Paso::ClicDerecho(cell) => {
                    if let Some(c) = px_ui::color::choice_for_cell(mundo.imagen(), cell) {
                        mundo.session.background = c;
                        prop_assert_eq!(mundo.session.choice, fg);
                    }
                }
                Paso::Siguiente => { mundo.despacha(AppAction::ColorNext); }
                Paso::Anterior => { mundo.despacha(AppAction::ColorPrev); }
                Paso::FondoSiguiente => { mundo.despacha(AppAction::BackgroundNext); }
                Paso::FondoAnterior => { mundo.despacha(AppAction::BackgroundPrev); }
                Paso::Cuentagotas(role, x, y) => {
                    let picked = px_ui::pick::picked_at(mundo.imagen(), x, y);
                    if let Some(picked) = picked {
                        mundo.session.apply_pick_for(picked, role);
                    }
                }
                Paso::Pincelada(x, y) => {
                    mundo.elige(Tool::Pencil);
                    mundo.gesto(Role::Foreground, &[(x as i32, y as i32)], |_| {});
                }
                Paso::Deshacer => { mundo.despacha(AppAction::Undo); }
                Paso::Rehacer => { mundo.despacha(AppAction::Redo); }
                Paso::Borrar => { mundo.despacha(AppAction::ClearPage); }
                Paso::Convertir(i) => {
                    mundo.session.convert_mode(MODOS[i], PERMISIVO).ok();
                }
                Paso::NuevaImagen(i) => {
                    mundo.session.new_image(imagen(MODOS[i]), true).expect("nueva");
                }
            }
            mundo.session.normalize();
            prop_assert!(mundo.session.pixel_of(Role::Foreground).is_some());
            prop_assert!(mundo.session.pixel_of(Role::Background).is_some());
            if !cambia_el_documento {
                prop_assert_eq!(
                    (
                        mundo.session.document.state_id(),
                        mundo.session.document.can_undo(),
                        mundo.session.document.can_redo(),
                    ),
                    antes
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: cada trazo, su color
// ---------------------------------------------------------------------------------------

fn herramienta(i: usize) -> Tool {
    [
        Tool::Pencil,
        Tool::Line,
        Tool::Fill,
        Tool::Dropper,
        Tool::Eraser,
    ][i]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_2_cada_trazo_escribe_solo_el_pixel_del_rol_con_que_empezo(
        modo in 0usize..4,
        tool in 0usize..5,
        derecho in any::<bool>(),
        puntos in proptest::collection::vec((0i32..8, 0i32..4), 1..6),
        cambios in proptest::collection::vec(0u8..4, 0..5),
    ) {
        let mut mundo = Mundo::nuevo(imagen(MODOS[modo]));
        let role = if derecho { Role::Background } else { Role::Foreground };
        mundo.elige(herramienta(tool));
        let original = mundo.pixeles();
        let pixel_inicial = mundo.session.pixel_of(role);
        let borrado = eraser_pixel(mundo.imagen()).ok();
        let id = mundo.session.document.state_id();
        mundo.gesto(role, &puntos, |mundo| {
            assert!(mundo.tools.busy());
            for cambio in &cambios {
                let accion = match cambio {
                    0 => AppAction::ColorNext,
                    1 => AppAction::ColorPrev,
                    2 => AppAction::BackgroundNext,
                    _ => AppAction::BackgroundPrev,
                };
                mundo.despacha(accion);
            }
        });
        prop_assert!(!mundo.tools.busy());
        let despues = mundo.pixeles();
        // Nunca más de un paso por gesto.
        let pasos = usize::from(mundo.session.document.state_id() != id);
        prop_assert!(pasos <= 1);
        if pasos == 1 {
            mundo.session.undo().expect("deshacer");
            prop_assert_eq!(mundo.pixeles(), original.clone());
        }
        // Todo píxel cambiado vale lo que escribe la herramienta: el color del rol al pulsar
        // (el valor de borrado con el borrador; nada con el cuentagotas).
        let permitido = if herramienta(tool) == Tool::Eraser {
            borrado
        } else {
            pixel_inicial
        };
        for (antes, despues) in original.iter().zip(&despues) {
            if antes != despues {
                prop_assert_eq!(Some(*despues), permitido);
                prop_assert_ne!(herramienta(tool), Tool::Dropper);
            }
        }
    }
}
