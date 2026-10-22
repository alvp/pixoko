//! Tests de `SlotColors`, `SlotBank`, las variantes `_with` de `color` y de `ToolState`
//! (specs/hito-2-editar-color.md, unidad 11c, decisión 2 y «Piezas»).
//!
//! - `SlotColors::defaults(mode_2)` da lo mismo que `pixel_for_slot` en cada modo y ranura 1 a 9:
//!   con esos valores, las funciones `_with` coinciden con las de siempre (`pixel_for_choice`,
//!   `cell_color`, `is_transparent_cell`, `cell_label` y `choice_label`).
//! - `get` y `set` (las ranuras 0 y 10 no existen), `SlotBank` (una tabla por modo,
//!   independientes; indexadas comparte la de RGBA).
//! - Una ranura editada en RGBA, Gray y GrayAlpha llega a `pixel_for_choice_with`, a las
//!   etiquetas y a las muestras.
//! - `ToolState::canvas_event_with` y `canvas_input_with` dibujan el `Pixel` dado (lápiz, línea
//!   y relleno) y, con `None`, no empiezan gesto.

use px_core::{Document, Image, ImageId, Mode, Palette, Pixel, Rgba};
use px_ui::canvas::{CanvasEvent, CanvasInput};
use px_ui::color::{
    ColorChoice, SlotBank, SlotColors, cell_color, cell_color_with, cell_count, cell_label,
    cell_label_with, choice_label, choice_label_with, is_transparent_cell,
    is_transparent_cell_with, pixel_for_choice, pixel_for_choice_with, pixel_for_slot,
};
use px_ui::tool::{Tool, ToolAction, ToolState};

const TEST_MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// Color que no es ninguno de los de las ranuras por defecto.
fn own() -> Rgba {
    Rgba::opaque(12, 34, 56)
}

/// Indexada de 9 entradas opacas y distintas (todas las ranuras valen).
fn indexed_9() -> Image {
    let colors_2: Vec<Rgba> = (0..9u8)
        .map(|i| Rgba::opaque(i * 20, 255 - i * 20, 7))
        .collect();
    let palette = Palette::from_colors(colors_2).expect("paleta válida");
    Image::new_indexed(8, 8, palette).expect("imagen válida")
}

/// Indexada de 3 entradas, con la 1 transparente.
fn indexed_3_with_transparent() -> Image {
    let mut palette = Palette::from_colors([
        Rgba::opaque(10, 10, 10),
        Rgba::opaque(20, 20, 20),
        Rgba::opaque(30, 30, 30),
    ])
    .expect("paleta válida");
    palette
        .set_transparent_index(Some(1))
        .expect("índice válido");
    Image::new_indexed(8, 8, palette).expect("imagen válida")
}

fn image_2(mode_2: Mode) -> Image {
    Image::new(8, 8, mode_2).expect("imagen válida")
}

/// Todas las imágenes de prueba, una por modo (y dos indexadas).
fn images() -> Vec<Image> {
    vec![
        image_2(Mode::Rgba),
        image_2(Mode::Gray),
        image_2(Mode::GrayAlpha),
        indexed_9(),
        indexed_3_with_transparent(),
    ]
}

fn options_2() -> Vec<ColorChoice> {
    let mut options_2: Vec<ColorChoice> = (0..=10).map(ColorChoice::Slot).collect();
    options_2.extend((0..=10).map(ColorChoice::Entry));
    options_2.push(ColorChoice::Entry(255));
    options_2
}

// Valores por defecto.

#[test]
fn the_default_values_of_rgba() {
    let tabla = SlotColors::defaults(Mode::Rgba);
    let expected_2 = [
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(255, 255, 255),
        Rgba::opaque(255, 0, 0),
        Rgba::opaque(0, 255, 0),
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(255, 255, 0),
        Rgba::opaque(255, 0, 255),
        Rgba::opaque(0, 255, 255),
        Rgba::TRANSPARENT,
    ];
    for (i, color) in expected_2.into_iter().enumerate() {
        let slot = i as u8 + 1;
        assert_eq!(tabla.get(slot), Some(color), "ranura {slot}");
    }
}

#[test]
fn the_default_values_of_gray_and_gray_alpha() {
    let gray_2 = SlotColors::defaults(Mode::Gray);
    let with_alpha = SlotColors::defaults(Mode::GrayAlpha);
    for slot in 1..=9u8 {
        let valor = if slot == 9 { 255 } else { (slot - 1) * 32 };
        assert_eq!(
            gray_2.get(slot),
            Some(Rgba::opaque(valor, valor, valor)),
            "Gray, ranura {slot}"
        );
        let expected_2 = if slot == 9 {
            Rgba::TRANSPARENT
        } else {
            Rgba::opaque(valor, valor, valor)
        };
        assert_eq!(
            with_alpha.get(slot),
            Some(expected_2),
            "GrayAlpha, ranura {slot}"
        );
    }
}

#[test]
fn with_the_default_values_pixel_for_choice_with_is_pixel_for_choice() {
    for image in images() {
        let tabla = SlotColors::defaults(image.mode());
        for choice in options_2() {
            assert_eq!(
                pixel_for_choice_with(&image, choice, &tabla),
                pixel_for_choice(&image, choice),
                "{:?} {choice:?}",
                image.mode()
            );
        }
    }
}

#[test]
fn with_the_default_values_each_slot_gives_what_pixel_for_slot_gives() {
    for image in images() {
        let tabla = SlotColors::defaults(image.mode());
        for slot in 1..=9u8 {
            assert_eq!(
                pixel_for_choice_with(&image, ColorChoice::Slot(slot), &tabla),
                pixel_for_slot(&image, slot),
                "{:?} ranura {slot}",
                image.mode()
            );
        }
    }
}

#[test]
fn with_the_default_values_the_swatches_and_labels_do_not_change() {
    for image in images() {
        let tabla = SlotColors::defaults(image.mode());
        // Hasta dos más allá de la última muestra: las inexistentes dan lo mismo (None, false).
        for cel in 0..cell_count(&image) + 2 {
            assert_eq!(
                cell_color_with(&image, cel, &tabla),
                cell_color(&image, cel),
                "{:?} celda {cel}",
                image.mode()
            );
            assert_eq!(
                cell_label_with(&image, cel, &tabla),
                cell_label(&image, cel),
                "{:?} celda {cel}",
                image.mode()
            );
            assert_eq!(
                is_transparent_cell_with(&image, cel, &tabla),
                is_transparent_cell(&image, cel),
                "{:?} celda {cel}",
                image.mode()
            );
        }
        for choice in options_2() {
            assert_eq!(
                choice_label_with(&image, choice, &tabla),
                choice_label(&image, choice),
                "{:?} {choice:?}",
                image.mode()
            );
        }
    }
}

// `get` y `set`.

#[test]
fn get_outside_1_to_9_gives_none() {
    for mode_2 in TEST_MODES {
        let tabla = SlotColors::defaults(mode_2);
        assert_eq!(tabla.get(0), None);
        assert_eq!(tabla.get(10), None);
        assert_eq!(tabla.get(255), None);
        for slot in 1..=9 {
            assert!(tabla.get(slot).is_some());
        }
    }
}

#[test]
fn set_changes_only_that_slot() {
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    let original = tabla.clone();
    assert!(tabla.set(3, rgba(1, 2, 3, 4)));
    assert_eq!(tabla.get(3), Some(rgba(1, 2, 3, 4)));
    for slot in (1..=9).filter(|r| *r != 3) {
        assert_eq!(tabla.get(slot), original.get(slot), "ranura {slot}");
    }
    assert_ne!(tabla, original);
    // Volver al valor por defecto da la tabla original.
    assert!(tabla.set(3, original.get(3).expect("existe")));
    assert_eq!(tabla, original);
}

#[test]
fn set_in_all_the_slots_from_1_to_9() {
    let mut tabla = SlotColors::defaults(Mode::GrayAlpha);
    for slot in 1..=9u8 {
        let color = rgba(slot, slot, slot, 100 + slot);
        assert!(tabla.set(slot, color), "ranura {slot}");
        assert_eq!(tabla.get(slot), Some(color));
    }
}

#[test]
fn set_outside_1_to_9_gives_false_and_changes_nothing() {
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    let original = tabla.clone();
    for slot in [0u8, 10, 11, 255] {
        assert!(!tabla.set(slot, own()), "ranura {slot}");
        assert_eq!(tabla, original, "ranura {slot}");
    }
}

// `SlotBank`.

#[test]
fn a_new_bench_has_all_the_default_tables() {
    let bench = SlotBank::new();
    for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        assert_eq!(
            bench.get(mode_2),
            &SlotColors::defaults(mode_2),
            "{mode_2:?}"
        );
    }
    assert_eq!(SlotBank::default(), bench);
}

#[test]
fn the_bench_tables_are_independent_per_mode() {
    let mut bench = SlotBank::new();
    let (rgba_antes, gray_before, alpha_before) = (
        bench.get(Mode::Rgba).clone(),
        bench.get(Mode::Gray).clone(),
        bench.get(Mode::GrayAlpha).clone(),
    );
    assert!(bench.get_mut(Mode::Rgba).set(3, own()));
    assert_eq!(bench.get(Mode::Rgba).get(3), Some(own()));
    assert_eq!(bench.get(Mode::Gray), &gray_before);
    assert_eq!(bench.get(Mode::GrayAlpha), &alpha_before);

    assert!(
        bench
            .get_mut(Mode::Gray)
            .set(5, Rgba::opaque(200, 200, 200))
    );
    assert_eq!(
        bench.get(Mode::Gray).get(5),
        Some(Rgba::opaque(200, 200, 200))
    );
    assert_eq!(bench.get(Mode::GrayAlpha), &alpha_before);
    assert_ne!(bench.get(Mode::Rgba), &rgba_antes);

    assert!(bench.get_mut(Mode::GrayAlpha).set(9, rgba(9, 9, 9, 255)));
    assert_eq!(bench.get(Mode::GrayAlpha).get(9), Some(rgba(9, 9, 9, 255)));
    // Lo anterior sigue como estaba.
    assert_eq!(bench.get(Mode::Rgba).get(3), Some(own()));
    assert_eq!(
        bench.get(Mode::Gray).get(5),
        Some(Rgba::opaque(200, 200, 200))
    );
}

#[test]
fn indexed_shares_the_table_of_rgba() {
    let mut bench = SlotBank::new();
    assert_eq!(bench.get(Mode::Indexed), bench.get(Mode::Rgba));
    assert!(bench.get_mut(Mode::Indexed).set(4, own()));
    assert_eq!(bench.get(Mode::Rgba).get(4), Some(own()));
    assert!(bench.get_mut(Mode::Rgba).set(6, rgba(1, 1, 1, 1)));
    assert_eq!(bench.get(Mode::Indexed).get(6), Some(rgba(1, 1, 1, 1)));
    // Y no toca las de gris.
    assert_eq!(bench.get(Mode::Gray), &SlotColors::defaults(Mode::Gray));
}

#[test]
fn the_bench_can_be_cloned_and_compared() {
    let mut bench = SlotBank::new();
    let copy = bench.clone();
    assert_eq!(bench, copy);
    bench.get_mut(Mode::Rgba).set(1, own());
    assert_ne!(bench, copy);
}

// Una ranura editada.

#[test]
fn rgba_an_edited_slot_is_drawn_with_its_color() {
    let image = image_2(Mode::Rgba);
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    assert!(tabla.set(3, YELLOW));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(3), &tabla),
        Some(Pixel::Rgba(YELLOW))
    );
    // Las demás no cambian.
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(2), &tabla),
        pixel_for_slot(&image, 2)
    );
    // Y las funciones sin `_with` siguen con los valores por defecto.
    assert_eq!(
        pixel_for_choice(&image, ColorChoice::Slot(3)),
        pixel_for_slot(&image, 3)
    );
}

const YELLOW: Rgba = Rgba::opaque(255, 255, 0);

#[test]
fn rgba_the_swatch_and_the_labels_read_the_edited_slot() {
    // Fila 15: `cell_label(2)` da `color 3 #ffff00`.
    let image = image_2(Mode::Rgba);
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    tabla.set(3, YELLOW);
    assert_eq!(cell_color_with(&image, 2, &tabla), Some(YELLOW));
    assert_eq!(
        cell_label_with(&image, 2, &tabla).as_deref(),
        Some("color 3 #ffff00")
    );
    assert_eq!(
        choice_label_with(&image, ColorChoice::Slot(3), &tabla).as_deref(),
        Some("color 3 #ffff00")
    );
    assert!(!is_transparent_cell_with(&image, 2, &tabla));
}

#[test]
fn rgba_slot_9_goes_from_transparent_to_opaque() {
    let image = image_2(Mode::Rgba);
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    assert!(is_transparent_cell_with(&image, 8, &tabla));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(9), &tabla),
        Some(Pixel::Rgba(Rgba::TRANSPARENT))
    );
    // Fila 15: la 9 (por defecto `(0,0,0,0)`) con A a 255.
    assert!(tabla.set(9, Rgba::opaque(0, 0, 0)));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(9), &tabla),
        Some(Pixel::Rgba(Rgba::opaque(0, 0, 0)))
    );
    assert!(!is_transparent_cell_with(&image, 8, &tabla));
    assert_eq!(
        cell_color_with(&image, 8, &tabla),
        Some(Rgba::opaque(0, 0, 0))
    );
}

#[test]
fn rgba_a_slot_with_alpha_zero_is_transparent() {
    let image = image_2(Mode::Rgba);
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    tabla.set(2, rgba(255, 255, 255, 0));
    assert!(is_transparent_cell_with(&image, 1, &tabla));
}

#[test]
fn gray_an_edited_slot_gives_pixel_gray_with_the_red() {
    let image = image_2(Mode::Gray);
    let mut tabla = SlotColors::defaults(Mode::Gray);
    // Fila 15: Gray, `Slot(5)` (gris 128) con G a 200.
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(5), &tabla),
        Some(Pixel::Gray(128))
    );
    assert!(tabla.set(5, Rgba::opaque(200, 200, 200)));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(5), &tabla),
        Some(Pixel::Gray(200))
    );
    assert_eq!(
        cell_label_with(&image, 4, &tabla).as_deref(),
        Some("color 5 gris 200")
    );
    assert_eq!(
        cell_color_with(&image, 4, &tabla),
        Some(Rgba::opaque(200, 200, 200))
    );
}

#[test]
fn gray_alpha_an_edited_slot_carries_gray_and_alpha() {
    let image = image_2(Mode::GrayAlpha);
    let mut tabla = SlotColors::defaults(Mode::GrayAlpha);
    assert!(tabla.set(9, rgba(40, 40, 40, 128)));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(9), &tabla),
        Some(Pixel::GrayAlpha {
            gray: 40,
            alpha: 128
        })
    );
    assert_eq!(
        cell_label_with(&image, 8, &tabla).as_deref(),
        Some("color 9 gris 40 alfa 128")
    );
    assert!(!is_transparent_cell_with(&image, 8, &tabla));

    // A 255 se rotula sin alfa, y con alfa 0 es transparente.
    assert!(tabla.set(2, rgba(77, 77, 77, 255)));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(2), &tabla),
        Some(Pixel::GrayAlpha {
            gray: 77,
            alpha: 255
        })
    );
    assert_eq!(
        cell_label_with(&image, 1, &tabla).as_deref(),
        Some("color 2 gris 77")
    );
    assert!(tabla.set(3, rgba(10, 10, 10, 0)));
    assert!(is_transparent_cell_with(&image, 2, &tabla));
}

#[test]
fn in_indexed_the_edited_slots_do_not_count() {
    for image in [indexed_9(), indexed_3_with_transparent()] {
        let mut tabla = SlotColors::defaults(Mode::Indexed);
        for slot in 1..=9u8 {
            tabla.set(slot, own());
        }
        for choice in options_2() {
            assert_eq!(
                pixel_for_choice_with(&image, choice, &tabla),
                pixel_for_choice(&image, choice),
                "{choice:?}"
            );
        }
        for cel in 0..cell_count(&image) + 2 {
            assert_eq!(
                cell_color_with(&image, cel, &tabla),
                cell_color(&image, cel)
            );
            assert_eq!(
                cell_label_with(&image, cel, &tabla),
                cell_label(&image, cel)
            );
        }
    }
}

#[test]
fn choices_that_are_not_valid_stay_invalid_with_an_edited_table() {
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    tabla.set(1, own());
    let image = image_2(Mode::Rgba);
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(0), &tabla),
        None
    );
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(10), &tabla),
        None
    );
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Entry(0), &tabla),
        None
    );
    assert_eq!(cell_color_with(&image, 9, &tabla), None);
    assert_eq!(cell_label_with(&image, 9, &tabla), None);
    assert_eq!(
        choice_label_with(&image, ColorChoice::Slot(10), &tabla),
        None
    );
    assert!(!is_transparent_cell_with(&image, 9, &tabla));
}

// `ToolState::canvas_event_with` y `canvas_input_with`.

fn begin(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeBegin((x, y))
}

fn mov(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeMove((x, y))
}

fn fin() -> CanvasEvent {
    CanvasEvent::StrokeEnd
}

struct Scene {
    doc: Document,
    id: ImageId,
    tool: ToolState,
}

impl Scene {
    fn new(image: Image, tool_2: Tool) -> Scene {
        let doc = Document::new(image);
        let id = ImageId::from_raw(0);
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(tool_2));
        Scene { doc, id, tool }
    }

    fn rgba(tool_2: Tool) -> Scene {
        Scene::new(image_2(Mode::Rgba), tool_2)
    }

    fn image_2(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    fn pixel(&self, x: u32, y: u32) -> Pixel {
        self.image_2().get_pixel(x, y).expect("píxel dentro")
    }

    /// Un evento con `canvas_event_with`; un `Err` es un fallo del test.
    fn ev(&mut self, pixel: Option<Pixel>, event: CanvasEvent) -> bool {
        self.tool
            .canvas_event_with(&mut self.doc, self.id, pixel, event)
            .unwrap_or_else(|e| panic!("{event:?} dio el error {e:?}"))
    }

    fn input_2(&mut self, pixel: Option<Pixel>, input_2: CanvasInput) -> bool {
        self.tool
            .canvas_input_with(&mut self.doc, self.id, pixel, input_2)
            .unwrap_or_else(|e| panic!("{input_2:?} dio el error {e:?}"))
    }
}

fn pixel_own() -> Pixel {
    Pixel::Rgba(own())
}

#[test]
fn the_pencil_draws_the_given_pixel() {
    let mut scene = Scene::rgba(Tool::Pencil);
    let empty = scene.pixel(0, 0);
    assert!(!scene.ev(Some(pixel_own()), begin(2, 2)));
    assert!(!scene.ev(Some(pixel_own()), mov(5, 2)));
    assert!(
        scene.ev(Some(pixel_own()), fin()),
        "el trazo cambia el documento"
    );
    assert_eq!(scene.pixel(2, 2), pixel_own());
    assert_eq!(scene.pixel(5, 2), pixel_own());
    assert_eq!(scene.pixel(0, 0), empty);
    assert!(scene.doc.can_undo());
    assert!(!scene.tool.busy());
}

#[test]
fn a_click_without_moving_with_the_pencil_paints_a_pixel() {
    let mut scene = Scene::rgba(Tool::Pencil);
    scene.ev(Some(pixel_own()), begin(3, 4));
    assert!(scene.ev(Some(pixel_own()), fin()));
    assert_eq!(scene.pixel(3, 4), pixel_own());
    assert_eq!(
        scene.pixel(4, 4),
        image_2(Mode::Rgba).get_pixel(4, 4).unwrap()
    );
}

#[test]
fn the_line_draws_the_given_pixel() {
    let mut scene = Scene::rgba(Tool::Line);
    scene.ev(Some(pixel_own()), begin(0, 0));
    scene.ev(Some(pixel_own()), mov(3, 0));
    assert_eq!(scene.tool.preview().count(), 4, "la línea pendiente se ve");
    assert!(scene.ev(Some(pixel_own()), fin()));
    for x in 0..=3 {
        assert_eq!(scene.pixel(x, 0), pixel_own(), "x = {x}");
    }
    assert_ne!(scene.pixel(4, 0), pixel_own());
}

#[test]
fn the_fill_paints_with_the_given_pixel() {
    let mut scene = Scene::rgba(Tool::Fill);
    assert!(scene.ev(Some(pixel_own()), begin(1, 1)));
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(scene.pixel(x, y), pixel_own(), "({x}, {y})");
        }
    }
    assert!(!scene.ev(Some(pixel_own()), fin()));
    assert!(scene.doc.can_undo());
}

#[test]
fn the_given_pixel_is_valid_in_gray_gray_alpha_and_indexed() {
    let cases = [
        (image_2(Mode::Gray), Pixel::Gray(200)),
        (
            image_2(Mode::GrayAlpha),
            Pixel::GrayAlpha {
                gray: 40,
                alpha: 128,
            },
        ),
        (indexed_9(), Pixel::Indexed(1)),
    ];
    for (image, pixel) in cases {
        let mode_2 = image.mode();
        let mut scene = Scene::new(image, Tool::Fill);
        assert!(scene.ev(Some(pixel), begin(0, 0)), "{mode_2:?}");
        assert_eq!(scene.pixel(7, 7), pixel, "{mode_2:?}");
    }
    let mut scene = Scene::new(image_2(Mode::Gray), Tool::Pencil);
    scene.ev(Some(Pixel::Gray(200)), begin(1, 1));
    assert!(scene.ev(Some(Pixel::Gray(200)), fin()));
    assert_eq!(scene.pixel(1, 1), Pixel::Gray(200));
}

#[test]
fn without_a_pixel_no_tool_starts_a_gesture_or_changes_the_document() {
    for tool_2 in [Tool::Pencil, Tool::Line, Tool::Fill] {
        let mut scene = Scene::rgba(tool_2);
        let original = scene.image_2().clone();
        let state_2 = scene.doc.state_id();
        assert!(!scene.ev(None, begin(2, 2)), "{tool_2:?}");
        assert!(!scene.ev(None, mov(5, 2)), "{tool_2:?}");
        assert_eq!(scene.tool.preview().count(), 0, "{tool_2:?}");
        assert!(!scene.ev(None, fin()), "{tool_2:?}");
        assert_eq!(scene.image_2(), &original, "{tool_2:?}");
        assert_eq!(scene.doc.state_id(), state_2, "{tool_2:?}");
        assert!(!scene.doc.can_undo(), "{tool_2:?}");
        assert!(!scene.tool.busy(), "{tool_2:?}");
    }
}

#[test]
fn with_a_given_pixel_and_then_without_it_drawing_works_again() {
    let mut scene = Scene::rgba(Tool::Pencil);
    scene.ev(None, begin(2, 2));
    scene.ev(None, fin());
    scene.ev(Some(pixel_own()), begin(2, 2));
    assert!(scene.ev(Some(pixel_own()), fin()));
    assert_eq!(scene.pixel(2, 2), pixel_own());
}

#[test]
fn canvas_input_with_with_stroke_events_is_equivalent_to_canvas_event_with() {
    for tool_2 in [Tool::Pencil, Tool::Line, Tool::Fill] {
        let mut direct = Scene::rgba(tool_2);
        let mut ordered = Scene::rgba(tool_2);
        for event in [begin(1, 1), mov(4, 1), mov(4, 4), fin()] {
            let a = direct.ev(Some(pixel_own()), event);
            let b = ordered.input_2(Some(pixel_own()), CanvasInput::Stroke(event));
            assert_eq!(a, b, "{tool_2:?} {event:?}");
        }
        assert_eq!(direct.image_2(), ordered.image_2(), "{tool_2:?}");
    }
}

#[test]
fn canvas_input_with_escape_cancels_the_pending_line() {
    let mut scene = Scene::rgba(Tool::Line);
    let original = scene.image_2().clone();
    scene.input_2(Some(pixel_own()), CanvasInput::Stroke(begin(0, 0)));
    scene.input_2(Some(pixel_own()), CanvasInput::Stroke(mov(3, 0)));
    assert!(!scene.input_2(Some(pixel_own()), CanvasInput::Escape));
    assert_eq!(scene.tool.preview().count(), 0);
    assert!(!scene.input_2(Some(pixel_own()), CanvasInput::Stroke(fin())));
    assert_eq!(scene.image_2(), &original);
    assert!(!scene.tool.busy());
}

#[test]
fn with_the_pixel_of_pixel_for_choice_the_with_variants_give_the_same_as_the_usual_ones() {
    for tool_2 in [Tool::Pencil, Tool::Line, Tool::Fill] {
        let mut with = Scene::rgba(tool_2);
        let mut without = Scene::rgba(tool_2);
        let pixel = pixel_for_choice(&image_2(Mode::Rgba), ColorChoice::Slot(3));
        assert_eq!(pixel, Some(Pixel::Rgba(Rgba::opaque(255, 0, 0))));
        for event in [begin(1, 1), mov(5, 5), fin()] {
            let a = with.ev(pixel, event);
            let b = without
                .tool
                .canvas_event(&mut without.doc, without.id, 3, event)
                .expect("sin error");
            assert_eq!(a, b, "{tool_2:?} {event:?}");
        }
        assert_eq!(with.image_2(), without.image_2(), "{tool_2:?}");
    }
}

#[test]
fn choice_label_with_labels_the_edited_slot() {
    // Como `slot_label`: `color 3 #ffff00`, `color 5 gray_2 200`, `color 9 transparent`.
    let rgba_img = image_2(Mode::Rgba);
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    tabla.set(3, YELLOW);
    assert_eq!(
        choice_label_with(&rgba_img, ColorChoice::Slot(3), &tabla).as_deref(),
        Some("color 3 #ffff00")
    );
    assert_eq!(
        choice_label_with(&rgba_img, ColorChoice::Slot(9), &tabla).as_deref(),
        Some("color 9 transparente")
    );
    tabla.set(9, Rgba::opaque(0, 0, 0));
    assert_eq!(
        choice_label_with(&rgba_img, ColorChoice::Slot(9), &tabla).as_deref(),
        Some("color 9 #000000")
    );

    let gray_2 = image_2(Mode::Gray);
    let mut tabla = SlotColors::defaults(Mode::Gray);
    tabla.set(5, Rgba::opaque(200, 200, 200));
    assert_eq!(
        choice_label_with(&gray_2, ColorChoice::Slot(5), &tabla).as_deref(),
        Some("color 5 gris 200")
    );
}
