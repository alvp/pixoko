//! Tests de los textos de los chunks auxiliares (specs/hito-2-chunks-auxiliares.md, unidad 13b):
//! filas 8 y 12 y propiedad 4, sobre `describe_open_aux` y `aux_cause`. En ROJO hasta la
//! implementación.
//!
//! **Suposiciones y ambigüedades:**
//! - «Cláusula ≤ 40» se mide sin el separador ` · `: `ignorados: text_2, Exif, animación, other`
//!   son 40 caracteres justos; con el separador serían 43.
//! - Con causa **y** chunks de texto, Exif, animación u otros, la spec no dice si la cláusula
//!   `ignorados:` se añade. Ningún test lo fija: la propiedad 4 solo comprueba, si aparece, que
//!   nombra los cuatro rótulos permitidos y mide ≤ 40, y exige la lista exacta solo sin causa.
//! - La frase de guardado de la fila 12 se construye con `describe_saved` (de `session`).

use super::*;

use std::path::PathBuf;

use proptest::prelude::*;
use proptest::sample::subsequence;
use px_formats::{AuxChunk, AuxReport};
use px_ui::edit_layout::min_window_width_pt;
use px_ui::status::columns;
use px_ui::text::INTERFACE_CHARSET;

use crate::session::{Saved, describe_saved};

// ---------------------------------------------------------------------------------------
// Ayudantes.
// ---------------------------------------------------------------------------------------

const SUFFIX: &str = "; Ctrl+S guardará una copia";

const CAUSES: [(AuxChunk, &str); 5] = [
    (AuxChunk::Icc, "perfil de color ICC ignorado"),
    (AuxChunk::CodePoints, "espacio de color cICP ignorado"),
    (AuxChunk::Gamma, "gamma no sRGB ignorada"),
    (AuxChunk::Chromaticities, "primarios no sRGB ignorados"),
    (AuxChunk::Animation, "APNG: solo el 1.er fotograma"),
];

const ALL: [AuxChunk; 11] = [
    AuxChunk::Icc,
    AuxChunk::CodePoints,
    AuxChunk::Srgb,
    AuxChunk::Gamma,
    AuxChunk::Chromaticities,
    AuxChunk::Physical,
    AuxChunk::Text,
    AuxChunk::Time,
    AuxChunk::Exif,
    AuxChunk::Animation,
    AuxChunk::Other,
];

/// Imagen RGBA de 16×16, la de los ejemplos de la spec.
fn matches() -> Image {
    Image::new(16, 16, Mode::Rgba).expect("imagen RGBA de 16×16")
}

const OPEN: &str = "abierto casa.png · 16×16 · RGBA";

fn report(found: &[AuxChunk], appearance: Option<AuxChunk>) -> AuxReport {
    AuxReport {
        found: found.to_vec(),
        appearance,
    }
}

fn notice(report: &[Conversion], aux: &AuxReport) -> String {
    describe_open_aux("casa.png", &matches(), report, aux)
}

fn times(text_2: &str, searched: &str) -> usize {
    text_2.matches(searched).count()
}

fn in_the_character_set(text_2: &str) -> bool {
    text_2.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

// ---------------------------------------------------------------------------------------
// `aux_cause`.
// ---------------------------------------------------------------------------------------

#[test]
fn aux_cause_gives_the_five_causes_and_ignored_chunk_for_the_rest() {
    for (chunk, cause) in CAUSES {
        assert_eq!(aux_cause(chunk), cause, "{chunk:?}");
    }
    for chunk in [
        AuxChunk::Srgb,
        AuxChunk::Physical,
        AuxChunk::Text,
        AuxChunk::Time,
        AuxChunk::Exif,
        AuxChunk::Other,
    ] {
        assert_eq!(aux_cause(chunk), "chunk ignorado", "{chunk:?}");
    }
}

// ---------------------------------------------------------------------------------------
// Fila 8: `describe_open_aux`.
// ---------------------------------------------------------------------------------------

#[test]
fn row_8_empty_report_is_todays_describe_open() {
    let empty = AuxReport::default();
    assert_eq!(notice(&[], &empty), OPEN);
    assert_eq!(notice(&[], &empty), describe_open("casa.png", &matches(), &[]));
    let conversion = [Conversion::RgbToRgba];
    assert_eq!(
        notice(&conversion, &empty),
        describe_open("casa.png", &matches(), &conversion)
    );
    // La forma de hoy: la coletilla va al final cuando solo hay conversiones.
    assert_eq!(
        notice(&conversion, &empty),
        format!("{OPEN} · convertido: RGB convertido a RGBA · Ctrl+S guardará una copia")
    );
}

#[test]
fn row_8_iccp_puts_the_cause_first_with_its_suffix() {
    let aux = report(&[AuxChunk::Icc], Some(AuxChunk::Icc));
    assert_eq!(
        notice(&[], &aux),
        "perfil de color ICC ignorado; Ctrl+S guardará una copia · abierto casa.png · 16×16 · RGBA"
    );
}

#[test]
fn row_8_iccp_and_conversion_do_not_repeat_the_suffix() {
    let aux = report(&[AuxChunk::Icc], Some(AuxChunk::Icc));
    let text = notice(&[Conversion::DepthExpanded { from_bits: 4 }], &aux);
    assert_eq!(
        text,
        "perfil de color ICC ignorado; Ctrl+S guardará una copia · abierto casa.png · 16×16 · RGBA · convertido: gris de 4 bits ampliado a 8"
    );
    assert_eq!(times(&text, "Ctrl+S guardará una copia"), 1);
}

#[test]
fn row_8_only_text_and_date_names_only_text() {
    let aux = report(&[AuxChunk::Text, AuxChunk::Time], None);
    assert_eq!(notice(&[], &aux), format!("{OPEN} · ignorados: texto"));
}

#[test]
fn row_8_resolution_date_and_color_chunks_without_a_cause_say_nothing() {
    let without_nothing = notice(&[], &AuxReport::default());
    for (name_2, aux) in [
        (
            "pHYs + tIME",
            report(&[AuxChunk::Physical, AuxChunk::Time], None),
        ),
        (
            "sRGB + gAMA 45455 + cHRM sRGB",
            report(
                &[AuxChunk::Srgb, AuxChunk::Gamma, AuxChunk::Chromaticities],
                None,
            ),
        ),
        ("solo cICP sRGB", report(&[AuxChunk::CodePoints], None)),
    ] {
        assert_eq!(notice(&[], &aux), without_nothing, "{name_2}");
    }
}

#[test]
fn row_8_text_exif_fctl_and_unknown_give_the_four_labels() {
    let aux = report(
        &[
            AuxChunk::Text,
            AuxChunk::Exif,
            AuxChunk::Animation,
            AuxChunk::Other,
        ],
        None,
    );
    assert_eq!(
        notice(&[], &aux),
        format!("{OPEN} · ignorados: texto, Exif, animación, otros")
    );
}

#[test]
fn row_8_the_clause_goes_after_the_conversions_when_there_is_no_cause() {
    // Tras el aviso de siempre, que con conversiones ya lleva su coletilla.
    let aux = report(&[AuxChunk::Text], None);
    let conversion = [Conversion::RgbToRgba];
    let text = notice(&conversion, &aux);
    assert!(
        text.starts_with(&describe_open("casa.png", &matches(), &conversion)),
        "{text}"
    );
    assert!(text.ends_with(" · ignorados: texto"), "{text}");
    assert_eq!(times(&text, "Ctrl+S guardará una copia"), 1);
}

#[test]
fn row_8_the_other_four_causes() {
    for (chunk, cause) in CAUSES.into_iter().skip(1) {
        let aux = report(&[chunk], Some(chunk));
        assert_eq!(
            notice(&[], &aux),
            format!("{cause}; Ctrl+S guardará una copia · {OPEN}"),
            "{chunk:?}"
        );
    }
    assert_eq!(
        notice(
            &[],
            &report(&[AuxChunk::Animation], Some(AuxChunk::Animation))
        ),
        "APNG: solo el 1.er fotograma; Ctrl+S guardará una copia · abierto casa.png · 16×16 · RGBA"
    );
}

// ---------------------------------------------------------------------------------------
// Fila 12: anchos.
// ---------------------------------------------------------------------------------------

const SCALES: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];
/// Columnas del texto de un aviso en la barra a la ventana mínima (66 menos `notice: `).
const COLUMNS_OF_TEXT: usize = 59;

fn largo(text_2: &str) -> usize {
    text_2.chars().count()
}

#[test]
fn row_12_the_bar_at_the_minimum_window_has_66_columns_at_the_five_scales() {
    for ppp in SCALES {
        let width_px = min_window_width_pt(ppp) * ppp;
        assert_eq!(columns(width_px, ppp), 66, "ppp {ppp}");
    }
}

#[test]
fn row_12_each_cause_with_its_suffix_fits_whole() {
    for (chunk, cause) in CAUSES {
        let sentence = format!("{cause}{SUFFIX}");
        assert!(
            largo(&sentence) <= COLUMNS_OF_TEXT,
            "{chunk:?}: {} columnas",
            largo(&sentence)
        );
        assert!(in_the_character_set(&sentence), "{sentence}");
        // Y es lo que `describe_open_aux` pone al principio, que es lo que no se recorta.
        let text = notice(&[], &report(&[chunk], Some(chunk)));
        assert!(text.starts_with(&sentence), "{text}");
    }
    // Las medidas de la spec.
    assert_eq!(largo(&format!("{}{SUFFIX}", CAUSES[0].1)), 55);
    assert_eq!(largo(&format!("{}{SUFFIX}", CAUSES[1].1)), 57);
    assert_eq!(largo(&format!("{}{SUFFIX}", CAUSES[2].1)), 49);
    assert_eq!(largo(&format!("{}{SUFFIX}", CAUSES[3].1)), 54);
    assert_eq!(largo(&format!("{}{SUFFIX}", CAUSES[4].1)), 55);
}

#[test]
fn row_12_the_save_sentence_with_a_15_character_name_fits_whole() {
    let name_2 = "casas-copia.png";
    assert_eq!(largo(name_2), 15);
    for (chunk, cause) in [CAUSES[0], CAUSES[4]] {
        let saved = Saved {
            path: PathBuf::from("/algun/directorio").join(name_2),
            copy_reason: Some(cause.to_string()),
        };
        let sentence = describe_saved(&saved);
        assert_eq!(
            sentence,
            format!("{cause}; guardado como {name_2}"),
            "{chunk:?}"
        );
        assert!(
            largo(&sentence) <= COLUMNS_OF_TEXT,
            "{chunk:?}: {}",
            largo(&sentence)
        );
        // Salvo el nombre, todo es texto fijo de la interfaz.
        let fixed = sentence.replace(name_2, "");
        assert!(in_the_character_set(&fixed), "{fixed}");
    }
}

#[test]
fn row_12_the_clause_with_the_four_labels_measures_40_or_less() {
    let aux = report(
        &[
            AuxChunk::Text,
            AuxChunk::Exif,
            AuxChunk::Animation,
            AuxChunk::Other,
        ],
        None,
    );
    let text = notice(&[], &aux);
    let clause = text.rsplit(" · ").next().expect("cláusula");
    assert_eq!(clause, "ignorados: texto, Exif, animación, otros");
    assert!(largo(clause) <= 40, "{} columnas", largo(clause));
    assert!(in_the_character_set(&text), "{text}");
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: textos.
// ---------------------------------------------------------------------------------------

fn aux_arbitrary() -> impl Strategy<Value = AuxReport> {
    (
        subsequence(ALL.to_vec(), 0..=ALL.len()),
        proptest::option::of(proptest::sample::select(
            CAUSES.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        )),
    )
        .prop_map(|(found, appearance)| AuxReport { found, appearance })
}

fn conversions_arbitrary() -> impl Strategy<Value = Vec<Conversion>> {
    let all = vec![
        Conversion::DepthExpanded { from_bits: 1 },
        Conversion::DepthExpanded { from_bits: 2 },
        Conversion::DepthExpanded { from_bits: 4 },
        Conversion::Depth16Reduced,
        Conversion::RgbToRgba,
        Conversion::ColorKeyToAlpha,
    ];
    subsequence(all, 0..=6)
}

proptest! {
    #[test]
    fn property_4_texts(aux in aux_arbitrary(), report in conversions_arbitrary()) {
        let image = matches();
        let text = describe_open_aux("casa.png", &image, &report, &aux);

        // Con el informe vacío es `describe_open`.
        prop_assert_eq!(
            describe_open_aux("casa.png", &image, &report, &AuxReport::default()),
            describe_open("casa.png", &image, &report)
        );

        // Empieza por la causa si y solo si hay `appearance`.
        match aux.appearance {
            Some(cause) => prop_assert!(
                text.starts_with(&format!("{}{SUFFIX} · abierto casa.png", aux_cause(cause))),
                "{}", text
            ),
            None => prop_assert!(text.starts_with("abierto casa.png"), "{}", text),
        }

        // La coletilla, una sola vez si hay causa o conversiones y ninguna si no.
        let expected_2_2 = usize::from(aux.appearance.is_some() || !report.is_empty());
        prop_assert_eq!(times(&text, "Ctrl+S guardará una copia"), expected_2_2, "{}", text);

        // La cláusula: solo los cuatro rótulos, en orden, y ≤ 40 columnas.
        if let Some(start_2) = text.find("ignorados:") {
            let clause = &text[start_2..];
            prop_assert!(largo(clause) <= 40, "{}", clause);
            prop_assert!(text[..start_2].ends_with(" · "), "{}", text);
            let labels: Vec<&str> = clause["ignorados:".len()..].split(',').map(str::trim).collect();
            let allowed = ["texto", "Exif", "animación", "otros"];
            prop_assert!(labels.iter().all(|r| allowed.contains(r)), "{:?}", labels);
            let positions: Vec<usize> = labels
                .iter()
                .filter_map(|r| allowed.iter().position(|p| p == r))
                .collect();
            prop_assert!(positions.windows(2).all(|w| w[0] < w[1]), "{:?}", labels);
        }
        // Sin causa, la cláusula es exactamente lo que haya de texto, Exif, animación y otros.
        if aux.appearance.is_none() {
            let labels: Vec<&str> = [
                (AuxChunk::Text, "texto"),
                (AuxChunk::Exif, "Exif"),
                (AuxChunk::Animation, "animación"),
                (AuxChunk::Other, "otros"),
            ]
            .into_iter()
            .filter(|(c, _)| aux.found.contains(c))
            .map(|(_, r)| r)
            .collect();
            let base = describe_open("casa.png", &image, &report);
            if labels.is_empty() {
                prop_assert_eq!(&text, &base);
            } else {
                prop_assert_eq!(&text, &format!("{base} · ignorados: {}", labels.join(", ")));
            }
        }

        // Ni resolución, ni fecha, ni chunks de color en el aviso, y todo el texto fijo en el juego.
        prop_assert!(!text.contains("resolución") && !text.contains("fecha"), "{}", text);
        prop_assert!(in_the_character_set(&text), "{}", text);
    }
}
