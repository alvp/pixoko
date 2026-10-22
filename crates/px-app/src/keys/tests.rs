//! Tests de `keys::select_tool` (specs/hito-2-cuentagotas-borrador.md): fila 11 (aviso al elegir
//! el borrador) y fila 13 (`Select` con `App::busy()` completo).
//!
//! `busy` es el parámetro que `main.rs` calcula con `App::busy()`; aquí lo dan a mano el test (un
//! diálogo) o `Session::busy()` (deslizador del editor o arrastre de muestra). Un gesto de la
//! herramienta lo cubre `ToolState::busy()` por su cuenta.

use super::select_tool;
use px_core::ImageId;

use px_core::{Document, Image, Mode, Palette, Rgba};
use px_ui::canvas::CanvasEvent;
use px_ui::color_edit::Control;
use px_ui::pick::{EraseError, eraser_pixel};
use px_ui::tool::{Tool, ToolAction, ToolEvent, ToolState};

use crate::session::Session;

// ---------------------------------------------------------------------------------------
// Ayudantes.
// ---------------------------------------------------------------------------------------

const GRAY_NOTICE: &str = "el modo gris no tiene transparencia";
const NO_INDEX_NOTICE: &str = "esta imagen no tiene índice transparente";

fn distinct(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect()
}

fn rgba() -> Image {
    Image::new(4, 4, Mode::Rgba).expect("imagen RGBA")
}

fn gray() -> Image {
    Image::new(4, 4, Mode::Gray).expect("imagen Gray")
}

fn gray_alpha() -> Image {
    Image::new(4, 4, Mode::GrayAlpha).expect("imagen GrayAlpha")
}

/// Indexada de 4 entradas con el índice 3 transparente, o sin índice transparente.
fn indexed(transparent: Option<u8>) -> Image {
    let mut palette = Palette::from_colors(distinct(4)).expect("paleta");
    palette
        .set_transparent_index(transparent)
        .expect("índice transparente");
    Image::new_indexed(4, 4, palette).expect("imagen indexada")
}

fn tools_with(tool: Tool) -> ToolState {
    let mut tools = ToolState::new();
    tools.apply_action(ToolAction::Select(tool));
    assert_eq!(tools.tool(), tool);
    tools
}

fn select(tools: &mut ToolState, tool: Tool, busy: bool, image: &Image) -> Option<String> {
    select_tool(tools, ToolAction::Select(tool), busy, Some(image))
}

fn session_of(image: Image) -> Session {
    Session::sample(Document::new(image))
}

/// Deja el deslizador rojo del editor pulsado (la sesión debe tener un destino que editar).
fn press_slider(session: &mut Session) {
    session.sync_editor();
    assert!(
        session.editor.begin(Control::Red, 10),
        "el editor admite el arrastre"
    );
}

/// Empieza un gesto de la herramienta de `tools` sobre `doc` (sin soltar).
fn begin_stroke(tools: &mut ToolState, doc: &mut Document, pixel: (i32, i32)) {
    let id = ImageId::from_raw(0);
    tools
        .canvas_event_with(doc, id, None, CanvasEvent::StrokeBegin(pixel))
        .expect("pulsar");
    assert!(tools.busy(), "el gesto está en curso");
    tools.take_events();
}

fn end_stroke(tools: &mut ToolState, doc: &mut Document) {
    let id = ImageId::from_raw(0);
    tools
        .canvas_event_with(doc, id, None, CanvasEvent::StrokeEnd)
        .expect("soltar");
    assert!(!tools.busy());
}

// ---------------------------------------------------------------------------------------
// Fila 11: aviso al elegir el borrador.
// ---------------------------------------------------------------------------------------

#[test]
fn row_11_eraser_with_gray_chooses_and_warns() {
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, false, &gray());

    assert_eq!(tools.tool(), Tool::Eraser, "queda elegida igualmente");
    assert_eq!(notice.as_deref(), Some(GRAY_NOTICE));
}

#[test]
fn row_11_eraser_with_indexed_without_transparent_chooses_and_warns() {
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, false, &indexed(None));

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice.as_deref(), Some(NO_INDEX_NOTICE));
}

#[test]
fn row_11_eraser_with_an_image_that_allows_erasing_does_not_warn() {
    for (name, image) in [
        ("RGBA", rgba()),
        ("GrayAlpha", gray_alpha()),
        ("indexada con transparente", indexed(Some(3))),
    ] {
        let mut tools = ToolState::new();

        let notice = select(&mut tools, Tool::Eraser, false, &image);

        assert_eq!(tools.tool(), Tool::Eraser, "{name}");
        assert_eq!(notice, None, "{name}");
    }
}

#[test]
fn row_11_the_texts_are_those_of_erase_error() {
    assert_eq!(
        eraser_pixel(&gray()).unwrap_err().to_string(),
        GRAY_NOTICE,
        "Gray"
    );
    assert_eq!(
        eraser_pixel(&indexed(None)).unwrap_err(),
        EraseError::NoTransparentIndex
    );
    let mut tools = ToolState::new();
    assert_eq!(
        select(&mut tools, Tool::Eraser, false, &gray()),
        Some(EraseError::NoTransparency.to_string())
    );
    let mut tools = ToolState::new();
    assert_eq!(
        select(&mut tools, Tool::Eraser, false, &indexed(None)),
        Some(EraseError::NoTransparentIndex.to_string())
    );
}

#[test]
fn row_11_repeating_e_with_the_eraser_already_chosen_repeats_the_notice() {
    let mut tools = tools_with(Tool::Eraser);
    let image = gray();

    let first = select(&mut tools, Tool::Eraser, false, &image);
    let second = select(&mut tools, Tool::Eraser, false, &image);

    assert_eq!(first.as_deref(), Some(GRAY_NOTICE));
    assert_eq!(second, first);
    assert_eq!(tools.tool(), Tool::Eraser);
}

#[test]
fn row_11_going_back_to_the_eraser_after_another_tool_warns_again() {
    let mut tools = ToolState::new();
    let image = indexed(None);

    assert_eq!(
        select(&mut tools, Tool::Eraser, false, &image).as_deref(),
        Some(NO_INDEX_NOTICE)
    );
    assert_eq!(select(&mut tools, Tool::Pencil, false, &image), None);
    assert_eq!(
        select(&mut tools, Tool::Eraser, false, &image).as_deref(),
        Some(NO_INDEX_NOTICE)
    );
}

#[test]
fn row_11_with_another_image_the_notice_is_the_corresponding_one() {
    let mut tools = tools_with(Tool::Eraser);

    assert_eq!(
        select(&mut tools, Tool::Eraser, false, &gray()).as_deref(),
        Some(GRAY_NOTICE)
    );
    assert_eq!(select(&mut tools, Tool::Eraser, false, &rgba()), None);
    assert_eq!(
        select(&mut tools, Tool::Eraser, false, &indexed(None)).as_deref(),
        Some(NO_INDEX_NOTICE)
    );
}

#[test]
fn row_11_the_other_tools_with_gray_do_not_warn() {
    for tool in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Dropper] {
        let mut tools = tools_with(Tool::Eraser);

        let notice = select(&mut tools, tool, false, &gray());

        assert_eq!(tools.tool(), tool);
        assert_eq!(notice, None, "{tool:?}");
    }
    let mut tools = ToolState::new();
    assert_eq!(
        select(&mut tools, Tool::Pencil, false, &indexed(None)),
        None
    );
}

#[test]
fn row_11_cancel_changes_neither_the_tool_nor_warns() {
    let mut tools = tools_with(Tool::Eraser);

    let notice = select_tool(&mut tools, ToolAction::Cancel, false, Some(&gray()));

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice, None);
}

#[test]
fn row_11_with_busy_true_it_changes_neither_the_tool_nor_warns() {
    for image in [gray(), indexed(None), rgba()] {
        let mut tools = ToolState::new();

        let notice = select(&mut tools, Tool::Eraser, true, &image);

        assert_eq!(tools.tool(), Tool::Pencil, "{:?}", image.mode());
        assert_eq!(notice, None, "{:?}", image.mode());
    }
}

#[test]
fn row_11_with_busy_true_and_the_eraser_already_chosen_it_does_not_repeat_the_notice_either() {
    let mut tools = tools_with(Tool::Eraser);

    let notice = select(&mut tools, Tool::Eraser, true, &gray());

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice, None);
}

#[test]
fn row_11_without_an_image_it_chooses_the_tool_without_a_notice() {
    let mut tools = ToolState::new();

    let notice = select_tool(&mut tools, ToolAction::Select(Tool::Eraser), false, None);

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice, None);
}

#[test]
fn row_11_with_an_eraser_gesture_in_progress_it_changes_nothing_and_does_not_warn() {
    let mut doc = Document::new(gray());
    let mut tools = tools_with(Tool::Eraser);
    begin_stroke(&mut tools, &mut doc, (1, 1));
    let image = gray();

    let eraser_again = select(&mut tools, Tool::Eraser, false, &image);
    let pencil = select(&mut tools, Tool::Pencil, false, &image);

    assert_eq!(eraser_again, None, "ni repite el aviso");
    assert_eq!(pencil, None);
    assert_eq!(tools.tool(), Tool::Eraser);
}

// ---------------------------------------------------------------------------------------
// Fila 13: `Select` con `App::busy()` completo.
// ---------------------------------------------------------------------------------------

#[test]
fn row_13_session_busy_is_true_during_the_slider_drag_and_false_on_release() {
    let mut session = session_of(rgba());
    assert!(!session.busy());

    press_slider(&mut session);
    assert!(session.busy());
    session.editor.drag(200);
    assert!(session.busy());

    session.editor.release();
    assert!(!session.busy());
}

#[test]
fn row_13_session_busy_with_a_real_swatch_drag() {
    let mut session =
        session_of(Image::new_indexed(1, 1, Palette::from_colors(distinct(6)).unwrap()).unwrap());
    assert!(!session.busy());

    session.ops.begin_drag(0, (0, 0));
    assert!(!session.busy(), "pulsar la muestra aún es un clic");
    session
        .ops
        .drag()
        .expect("hay arrastre")
        .update((1000, 1000), 1, Some(3));
    assert!(session.busy());

    session.ops.end_drag();
    assert!(!session.busy());
}

#[test]
fn row_13_with_the_slider_pressed_dropper_to_pencil_does_not_change_and_after_releasing_it_does() {
    let mut session = session_of(rgba());
    let mut tools = tools_with(Tool::Dropper);
    press_slider(&mut session);

    let blocked = select(&mut tools, Tool::Pencil, session.busy(), &rgba());
    assert_eq!(tools.tool(), Tool::Dropper);
    assert_eq!(blocked, None);

    session.editor.release();
    let free = select(&mut tools, Tool::Pencil, session.busy(), &rgba());
    assert_eq!(tools.tool(), Tool::Pencil);
    assert_eq!(free, None);
}

#[test]
fn row_13_with_the_slider_pressed_pencil_to_eraser_does_not_change_or_warn_and_after_releasing_it_does()
 {
    let image = indexed(None);
    let mut session = session_of(image.clone());
    let mut tools = tools_with(Tool::Pencil);
    press_slider(&mut session);

    let blocked = select(&mut tools, Tool::Eraser, session.busy(), &image);
    assert_eq!(tools.tool(), Tool::Pencil);
    assert_eq!(blocked, None, "sin aviso mientras no cambia");

    session.editor.release();
    let free = select(&mut tools, Tool::Eraser, session.busy(), &image);
    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(free.as_deref(), Some(NO_INDEX_NOTICE));
}

#[test]
fn row_13_with_a_swatch_drag_pencil_to_eraser_does_not_change_and_after_releasing_it_does() {
    let image = indexed(None);
    let mut session = session_of(image.clone());
    let mut tools = tools_with(Tool::Pencil);
    session.ops.begin_drag(0, (0, 0));
    session
        .ops
        .drag()
        .expect("hay arrastre")
        .update((1000, 1000), 1, Some(2));

    let blocked = select(&mut tools, Tool::Eraser, session.busy(), &image);
    assert_eq!(tools.tool(), Tool::Pencil);
    assert_eq!(blocked, None);

    session.ops.end_drag();
    let free = select(&mut tools, Tool::Eraser, session.busy(), &image);
    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(free.as_deref(), Some(NO_INDEX_NOTICE));
}

#[test]
fn row_13_with_a_stroke_in_progress_dropper_to_pencil_does_not_change_and_after_releasing_it_does()
{
    let mut doc = Document::new(rgba());
    let mut tools = tools_with(Tool::Dropper);
    begin_stroke(&mut tools, &mut doc, (1, 1));

    let blocked = select(&mut tools, Tool::Pencil, false, &rgba());
    assert_eq!(tools.tool(), Tool::Dropper);
    assert_eq!(blocked, None);

    end_stroke(&mut tools, &mut doc);
    let free = select(&mut tools, Tool::Pencil, false, &rgba());
    assert_eq!(tools.tool(), Tool::Pencil);
    assert_eq!(free, None);
}

#[test]
fn row_13_with_a_stroke_in_progress_pencil_to_eraser_does_not_change_or_warn_and_after_releasing_it_does()
 {
    let image = indexed(None);
    let mut doc = Document::new(image.clone());
    let mut tools = tools_with(Tool::Pencil);
    begin_stroke(&mut tools, &mut doc, (1, 1));

    let blocked = select(&mut tools, Tool::Eraser, false, &image);
    assert_eq!(tools.tool(), Tool::Pencil);
    assert_eq!(blocked, None);

    end_stroke(&mut tools, &mut doc);
    let free = select(&mut tools, Tool::Eraser, false, &image);
    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(free.as_deref(), Some(NO_INDEX_NOTICE));
}

#[test]
fn row_13_with_busy_true_because_of_a_dialog_dropper_to_pencil_does_not_change() {
    let mut tools = tools_with(Tool::Dropper);

    let blocked = select(&mut tools, Tool::Pencil, true, &rgba());
    assert_eq!(tools.tool(), Tool::Dropper);
    assert_eq!(blocked, None);

    let free = select(&mut tools, Tool::Pencil, false, &rgba());
    assert_eq!(tools.tool(), Tool::Pencil);
    assert_eq!(free, None);
}

#[test]
fn row_13_with_busy_true_because_of_a_dialog_pencil_to_eraser_does_not_change_or_warn() {
    let image = gray();
    let mut tools = tools_with(Tool::Pencil);

    let blocked = select(&mut tools, Tool::Eraser, true, &image);
    assert_eq!(tools.tool(), Tool::Pencil);
    assert_eq!(blocked, None);

    let free = select(&mut tools, Tool::Eraser, false, &image);
    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(free.as_deref(), Some(GRAY_NOTICE));
}

#[test]
fn row_13_with_several_causes_at_once_one_is_enough_to_block() {
    let image = indexed(None);
    let mut doc = Document::new(image.clone());
    let mut session = session_of(image.clone());
    let mut tools = tools_with(Tool::Pencil);
    press_slider(&mut session);
    begin_stroke(&mut tools, &mut doc, (0, 0));

    assert_eq!(select(&mut tools, Tool::Eraser, true, &image), None);
    assert_eq!(tools.tool(), Tool::Pencil);

    session.editor.release();
    assert_eq!(
        select(&mut tools, Tool::Eraser, session.busy(), &image),
        None
    );
    assert_eq!(tools.tool(), Tool::Pencil, "el trazo sigue en curso");

    end_stroke(&mut tools, &mut doc);
    assert_eq!(
        select(&mut tools, Tool::Eraser, session.busy(), &image).as_deref(),
        Some(NO_INDEX_NOTICE)
    );
    assert_eq!(tools.tool(), Tool::Eraser);
}

// ---------------------------------------------------------------------------------------
// Fila 16: la regla de la entrada de alfa 0 en el aviso de elegir el borrador.
// ---------------------------------------------------------------------------------------

/// La paleta de la fila 14: `[blue, rojo, (0,255,0,α0), blue claro, (9,9,9,α0)]`, sin
/// `transparent_index`; `alpha_2` y `alpha_4` son el alfa guardado de las entradas 2 y 4.
fn indexed_with_alpha_zero_entries(alpha_2: u8, alpha_4: u8) -> Image {
    let palette = Palette::from_colors([
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(255, 0, 0),
        Rgba::new(0, 255, 0, alpha_2),
        Rgba::opaque(100, 150, 255),
        Rgba::new(9, 9, 9, alpha_4),
    ])
    .expect("paleta");
    Image::new_indexed(8, 8, palette).expect("imagen indexada")
}

/// Caso 14(a): sin `transparent_index`, con las entradas 2 y 4 de alfa 0.
fn case_14a() -> Image {
    indexed_with_alpha_zero_entries(0, 0)
}

/// Caso 14(d): sin `transparent_index` y con solo la entrada 4 de alfa 0.
fn case_14d() -> Image {
    indexed_with_alpha_zero_entries(255, 0)
}

#[test]
fn row_16_eraser_with_case_14a_chooses_and_does_not_warn() {
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, false, &case_14a());

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice, None, "hay una entrada de alfa 0: no avisa");
}

#[test]
fn row_16_eraser_with_case_14d_chooses_and_does_not_warn() {
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, false, &case_14d());

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice, None, "solo la entrada 4 es de alfa 0: no avisa");
}

#[test]
fn row_16_repeating_e_with_an_alpha_0_entry_does_not_warn_either_time() {
    let mut tools = tools_with(Tool::Eraser);
    let image = case_14a();

    assert_eq!(select(&mut tools, Tool::Eraser, false, &image), None);
    assert_eq!(select(&mut tools, Tool::Eraser, false, &image), None);
    assert_eq!(tools.tool(), Tool::Eraser);
}

#[test]
fn row_16_with_the_alpha_0_entries_turned_opaque_it_warns() {
    // Los casos 14(a) y 14(d) con todas sus entradas de alfa 0 pasadas a alfa 255.
    let image = indexed_with_alpha_zero_entries(255, 255);
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, false, &image);

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice.as_deref(), Some(NO_INDEX_NOTICE));
}

#[test]
fn row_16_a_single_entry_going_from_alpha_0_to_opaque_changes_the_notice() {
    // Se calcula al elegir, no se guarda: la misma paleta, antes y después de editarla.
    let mut tools = ToolState::new();

    assert_eq!(select(&mut tools, Tool::Eraser, false, &case_14d()), None);
    let edited = indexed_with_alpha_zero_entries(255, 255);
    assert_eq!(
        select(&mut tools, Tool::Eraser, false, &edited).as_deref(),
        Some(NO_INDEX_NOTICE)
    );
}

#[test]
fn row_16_with_busy_true_and_an_alpha_0_entry_it_changes_nothing_either() {
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, true, &case_14a());

    assert_eq!(tools.tool(), Tool::Pencil);
    assert_eq!(notice, None);
}

#[test]
fn row_16_the_press_notice_does_not_come_out_either_with_an_alpha_0_entry() {
    // Sin `Refused` ni en el `Begin` ni durante el trazo ni en el `End`.
    for image in [case_14a(), case_14d()] {
        let mut doc = Document::new(image);
        let id = ImageId::from_raw(0);
        let mut tools = tools_with(Tool::Eraser);

        for event in [
            CanvasEvent::StrokeBegin((1, 1)),
            CanvasEvent::StrokeMove((3, 1)),
            CanvasEvent::StrokeEnd,
        ] {
            tools
                .canvas_event_with(&mut doc, id, None, event)
                .expect("evento");
            assert!(tools.take_events().is_empty(), "sin Refused tras {event:?}");
        }
    }
}

#[test]
fn row_16_the_press_notice_comes_out_with_the_alpha_0_entries_turned_opaque() {
    let mut doc = Document::new(indexed_with_alpha_zero_entries(255, 255));
    let id = ImageId::from_raw(0);
    let mut tools = tools_with(Tool::Eraser);

    tools
        .canvas_event_with(&mut doc, id, None, CanvasEvent::StrokeBegin((1, 1)))
        .expect("pulsar");

    assert_eq!(
        tools.take_events(),
        vec![ToolEvent::Refused(EraseError::NoTransparentIndex)]
    );
}
