//! Tests de las licencias de terceros `px_ui::licenses_view` (specs/hito-2-menu-herramientas.md,
//! «Casos» fila 11 y decisión 7), **adaptados a la unidad 18** (specs/hito-2-dialogos-modales.md,
//! decisión 9 y «Tests de la 17 que cambian»): el panel que sustituía al lienzo es ahora un diálogo
//! modal (`licenses_form` + `show_dialog`).
//!
//! - Parte pura: `wrap_text` (ajuste duro a `columns` caracteres, cortando por el último espacio de
//!   la línea si lo hay y, si no, en seco; conserva las líneas vacías; `columns = 0` da vacío) con
//!   `THIRD_PARTY` a 18, 40 y 200 columnas, el texto vacío, `c = 0` y una palabra de 500 letras;
//!   `clamp_first_line` recorta a `0..=total − visible_2`.
//! - Banco de egui sin ventana: el diálogo de Licencias con la rueda (avanza por líneas y se
//!   recorta), `Escape` y el botón `Cerrar`. Lo que cambia respecto a la 17, y solo esto: se llama a
//!   `show_dialog` sobre un `Dialog` abierto con `licenses_form` en vez de a `show_licenses`; la
//!   «primera línea» se lee y se escribe con `Dialog::first_line` y `set_first_line`; «se cierra» es
//!   que la `Reaction` sea `Accepted` o `Cancelled`; y el puntero de las pruebas de la rueda va al
//!   centro de la pantalla (el diálogo ya no ocupa toda la pantalla, sino el centro).
//!   `fila_11_el_panel_cubre_el_rectangulo_del_lienzo` **desaparece** (decisión D3: garantizaba que
//!   el lienzo no recibe entrada mientras el panel está abierto; lo comprueban ahora las filas 6, 10,
//!   7 y 11 de la 18). «Con él abierto todo lo demás da `Blocked` salvo `Quit`» es de `px-app`
//!   (`dispatch/tests.rs`) y de `actions.rs`.
//!
//! Decisiones donde la spec no fija un detalle: de `wrap_text("", c)` solo se exige «sin pánico y
//! ninguna línea con caracteres» (puede ser `[]` o `[""]`); el ejemplo del corte por el último
//! espacio se compara sin los espacios finales de cada línea; el número de líneas por muesca de
//! la rueda no se fija (solo la dirección y el recorte).

use egui::{Context, Event, Vec2, pos2, vec2};
use proptest::prelude::*;
use px_ui::dialog::{Dialog, DialogKind, Reaction};
use px_ui::dialog_keys::FocusTarget;
use px_ui::dialog_view::show_dialog;
use px_ui::licenses_view::{LICENSES_FIELD, clamp_first_line, licenses_form, wrap_text};
use px_ui::status::ADVANCE_PX;

mod common;
use common::{
    PPPS, Painted, button_2, context, frame, key_2, move_pointer, n_of, painted, wheel_line,
};

/// `docs/THIRD-PARTY.md`, el texto que muestra el panel.
const THIRD_PARTY: &str = include_str!("../../../docs/THIRD-PARTY.md");

fn without_spaces(text_2: &str) -> String {
    text_2.chars().filter(|c| !c.is_whitespace()).collect()
}

fn lines_empty_of(text_2: &str) -> usize {
    text_2.lines().filter(|l| l.is_empty()).count()
}

/// Lo común a toda salida de `wrap_text`.
fn checks_fit(text_2: &str, columns_2: usize, lines: &[String]) {
    for (i, line) in lines.iter().enumerate() {
        assert!(
            line.chars().count() <= columns_2,
            "la línea {i} mide {} y no cabe en {columns_2}: {line:?}",
            line.chars().count()
        );
    }
    let joined: String = lines.concat();
    assert_eq!(
        without_spaces(&joined),
        without_spaces(text_2),
        "se perdió o cambió de orden algún carácter con {columns_2} columnas"
    );
}

// ---------------------------------------------------------------------------------------------
// wrap_text
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_third_party_at_18_40_and_200_columns_no_line_passes_and_nothing_is_lost() {
    // 18 = el mínimo que cabe en el lienzo mínimo (128 u / 7).
    assert_eq!(128 / ADVANCE_PX, 18);
    for columns_2 in [18, 40, 200] {
        let lines = wrap_text(THIRD_PARTY, columns_2);
        assert!(!lines.is_empty(), "{columns_2} columnas");
        checks_fit(THIRD_PARTY, columns_2, &lines);
        assert!(
            lines.iter().filter(|l| l.is_empty()).count() >= lines_empty_of(THIRD_PARTY),
            "{columns_2} columnas: se perdieron líneas vacías"
        );
    }
}

#[test]
fn row_11_with_fewer_columns_there_are_more_lines() {
    let a18 = wrap_text(THIRD_PARTY, 18).len();
    let a40 = wrap_text(THIRD_PARTY, 40).len();
    let a200 = wrap_text(THIRD_PARTY, 200).len();
    assert!(a18 > a40 && a40 > a200, "{a18} > {a40} > {a200}");
    // Con columnas de sobra, solo cambian las líneas más largas que el ancho.
    assert!(a200 >= THIRD_PARTY.lines().count());
}

#[test]
fn row_11_columns_0_gives_empty() {
    assert!(wrap_text(THIRD_PARTY, 0).is_empty());
    assert!(wrap_text("abc", 0).is_empty());
    assert!(wrap_text("", 0).is_empty());
    assert!(wrap_text("a\n\nb", 0).is_empty());
}

#[test]
fn row_11_empty_text_gives_no_line_with_characters() {
    for columns_2 in [0, 1, 18, 40, 200] {
        let lines = wrap_text("", columns_2);
        assert!(lines.len() <= 1, "{columns_2}: {lines:?}");
        assert!(lines.iter().all(|l| l.is_empty()), "{columns_2}: {lines:?}");
    }
}

#[test]
fn row_11_a_500_letter_word_is_cut_dry_at_the_columns() {
    let word: String = (0..500)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    for columns_2 in [1, 18, 40, 200, 499, 500, 501] {
        let lines = wrap_text(&word, columns_2);
        checks_fit(&word, columns_2, &lines);
        // Sin espacios no hay dónde cortar: cada línea llena menos la última.
        assert_eq!(lines.concat(), word, "{columns_2} columnas");
        assert_eq!(
            lines.len(),
            500usize.div_ceil(columns_2),
            "{columns_2} columnas"
        );
        for line in &lines[..lines.len() - 1] {
            assert_eq!(line.chars().count(), columns_2, "{columns_2} columnas");
        }
    }
}

#[test]
fn row_11_cut_examples() {
    let text_2 = |lines: Vec<String>| -> Vec<String> { lines };
    // En seco, sin espacios.
    assert_eq!(
        text_2(wrap_text("abcdefghij", 4)),
        ["abcd", "efgh", "ij"].map(String::from)
    );
    // Cabe entero.
    assert_eq!(text_2(wrap_text("abc", 10)), ["abc".to_string()]);
    // Las líneas vacías se conservan.
    assert_eq!(
        text_2(wrap_text("ab\n\ncd", 10)),
        ["ab", "", "cd"].map(String::from)
    );
    assert_eq!(
        text_2(wrap_text("a\n\nb\n\n\nc", 10)),
        ["a", "", "b", "", "", "c"].map(String::from)
    );
    // Por el último espacio de la línea (sin contar los espacios finales de cada una).
    let cut: Vec<String> = wrap_text("hello world foo", 8)
        .iter()
        .map(|l| l.trim_end().to_string())
        .collect();
    assert_eq!(cut, ["hello", "world", "foo"].map(String::from));
}

#[test]
fn row_11_a_text_that_fits_in_one_line_is_not_touched() {
    for columns_2 in [11, 12, 40] {
        assert_eq!(
            wrap_text("hello world", columns_2),
            ["hello world".to_string()]
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn row_11_wrap_text_with_any_text_and_columns(
        text_2 in "[a-zñ ]{0,12}(\n[a-zñ ]{0,40}){0,12}",
        columns_2 in 0usize..=60,
    ) {
        let lines = wrap_text(&text_2, columns_2);
        if columns_2 == 0 {
            prop_assert!(lines.is_empty());
        } else {
            for line in &lines {
                prop_assert!(line.chars().count() <= columns_2, "{:?}", line);
            }
            let joined: String = lines.concat();
            prop_assert_eq!(without_spaces(&joined), without_spaces(&text_2));
            prop_assert!(
                lines.iter().filter(|l| l.is_empty()).count() >= lines_empty_of(&text_2)
            );
        }
    }

    /// Cortar por el último espacio: una palabra que cabe en una línea nunca se parte.
    #[test]
    fn row_11_wrap_text_does_not_split_the_words_that_fit(
        words in prop::collection::vec("[a-z]{1,12}", 0..40),
        columns_2 in 12usize..=60,
    ) {
        let text_2 = words.join(" ");
        let lines = wrap_text(&text_2, columns_2);
        let gathered: Vec<&str> = lines.iter().flat_map(|l| l.split_whitespace()).collect();
        let original: Vec<&str> = text_2.split_whitespace().collect();
        prop_assert_eq!(gathered, original);
        for line in &lines {
            prop_assert!(line.chars().count() <= columns_2);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// clamp_first_line
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_clamp_first_line_clips_to_0_up_to_total_minus_visible() {
    // (first, total, visible, esperado)
    for (first, total, visible, expected_2) in [
        (0, 10, 3, 0),
        (5, 10, 3, 5),
        (7, 10, 3, 7),
        (8, 10, 3, 7),
        (100, 10, 3, 7),
        (3, 2, 5, 0),
        (3, 5, 5, 0),
        (3, 0, 0, 0),
        (0, 0, 0, 0),
        (10, 10, 0, 10),
        (11, 10, 0, 10),
        (usize::MAX, 1000, 40, 960),
    ] {
        assert_eq!(
            clamp_first_line(first, total, visible),
            expected_2,
            "first {first}, total {total}, visibles {visible}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn row_11_clamp_first_line_is_the_minimum_with_the_maximum_and_is_idempotent(
        first in 0usize..=10_000,
        total in 0usize..=10_000,
        visible in 0usize..=10_000,
    ) {
        let max = total.saturating_sub(visible);
        let r = clamp_first_line(first, total, visible);
        prop_assert_eq!(r, first.min(max));
        prop_assert!(r <= max);
        prop_assert_eq!(clamp_first_line(r, total, visible), r);
    }
}

// ---------------------------------------------------------------------------------------------
// Banco de egui: show_licenses
// ---------------------------------------------------------------------------------------------

const TEST_SCREEN: Vec2 = vec2(600.0, 300.0);

struct Bench {
    ctx: Context,
    dialog: Dialog,
    focus: Option<FocusTarget>,
}

/// La respuesta cierra el diálogo (`Cerrar` lo acepta y `Escape` lo cancela).
fn closes(reaction: &Reaction) -> bool {
    matches!(reaction, Reaction::Accepted(_) | Reaction::Cancelled)
}

impl Bench {
    fn new_2(ppp: f32, text_2: &str) -> Bench {
        Bench {
            ctx: context(ppp),
            dialog: Dialog::new(DialogKind::Licenses, licenses_form(text_2))
                .expect("la spec de Licencias es válida"),
            focus: None,
        }
    }

    fn first(&self) -> usize {
        self.dialog.first_line(LICENSES_FIELD)
    }

    fn frame(&mut self, events: Vec<Event>) -> (Painted, Reaction) {
        let mut output_2 = None;
        let (dialog, focus) = (&mut self.dialog, &mut self.focus);
        let shapes_2 = frame(&self.ctx, TEST_SCREEN, events, |ui| {
            output_2 = Some(show_dialog(ui.ctx(), dialog, focus));
        });
        (painted(&shapes_2), output_2.expect("el diálogo se dibuja"))
    }

    fn rest_2_2(&mut self) -> Painted {
        self.frame(vec![]);
        self.frame(vec![]).0
    }

    /// Pone el puntero sobre el texto (el centro de la pantalla, que es el centro del diálogo) y
    /// espera un fotograma estable.
    fn with_pointer(&mut self) {
        self.frame(vec![move_pointer(pos2(300.0, 150.0))]);
        self.rest_2_2();
    }

    fn wheel(&mut self, below: bool, notches: usize) -> Painted {
        let events = (0..notches).map(|_| wheel_line(!below)).collect();
        self.frame(events).0
    }
}

/// Texto de 200 líneas cortas: «linea 0» … «linea 199».
fn two_hundred_lines() -> String {
    (0..200)
        .map(|i| format!("linea {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Números de las líneas «linea N» que se pintan, ordenados.
fn lines_painted(painted: &Painted) -> Vec<usize> {
    let mut v: Vec<usize> = painted
        .texts
        .iter()
        .flat_map(|t| t.text_2.lines())
        .filter_map(|l| l.trim().strip_prefix("linea ")?.parse().ok())
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn row_11_when_opening_the_start_is_seen_without_closing() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, &two_hundred_lines());
        let (painted, reaction) = bench.frame(vec![]);
        assert!(!closes(&reaction), "ppp {ppp}: se cerró solo");
        assert_eq!(bench.first(), 0);
        let visible_2 = lines_painted(&painted);
        assert_eq!(visible_2.first(), Some(&0), "ppp {ppp}: {visible_2:?}");
        assert!(
            visible_2.len() > 3,
            "ppp {ppp}: se ven {} líneas",
            visible_2.len()
        );
        assert!(!visible_2.contains(&199));
    }
}

#[test]
fn row_11_the_wheel_advances_by_lines_and_the_first_painted_line_is_first_line() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, &two_hundred_lines());
        bench.with_pointer();
        bench.wheel(true, 1);
        let after_a = bench.first();
        assert!(after_a >= 1, "ppp {ppp}: una muesca hacia abajo avanza");
        let painted = bench.rest_2_2();
        let visible_2 = lines_painted(&painted);
        assert_eq!(
            visible_2.first(),
            Some(&after_a),
            "ppp {ppp}: {visible_2:?}"
        );
        // Líneas consecutivas.
        assert!(
            visible_2.windows(2).all(|w| w[1] == w[0] + 1),
            "ppp {ppp}: {visible_2:?}"
        );
        bench.wheel(true, 3);
        assert!(bench.first() > after_a, "ppp {ppp}: más muescas, más abajo");
        let before = bench.first();
        bench.wheel(false, 1);
        assert!(bench.first() < before, "ppp {ppp}: hacia arriba retrocede");
        bench.wheel(false, 1000);
        assert_eq!(bench.first(), 0, "ppp {ppp}: no baja de 0");
    }
}

#[test]
fn row_11_the_wheel_is_clipped_to_total_minus_visible_and_the_last_line_stays_in_view() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, &two_hundred_lines());
        bench.with_pointer();
        bench.wheel(true, 2000);
        let maximum_2 = bench.first();
        assert!((1..=199).contains(&maximum_2), "ppp {ppp}: {maximum_2}");
        let painted = bench.rest_2_2();
        let visible_2 = lines_painted(&painted);
        assert_eq!(
            visible_2.last(),
            Some(&199),
            "ppp {ppp}: en el máximo se ve la última línea; se ven {visible_2:?}"
        );
        // Un recorte exacto: ver n líneas con la primera en `maximum_2` agota las 200.
        assert_eq!(
            visible_2.first(),
            Some(&maximum_2),
            "ppp {ppp}: {visible_2:?}"
        );
        // Otra muesca hacia abajo no se mueve.
        bench.wheel(true, 1);
        assert_eq!(
            bench.first(),
            maximum_2,
            "ppp {ppp}: se recorta, no se pasa"
        );
        bench.rest_2_2();
        assert_eq!(bench.first(), maximum_2);
    }
}

#[test]
fn row_11_a_short_text_does_not_scroll() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, "uno\ndos\ntres");
        bench.with_pointer();
        bench.wheel(true, 50);
        assert_eq!(bench.first(), 0, "ppp {ppp}");
    }
}

#[test]
fn row_11_an_out_of_range_first_line_is_clipped_when_painting() {
    let mut bench = Bench::new_2(1.0, &two_hundred_lines());
    bench.dialog.set_first_line(LICENSES_FIELD, usize::MAX / 2);
    bench.rest_2_2();
    assert!(bench.first() <= 199, "{}", bench.first());
    let painted = bench.rest_2_2();
    assert_eq!(lines_painted(&painted).last(), Some(&199));
}

#[test]
fn row_11_escape_closes_and_without_escape_it_does_not() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, &two_hundred_lines());
        bench.rest_2_2();
        for key_any in [egui::Key::A, egui::Key::Enter, egui::Key::Space] {
            let (_, reaction) = bench.frame(vec![key_2(key_any, egui::Modifiers::NONE)]);
            assert!(!closes(&reaction), "ppp {ppp}: {key_any:?} no cierra");
        }
        let (_, reaction) = bench.frame(vec![key_2(egui::Key::Escape, egui::Modifiers::NONE)]);
        assert!(closes(&reaction), "ppp {ppp}: Escape cierra");
    }
}

#[test]
fn row_11_the_close_button_closes_with_a_click() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, &two_hundred_lines());
        let painted = bench.rest_2_2();
        let close = painted.text_with("Cerrar").unwrap_or_else(|| {
            panic!(
                "ppp {ppp}: no se pinta `Cerrar`; hay {:?}",
                painted.all_the_texts().len()
            )
        });
        let center = close.rect().center();
        let mut closed = false;
        for events in [
            vec![move_pointer(center)],
            vec![button_2(center, true)],
            vec![button_2(center, false)],
        ] {
            let (_, reaction) = bench.frame(events);
            closed |= closes(&reaction);
        }
        assert!(closed, "ppp {ppp}: el clic en `Cerrar` cierra");
    }
}

#[test]
fn row_11_a_click_outside_the_button_does_not_close() {
    let mut bench = Bench::new_2(1.0, &two_hundred_lines());
    bench.rest_2_2();
    let mut closed = false;
    let pos = pos2(300.0, 150.0);
    for events in [
        vec![move_pointer(pos)],
        vec![button_2(pos, true)],
        vec![button_2(pos, false)],
    ] {
        closed |= closes(&bench.frame(events).1);
    }
    assert!(!closed);
}

#[test]
fn row_11_the_panel_paints_the_text_wrapped_to_the_columns_that_fit() {
    let text_2 = format!(
        "{}\n\nuna frase normal con varias palabras para ver el ajuste de las lineas\n{}",
        "x".repeat(500),
        THIRD_PARTY
    );
    for ppp in PPPS {
        let n = n_of(ppp);
        let mut bench = Bench::new_2(ppp, &text_2);
        let painted = bench.rest_2_2();
        // Columnas que caben como mucho: el ancho entero del lienzo entre el avance del texto.
        let columns_2 =
            ((f64::from(TEST_SCREEN.x) * f64::from(ppp)) / (7.0 * n as f64)).floor() as usize;
        let mut lines = 0;
        for t in painted
            .texts
            .iter()
            .filter(|t| !t.text_2.contains("Cerrar"))
        {
            for line in t.text_2.lines() {
                lines += 1;
                assert!(
                    line.chars().count() <= columns_2,
                    "ppp {ppp}: una línea pintada mide {} y caben {columns_2}: {line:?}",
                    line.chars().count()
                );
            }
        }
        assert!(lines > 3, "ppp {ppp}: se pintan {lines} líneas");
        // Nada se pinta fuera del rectángulo del panel.
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, TEST_SCREEN).expand(0.01);
        for t in &painted.texts {
            assert!(
                screen.contains_rect(t.clip),
                "ppp {ppp}: recorte fuera de la pantalla: {:?}",
                t.clip
            );
        }
    }
}

#[test]
fn row_11_external_text_goes_through_printable_and_does_not_cause_a_missing_glyph() {
    // El contexto del banco entra en pánico ante un glifo ausente: `show_licenses` tiene que
    // sustituir lo que la fuente no dibuja (`replace_missing`).
    let mut bench = Bench::new_2(1.0, "a日b ✓ c\nsegunda");
    let painted = bench.rest_2_2();
    let all: String = painted.all_the_texts().concat();
    assert!(!all.contains('日'), "{all:?}");
    assert!(!all.contains('✓'), "{all:?}");
    assert!(all.contains('a') && all.contains('b'));
}

// ---------------------------------------------------------------------------------------------
// Fila 12 de la unidad 18: Licencias migradas (specs/hito-2-dialogos-modales.md). En ROJO hasta la
// implementación: `licenses_form`, `Modal` y `show_dialog` no existen todavía.
// ---------------------------------------------------------------------------------------------

mod row_12 {
    //! Lo nuevo de la fila 12 que toca a Licencias (`demo_form` y `demo_notice` están en `px-app`):
    //!
    //! - `licenses_form(THIRD_PARTY)`: `validate` correcto, título «Licencias», un único campo
    //!   `ReadOnly` con `Rows::Fill` de id `LICENSES_FIELD`, botón «Cerrar» y sin Cancelar. El texto
    //!   del campo es el recibido, tal cual (decisión: la spec no dice que se transforme), y como
    //!   texto externo no se valida contra `INTERFACE_CHARSET` (`日✓` y el texto vacío valen).
    //! - `Modal::open` con y sin bloqueos (las 32 combinaciones de los cinco; solo con todos en
    //!   falso abre, un segundo `open` y una spec inválida dan `false`), `Accepted` y `Cancelled` lo
    //!   cierran, `abandon` y `close_kind(Licenses)`. «`Licenses(false)` no cierra otro tipo» no se
    //!   puede probar todavía: la spec solo tiene `DialogKind::Licenses` (la 19 y la 20 añaden los
    //!   suyos), así que se prueba que `close_kind` cierra solo cuando está abierto de ese tipo.
    //! - Banco a la ventana mínima real de cada escala (`468n × 333n` px): 16 filas de 42 columnas del
    //!   texto envuelto (con 200 líneas cortas: «linea 0» a «linea 15»; con `THIRD_PARTY`: las 16
    //!   primeras filas de `wrap_text(THIRD_PARTY, 42)` y ninguna línea pintada pasa de 42
    //!   caracteres), el máximo de desplazamiento es `total − 16`, el foco inicial es la zona, `Tab`
    //!   la cicla con `Cerrar` (no hay Cancelar) e `Intro` con el foco en `Cerrar` solo lo acepta.
    //!   Las filas se comparan tras `trim_end` y pasadas por `printable`, como las pinta el diálogo.

    use super::*;
    use px_ui::actions::Locks;
    use px_ui::dialog::{
        DialogKind, Event as TestEvent, FieldKind, FieldSpec, FormSpec, Modal, Rows,
    };
    use px_ui::text::{apply_scale, printable};
    use px_ui::theme::visuals;

    fn spec_valid() -> FormSpec {
        licenses_form(THIRD_PARTY)
    }

    fn locks_of(bits: u8) -> Locks {
        Locks {
            gesture: bits & 1 != 0,
            stroke: false,
            dialog: bits & 2 != 0,
            text_focus: bits & 4 != 0,
            removal: bits & 8 != 0,
            modal: bits & 16 != 0,
        }
    }

    #[test]
    fn row_12_licenses_form_of_third_party_is_valid_and_has_the_requested_shape() {
        let spec = spec_valid();
        assert!(spec.validate().is_ok());
        assert_eq!(spec.title, "Licencias");
        assert_eq!(spec.accept, "Cerrar");
        assert_eq!(spec.cancel, None, "sin Cancelar");
        assert_eq!(spec.fields.len(), 1, "un único campo");
        let field_2 = &spec.fields[0];
        assert_eq!(field_2.id, LICENSES_FIELD);
        assert_eq!(LICENSES_FIELD, "texto");
        assert!(field_2.shown_when.is_none());
        match &field_2.kind {
            FieldKind::ReadOnly { text, rows } => {
                assert_eq!(text, THIRD_PARTY, "el texto es el recibido");
                assert!(matches!(rows, Rows::Fill), "la zona ocupa el cuerpo");
            }
            other_2 => panic!("el campo es una zona de solo lectura, no {other_2:?}"),
        }
    }

    #[test]
    fn row_12_licenses_form_accepts_any_external_text() {
        for text_2 in [
            "",
            "a",
            "日✓",
            "á é ñ © — ✓ 日",
            &"x".repeat(100_000),
            THIRD_PARTY,
        ] {
            assert!(
                licenses_form(text_2).validate().is_ok(),
                "el texto externo no se valida contra el juego de caracteres"
            );
            assert!(Dialog::new(DialogKind::Licenses, licenses_form(text_2)).is_ok());
        }
    }

    #[test]
    fn row_12_the_licenses_dialog_is_fill_without_cancel_and_with_the_natural_content_of_330_u() {
        let dialog = Dialog::new(DialogKind::Licenses, spec_valid()).expect("válida");
        assert_eq!(dialog.kind(), DialogKind::Licenses);
        assert!(dialog.is_fill());
        assert_eq!(dialog.content_h_u(), 330, "16 + 6 + 22·14");
        assert!(dialog.is_valid());
        assert_eq!(dialog.reason(), None);
        assert_eq!(dialog.first_line(LICENSES_FIELD), 0);
    }

    #[test]
    fn row_12_modal_open_only_opens_with_the_five_locks_false() {
        for bits in 0u8..32 {
            let mut modal = Modal::default();
            assert!(!modal.is_open());
            assert_eq!(modal.kind(), None);
            let opens = modal.open(DialogKind::Licenses, spec_valid(), locks_of(bits));
            assert_eq!(opens, bits == 0, "bloqueos {:?}", locks_of(bits));
            assert_eq!(modal.is_open(), bits == 0, "bloqueos {:?}", locks_of(bits));
            assert_eq!(
                modal.kind(),
                (bits == 0).then_some(DialogKind::Licenses),
                "bloqueos {:?}",
                locks_of(bits)
            );
        }
    }

    #[test]
    fn row_12_modal_open_with_one_open_or_with_an_invalid_spec_gives_false() {
        let mut modal = Modal::default();
        assert!(modal.open(DialogKind::Licenses, spec_valid(), Locks::default()));
        // Un segundo `open`, ya con todo en falso, no toca el diálogo abierto.
        assert!(!modal.open(
            DialogKind::Licenses,
            licenses_form("otro"),
            Locks::default()
        ));
        assert!(modal.is_open());
        assert!(!modal.open(
            DialogKind::Licenses,
            spec_valid(),
            Locks {
                modal: true,
                ..Locks::default()
            }
        ));
        assert!(modal.is_open());

        // Una spec inválida (`min > max`) no abre.
        let invalid = FormSpec {
            title: "Prueba",
            accept: "Aceptar",
            cancel: None,
            fields: vec![FieldSpec {
                id: "n",
                label: "N",
                kind: FieldKind::Number {
                    min: 5,
                    max: 1,
                    default: 3,
                },
                shown_when: None,
            }],
            check: None,
        };
        assert!(invalid.validate().is_err());
        let mut closed_2 = Modal::default();
        assert!(!closed_2.open(DialogKind::Licenses, invalid, Locks::default()));
        assert!(!closed_2.is_open());
        assert_eq!(closed_2.kind(), None);
    }

    #[test]
    fn row_12_close_escape_and_abandon_close_and_close_kind_closes_only_the_open_kind() {
        let opens = |modal: &mut Modal| {
            assert!(modal.open(DialogKind::Licenses, spec_valid(), Locks::default()));
        };
        let mut modal = Modal::default();

        // Cerrado: nada que hacer.
        assert_eq!(modal.event(TestEvent::Accept), Reaction::None);
        assert_eq!(modal.event(TestEvent::Cancel), Reaction::None);
        assert!(!modal.abandon());
        assert!(!modal.close_kind(DialogKind::Licenses));

        // `Cerrar` (aceptar) cierra y entrega sus valores (ninguno: es solo lectura).
        opens(&mut modal);
        assert!(matches!(
            modal.event(TestEvent::Accept),
            Reaction::Accepted(_)
        ));
        assert!(!modal.is_open());
        assert_eq!(
            modal.event(TestEvent::Accept),
            Reaction::None,
            "tras cerrar, nada"
        );

        // `Escape` (cancelar) cierra.
        opens(&mut modal);
        assert_eq!(modal.event(TestEvent::Cancel), Reaction::Cancelled);
        assert!(!modal.is_open());
        assert_eq!(modal.event(TestEvent::Cancel), Reaction::None);

        // `abandon` cierra sin `Reaction` y la segunda vez da `false`.
        opens(&mut modal);
        assert!(modal.abandon());
        assert!(!modal.is_open());
        assert!(!modal.abandon());

        // `close_kind` de Licencias cierra el diálogo de Licencias y solo si está abierto.
        opens(&mut modal);
        assert!(modal.close_kind(DialogKind::Licenses));
        assert!(!modal.is_open());
        assert!(!modal.close_kind(DialogKind::Licenses));
        // Y se puede volver a abrir.
        opens(&mut modal);
        assert_eq!(modal.kind(), Some(DialogKind::Licenses));
    }

    // -----------------------------------------------------------------------------------------
    // Banco a la ventana mínima real de cada escala
    // -----------------------------------------------------------------------------------------

    /// La zona de texto de Licencias: 16 filas de 42 columnas (4 u de 8 u + 308 u − 14 u de
    /// caja… lo da `dialog_layout`; aquí solo se mide lo pintado).
    const ROWS: usize = 16;
    const TEST_COLUMNS: usize = 42;

    struct BenchMinimum {
        ctx: Context,
        ppp: f32,
        dialog: Dialog,
        focus: Option<FocusTarget>,
        reactions: Vec<Reaction>,
    }

    impl BenchMinimum {
        fn new_2(ppp: f32, text_2: &str) -> BenchMinimum {
            let ctx = context(ppp);
            ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
            BenchMinimum {
                ctx,
                ppp,
                dialog: Dialog::new(DialogKind::Licenses, licenses_form(text_2)).expect("válida"),
                focus: None,
                reactions: Vec::new(),
            }
        }

        /// La ventana mínima real: `468n × 333n` píxeles, en puntos.
        fn screen(&self) -> Vec2 {
            let n = n_of(self.ppp);
            vec2((468 * n) as f32 / self.ppp, (333 * n) as f32 / self.ppp)
        }

        fn frame(&mut self, events: Vec<Event>) -> Painted {
            let screen = self.screen();
            let mut reaction = Reaction::None;
            let (dialog, focus) = (&mut self.dialog, &mut self.focus);
            let shapes_2 = frame(&self.ctx, screen, events, |ui| {
                let c = ui.ctx().clone();
                apply_scale(&c);
                reaction = show_dialog(&c, dialog, focus);
            });
            if !matches!(reaction, Reaction::None) {
                self.reactions.push(reaction);
            }
            painted(&shapes_2)
        }

        fn rest_2_2(&mut self) -> Painted {
            self.frame(vec![]);
            self.frame(vec![]);
            self.frame(vec![])
        }

        fn tab(&mut self) -> Painted {
            self.frame(vec![key_2(egui::Key::Tab, egui::Modifiers::NONE)]);
            self.rest_2_2()
        }

        /// Las filas de la zona que se pintan, sin el título, el botón ni las filas vacías.
        fn rows_2(&self, p: &Painted) -> Vec<String> {
            p.texts
                .iter()
                .flat_map(|t| t.text_2.lines())
                .map(|l| l.trim_end().to_owned())
                .filter(|l| !l.is_empty() && l != "Licencias" && l != "Cerrar")
                .collect()
        }
    }

    #[test]
    fn row_12_at_the_minimum_window_16_rows_of_42_columns_of_the_wrapped_text_are_seen() {
        for ppp in PPPS {
            let mut bench = BenchMinimum::new_2(ppp, &two_hundred_lines());
            let p = bench.rest_2_2();
            let expected_2: Vec<String> = (0..ROWS).map(|i| format!("linea {i}")).collect();
            assert_eq!(
                bench.rows_2(&p),
                expected_2,
                "ppp {ppp}: 16 filas desde el principio"
            );
            assert_eq!(
                lines_painted(&p),
                (0..ROWS).collect::<Vec<_>>(),
                "ppp {ppp}"
            );

            // Con la rueda hasta el final, `total − 16`: son 16 las filas que caben.
            let center = pos2(bench.screen().x / 2.0, bench.screen().y / 2.0);
            bench.frame(vec![move_pointer(center)]);
            bench.rest_2_2();
            bench.frame((0..5000).map(|_| wheel_line(false)).collect());
            let p = bench.rest_2_2();
            assert_eq!(
                bench.dialog.first_line(LICENSES_FIELD),
                200 - ROWS,
                "ppp {ppp}: el máximo es total − 16"
            );
            assert_eq!(
                lines_painted(&p),
                (200 - ROWS..200).collect::<Vec<_>>(),
                "ppp {ppp}: al final se ven las últimas 16"
            );
        }
    }

    #[test]
    fn row_12_with_third_party_the_first_16_rows_wrapped_at_42_are_seen_and_none_passes_42() {
        let wrapped = wrap_text(THIRD_PARTY, TEST_COLUMNS);
        assert!(
            wrapped.len() > 2 * ROWS,
            "THIRD_PARTY da más de dos pantallas"
        );
        for ppp in PPPS {
            let mut bench = BenchMinimum::new_2(ppp, THIRD_PARTY);
            let p = bench.rest_2_2();
            let expected_2: Vec<String> = wrapped[..ROWS]
                .iter()
                .map(|l| printable(&bench.ctx, l.trim_end()))
                .filter(|l| !l.is_empty())
                .collect();
            assert_eq!(
                bench.rows_2(&p),
                expected_2,
                "ppp {ppp}: las 16 primeras filas del texto envuelto a 42"
            );
            for t in &p.texts {
                for line in t.text_2.lines() {
                    assert!(
                        line.chars().count() <= TEST_COLUMNS,
                        "ppp {ppp}: una línea pintada mide {} (> 42): {line:?}",
                        line.chars().count()
                    );
                }
            }
            // El máximo de desplazamiento con THIRD_PARTY.
            let center = pos2(bench.screen().x / 2.0, bench.screen().y / 2.0);
            bench.frame(vec![move_pointer(center)]);
            bench.rest_2_2();
            bench.frame((0..5000).map(|_| wheel_line(false)).collect());
            let p = bench.rest_2_2();
            assert_eq!(
                bench.dialog.first_line(LICENSES_FIELD),
                clamp_first_line(usize::MAX / 2, wrapped.len(), ROWS),
                "ppp {ppp}"
            );
            for t in &p.texts {
                for line in t.text_2.lines() {
                    assert!(line.chars().count() <= TEST_COLUMNS, "ppp {ppp}: {line:?}");
                }
            }
        }
    }

    #[test]
    fn row_12_the_initial_focus_is_the_area_and_tab_cycles_it_with_close_without_cancel() {
        for ppp in PPPS {
            let mut bench = BenchMinimum::new_2(ppp, &two_hundred_lines());
            bench.rest_2_2();
            assert_eq!(
                bench.focus,
                Some(FocusTarget::Zone(LICENSES_FIELD)),
                "ppp {ppp}"
            );
            bench.tab();
            assert_eq!(bench.focus, Some(FocusTarget::Accept), "ppp {ppp}");
            bench.tab();
            assert_eq!(
                bench.focus,
                Some(FocusTarget::Zone(LICENSES_FIELD)),
                "ppp {ppp}: sin Cancelar, la tabulación da la vuelta"
            );
        }
    }

    #[test]
    fn row_12_enter_and_space_with_the_focus_on_close_only_accept_it() {
        for ppp in PPPS {
            for key_button in [egui::Key::Enter, egui::Key::Space] {
                let mut bench = BenchMinimum::new_2(ppp, &two_hundred_lines());
                bench.rest_2_2();
                bench.tab();
                assert_eq!(bench.focus, Some(FocusTarget::Accept), "ppp {ppp}");
                bench.reactions.clear();
                bench.frame(vec![key_2(key_button, egui::Modifiers::NONE)]);
                bench.rest_2_2();
                let accepted = bench
                    .reactions
                    .iter()
                    .filter(|r| matches!(r, Reaction::Accepted(_)))
                    .count();
                assert_eq!(
                    accepted, 1,
                    "ppp {ppp} {key_button:?}: {:?}",
                    bench.reactions
                );
                assert!(
                    !bench.reactions.contains(&Reaction::Cancelled),
                    "ppp {ppp} {key_button:?}"
                );
            }
        }
    }
}
