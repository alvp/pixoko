//! Tests de `keys::select_tool` (specs/hito-2-cuentagotas-borrador.md): fila 11 (aviso al elegir
//! el borrador) y fila 13 (`Select` con `App::busy()` completo).
//!
//! `busy` es el parámetro que `main.rs` calcula con `App::busy()`; aquí lo dan a mano el test (un
//! diálogo) o `Session::busy()` (deslizador del editor o arrastre de muestra). Un gesto de la
//! herramienta lo cubre `ToolState::busy()` por su cuenta.

use super::select_tool;

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
    let id = doc.main_image();
    tools
        .canvas_event_with(doc, id, None, CanvasEvent::StrokeBegin(pixel))
        .expect("pulsar");
    assert!(tools.busy(), "el gesto está en curso");
    tools.take_events();
}

fn end_stroke(tools: &mut ToolState, doc: &mut Document) {
    let id = doc.main_image();
    tools
        .canvas_event_with(doc, id, None, CanvasEvent::StrokeEnd)
        .expect("soltar");
    assert!(!tools.busy());
}

// ---------------------------------------------------------------------------------------
// Fila 11: aviso al elegir el borrador.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_11_borrador_con_gray_elige_y_avisa() {
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, false, &gray());

    assert_eq!(tools.tool(), Tool::Eraser, "queda elegida igualmente");
    assert_eq!(notice.as_deref(), Some(GRAY_NOTICE));
}

#[test]
fn fila_11_borrador_con_indexada_sin_transparente_elige_y_avisa() {
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, false, &indexed(None));

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice.as_deref(), Some(NO_INDEX_NOTICE));
}

#[test]
fn fila_11_borrador_con_imagen_que_admite_borrar_no_avisa() {
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
fn fila_11_los_textos_son_los_de_erase_error() {
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
fn fila_11_repetir_e_con_el_borrador_ya_elegido_repite_el_aviso() {
    let mut tools = tools_with(Tool::Eraser);
    let image = gray();

    let first = select(&mut tools, Tool::Eraser, false, &image);
    let second = select(&mut tools, Tool::Eraser, false, &image);

    assert_eq!(first.as_deref(), Some(GRAY_NOTICE));
    assert_eq!(second, first);
    assert_eq!(tools.tool(), Tool::Eraser);
}

#[test]
fn fila_11_volver_al_borrador_tras_otra_herramienta_avisa_otra_vez() {
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
fn fila_11_con_otra_imagen_el_aviso_es_el_que_corresponde() {
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
fn fila_11_las_otras_herramientas_con_gray_no_avisan() {
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
fn fila_11_cancel_no_cambia_la_herramienta_ni_avisa() {
    let mut tools = tools_with(Tool::Eraser);

    let notice = select_tool(&mut tools, ToolAction::Cancel, false, Some(&gray()));

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice, None);
}

#[test]
fn fila_11_con_busy_verdadero_no_cambia_la_herramienta_ni_avisa() {
    for image in [gray(), indexed(None), rgba()] {
        let mut tools = ToolState::new();

        let notice = select(&mut tools, Tool::Eraser, true, &image);

        assert_eq!(tools.tool(), Tool::Pencil, "{:?}", image.mode());
        assert_eq!(notice, None, "{:?}", image.mode());
    }
}

#[test]
fn fila_11_con_busy_verdadero_y_el_borrador_ya_elegido_tampoco_repite_el_aviso() {
    let mut tools = tools_with(Tool::Eraser);

    let notice = select(&mut tools, Tool::Eraser, true, &gray());

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice, None);
}

#[test]
fn fila_11_sin_imagen_elige_la_herramienta_sin_aviso() {
    let mut tools = ToolState::new();

    let notice = select_tool(&mut tools, ToolAction::Select(Tool::Eraser), false, None);

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice, None);
}

#[test]
fn fila_11_con_un_gesto_del_borrador_en_curso_no_cambia_ni_avisa() {
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
fn fila_13_session_busy_es_cierto_durante_el_arrastre_del_deslizador_y_falso_al_soltar() {
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
fn fila_13_session_busy_con_un_arrastre_de_muestra_de_verdad() {
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
fn fila_13_con_el_deslizador_pulsado_cuentagotas_a_lapiz_no_cambia_y_tras_soltar_si() {
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
fn fila_13_con_el_deslizador_pulsado_lapiz_a_borrador_no_cambia_ni_avisa_y_tras_soltar_si() {
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
fn fila_13_con_un_arrastre_de_muestra_lapiz_a_borrador_no_cambia_y_tras_soltar_si() {
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
fn fila_13_con_un_trazo_en_curso_cuentagotas_a_lapiz_no_cambia_y_tras_soltar_si() {
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
fn fila_13_con_un_trazo_en_curso_lapiz_a_borrador_no_cambia_ni_avisa_y_tras_soltar_si() {
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
fn fila_13_con_busy_verdadero_por_un_dialogo_cuentagotas_a_lapiz_no_cambia() {
    let mut tools = tools_with(Tool::Dropper);

    let blocked = select(&mut tools, Tool::Pencil, true, &rgba());
    assert_eq!(tools.tool(), Tool::Dropper);
    assert_eq!(blocked, None);

    let free = select(&mut tools, Tool::Pencil, false, &rgba());
    assert_eq!(tools.tool(), Tool::Pencil);
    assert_eq!(free, None);
}

#[test]
fn fila_13_con_busy_verdadero_por_un_dialogo_lapiz_a_borrador_no_cambia_ni_avisa() {
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
fn fila_13_con_varias_causas_a_la_vez_basta_con_una_para_bloquear() {
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

/// La paleta de la fila 14: `[azul, rojo, (0,255,0,α0), azul claro, (9,9,9,α0)]`, sin
/// `transparent_index`; `alfa_2` y `alfa_4` son el alfa guardado de las entradas 2 y 4.
fn indexed_with_alpha_zero_entries(alfa_2: u8, alfa_4: u8) -> Image {
    let palette = Palette::from_colors([
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(255, 0, 0),
        Rgba::new(0, 255, 0, alfa_2),
        Rgba::opaque(100, 150, 255),
        Rgba::new(9, 9, 9, alfa_4),
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
fn fila_16_borrador_con_el_caso_14a_elige_y_no_avisa() {
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, false, &case_14a());

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice, None, "hay una entrada de alfa 0: no avisa");
}

#[test]
fn fila_16_borrador_con_el_caso_14d_elige_y_no_avisa() {
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, false, &case_14d());

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice, None, "solo la entrada 4 es de alfa 0: no avisa");
}

#[test]
fn fila_16_repetir_e_con_entrada_de_alfa_0_no_avisa_ninguna_de_las_veces() {
    let mut tools = tools_with(Tool::Eraser);
    let image = case_14a();

    assert_eq!(select(&mut tools, Tool::Eraser, false, &image), None);
    assert_eq!(select(&mut tools, Tool::Eraser, false, &image), None);
    assert_eq!(tools.tool(), Tool::Eraser);
}

#[test]
fn fila_16_con_las_entradas_de_alfa_0_convertidas_en_opacas_avisa() {
    // Los casos 14(a) y 14(d) con todas sus entradas de alfa 0 pasadas a alfa 255.
    let image = indexed_with_alpha_zero_entries(255, 255);
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, false, &image);

    assert_eq!(tools.tool(), Tool::Eraser);
    assert_eq!(notice.as_deref(), Some(NO_INDEX_NOTICE));
}

#[test]
fn fila_16_una_sola_entrada_que_pasa_de_alfa_0_a_opaca_cambia_el_aviso() {
    // Se calcula al elegir, no se guarda: la misma paleta, antes y después de editarla.
    let mut tools = ToolState::new();

    assert_eq!(select(&mut tools, Tool::Eraser, false, &case_14d()), None);
    let editada = indexed_with_alpha_zero_entries(255, 255);
    assert_eq!(
        select(&mut tools, Tool::Eraser, false, &editada).as_deref(),
        Some(NO_INDEX_NOTICE)
    );
}

#[test]
fn fila_16_con_busy_verdadero_y_entrada_de_alfa_0_tampoco_cambia_nada() {
    let mut tools = ToolState::new();

    let notice = select(&mut tools, Tool::Eraser, true, &case_14a());

    assert_eq!(tools.tool(), Tool::Pencil);
    assert_eq!(notice, None);
}

#[test]
fn fila_16_el_aviso_de_pulsar_tampoco_sale_con_entrada_de_alfa_0() {
    // Sin `Refused` ni en el `Begin` ni durante el trazo ni en el `End`.
    for image in [case_14a(), case_14d()] {
        let mut doc = Document::new(image);
        let id = doc.main_image();
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
fn fila_16_el_aviso_de_pulsar_sale_con_las_entradas_de_alfa_0_convertidas_en_opacas() {
    let mut doc = Document::new(indexed_with_alpha_zero_entries(255, 255));
    let id = doc.main_image();
    let mut tools = tools_with(Tool::Eraser);

    tools
        .canvas_event_with(&mut doc, id, None, CanvasEvent::StrokeBegin((1, 1)))
        .expect("pulsar");

    assert_eq!(
        tools.take_events(),
        vec![ToolEvent::Refused(EraseError::NoTransparentIndex)]
    );
}
