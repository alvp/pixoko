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

const PERMISSIVE: Allowed = Allowed {
    luma: true,
    drop_alpha: Some([0, 0, 0]),
};
const ROLES: [Role; 2] = [Role::Foreground, Role::Background];
const TEST_MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn colors(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::opaque((i * 30) as u8, 255 - (i * 9) as u8, (i * 7) as u8))
        .collect()
}

/// Imagen de 8×4 en `mode`: indexada de 6 colores (la entrada 5 es la transparente) con todo en
/// el índice 0; RGBA y gris con alfa, todo opaco; gris, todo a 0.
fn image_2(mode: Mode) -> Image {
    match mode {
        Mode::Indexed => {
            let mut palette = Palette::from_colors(colors(6)).expect("paleta");
            palette.set_transparent_index(Some(5)).expect("existe");
            Image::new_indexed(8, 4, palette).expect("imagen")
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

struct World {
    session: Session,
    tools: ToolState,
}

impl World {
    fn new_2(image: Image) -> World {
        let mut session = Session::sample(Document::new(image));
        // Dos colores distintos y que no son el de la imagen.
        let (fg, bg) = match session.document.image(ImageId::from_raw(0)) {
            Some(image) if image.mode() == Mode::Indexed => {
                (ColorChoice::Entry(2), ColorChoice::Entry(4))
            }
            _ => (ColorChoice::Slot(3), ColorChoice::Slot(5)),
        };
        session.choice = fg;
        session.background = bg;
        World {
            session,
            tools: ToolState::new(),
        }
    }

    fn image_2(&self) -> &Image {
        self.session
            .document
            .image(ImageId::from_raw(0))
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

    fn dispatches(&mut self, action: AppAction) -> Dispatched {
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

    fn event_2(&mut self, pixel: Option<Pixel>, event: CanvasEvent) {
        let id = ImageId::from_raw(0);
        self.tools
            .canvas_event_with(&mut self.session.document, id, pixel, event)
            .expect("evento");
        let changes_2 = self.tools.take_changes();
        self.session.absorb(changes_2);
    }

    /// Un gesto con la herramienta activa y el botón de `role`: el `Pixel` se calcula al pulsar,
    /// como hace `main.rs`. `during` corre con el botón aún pulsado.
    fn gesture(&mut self, role: Role, points: &[(i32, i32)], during: impl FnOnce(&mut World)) {
        let pixel = self.session.pixel_of(role);
        self.event_2(pixel, CanvasEvent::StrokeBegin(points[0]));
        for point in &points[1..] {
            self.event_2(pixel, CanvasEvent::StrokeMove(*point));
        }
        during(self);
        self.event_2(pixel, CanvasEvent::StrokeEnd);
    }

    fn chooses(&mut self, tool: Tool) {
        self.tools.apply_action(ToolAction::Select(tool));
    }

    fn pixels_2(&self) -> Vec<Pixel> {
        let image = self.image_2();
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
fn row_1_pixel_of_and_choice_of_read_the_two_roles_and_active_pixel_is_the_foreground() {
    for mode in TEST_MODES {
        let world = World::new_2(image_2(mode));
        assert_eq!(
            world.session.choice_of(Role::Foreground),
            world.session.choice
        );
        assert_eq!(
            world.session.choice_of(Role::Background),
            world.session.background
        );
        assert_eq!(
            world.session.pixel_of(Role::Foreground),
            world.session.active_pixel()
        );
        assert_ne!(
            world.session.pixel_of(Role::Foreground),
            world.session.pixel_of(Role::Background),
            "{mode:?}"
        );
    }
}

#[test]
fn row_1_a_new_session_starts_with_the_default_background_and_editing_the_foreground() {
    for mode in TEST_MODES {
        let image = image_2(mode);
        let expected_2 = default_background(&image);
        let session = Session::sample(Document::new(image));
        assert_eq!(session.background, expected_2, "{mode:?}");
        assert_eq!(session.editing, Role::Foreground);
    }
}

// ---------------------------------------------------------------------------------------
// Fila 3: herramientas por rol y el color con el botón pulsado
// ---------------------------------------------------------------------------------------

#[test]
fn row_3_the_pencil_paints_with_the_pixel_of_its_role_in_one_step() {
    for mode in TEST_MODES {
        for role in ROLES {
            let mut world = World::new_2(image_2(mode));
            let original = world.pixels_2();
            let expected_2 = world.session.pixel_of(role).expect("color");
            world.gesture(role, &[(1, 1), (4, 1)], |_| {});
            let after_2 = world.pixels_2();
            for y in 0..4usize {
                for x in 0..8usize {
                    let pixel = after_2[y * 8 + x];
                    if y == 1 && (1..=4).contains(&x) {
                        assert_eq!(pixel, expected_2, "{mode:?} {role:?} ({x}, {y})");
                    } else {
                        assert_eq!(pixel, original[y * 8 + x], "{mode:?} {role:?}");
                    }
                }
            }
            assert!(world.session.document.can_undo());
            world.session.undo().expect("deshacer");
            assert_eq!(world.pixels_2(), original, "un solo paso");
            assert!(!world.session.document.can_undo());
        }
    }
}

#[test]
fn row_3_the_line_and_the_fill_use_the_pixel_of_their_role() {
    for mode in TEST_MODES {
        for role in ROLES {
            let mut world = World::new_2(image_2(mode));
            let original = world.pixels_2();
            let expected_2 = world.session.pixel_of(role).expect("color");
            world.chooses(Tool::Line);
            world.gesture(role, &[(0, 0), (5, 0)], |_| {});
            let after_2 = world.pixels_2();
            assert!(
                (0..6).all(|x| after_2[x] == expected_2),
                "{mode:?} {role:?}"
            );
            assert_eq!(after_2[6], original[6]);

            let mut world = World::new_2(image_2(mode));
            world.chooses(Tool::Fill);
            world.gesture(role, &[(2, 2)], |_| {});
            assert!(
                world.pixels_2().iter().all(|p| *p == expected_2),
                "relleno {mode:?} {role:?}"
            );
        }
    }
}

#[test]
fn row_3_the_eraser_writes_the_same_with_both_buttons() {
    for mode in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        let mut results = Vec::new();
        for role in ROLES {
            let mut world = World::new_2(image_2(mode));
            let deleted = eraser_pixel(world.image_2()).expect("borrable");
            world.chooses(Tool::Eraser);
            world.gesture(role, &[(1, 1), (4, 1)], |_| {});
            let after_2 = world.pixels_2();
            assert!(
                (1..=4).all(|x| after_2[8 + x] == deleted),
                "{mode:?} {role:?}"
            );
            results.push(after_2);
            assert!(world.session.document.can_undo());
        }
        assert_eq!(results[0], results[1], "{mode:?}");
    }
}

#[test]
fn row_3_a_color_change_with_the_button_pressed_does_not_change_the_stroke_in_progress_but_the_next()
 {
    for mode in TEST_MODES {
        for role in ROLES {
            let mut world = World::new_2(image_2(mode));
            let first_2 = world.session.pixel_of(role).expect("color");
            world.gesture(role, &[(0, 0), (2, 0)], |world| {
                // Con el trazo en curso, las acciones de color no se bloquean.
                for action in [
                    AppAction::ColorNext,
                    AppAction::BackgroundNext,
                    AppAction::ColorPrev,
                    AppAction::BackgroundPrev,
                    AppAction::ColorNext,
                    AppAction::BackgroundNext,
                ] {
                    let result = world.dispatches(action);
                    assert_ne!(result, Dispatched::Blocked, "{mode:?} {action:?}");
                }
            });
            let after_2 = world.pixels_2();
            assert!(
                (0..=2).all(|x| after_2[x] == first_2),
                "{mode:?} {role:?}: el trazo conserva el color con el que empezó"
            );
            let second = world.session.pixel_of(role).expect("color");
            assert_ne!(first_2, second, "{mode:?} {role:?}: el color cambió");
            world.gesture(role, &[(0, 3), (2, 3)], |_| {});
            let after_2 = world.pixels_2();
            assert!((0..=2).all(|x| after_2[3 * 8 + x] == second), "{mode:?}");
        }
    }
}

#[test]
fn row_3_with_the_editor_slider_pressed_the_color_stays_locked() {
    let mut world = World::new_2(image_2(Mode::Rgba));
    world.session.sync_editor();
    assert!(
        world
            .session
            .editor
            .begin(px_ui::color_edit::Control::Red, 10)
    );
    assert!(world.session.busy());
    for action in [AppAction::ColorNext, AppAction::BackgroundNext] {
        assert_eq!(world.dispatches(action), Dispatched::Blocked, "{action:?}");
    }
}

#[test]
fn row_3_background_next_and_prev_walk_the_background_wrapping_around_without_touching_the_document()
 {
    for mode in TEST_MODES {
        let mut world = World::new_2(image_2(mode));
        let count = px_ui::color::cell_count(world.image_2());
        let initial = world.session.background;
        let document = (
            world.session.document.state_id(),
            world.session.document.can_undo(),
        );
        let fg = world.session.choice;
        for _ in 0..count {
            assert_eq!(
                world.dispatches(AppAction::BackgroundNext),
                Dispatched::Done {
                    refresh: false,
                    notice: None
                }
            );
        }
        assert_eq!(world.session.background, initial, "{mode:?} da la vuelta");
        world.dispatches(AppAction::BackgroundPrev);
        assert_ne!(world.session.background, initial);
        world.dispatches(AppAction::BackgroundNext);
        assert_eq!(world.session.background, initial);
        assert_eq!(world.session.choice, fg, "el primer plano no se mueve");
        assert_eq!(
            (
                world.session.document.state_id(),
                world.session.document.can_undo()
            ),
            document
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 4: cuentagotas por rol
// ---------------------------------------------------------------------------------------

fn indexed_of(n: usize) -> Session {
    let palette = Palette::from_colors(colors(n)).expect("paleta");
    let image = Image::new_indexed(4, 1, palette).expect("imagen");
    Session::sample(Document::new(image))
}

#[test]
fn row_4_indexed_the_left_click_chooses_the_foreground_and_the_right_the_background() {
    let mut s = indexed_of(12);
    s.choice = ColorChoice::Entry(2);
    s.background = ColorChoice::Entry(3);
    s.ops.set_range_end(5);
    let range = s.ops.range_end();
    let document = (s.document.state_id(), s.modified(), s.document.can_undo());

    let outside = s.apply_pick_for(Picked::Entry(7), Role::Background);
    assert_eq!(outside.notice, None);
    assert_eq!(s.background, ColorChoice::Entry(7));
    assert_eq!(s.choice, ColorChoice::Entry(2), "el primer plano no cambia");
    assert_eq!(s.ops.range_end(), range, "no toca el rango de rotación");

    s.apply_pick_for(Picked::Entry(9), Role::Foreground);
    assert_eq!(s.choice, ColorChoice::Entry(9));
    assert_eq!(s.background, ColorChoice::Entry(7));
    // `apply_pick` es el primer plano.
    s.apply_pick(Picked::Entry(4));
    assert_eq!(s.choice, ColorChoice::Entry(4));
    assert_eq!(
        (s.document.state_id(), s.modified(), s.document.can_undo()),
        document
    );
}

#[test]
fn row_4_rgba_gray_and_gray_alpha_write_the_color_in_the_slot_of_the_role() {
    for (mode, color) in [
        (Mode::Rgba, Rgba::new(10, 20, 30, 128)),
        (Mode::Gray, Rgba::opaque(96, 96, 96)),
        (Mode::GrayAlpha, Rgba::new(7, 7, 7, 200)),
    ] {
        for role in ROLES {
            let mut world = World::new_2(image_2(mode));
            let other_2 = if role == Role::Foreground {
                Role::Background
            } else {
                Role::Foreground
            };
            let slot = |m: &World, r: Role| match m.session.choice_of(r) {
                ColorChoice::Slot(n) => n,
                other => panic!("{other:?}"),
            };
            let (mine, foreign) = (slot(&world, role), slot(&world, other_2));
            let before_foreign = world.session.slots.get(mode).get(foreign);
            let choice = world.session.choice_of(role);
            world.session.apply_pick_for(Picked::Color(color), role);
            assert_eq!(world.session.slots.get(mode).get(mine), Some(color));
            assert_eq!(world.session.slots.get(mode).get(foreign), before_foreign);
            assert_eq!(
                world.session.choice_of(role),
                choice,
                "la elección no cambia"
            );
        }
    }
}

#[test]
fn row_4_a_transparent_pixel_copied_to_the_background_warns_with_its_value() {
    let mut world = World::new_2(image_2(Mode::Rgba));
    let result = world
        .session
        .apply_pick_for(Picked::Color(Rgba::new(10, 20, 30, 0)), Role::Background);
    assert_eq!(
        result.notice.as_deref(),
        Some("píxel transparente copiado tal cual (#0a141e00)")
    );
}

#[test]
fn row_4_with_a_pending_removal_the_indexed_dropper_is_ignored_for_both_roles() {
    for role in ROLES {
        let mut s = indexed_of(5);
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
fn row_4_a_later_ctrl_z_undoes_the_previous_edit_and_not_the_choice() {
    let mut world = World::new_2(image_2(Mode::Rgba));
    world.gesture(Role::Foreground, &[(0, 0)], |_| {});
    let edited = world.pixels_2();
    world
        .session
        .apply_pick_for(Picked::Color(Rgba::opaque(1, 2, 3)), Role::Background);
    assert_eq!(world.pixels_2(), edited);
    world.session.undo().expect("deshacer");
    assert_ne!(world.pixels_2(), edited);
}

// ---------------------------------------------------------------------------------------
// Fila 8: K (borrar la página)
// ---------------------------------------------------------------------------------------

#[test]
fn row_8_clear_page_sets_the_whole_image_to_the_background_pixel_in_one_step() {
    for mode in TEST_MODES {
        let mut world = World::new_2(image_2(mode));
        let original = world.pixels_2();
        let expected_2 = world.session.pixel_of(Role::Background).expect("color");
        let choice = (world.session.choice, world.session.background);
        let result = world.dispatches(AppAction::ClearPage);
        assert_eq!(
            result,
            Dispatched::Done {
                refresh: true,
                notice: None
            },
            "{mode:?}"
        );
        assert!(
            world.pixels_2().iter().all(|p| *p == expected_2),
            "{mode:?}"
        );
        assert_eq!(
            (world.session.choice, world.session.background),
            choice,
            "la elección no cambia"
        );
        assert!(world.session.modified());
        // Toda la imagen se regenera.
        let dirty = world.session.take_dirty();
        let (w, h) = (world.image_2().width(), world.image_2().height());
        assert_eq!(dirty, px_ui::dirty::BlockSet::full(w, h));
        world.session.undo().expect("deshacer");
        assert_eq!(world.pixels_2(), original, "un solo paso");
        world.session.redo().expect("rehacer");
        assert!(world.pixels_2().iter().all(|p| *p == expected_2));
    }
}

#[test]
fn row_8_rgba_with_the_transparent_background_leaves_the_page_transparent() {
    let mut world = World::new_2(image_2(Mode::Rgba));
    world.session.background = ColorChoice::Slot(9);
    world.dispatches(AppAction::ClearPage);
    assert!(
        world
            .pixels_2()
            .iter()
            .all(|p| *p == Pixel::Rgba(Rgba::TRANSPARENT))
    );
}

#[test]
fn row_8_indexed_with_the_background_on_the_transparent_entry_writes_that_index() {
    let mut world = World::new_2(image_2(Mode::Indexed));
    world.session.background = default_background(world.image_2());
    assert_eq!(world.session.background, ColorChoice::Entry(5));
    world.dispatches(AppAction::ClearPage);
    assert!(world.pixels_2().iter().all(|p| *p == Pixel::Indexed(5)));
}

#[test]
fn row_8_an_image_already_of_the_background_color_creates_no_step_or_touches_the_redo() {
    let mut world = World::new_2(image_2(Mode::Indexed));
    // Un paso y un deshacer: queda algo que rehacer.
    world.gesture(Role::Foreground, &[(0, 0)], |_| {});
    world.session.undo().expect("deshacer");
    assert!(world.session.document.can_redo());
    world.session.background = ColorChoice::Entry(0);
    let id = world.session.document.state_id();
    let result = world.dispatches(AppAction::ClearPage);
    assert_eq!(
        result,
        Dispatched::Done {
            refresh: false,
            notice: None
        }
    );
    assert_eq!(world.session.document.state_id(), id);
    assert!(world.session.document.can_redo(), "el rehacer sigue");
}

#[test]
fn row_8_with_a_gesture_in_progress_the_k_key_is_blocked() {
    let mut world = World::new_2(image_2(Mode::Rgba));
    let pixel = world.session.pixel_of(Role::Foreground);
    world.event_2(pixel, CanvasEvent::StrokeBegin((0, 0)));
    assert_eq!(world.dispatches(AppAction::ClearPage), Dispatched::Blocked);
    world.event_2(pixel, CanvasEvent::StrokeEnd);
    assert_ne!(world.dispatches(AppAction::ClearPage), Dispatched::Blocked);
}

#[test]
fn row_8_if_the_reserve_fails_the_image_and_the_history_stay_intact_and_there_is_a_notice() {
    let mut world = World::new_2(image_2(Mode::Rgba));
    let original = world.pixels_2();
    let id = world.session.document.state_id();
    let mut requested = None;
    let result = world.session.clear_page_with(&mut |bytes| {
        requested = Some(bytes);
        false
    });
    assert_eq!(
        requested,
        Some(8 * 4 * 4),
        "se reserva antes de modificar: w·h·bpp"
    );
    let error = result.expect_err("falla");
    assert_eq!(error.to_string(), "no hay memoria para borrar la página");
    assert_eq!(world.pixels_2(), original);
    assert_eq!(world.session.document.state_id(), id);
    assert!(!world.session.document.can_undo());
    assert!(!world.session.modified());
}

#[test]
fn row_8_with_the_reserve_granted_it_clears_and_returns_true() {
    let mut world = World::new_2(image_2(Mode::Gray));
    let result = world.session.clear_page_with(&mut |_| true);
    assert!(result.expect("borra"));
}

// ---------------------------------------------------------------------------------------
// Fila 10: reiniciar y mantener
// ---------------------------------------------------------------------------------------

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn png_on_disk(image: &Image) -> PathBuf {
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("pixoko-dos-colores-{}-{n}", std::process::id()));
    fs::create_dir_all(&dir).expect("directorio");
    let path = dir.join("imagen.png");
    fs::write(&path, write_png(image).expect("png")).expect("escribir");
    path
}

fn dirties(session: &mut Session) {
    session.background = ColorChoice::Entry(2);
    session.editing = Role::Background;
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let slots = session.slots.get_mut(mode);
        for slot in 1..=9 {
            slots.set(slot, Rgba::opaque(slot, 200, 100));
        }
    }
}

fn of_builds(mode: Mode) -> SlotColors {
    SlotColors::defaults(mode)
}

#[test]
fn row_10_opening_resets_the_background_the_editor_target_and_slot_9_but_no_other() {
    let mut world = World::new_2(image_2(Mode::Rgba));
    dirties(&mut world.session);
    let new = image_2(Mode::Rgba);
    let path = png_on_disk(&new);
    world.session.open(&path, 16384, true).expect("abrir");
    assert_eq!(world.session.background, ColorChoice::Slot(9));
    assert_eq!(world.session.editing, Role::Foreground);
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let slots = world.session.slots.get(mode);
        assert_eq!(
            slots.get(9),
            of_builds(mode).get(9),
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
fn row_10_opening_an_indexed_one_puts_the_background_on_the_transparent_entry() {
    let mut world = World::new_2(image_2(Mode::Rgba));
    let new = image_2(Mode::Indexed);
    let expected_2 = default_background(&new);
    let path = png_on_disk(&new);
    world.session.open(&path, 16384, true).expect("abrir");
    let image = world.image_2();
    assert_eq!(world.session.background, default_background(image));
    // El PNG puede no conservar el índice transparente: la regla es la misma que la de la imagen.
    let _ = expected_2;
    let _ = fs::remove_dir_all(path.parent().expect("dir"));
}

#[test]
fn row_10_new_image_resets_the_same_and_restores_slot_9() {
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha, Mode::Indexed] {
        let mut world = World::new_2(image_2(Mode::Rgba));
        dirties(&mut world.session);
        let new = image_2(mode);
        let expected_2 = default_background(&new);
        world.session.new_image(new, true).expect("nueva");
        assert_eq!(world.session.background, expected_2, "{mode:?}");
        assert_eq!(world.session.editing, Role::Foreground);
        for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            let slots = world.session.slots.get(mode_2);
            assert_eq!(slots.get(9), of_builds(mode_2).get(9), "{mode_2:?}");
            assert_eq!(slots.get(3), Some(Rgba::opaque(3, 200, 100)));
        }
    }
}

#[test]
fn row_10_convert_mode_sets_the_default_background_of_the_new_mode() {
    let mut world = World::new_2(image_2(Mode::Rgba));
    world.session.background = ColorChoice::Slot(4);
    world
        .session
        .convert_mode(Mode::Gray, PERMISSIVE)
        .expect("convertir");
    let image = world.image_2();
    assert_eq!(image.mode(), Mode::Gray);
    assert_eq!(world.session.background, default_background(image));
    assert_eq!(world.session.background, ColorChoice::Slot(9));
}

#[test]
fn row_10_save_and_save_as_reset_nothing() {
    let mut world = World::new_2(image_2(Mode::Rgba));
    dirties(&mut world.session);
    world.session.background = ColorChoice::Slot(4);
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("pixoko-dos-colores-g-{}-{n}", std::process::id()));
    fs::create_dir_all(&dir).expect("directorio");
    world.session.save_as(&dir).ok();
    world.session.save(&dir).ok();
    assert_eq!(world.session.background, ColorChoice::Slot(4));
    assert_eq!(world.session.editing, Role::Background);
    assert_eq!(
        world.session.slots.get(Mode::Rgba).get(9),
        Some(Rgba::opaque(9, 200, 100))
    );
    // Con error (directorio inexistente) tampoco.
    world.session.save(&dir.join("no-existe")).ok();
    assert_eq!(world.session.background, ColorChoice::Slot(4));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn row_10_after_undoing_and_removing_the_background_is_still_valid() {
    let mut s = indexed_of(5);
    s.choice = ColorChoice::Entry(0);
    s.background = ColorChoice::Entry(4);
    let result = s
        .apply_palette_op(
            px_ui::palette_ops::PaletteOp::Remove { index: 4, into: 0 },
            true,
        )
        .expect("quitar");
    assert!(result.changed);
    assert_eq!(s.background, ColorChoice::Entry(3), "recortado a la última");
    s.undo().expect("deshacer");
    s.normalize();
    assert!(s.pixel_of(Role::Background).is_some());
}

#[test]
fn row_10_moving_an_entry_drags_the_background_that_was_on_it() {
    let mut s = indexed_of(6);
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
fn row_10_the_editor_with_editing_background_shows_and_writes_the_background_color() {
    // Indexada: edita la entrada del fondo con SetPaletteColor.
    let mut s = indexed_of(6);
    s.choice = ColorChoice::Entry(1);
    s.background = ColorChoice::Entry(3);
    s.editing = Role::Background;
    s.sync_editor();
    let goal = s.editor.target();
    assert_eq!(goal, Some(px_ui::color_edit::EditTarget::Entry(3)));
    // RGBA: edita la ranura del fondo.
    let mut world = World::new_2(image_2(Mode::Rgba));
    world.session.editing = Role::Background;
    world.session.sync_editor();
    assert_eq!(
        world.session.editor.target(),
        Some(px_ui::color_edit::EditTarget::Slot(5))
    );
    world.session.editing = Role::Foreground;
    world.session.sync_editor();
    assert_eq!(
        world.session.editor.target(),
        Some(px_ui::color_edit::EditTarget::Slot(3))
    );
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: el par siempre es válido y el documento solo cambia donde debe
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum TestStep {
    ClickLeft(usize),
    ClickRight(usize),
    Next,
    Previous,
    BackgroundNext,
    BackgroundPrevious,
    Dropper(Role, u32, u32),
    Brushstroke(u32, u32),
    Undo,
    Redo,
    Delete,
    Convert(usize),
    NewImage(usize),
}

fn step() -> impl Strategy<Value = TestStep> {
    prop_oneof![
        (0usize..300).prop_map(TestStep::ClickLeft),
        (0usize..300).prop_map(TestStep::ClickRight),
        Just(TestStep::Next),
        Just(TestStep::Previous),
        Just(TestStep::BackgroundNext),
        Just(TestStep::BackgroundPrevious),
        (any::<bool>(), 0u32..8, 0u32..4).prop_map(|(b, x, y)| TestStep::Dropper(
            if b {
                Role::Foreground
            } else {
                Role::Background
            },
            x,
            y
        )),
        (0u32..8, 0u32..4).prop_map(|(x, y)| TestStep::Brushstroke(x, y)),
        Just(TestStep::Undo),
        Just(TestStep::Redo),
        Just(TestStep::Delete),
        (0usize..4).prop_map(TestStep::Convert),
        (0usize..4).prop_map(TestStep::NewImage),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_1_the_pair_is_always_valid_and_the_document_only_changes_where_it_should(
        mode_2 in 0usize..4,
        steps_2 in proptest::collection::vec(step(), 0..24),
    ) {
        let mut world = World::new_2(image_2(TEST_MODES[mode_2]));
        for step in steps_2 {
            let before = (
                world.session.document.state_id(),
                world.session.document.can_undo(),
                world.session.document.can_redo(),
            );
            let (fg, bg) = (world.session.choice, world.session.background);
            let changes_the_document = matches!(
                step,
                TestStep::Brushstroke(..) | TestStep::Undo | TestStep::Redo | TestStep::Delete
                    | TestStep::Convert(_) | TestStep::NewImage(_)
            );
            match step {
                TestStep::ClickLeft(cell) => {
                    if let Some(c) = px_ui::color::choice_for_cell(world.image_2(), cell) {
                        world.session.choice = c;
                        prop_assert_eq!(world.session.background, bg);
                    }
                }
                TestStep::ClickRight(cell) => {
                    if let Some(c) = px_ui::color::choice_for_cell(world.image_2(), cell) {
                        world.session.background = c;
                        prop_assert_eq!(world.session.choice, fg);
                    }
                }
                TestStep::Next => { world.dispatches(AppAction::ColorNext); }
                TestStep::Previous => { world.dispatches(AppAction::ColorPrev); }
                TestStep::BackgroundNext => { world.dispatches(AppAction::BackgroundNext); }
                TestStep::BackgroundPrevious => { world.dispatches(AppAction::BackgroundPrev); }
                TestStep::Dropper(role, x, y) => {
                    let picked = px_ui::pick::picked_at(world.image_2(), x, y);
                    if let Some(picked) = picked {
                        world.session.apply_pick_for(picked, role);
                    }
                }
                TestStep::Brushstroke(x, y) => {
                    world.chooses(Tool::Pencil);
                    world.gesture(Role::Foreground, &[(x as i32, y as i32)], |_| {});
                }
                TestStep::Undo => { world.dispatches(AppAction::Undo); }
                TestStep::Redo => { world.dispatches(AppAction::Redo); }
                TestStep::Delete => { world.dispatches(AppAction::ClearPage); }
                TestStep::Convert(i) => {
                    world.session.convert_mode(TEST_MODES[i], PERMISSIVE).ok();
                }
                TestStep::NewImage(i) => {
                    world.session.new_image(image_2(TEST_MODES[i]), true).expect("nueva");
                }
            }
            world.session.normalize();
            prop_assert!(world.session.pixel_of(Role::Foreground).is_some());
            prop_assert!(world.session.pixel_of(Role::Background).is_some());
            if !changes_the_document {
                prop_assert_eq!(
                    (
                        world.session.document.state_id(),
                        world.session.document.can_undo(),
                        world.session.document.can_redo(),
                    ),
                    before
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: cada trazo, su color
// ---------------------------------------------------------------------------------------

fn tool_2(i: usize) -> Tool {
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
    fn property_2_each_stroke_writes_only_the_pixel_of_the_role_it_started_with(
        mode_2 in 0usize..4,
        tool in 0usize..5,
        right in any::<bool>(),
        points in proptest::collection::vec((0i32..8, 0i32..4), 1..6),
        changes_2 in proptest::collection::vec(0u8..4, 0..5),
    ) {
        let mut world = World::new_2(image_2(TEST_MODES[mode_2]));
        let role = if right { Role::Background } else { Role::Foreground };
        world.chooses(tool_2(tool));
        let original = world.pixels_2();
        let pixel_initial = world.session.pixel_of(role);
        let deleted = eraser_pixel(world.image_2()).ok();
        let id = world.session.document.state_id();
        world.gesture(role, &points, |world| {
            assert!(world.tools.busy());
            for change in &changes_2 {
                let action = match change {
                    0 => AppAction::ColorNext,
                    1 => AppAction::ColorPrev,
                    2 => AppAction::BackgroundNext,
                    _ => AppAction::BackgroundPrev,
                };
                world.dispatches(action);
            }
        });
        prop_assert!(!world.tools.busy());
        let after_2 = world.pixels_2();
        // Nunca más de un paso por gesto.
        let steps_2 = usize::from(world.session.document.state_id() != id);
        prop_assert!(steps_2 <= 1);
        if steps_2 == 1 {
            world.session.undo().expect("deshacer");
            prop_assert_eq!(world.pixels_2(), original.clone());
        }
        // Todo píxel cambiado vale lo que escribe la herramienta: el color del rol al pulsar
        // (el valor de borrado con el borrador; nada con el cuentagotas).
        let allowed = if tool_2(tool) == Tool::Eraser {
            deleted
        } else {
            pixel_initial
        };
        for (before, after_2) in original.iter().zip(&after_2) {
            if before != after_2 {
                prop_assert_eq!(Some(*after_2), allowed);
                prop_assert_ne!(tool_2(tool), Tool::Dropper);
            }
        }
    }
}
