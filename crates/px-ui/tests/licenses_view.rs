//! Tests de las licencias de terceros `px_ui::licenses_view` (specs/hito-2-menu-herramientas.md,
//! «Casos» fila 11 y decisión 7), **adaptados a la unidad 18** (specs/hito-2-dialogos-modales.md,
//! decisión 9 y «Tests de la 17 que cambian»): el panel que sustituía al lienzo es ahora un diálogo
//! modal (`licenses_form` + `show_dialog`).
//!
//! - Parte pura: `wrap_text` (ajuste duro a `columns` caracteres, cortando por el último espacio de
//!   la línea si lo hay y, si no, en seco; conserva las líneas vacías; `columns = 0` da vacío) con
//!   `THIRD_PARTY` a 18, 40 y 200 columnas, el texto vacío, `c = 0` y una palabra de 500 letras;
//!   `clamp_first_line` recorta a `0..=total − visibles`.
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
use common::{PPPS, Pintado, boton, contexto, fotograma, mover, n_de, pintado, rueda_linea, tecla};

/// `docs/THIRD-PARTY.md`, el texto que muestra el panel.
const THIRD_PARTY: &str = include_str!("../../../docs/THIRD-PARTY.md");

fn sin_espacios(texto: &str) -> String {
    texto.chars().filter(|c| !c.is_whitespace()).collect()
}

fn lineas_vacias_de(texto: &str) -> usize {
    texto.lines().filter(|l| l.is_empty()).count()
}

/// Lo común a toda salida de `wrap_text`.
fn comprueba_ajuste(texto: &str, columnas: usize, lineas: &[String]) {
    for (i, linea) in lineas.iter().enumerate() {
        assert!(
            linea.chars().count() <= columnas,
            "la línea {i} mide {} y no cabe en {columnas}: {linea:?}",
            linea.chars().count()
        );
    }
    let unidas: String = lineas.concat();
    assert_eq!(
        sin_espacios(&unidas),
        sin_espacios(texto),
        "se perdió o cambió de orden algún carácter con {columnas} columnas"
    );
}

// ---------------------------------------------------------------------------------------------
// wrap_text
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_third_party_a_18_40_y_200_columnas_ninguna_linea_pasa_y_no_se_pierde_nada() {
    // 18 = el mínimo que cabe en el lienzo mínimo (128 u / 7).
    assert_eq!(128 / ADVANCE_PX, 18);
    for columnas in [18, 40, 200] {
        let lineas = wrap_text(THIRD_PARTY, columnas);
        assert!(!lineas.is_empty(), "{columnas} columnas");
        comprueba_ajuste(THIRD_PARTY, columnas, &lineas);
        assert!(
            lineas.iter().filter(|l| l.is_empty()).count() >= lineas_vacias_de(THIRD_PARTY),
            "{columnas} columnas: se perdieron líneas vacías"
        );
    }
}

#[test]
fn fila_11_con_menos_columnas_hay_mas_lineas() {
    let a18 = wrap_text(THIRD_PARTY, 18).len();
    let a40 = wrap_text(THIRD_PARTY, 40).len();
    let a200 = wrap_text(THIRD_PARTY, 200).len();
    assert!(a18 > a40 && a40 > a200, "{a18} > {a40} > {a200}");
    // Con columnas de sobra, solo cambian las líneas más largas que el ancho.
    assert!(a200 >= THIRD_PARTY.lines().count());
}

#[test]
fn fila_11_columnas_0_da_vacio() {
    assert!(wrap_text(THIRD_PARTY, 0).is_empty());
    assert!(wrap_text("abc", 0).is_empty());
    assert!(wrap_text("", 0).is_empty());
    assert!(wrap_text("a\n\nb", 0).is_empty());
}

#[test]
fn fila_11_el_texto_vacio_no_da_ninguna_linea_con_caracteres() {
    for columnas in [0, 1, 18, 40, 200] {
        let lineas = wrap_text("", columnas);
        assert!(lineas.len() <= 1, "{columnas}: {lineas:?}");
        assert!(
            lineas.iter().all(|l| l.is_empty()),
            "{columnas}: {lineas:?}"
        );
    }
}

#[test]
fn fila_11_una_palabra_de_500_letras_se_corta_en_seco_a_las_columnas() {
    let palabra: String = (0..500)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    for columnas in [1, 18, 40, 200, 499, 500, 501] {
        let lineas = wrap_text(&palabra, columnas);
        comprueba_ajuste(&palabra, columnas, &lineas);
        // Sin espacios no hay dónde cortar: cada línea llena menos la última.
        assert_eq!(lineas.concat(), palabra, "{columnas} columnas");
        assert_eq!(
            lineas.len(),
            500usize.div_ceil(columnas),
            "{columnas} columnas"
        );
        for linea in &lineas[..lineas.len() - 1] {
            assert_eq!(linea.chars().count(), columnas, "{columnas} columnas");
        }
    }
}

#[test]
fn fila_11_ejemplos_de_corte() {
    let texto = |lineas: Vec<String>| -> Vec<String> { lineas };
    // En seco, sin espacios.
    assert_eq!(
        texto(wrap_text("abcdefghij", 4)),
        ["abcd", "efgh", "ij"].map(String::from)
    );
    // Cabe entero.
    assert_eq!(texto(wrap_text("abc", 10)), ["abc".to_string()]);
    // Las líneas vacías se conservan.
    assert_eq!(
        texto(wrap_text("ab\n\ncd", 10)),
        ["ab", "", "cd"].map(String::from)
    );
    assert_eq!(
        texto(wrap_text("a\n\nb\n\n\nc", 10)),
        ["a", "", "b", "", "", "c"].map(String::from)
    );
    // Por el último espacio de la línea (sin contar los espacios finales de cada una).
    let cortado: Vec<String> = wrap_text("hello world foo", 8)
        .iter()
        .map(|l| l.trim_end().to_string())
        .collect();
    assert_eq!(cortado, ["hello", "world", "foo"].map(String::from));
}

#[test]
fn fila_11_un_texto_que_cabe_en_una_linea_no_se_toca() {
    for columnas in [11, 12, 40] {
        assert_eq!(
            wrap_text("hello world", columnas),
            ["hello world".to_string()]
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn fila_11_wrap_text_con_cualquier_texto_y_columnas(
        texto in "[a-zñ ]{0,12}(\n[a-zñ ]{0,40}){0,12}",
        columnas in 0usize..=60,
    ) {
        let lineas = wrap_text(&texto, columnas);
        if columnas == 0 {
            prop_assert!(lineas.is_empty());
        } else {
            for linea in &lineas {
                prop_assert!(linea.chars().count() <= columnas, "{:?}", linea);
            }
            let unidas: String = lineas.concat();
            prop_assert_eq!(sin_espacios(&unidas), sin_espacios(&texto));
            prop_assert!(
                lineas.iter().filter(|l| l.is_empty()).count() >= lineas_vacias_de(&texto)
            );
        }
    }

    /// Cortar por el último espacio: una palabra que cabe en una línea nunca se parte.
    #[test]
    fn fila_11_wrap_text_no_parte_las_palabras_que_caben(
        palabras in prop::collection::vec("[a-z]{1,12}", 0..40),
        columnas in 12usize..=60,
    ) {
        let texto = palabras.join(" ");
        let lineas = wrap_text(&texto, columnas);
        let reunidas: Vec<&str> = lineas.iter().flat_map(|l| l.split_whitespace()).collect();
        let originales: Vec<&str> = texto.split_whitespace().collect();
        prop_assert_eq!(reunidas, originales);
        for linea in &lineas {
            prop_assert!(linea.chars().count() <= columnas);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// clamp_first_line
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_clamp_first_line_recorta_a_0_hasta_total_menos_visibles() {
    // (first, total, visible, esperado)
    for (first, total, visible, esperado) in [
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
            esperado,
            "first {first}, total {total}, visibles {visible}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn fila_11_clamp_first_line_es_el_minimo_con_el_maximo_y_es_idempotente(
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

const PANTALLA: Vec2 = vec2(600.0, 300.0);

struct Banco {
    ctx: Context,
    dialog: Dialog,
    focus: Option<FocusTarget>,
}

/// La respuesta cierra el diálogo (`Cerrar` lo acepta y `Escape` lo cancela).
fn cierra(reaction: &Reaction) -> bool {
    matches!(reaction, Reaction::Accepted(_) | Reaction::Cancelled)
}

impl Banco {
    fn nuevo(ppp: f32, texto: &str) -> Banco {
        Banco {
            ctx: contexto(ppp),
            dialog: Dialog::new(DialogKind::Licenses, licenses_form(texto))
                .expect("la spec de Licencias es válida"),
            focus: None,
        }
    }

    fn primera(&self) -> usize {
        self.dialog.first_line(LICENSES_FIELD)
    }

    fn fotograma(&mut self, events: Vec<Event>) -> (Pintado, Reaction) {
        let mut salida = None;
        let (dialog, focus) = (&mut self.dialog, &mut self.focus);
        let formas = fotograma(&self.ctx, PANTALLA, events, |ui| {
            salida = Some(show_dialog(ui.ctx(), dialog, focus));
        });
        (pintado(&formas), salida.expect("el diálogo se dibuja"))
    }

    fn reposo(&mut self) -> Pintado {
        self.fotograma(vec![]);
        self.fotograma(vec![]).0
    }

    /// Pone el puntero sobre el texto (el centro de la pantalla, que es el centro del diálogo) y
    /// espera un fotograma estable.
    fn con_puntero(&mut self) {
        self.fotograma(vec![mover(pos2(300.0, 150.0))]);
        self.reposo();
    }

    fn rueda(&mut self, abajo: bool, muescas: usize) -> Pintado {
        let events = (0..muescas).map(|_| rueda_linea(!abajo)).collect();
        self.fotograma(events).0
    }
}

/// Texto de 200 líneas cortas: «linea 0» … «linea 199».
fn doscientas_lineas() -> String {
    (0..200)
        .map(|i| format!("linea {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Números de las líneas «linea N» que se pintan, ordenados.
fn lineas_pintadas(pintado: &Pintado) -> Vec<usize> {
    let mut v: Vec<usize> = pintado
        .textos
        .iter()
        .flat_map(|t| t.texto.lines())
        .filter_map(|l| l.trim().strip_prefix("linea ")?.parse().ok())
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn fila_11_al_abrir_se_ve_el_principio_sin_cerrar() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, &doscientas_lineas());
        let (pintado, reaccion) = banco.fotograma(vec![]);
        assert!(!cierra(&reaccion), "ppp {ppp}: se cerró solo");
        assert_eq!(banco.primera(), 0);
        let visibles = lineas_pintadas(&pintado);
        assert_eq!(visibles.first(), Some(&0), "ppp {ppp}: {visibles:?}");
        assert!(
            visibles.len() > 3,
            "ppp {ppp}: se ven {} líneas",
            visibles.len()
        );
        assert!(!visibles.contains(&199));
    }
}

#[test]
fn fila_11_la_rueda_avanza_por_lineas_y_la_primera_linea_pintada_es_first_line() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, &doscientas_lineas());
        banco.con_puntero();
        banco.rueda(true, 1);
        let tras_una = banco.primera();
        assert!(tras_una >= 1, "ppp {ppp}: una muesca hacia abajo avanza");
        let pintado = banco.reposo();
        let visibles = lineas_pintadas(&pintado);
        assert_eq!(visibles.first(), Some(&tras_una), "ppp {ppp}: {visibles:?}");
        // Líneas consecutivas.
        assert!(
            visibles.windows(2).all(|w| w[1] == w[0] + 1),
            "ppp {ppp}: {visibles:?}"
        );
        banco.rueda(true, 3);
        assert!(
            banco.primera() > tras_una,
            "ppp {ppp}: más muescas, más abajo"
        );
        let antes = banco.primera();
        banco.rueda(false, 1);
        assert!(banco.primera() < antes, "ppp {ppp}: hacia arriba retrocede");
        banco.rueda(false, 1000);
        assert_eq!(banco.primera(), 0, "ppp {ppp}: no baja de 0");
    }
}

#[test]
fn fila_11_la_rueda_se_recorta_a_total_menos_visibles_y_la_ultima_linea_queda_a_la_vista() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, &doscientas_lineas());
        banco.con_puntero();
        banco.rueda(true, 2000);
        let maximo = banco.primera();
        assert!((1..=199).contains(&maximo), "ppp {ppp}: {maximo}");
        let pintado = banco.reposo();
        let visibles = lineas_pintadas(&pintado);
        assert_eq!(
            visibles.last(),
            Some(&199),
            "ppp {ppp}: en el máximo se ve la última línea; se ven {visibles:?}"
        );
        // Un recorte exacto: ver n líneas con la primera en `maximo` agota las 200.
        assert_eq!(visibles.first(), Some(&maximo), "ppp {ppp}: {visibles:?}");
        // Otra muesca hacia abajo no se mueve.
        banco.rueda(true, 1);
        assert_eq!(banco.primera(), maximo, "ppp {ppp}: se recorta, no se pasa");
        banco.reposo();
        assert_eq!(banco.primera(), maximo);
    }
}

#[test]
fn fila_11_un_texto_corto_no_se_desplaza() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, "uno\ndos\ntres");
        banco.con_puntero();
        banco.rueda(true, 50);
        assert_eq!(banco.primera(), 0, "ppp {ppp}");
    }
}

#[test]
fn fila_11_un_first_line_fuera_de_rango_se_recorta_al_pintar() {
    let mut banco = Banco::nuevo(1.0, &doscientas_lineas());
    banco.dialog.set_first_line(LICENSES_FIELD, usize::MAX / 2);
    banco.reposo();
    assert!(banco.primera() <= 199, "{}", banco.primera());
    let pintado = banco.reposo();
    assert_eq!(lineas_pintadas(&pintado).last(), Some(&199));
}

#[test]
fn fila_11_escape_cierra_y_sin_escape_no() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, &doscientas_lineas());
        banco.reposo();
        for tecla_cualquiera in [egui::Key::A, egui::Key::Enter, egui::Key::Space] {
            let (_, reaccion) =
                banco.fotograma(vec![tecla(tecla_cualquiera, egui::Modifiers::NONE)]);
            assert!(
                !cierra(&reaccion),
                "ppp {ppp}: {tecla_cualquiera:?} no cierra"
            );
        }
        let (_, reaccion) = banco.fotograma(vec![tecla(egui::Key::Escape, egui::Modifiers::NONE)]);
        assert!(cierra(&reaccion), "ppp {ppp}: Escape cierra");
    }
}

#[test]
fn fila_11_el_boton_cerrar_cierra_con_un_clic() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, &doscientas_lineas());
        let pintado = banco.reposo();
        let cerrar = pintado.texto_con("Cerrar").unwrap_or_else(|| {
            panic!(
                "ppp {ppp}: no se pinta `Cerrar`; hay {:?}",
                pintado.todos_los_textos().len()
            )
        });
        let centro = cerrar.rect().center();
        let mut cerro = false;
        for events in [
            vec![mover(centro)],
            vec![boton(centro, true)],
            vec![boton(centro, false)],
        ] {
            let (_, reaccion) = banco.fotograma(events);
            cerro |= cierra(&reaccion);
        }
        assert!(cerro, "ppp {ppp}: el clic en `Cerrar` cierra");
    }
}

#[test]
fn fila_11_un_clic_fuera_del_boton_no_cierra() {
    let mut banco = Banco::nuevo(1.0, &doscientas_lineas());
    banco.reposo();
    let mut cerro = false;
    let pos = pos2(300.0, 150.0);
    for events in [
        vec![mover(pos)],
        vec![boton(pos, true)],
        vec![boton(pos, false)],
    ] {
        cerro |= cierra(&banco.fotograma(events).1);
    }
    assert!(!cerro);
}

#[test]
fn fila_11_el_panel_pinta_el_texto_envuelto_a_las_columnas_que_caben() {
    let texto = format!(
        "{}\n\nuna frase normal con varias palabras para ver el ajuste de las lineas\n{}",
        "x".repeat(500),
        THIRD_PARTY
    );
    for ppp in PPPS {
        let n = n_de(ppp);
        let mut banco = Banco::nuevo(ppp, &texto);
        let pintado = banco.reposo();
        // Columnas que caben como mucho: el ancho entero del lienzo entre el avance del texto.
        let columnas =
            ((f64::from(PANTALLA.x) * f64::from(ppp)) / (7.0 * n as f64)).floor() as usize;
        let mut lineas = 0;
        for t in pintado
            .textos
            .iter()
            .filter(|t| !t.texto.contains("Cerrar"))
        {
            for linea in t.texto.lines() {
                lineas += 1;
                assert!(
                    linea.chars().count() <= columnas,
                    "ppp {ppp}: una línea pintada mide {} y caben {columnas}: {linea:?}",
                    linea.chars().count()
                );
            }
        }
        assert!(lineas > 3, "ppp {ppp}: se pintan {lineas} líneas");
        // Nada se pinta fuera del rectángulo del panel.
        let pantalla = egui::Rect::from_min_size(egui::Pos2::ZERO, PANTALLA).expand(0.01);
        for t in &pintado.textos {
            assert!(
                pantalla.contains_rect(t.clip),
                "ppp {ppp}: recorte fuera de la pantalla: {:?}",
                t.clip
            );
        }
    }
}

#[test]
fn fila_11_el_texto_externo_pasa_por_printable_y_no_provoca_un_glifo_ausente() {
    // El contexto del banco entra en pánico ante un glifo ausente: `show_licenses` tiene que
    // sustituir lo que la fuente no dibuja (`replace_missing`).
    let mut banco = Banco::nuevo(1.0, "a日b ✓ c\nsegunda");
    let pintado = banco.reposo();
    let todo: String = pintado.todos_los_textos().concat();
    assert!(!todo.contains('日'), "{todo:?}");
    assert!(!todo.contains('✓'), "{todo:?}");
    assert!(todo.contains('a') && todo.contains('b'));
}
