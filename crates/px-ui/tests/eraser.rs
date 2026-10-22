//! Tests del borrador como lógica de herramienta, sin ventana (specs/hito-2-cuentagotas-
//! borrador.md, «Casos límite» filas 3, 4 y 15, y propiedad 1 y su ampliación).
//!
//! Los eventos se dan a mano a `ToolState::canvas_event_with`. El borrador ignora el `Pixel` que
//! recibe: aquí se le pasa uno que no es ni del modo de la imagen, para comprobarlo. El oráculo
//! de la propiedad 1 es un lápiz (`Tool::Pencil`) con `eraser_pixel(image)` como color y, para
//! los píxeles que cambian, un recorrido con `Image::get_pixel` y `Image::set_pixel` que no usa
//! nada de `pick`.

use std::collections::BTreeSet;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Command, Document, Image, ImageId, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_ui::canvas::CanvasEvent;
use px_ui::pick::{EraseError, eraser_pixel};
use px_ui::texture::pixel_rgba;
use px_ui::tool::{Tool, ToolAction, ToolEvent, ToolState};

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;

// Apoyo común.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

fn begin(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeBegin((x, y))
}

fn mov(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeMove((x, y))
}

fn fin() -> CanvasEvent {
    CanvasEvent::StrokeEnd
}

/// Imagen RGBA de 8×8 azul opaca `(0, 0, 255, 255)`.
fn rgba_blue() -> Image {
    let mut image = Image::new(8, 8, Mode::Rgba).expect("imagen válida");
    fills(&mut image, Pixel::Rgba(Rgba::opaque(0, 0, 255)));
    image
}

/// Imagen RGBA de 8×8 vacía: todo `(0, 0, 0, 0)`.
fn rgba_transparent() -> Image {
    Image::new(8, 8, Mode::Rgba).expect("imagen válida")
}

/// Imagen de gris con alfa de 8×8, gris 100 opaco.
fn gray_alpha() -> Image {
    let mut image = Image::new(8, 8, Mode::GrayAlpha).expect("imagen válida");
    fills(
        &mut image,
        Pixel::GrayAlpha {
            gray: 100,
            alpha: 255,
        },
    );
    image
}

/// Indexada de 8×8: índice 0 azul, índice 1 transparente; todo el lienzo vale `Indexed(0)`.
fn indexed_with_transparent() -> Image {
    let mut palette = Palette::from_colors([Rgba::opaque(0, 0, 255), Rgba::opaque(255, 0, 255)])
        .expect("paleta válida");
    palette.set_transparent_index(Some(1)).expect("existe");
    Image::new_indexed(8, 8, palette).expect("imagen válida")
}

fn indexed_without_transparent() -> Image {
    let palette = Palette::from_colors([Rgba::opaque(0, 0, 255), Rgba::opaque(255, 0, 255)])
        .expect("paleta válida");
    Image::new_indexed(8, 8, palette).expect("imagen válida")
}

fn gray_2() -> Image {
    let mut image = Image::new(8, 8, Mode::Gray).expect("imagen válida");
    fills(&mut image, Pixel::Gray(100));
    image
}

fn fills(image: &mut Image, pixel: Pixel) {
    for y in 0..image.height() {
        for x in 0..image.width() {
            image.set_pixel(x, y, pixel).expect("dentro");
        }
    }
}

/// Un color cualquiera, de otro modo que casi todas las imágenes: el borrador lo ignora.
fn color_of_another_mode() -> Option<Pixel> {
    Some(Pixel::Rgba(Rgba::opaque(255, 0, 0)))
}

/// Documento y herramienta juntos, con el borrador elegido.
struct Scene {
    doc: Document,
    id: ImageId,
    tool: ToolState,
    /// El `Pixel` que recibe `canvas_event_with` (el borrador no lo usa).
    pixel: Option<Pixel>,
}

impl Scene {
    fn new(image: Image) -> Scene {
        Scene::with(image, Tool::Eraser)
    }

    fn with(image: Image, tool_2: Tool) -> Scene {
        let doc = Document::new(image);
        let id = ImageId::from_raw(0);
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(tool_2));
        assert_eq!(tool.tool(), tool_2);
        Scene {
            doc,
            id,
            tool,
            pixel: color_of_another_mode(),
        }
    }

    fn image_2(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    /// Da el evento a la herramienta; un `Err` aquí es un fallo del test.
    fn ev(&mut self, event: CanvasEvent) -> bool {
        self.tool
            .canvas_event_with(&mut self.doc, self.id, self.pixel, event)
            .unwrap_or_else(|e| panic!("{event:?} dio el error {e:?}"))
    }

    fn preview(&self) -> Vec<PixelEdit> {
        self.tool.preview().collect()
    }

    fn history(&self) -> (bool, bool, usize) {
        history(&self.doc)
    }
}

/// `can_undo`, `can_redo` e `history_bytes`.
fn history(doc: &Document) -> (bool, bool, usize) {
    (doc.can_undo(), doc.can_redo(), doc.history_bytes())
}

fn set(points: impl IntoIterator<Item = (u32, u32)>) -> BTreeSet<(u32, u32)> {
    points.into_iter().collect()
}

/// Coordenadas de los píxeles que difieren entre dos imágenes del mismo tamaño.
fn changed(a: &Image, b: &Image) -> BTreeSet<(u32, u32)> {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    (0..a.height())
        .flat_map(|y| (0..a.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| a.get_pixel(x, y).unwrap() != b.get_pixel(x, y).unwrap())
        .collect()
}

/// Los siete píxeles de la fila 3: `(1,1)…(5,1)`, `(5,2)` y `(5,3)`, en ese orden, con `pixel`.
fn the_seven(pixel: Pixel) -> Vec<PixelEdit> {
    [(1, 1), (2, 1), (3, 1), (4, 1), (5, 1), (5, 2), (5, 3)]
        .into_iter()
        .map(|(x, y)| PixelEdit { x, y, pixel })
        .collect()
}

/// El gesto de la fila 3 sin el `End`: `Begin(1,1)`, `Move(5,1)`, `Move(5,3)`.
fn gesture_of_row_3(es: &mut Scene) {
    assert!(!es.ev(begin(1, 1)), "Begin no cambia el documento");
    assert!(!es.ev(mov(5, 1)), "Move no cambia el documento");
    assert!(!es.ev(mov(5, 3)), "Move no cambia el documento");
}

// Fila 3.

/// La fila 3 completa para una imagen y su valor de borrado.
fn row_3(image: Image, deleted: Pixel) {
    let copy = image.clone();
    let mut es = Scene::new(image);
    let state_id = es.doc.state_id();

    // Antes del `End`: el documento no cambia y la vista previa es el trazo con el valor de borrado.
    gesture_of_row_3(&mut es);
    assert_eq!(es.image_2(), &copy);
    assert!(!es.doc.can_undo());
    assert_eq!(es.doc.state_id(), state_id);
    assert!(es.tool.busy());
    assert_eq!(es.preview(), the_seven(deleted));
    for editing in es.preview() {
        let color = pixel_rgba(&copy, editing.pixel).expect("el valor de borrado se ve");
        assert_eq!(color.a, 0, "({}, {}) con alfa 0", editing.x, editing.y);
    }
    assert_eq!(
        es.tool.take_events(),
        vec![],
        "sin eventos: no hay nada que decir"
    );

    // `End`: un solo paso con esos siete píxeles.
    assert!(es.ev(fin()), "End da Ok(true)");
    assert!(!es.tool.busy());
    assert_eq!(es.preview(), vec![], "nada pendiente tras el End");
    assert_eq!(es.tool.take_events(), vec![]);
    let seven = set(the_seven(deleted).iter().map(|e| (e.x, e.y)));
    assert_eq!(changed(es.image_2(), &copy), seven);
    for editing in the_seven(deleted) {
        assert_eq!(es.image_2().get_pixel(editing.x, editing.y), Ok(deleted));
    }
    let edited = es.image_2().clone();
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy, "un solo paso");
    assert!(!es.doc.can_undo());
    assert_eq!(es.doc.redo(), Ok(()));
    assert_eq!(es.image_2(), &edited);
}

#[test]
fn row_3_eraser_applied_in_rgba() {
    row_3(rgba_blue(), Pixel::Rgba(Rgba::TRANSPARENT));
}

#[test]
fn row_3_eraser_applied_in_gray_alpha() {
    row_3(gray_alpha(), Pixel::GrayAlpha { gray: 0, alpha: 0 });
}

#[test]
fn row_3_eraser_applied_in_indexed_with_transparent() {
    row_3(indexed_with_transparent(), Pixel::Indexed(1));
}

#[test]
fn the_eraser_ignores_the_pixel_it_receives() {
    // Con `None`, con un color del modo equivocado y con un índice que no existe: el mismo trazo.
    let images = [
        (rgba_blue(), Pixel::Rgba(Rgba::TRANSPARENT)),
        (gray_alpha(), Pixel::GrayAlpha { gray: 0, alpha: 0 }),
        (indexed_with_transparent(), Pixel::Indexed(1)),
    ];
    let received_2 = [
        None,
        Some(Pixel::Gray(3)),
        Some(Pixel::Indexed(200)),
        Some(Pixel::Rgba(Rgba::opaque(255, 0, 0))),
        Some(Pixel::GrayAlpha {
            gray: 9,
            alpha: 255,
        }),
    ];
    for (image, deleted) in images {
        let copy = image.clone();
        for received in received_2 {
            let mut es = Scene::new(image.clone());
            es.pixel = received;
            gesture_of_row_3(&mut es);
            assert_eq!(
                es.preview(),
                the_seven(deleted),
                "{:?} con {received:?}",
                copy.mode()
            );
            assert!(es.ev(fin()), "{:?} con {received:?}", copy.mode());
            assert_eq!(
                changed(es.image_2(), &copy).len(),
                7,
                "{:?} con {received:?}",
                copy.mode()
            );
            for editing in the_seven(deleted) {
                assert_eq!(es.image_2().get_pixel(editing.x, editing.y), Ok(deleted));
            }
        }
    }
}

#[test]
fn the_eraser_can_vary_the_pixel_between_events_without_effect() {
    // `Begin` fija el valor de borrado; lo que llegue después en `pixel` no cuenta.
    let copy = rgba_blue();
    let mut es = Scene::new(copy.clone());
    es.pixel = Some(Pixel::Gray(1));
    assert!(!es.ev(begin(1, 1)));
    es.pixel = None;
    assert!(!es.ev(mov(5, 1)));
    es.pixel = Some(Pixel::Indexed(7));
    assert!(!es.ev(mov(5, 3)));
    assert_eq!(es.preview(), the_seven(Pixel::Rgba(Rgba::TRANSPARENT)));
    es.pixel = color_of_another_mode();
    assert!(es.ev(fin()));
    assert_eq!(changed(es.image_2(), &copy).len(), 7);
}

#[test]
fn a_click_without_movement_erases_a_pixel() {
    let copy = rgba_blue();
    let mut es = Scene::new(copy.clone());
    assert!(!es.ev(begin(2, 3)));
    assert_eq!(es.preview().len(), 1);
    assert!(es.ev(fin()));
    assert_eq!(changed(es.image_2(), &copy), set([(2, 3)]));
    assert_eq!(
        es.image_2().get_pixel(2, 3),
        Ok(Pixel::Rgba(Rgba::TRANSPARENT))
    );
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy);
}

#[test]
fn the_eraser_stroke_is_clipped_to_the_canvas() {
    let copy = rgba_blue();
    let mut es = Scene::new(copy.clone());
    assert!(!es.ev(begin(-3, 2)));
    assert!(!es.ev(mov(3, 2)));
    assert!(es.ev(fin()));
    assert_eq!(changed(es.image_2(), &copy), set((0..=3).map(|x| (x, 2))));
}

#[test]
fn a_stroke_that_leaves_and_returns_joins_the_ends_like_the_pencil() {
    // Mismas posiciones que el lápiz: la parte de fuera se recorta y no se «cierra» por dentro.
    let copy = rgba_blue();
    let mut eraser = Scene::new(copy.clone());
    let mut pencil = Scene::with(copy.clone(), Tool::Pencil);
    pencil.pixel = Some(Pixel::Rgba(Rgba::TRANSPARENT));
    for event in [begin(6, 2), mov(20, 2), mov(6, 4), fin()] {
        eraser.ev(event);
        pencil.ev(event);
        assert_eq!(eraser.preview(), pencil.preview(), "{event:?}");
    }
    assert_eq!(eraser.image_2(), pencil.image_2());
    assert_eq!(
        changed(eraser.image_2(), &copy),
        set([(6, 2), (7, 2), (7, 4), (6, 4)])
    );
}

#[test]
fn two_strokes_are_two_steps_and_an_undo_undoes_only_the_last() {
    let copy = rgba_blue();
    let mut es = Scene::new(copy.clone());
    es.ev(begin(0, 0));
    es.ev(mov(2, 0));
    assert!(es.ev(fin()));
    let first = es.image_2().clone();
    es.ev(begin(0, 5));
    es.ev(mov(2, 5));
    assert!(es.ev(fin()));
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &first);
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy);
}

// Fila 4.

#[test]
fn row_4a_an_already_transparent_zone_changes_nothing_or_creates_a_step() {
    // `(255, 0, 0, 0)` es lo habitual en PNG reales y no se limpia.
    let mut image = rgba_transparent();
    image
        .set_pixel(3, 2, Pixel::Rgba(rgba(255, 0, 0, 0)))
        .expect("dentro");
    let copy = image.clone();
    let mut es = Scene::new(image);
    let before = es.history();
    let state_id = es.doc.state_id();

    assert!(!es.ev(begin(2, 2)));
    assert!(!es.ev(mov(4, 2)));
    assert_eq!(
        es.preview().len(),
        3,
        "la vista previa muestra todo el trazo"
    );
    assert!(!es.ev(fin()), "Ok(false)");
    assert_eq!(es.image_2(), &copy);
    assert_eq!(
        es.image_2().get_pixel(3, 2),
        Ok(Pixel::Rgba(rgba(255, 0, 0, 0))),
        "el RGB oculto sigue siendo 255, 0, 0"
    );
    assert_eq!(es.history(), before);
    assert_eq!(es.doc.state_id(), state_id);
    assert!(!es.doc.can_undo());
}

#[test]
fn row_4a_gray_alpha_already_transparent_changes_nothing() {
    let mut image = Image::new(8, 8, Mode::GrayAlpha).expect("imagen válida");
    image
        .set_pixel(3, 2, Pixel::GrayAlpha { gray: 7, alpha: 0 })
        .expect("dentro");
    let copy = image.clone();
    let mut es = Scene::new(image);
    es.ev(begin(2, 2));
    es.ev(mov(4, 2));
    assert!(!es.ev(fin()));
    assert_eq!(es.image_2(), &copy);
    assert!(!es.doc.can_undo());
}

#[test]
fn row_4a_indexed_with_the_pixels_already_transparent_changes_nothing() {
    let mut image = indexed_with_transparent();
    for x in 2..=4 {
        image.set_pixel(x, 2, Pixel::Indexed(1)).expect("dentro");
    }
    let copy = image.clone();
    let mut es = Scene::new(image);
    es.ev(begin(2, 2));
    es.ev(mov(4, 2));
    assert!(!es.ev(fin()));
    assert_eq!(es.image_2(), &copy);
    assert!(!es.doc.can_undo());
}

#[test]
fn in_a_mixed_stroke_only_the_pixels_that_were_not_transparent_change() {
    // (2,2) opaco, (3,2) `(255, 0, 0, 0)`, (4,2) `(0, 0, 0, 0)`, (5,2) con alfa 1.
    let mut image = rgba_transparent();
    image
        .set_pixel(2, 2, Pixel::Rgba(Rgba::opaque(0, 0, 255)))
        .expect("dentro");
    image
        .set_pixel(3, 2, Pixel::Rgba(rgba(255, 0, 0, 0)))
        .expect("dentro");
    image
        .set_pixel(5, 2, Pixel::Rgba(rgba(9, 9, 9, 1)))
        .expect("dentro");
    let copy = image.clone();
    let mut es = Scene::new(image);
    es.ev(begin(2, 2));
    es.ev(mov(5, 2));
    assert!(es.ev(fin()), "cambian dos píxeles");
    assert_eq!(changed(es.image_2(), &copy), set([(2, 2), (5, 2)]));
    assert_eq!(
        es.image_2().get_pixel(3, 2),
        Ok(Pixel::Rgba(rgba(255, 0, 0, 0))),
        "el transparente con RGB oculto no se toca"
    );
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy);
    assert!(!es.doc.can_undo());
}

#[test]
fn row_4b_after_an_undo_the_eraser_without_effect_does_not_clear_the_redo() {
    let mut es = Scene::new(rgba_transparent());
    let copy = es.image_2().clone();
    // Un paso previo, deshecho: hay rehacer.
    let step = Command::SetPixels {
        image: es.id,
        pixels: vec![PixelEdit {
            x: 0,
            y: 0,
            pixel: Pixel::Rgba(Rgba::opaque(1, 2, 3)),
        }],
    };
    assert_eq!(es.doc.apply(step), Ok(true));
    assert_eq!(es.doc.undo(), Ok(()));
    assert!(es.doc.can_redo());
    assert_eq!(es.image_2(), &copy);
    let before = es.history();

    es.ev(begin(2, 2));
    es.ev(mov(4, 2));
    assert!(!es.ev(fin()));
    assert_eq!(es.image_2(), &copy);
    assert_eq!(es.history(), before);
    assert!(es.doc.can_redo(), "el rehacer sigue disponible");
}

/// La fila 4c: el `Begin` no abre gesto, avisa una sola vez y el gesto queda en espera.
fn row_4_c(image: Image, error: EraseError) {
    let copy = image.clone();
    let mut es = Scene::new(image);
    let before = es.history();
    let state_id = es.doc.state_id();
    assert_eq!(eraser_pixel(&copy), Err(error));

    assert!(!es.ev(begin(2, 2)), "Begin: Ok(false)");
    assert_eq!(
        es.tool.take_events(),
        vec![ToolEvent::Refused(error)],
        "un solo Refused, en el Begin"
    );
    assert!(es.tool.busy(), "busy desde el Begin");
    assert_eq!(es.preview(), vec![], "sin vista previa");

    assert!(!es.ev(mov(4, 2)));
    assert!(es.tool.busy(), "busy hasta el End");
    assert_eq!(es.preview(), vec![]);
    assert_eq!(es.tool.take_events(), vec![], "no se vuelve a avisar");

    assert!(!es.ev(fin()), "Ok(false)");
    assert!(!es.tool.busy());
    assert_eq!(es.tool.take_events(), vec![], "ni en el End");
    assert_eq!(es.image_2(), &copy);
    assert_eq!(es.history(), before);
    assert_eq!(es.doc.state_id(), state_id);
}

#[test]
fn row_4c_indexed_without_transparent_warns_and_changes_nothing() {
    row_4_c(
        indexed_without_transparent(),
        EraseError::NoTransparentIndex,
    );
}

#[test]
fn row_4c_gray_warns_and_changes_nothing() {
    row_4_c(gray_2(), EraseError::NoTransparency);
}

#[test]
fn row_4c_each_press_warns_once() {
    let mut es = Scene::new(gray_2());
    for _ in 0..3 {
        assert!(!es.ev(begin(2, 2)));
        assert!(!es.ev(mov(3, 2)));
        assert!(!es.ev(fin()));
        assert_eq!(
            es.tool.take_events(),
            vec![ToolEvent::Refused(EraseError::NoTransparency)]
        );
    }
}

#[test]
fn row_4c_a_begin_with_the_gesture_waiting_does_not_warn_again() {
    let mut es = Scene::new(gray_2());
    assert!(!es.ev(begin(2, 2)));
    assert_eq!(es.tool.take_events().len(), 1);
    // `Canvas` no emite dos `Begin` seguidos, pero si llegara, el gesto sigue siendo el mismo.
    assert!(!es.ev(begin(3, 3)));
    assert_eq!(es.tool.take_events(), vec![]);
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
}

#[test]
fn with_the_eraser_unable_the_tool_can_be_changed_after_the_end() {
    let mut es = Scene::new(gray_2());
    es.ev(begin(2, 2));
    es.tool.apply_action(ToolAction::Select(Tool::Pencil));
    assert_eq!(es.tool.tool(), Tool::Eraser, "busy hasta el End");
    es.ev(fin());
    es.tool.apply_action(ToolAction::Select(Tool::Pencil));
    assert_eq!(es.tool.tool(), Tool::Pencil);
}

#[test]
fn the_eraser_leaves_no_events_if_it_can_erase() {
    for image in [rgba_blue(), gray_alpha(), indexed_with_transparent()] {
        let mut es = Scene::new(image);
        es.ev(begin(1, 1));
        es.ev(mov(5, 1));
        es.ev(fin());
        assert_eq!(es.tool.take_events(), vec![]);
    }
}

// Estrategias de la propiedad 1.

/// Valores de un fondo con transparentes de todas clases: alfa 0 con RGB oculto, alfa 1, opaco.
fn pool(mode: Mode) -> Vec<Pixel> {
    match mode {
        Mode::Rgba => vec![
            Pixel::Rgba(rgba(10, 20, 30, 255)),
            Pixel::Rgba(rgba(255, 0, 0, 0)),
            Pixel::Rgba(rgba(0, 0, 0, 0)),
            Pixel::Rgba(rgba(9, 9, 9, 1)),
            Pixel::Rgba(rgba(40, 50, 60, 128)),
            Pixel::Rgba(rgba(0, 0, 0, 255)),
        ],
        Mode::GrayAlpha => vec![
            Pixel::GrayAlpha {
                gray: 5,
                alpha: 255,
            },
            Pixel::GrayAlpha { gray: 5, alpha: 0 },
            Pixel::GrayAlpha { gray: 0, alpha: 0 },
            Pixel::GrayAlpha { gray: 9, alpha: 1 },
            Pixel::GrayAlpha {
                gray: 7,
                alpha: 128,
            },
        ],
        Mode::Indexed => (0..6).map(Pixel::Indexed).collect(),
        Mode::Gray => vec![Pixel::Gray(1), Pixel::Gray(2)],
    }
}

/// Cualquier `i32`, con el interior de un lienzo de hasta 16 de lado y los extremos siempre
/// presentes.
fn coordinate() -> impl Strategy<Value = i32> {
    prop_oneof![
        6 => -2i32..=17,
        2 => any::<i32>(),
        1 => Just(MIN),
        1 => Just(MAX),
        1 => Just(MIN + 1),
        1 => Just(MAX - 1),
    ]
}

fn position() -> impl Strategy<Value = (i32, i32)> {
    (coordinate(), coordinate())
}

/// Un paso previo en la historia: `0` nada, `1` un paso, `2` un paso deshecho (hay rehacer).
fn step_previous(es: &mut Scene, variant: u8) {
    if variant == 0 {
        return;
    }
    let actual = es.image_2().get_pixel(0, 0).unwrap();
    let other_2 = pool(es.image_2().mode())
        .into_iter()
        .find(|&p| p != actual)
        .expect("el fondo tiene al menos dos valores");
    let command = Command::SetPixels {
        image: es.id,
        pixels: vec![PixelEdit {
            x: 0,
            y: 0,
            pixel: other_2,
        }],
    };
    assert_eq!(es.doc.apply(command), Ok(true));
    if variant == 2 {
        assert_eq!(es.doc.undo(), Ok(()));
    }
}

// Fila 15: borrar y omitir con la regla de la entrada de alfa 0.
//
// Paleta de 5: `[blue, red, (0,255,0,α0), blue light, (9,9,9,α0)]` (la de la fila 14). Fila de 6
// píxeles `(0,0)…(5,0)` con `Indexed(0)`, `(2)`, `(4)`, `(1)`, `(3)`, `(0)`; el resto, `Indexed(0)`.

fn palette_row_15(transparent: Option<u8>) -> Palette {
    let mut palette = Palette::from_colors([
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(255, 0, 0),
        rgba(0, 255, 0, 0),
        Rgba::opaque(100, 150, 255),
        rgba(9, 9, 9, 0),
    ])
    .expect("paleta válida");
    palette
        .set_transparent_index(transparent)
        .expect("el índice existe");
    palette
}

fn image_row_15(transparent: Option<u8>) -> Image {
    let mut image = Image::new_indexed(8, 8, palette_row_15(transparent)).expect("imagen válida");
    for (x, index_2) in [0u8, 2, 4, 1, 3, 0].into_iter().enumerate() {
        image
            .set_pixel(u32::try_from(x).unwrap(), 0, Pixel::Indexed(index_2))
            .expect("dentro");
    }
    image
}

/// Las seis posiciones del trazo `(0,0)…(5,0)`, con `pixel`.
fn the_row(pixel: Pixel) -> Vec<PixelEdit> {
    (0..6).map(|x| PixelEdit { x, y: 0, pixel }).collect()
}

/// Un paso previo, deshecho: hay rehacer y la imagen es la de antes.
fn with_a_previous_undo(es: &mut Scene) {
    let copy = es.image_2().clone();
    let step = Command::SetPixels {
        image: es.id,
        pixels: vec![PixelEdit {
            x: 7,
            y: 7,
            pixel: Pixel::Indexed(1),
        }],
    };
    assert_eq!(es.doc.apply(step), Ok(true));
    assert_eq!(es.doc.undo(), Ok(()));
    assert!(es.doc.can_redo());
    assert_eq!(es.image_2(), &copy);
}

#[test]
fn row_15a_without_transparent_index_it_erases_to_entry_2_and_skips_the_alpha_0_ones() {
    let copy = image_row_15(None);
    let mut es = Scene::new(copy.clone());
    let state_id = es.doc.state_id();

    // Antes del `End`: el documento no cambia y la vista previa es el trazo entero.
    assert!(!es.ev(begin(0, 0)));
    assert!(!es.ev(mov(5, 0)));
    assert_eq!(es.preview(), the_row(Pixel::Indexed(2)), "trazo entero");
    assert_eq!(es.image_2(), &copy);
    assert_eq!(es.doc.state_id(), state_id);
    assert!(!es.doc.can_undo());
    assert!(es.tool.take_events().is_empty(), "no hay Refused");

    assert!(es.ev(fin()), "Ok(true)");
    assert_eq!(
        changed(es.image_2(), &copy),
        set([(0, 0), (3, 0), (4, 0), (5, 0)])
    );
    for x in [0, 3, 4, 5] {
        assert_eq!(
            es.image_2().get_pixel(x, 0),
            Ok(Pixel::Indexed(2)),
            "({x}, 0)"
        );
    }
    // Las entradas 2 y 4 (alfa 0) no se tocan: el `Indexed(4)` no pasa a 2.
    assert_eq!(es.image_2().get_pixel(1, 0), Ok(Pixel::Indexed(2)));
    assert_eq!(es.image_2().get_pixel(2, 0), Ok(Pixel::Indexed(4)));
    assert!(es.tool.take_events().is_empty());

    // Un solo paso.
    let edited = es.image_2().clone();
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy);
    assert!(!es.doc.can_undo(), "un solo paso");
    assert_eq!(es.doc.redo(), Ok(()));
    assert_eq!(es.image_2(), &edited);
}

#[test]
fn row_15b_with_transparent_index_3_it_erases_to_3_and_skips_alpha_0_and_the_3_itself() {
    let copy = image_row_15(Some(3));
    let mut es = Scene::new(copy.clone());
    let state_id = es.doc.state_id();

    assert!(!es.ev(begin(0, 0)));
    assert!(!es.ev(mov(5, 0)));
    assert_eq!(es.preview(), the_row(Pixel::Indexed(3)), "trazo entero");
    assert_eq!(es.image_2(), &copy);
    assert_eq!(es.doc.state_id(), state_id);
    assert!(!es.doc.can_undo());

    assert!(es.ev(fin()), "Ok(true)");
    // (1,0) es la entrada 2 y (2,0) la 4 (alfa 0 guardado); (4,0) ya es `Indexed(3)`.
    assert_eq!(changed(es.image_2(), &copy), set([(0, 0), (3, 0), (5, 0)]));
    for x in [0, 3, 5] {
        assert_eq!(
            es.image_2().get_pixel(x, 0),
            Ok(Pixel::Indexed(3)),
            "({x}, 0)"
        );
    }
    assert_eq!(es.image_2().get_pixel(1, 0), Ok(Pixel::Indexed(2)));
    assert_eq!(es.image_2().get_pixel(2, 0), Ok(Pixel::Indexed(4)));
    assert_eq!(es.image_2().get_pixel(4, 0), Ok(Pixel::Indexed(3)));

    let edited = es.image_2().clone();
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.image_2(), &copy);
    assert!(!es.doc.can_undo(), "un solo paso");
    assert_eq!(es.doc.redo(), Ok(()));
    assert_eq!(es.image_2(), &edited);
}

#[test]
fn row_15c_a_stroke_only_over_alpha_0_changes_nothing_creates_no_step_and_does_not_warn() {
    let copy = image_row_15(None);
    let mut es = Scene::new(copy.clone());
    let before = es.history();
    let state_id = es.doc.state_id();

    assert!(!es.ev(begin(1, 0)));
    assert!(!es.ev(mov(2, 0)));
    assert_eq!(
        es.preview(),
        vec![
            PixelEdit {
                x: 1,
                y: 0,
                pixel: Pixel::Indexed(2)
            },
            PixelEdit {
                x: 2,
                y: 0,
                pixel: Pixel::Indexed(2)
            },
        ],
        "la vista previa muestra todo el trazo"
    );
    assert!(!es.ev(fin()), "Ok(false)");
    assert!(!es.tool.busy());
    assert_eq!(es.image_2(), &copy);
    assert_eq!(es.history(), before);
    assert_eq!(es.doc.state_id(), state_id);
    assert!(!es.doc.can_undo());
    assert_eq!(es.tool.take_events(), vec![], "sin Refused");
}

#[test]
fn row_15c_with_transparent_index_the_alpha_0_ones_are_also_skipped_without_warning() {
    let copy = image_row_15(Some(3));
    let mut es = Scene::new(copy.clone());
    let before = es.history();
    // (1,0) entrada 2, (2,0) entrada 4 y (4,0) `Indexed(3)`: los tres ya son transparentes.
    for (a, b) in [((1, 0), (2, 0)), ((4, 0), (4, 0))] {
        es.ev(begin(a.0, a.1));
        es.ev(mov(b.0, b.1));
        assert!(!es.ev(fin()), "{a:?}-{b:?}");
        assert_eq!(es.image_2(), &copy);
        assert_eq!(es.history(), before);
        assert_eq!(es.tool.take_events(), vec![]);
    }
}

#[test]
fn row_15d_with_a_previous_undo_the_stroke_without_effect_does_not_clear_the_redo() {
    let mut es = Scene::new(image_row_15(None));
    with_a_previous_undo(&mut es);
    let copy = es.image_2().clone();
    let before = es.history();

    es.ev(begin(1, 0));
    es.ev(mov(2, 0));
    assert!(!es.ev(fin()), "Ok(false)");
    assert_eq!(es.image_2(), &copy);
    assert_eq!(es.history(), before);
    assert!(es.doc.can_redo(), "el rehacer sigue disponible");
    assert_eq!(es.tool.take_events(), vec![], "sin Refused");
}

#[test]
fn row_15_the_value_is_recomputed_on_each_press() {
    // Tras editar la paleta (la entrada 2 pasa a opaca) el siguiente trazo escribe la entrada 4.
    let mut es = Scene::new(image_row_15(None));
    let step = Command::SetPaletteColor {
        image: es.id,
        index: 2,
        color: Rgba::opaque(0, 255, 0),
    };
    assert_eq!(es.doc.apply(step), Ok(true));

    es.ev(begin(0, 0));
    assert_eq!(
        es.preview(),
        vec![PixelEdit {
            x: 0,
            y: 0,
            pixel: Pixel::Indexed(4)
        }]
    );
    es.ev(fin());
    assert_eq!(es.image_2().get_pixel(0, 0), Ok(Pixel::Indexed(4)));
    // La entrada 2 ya es opaca: el `Indexed(2)` de (1,0) se borra; el `Indexed(4)` de (2,0), no.
    es.ev(begin(1, 0));
    es.ev(mov(2, 0));
    assert!(es.ev(fin()));
    assert_eq!(es.image_2().get_pixel(1, 0), Ok(Pixel::Indexed(4)));
    assert_eq!(es.image_2().get_pixel(2, 0), Ok(Pixel::Indexed(4)));
}

// Propiedad 1: el borrador es un lápiz transparente, en RGBA, GrayAlpha e indexada con o sin
// `transparent_index` y con entradas de alfa 0 (o sin ellas). El oráculo de «ya transparente» es el
// valor resuelto con alfa 0 (`pixel_rgba`), no el índice; el valor de borrado se calcula aparte de
// `eraser_pixel`, con las entradas guardadas.

/// Paleta de seis entradas: las de `mask` (bit `i` para la entrada `i`) con alfa guardado 0 y
/// el resto opacas; `transparent` fijado o no. Sin `transparent`, como mínimo la entrada `k`
/// lleva alfa 0 (se fuerza).
fn palette_varied(transparent: Option<u8>, mask: u8, k: u8) -> Palette {
    let mask = if transparent.is_none() {
        mask | (1 << k)
    } else {
        mask
    };
    let mut palette = Palette::from_colors((0..6u8).map(|i| {
        let alpha = if mask & (1 << i) != 0 { 0 } else { 255 };
        rgba(10 * i, 20, 30, alpha)
    }))
    .expect("paleta válida");
    palette.set_transparent_index(transparent).expect("existe");
    palette
}

/// Parámetros de una paleta indexada: `transparent_index` (o no), máscara de alfa 0 y `k`.
fn palette_indexed() -> impl Strategy<Value = Palette> {
    (
        prop_oneof![
            1 => Just(None),
            2 => (0u8..6).prop_map(Some),
        ],
        prop_oneof![
            // Mitad de los casos: ninguna entrada extra de alfa 0, la propiedad de antes.
            1 => Just(0u8),
            3 => 0u8..64,
        ],
        0u8..6,
    )
        .prop_map(|(transparent, mask, k)| palette_varied(transparent, mask, k))
}

/// Imagen de 1 a 16 de lado en uno de los tres modos con borrado, con fondo variado; en indexada,
/// con la paleta de `palette_indexed` (los seis índices aparecen en el fondo).
fn images() -> impl Strategy<Value = Image> {
    (
        prop_oneof![
            1 => Just(Mode::Rgba),
            1 => Just(Mode::GrayAlpha),
            4 => Just(Mode::Indexed),
        ],
        1u32..=16,
        1u32..=16,
        palette_indexed(),
    )
        .prop_flat_map(|(mode, w, h, palette)| {
            let cels = usize::try_from(w * h).unwrap();
            (
                Just((mode, w, h, palette)),
                prop::collection::vec(any::<u8>(), cels),
            )
        })
        .prop_map(|((mode, w, h, palette), cels)| {
            let pool = pool(mode);
            let mut image = match mode {
                Mode::Indexed => Image::new_indexed(w, h, palette).unwrap(),
                _ => Image::new(w, h, mode).unwrap(),
            };
            for y in 0..h {
                for x in 0..w {
                    let byte = cels[usize::try_from(y * w + x).unwrap()];
                    // Una parte de las casillas valen el primer valor del fondo.
                    let k = if byte < 64 {
                        0
                    } else {
                        usize::from(byte) % pool.len()
                    };
                    image.set_pixel(x, y, pool[k]).unwrap();
                }
            }
            image
        })
}

/// Lo que debe escribir el borrador, escrito aparte de `eraser_pixel` y sin `Palette::get`: el
/// índice transparente si lo hay; si no, la primera entrada de `colors()` con alfa 0. `None` si el
/// borrador no puede con la imagen (indexada sin ninguna de las dos cosas).
fn deleted_expected(image: &Image) -> Option<Pixel> {
    match image.mode() {
        Mode::Rgba => Some(Pixel::Rgba(Rgba::TRANSPARENT)),
        Mode::GrayAlpha => Some(Pixel::GrayAlpha { gray: 0, alpha: 0 }),
        Mode::Indexed => {
            let palette = image.palette().expect("indexada con paleta");
            palette
                .transparent_index()
                .or_else(|| {
                    palette
                        .colors()
                        .iter()
                        .position(|color| color.a == 0)
                        .map(|i| u8::try_from(i).unwrap())
                })
                .map(Pixel::Indexed)
        }
        Mode::Gray => unreachable!("la propiedad no usa Gray"),
    }
}

/// El píxel ya es transparente: su valor resuelto tiene alfa 0, sea cual sea su índice.
fn already_is_transparent(image: &Image, pixel: Pixel) -> bool {
    pixel_rgba(image, pixel).expect("el píxel se ve").a == 0
}

fn checks_property_1(
    image: Image,
    previous: u8,
    a: (i32, i32),
    movements: &[(i32, i32)],
) -> Result<(), TestCaseError> {
    let deleted = deleted_expected(&image);
    // Las imágenes de esta propiedad siempre admiten el borrado: la paleta lleva índice
    // transparente o una entrada de alfa 0.
    let deleted = deleted.expect("la imagen admite el borrado");
    prop_assert_eq!(eraser_pixel(&image), Ok(deleted));

    let mut eraser = Scene::new(image.clone());
    let mut pencil = Scene::with(image, Tool::Pencil);
    pencil.pixel = Some(deleted);
    step_previous(&mut eraser, previous);
    step_previous(&mut pencil, previous);
    let original = eraser.image_2().clone();
    let history_0 = eraser.history();
    let state_id0 = eraser.doc.state_id();

    let mut events_2 = vec![begin(a.0, a.1)];
    events_2.extend(movements.iter().map(|&(x, y)| mov(x, y)));
    for event in events_2 {
        prop_assert!(!eraser.ev(event), "{:?} no cambia el documento", event);
        prop_assert!(!pencil.ev(event));
        // Mismas posiciones que el lápiz con `eraser_pixel`, todas con el valor de borrado.
        let view = eraser.preview();
        prop_assert_eq!(&view, &pencil.preview(), "{:?}", event);
        for editing in &view {
            prop_assert_eq!(editing.pixel, deleted);
        }
        // El documento no cambia antes del `End`.
        prop_assert_eq!(eraser.image_2(), &original);
        prop_assert_eq!(eraser.history(), history_0);
        prop_assert_eq!(eraser.doc.state_id(), state_id0);
        prop_assert!(eraser.tool.busy());
        prop_assert!(eraser.tool.take_events().is_empty());
    }

    let visited: Vec<(u32, u32)> = eraser.preview().iter().map(|e| (e.x, e.y)).collect();
    let mut expected = original.clone();
    for &(x, y) in &visited {
        if !already_is_transparent(&original, original.get_pixel(x, y).unwrap()) {
            expected.set_pixel(x, y, deleted).unwrap();
        }
    }

    let change = eraser.ev(fin());
    pencil.ev(fin());
    prop_assert!(!eraser.tool.busy());
    prop_assert!(eraser.preview().is_empty());
    prop_assert!(eraser.tool.take_events().is_empty(), "sin Refused");
    prop_assert_eq!(
        eraser.image_2(),
        &expected,
        "los que no eran transparentes, borrados"
    );
    prop_assert_eq!(
        change,
        expected != original,
        "Ok(true) si y solo si alguno cambió"
    );
    if change {
        // Es un paso y `undo` deja la imagen exacta.
        prop_assert!(eraser.doc.can_undo());
        prop_assert_eq!(eraser.doc.undo(), Ok(()));
        prop_assert_eq!(eraser.image_2(), &original);
        prop_assert_eq!(eraser.doc.can_undo(), history_0.0, "un solo paso");
        prop_assert_eq!(eraser.doc.redo(), Ok(()));
        prop_assert_eq!(eraser.image_2(), &expected);
    } else {
        prop_assert_eq!(
            eraser.history(),
            history_0,
            "sin paso y sin tocar el rehacer"
        );
        prop_assert_eq!(eraser.doc.state_id(), state_id0);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn property_1_the_eraser_is_a_transparent_pencil(
        image in images(),
        previous in 0u8..=2,
        a in position(),
        movements in prop::collection::vec(position(), 0..=4),
    ) {
        checks_property_1(image, previous, a, &movements)?;
    }
}

/// Un fondo de las paletas de la propiedad 1: cubre a mano lo que el azar puede tardar en dar.
#[test]
fn property_1_example_without_transparent_index_with_entry_1_of_alpha_0() {
    let palette = palette_varied(None, 0b000010, 1);
    let mut image = Image::new_indexed(4, 1, palette).expect("imagen válida");
    for (x, i) in [0u8, 1, 2, 5].into_iter().enumerate() {
        image
            .set_pixel(u32::try_from(x).unwrap(), 0, Pixel::Indexed(i))
            .unwrap();
    }
    // El valor de borrado es `Indexed(1)`; el píxel `Indexed(1)` se omite y los demás pasan a 1.
    let mut expected = image.clone();
    for x in [0, 2, 3] {
        expected.set_pixel(x, 0, Pixel::Indexed(1)).unwrap();
    }
    let mut es = Scene::new(image);
    es.ev(begin(0, 0));
    es.ev(mov(3, 0));
    assert!(es.ev(fin()));
    assert_eq!(es.image_2(), &expected);
}
