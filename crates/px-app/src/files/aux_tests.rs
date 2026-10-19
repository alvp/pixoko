//! Tests de los textos de los chunks auxiliares (specs/hito-2-chunks-auxiliares.md, unidad 13b):
//! filas 8 y 12 y propiedad 4, sobre `describe_open_aux` y `aux_cause`. En ROJO hasta la
//! implementación.
//!
//! **Suposiciones y ambigüedades:**
//! - «Cláusula ≤ 40» se mide sin el separador ` · `: `ignorados: texto, Exif, animación, otros`
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

const COLETILLA: &str = "; Ctrl+S guardará una copia";

const CAUSAS: [(AuxChunk, &str); 5] = [
    (AuxChunk::Icc, "perfil de color ICC ignorado"),
    (AuxChunk::CodePoints, "espacio de color cICP ignorado"),
    (AuxChunk::Gamma, "gamma no sRGB ignorada"),
    (AuxChunk::Chromaticities, "primarios no sRGB ignorados"),
    (AuxChunk::Animation, "APNG: solo el 1.er fotograma"),
];

const TODOS: [AuxChunk; 11] = [
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
fn casa() -> Image {
    Image::new(16, 16, Mode::Rgba).expect("imagen RGBA de 16×16")
}

const ABIERTO: &str = "abierto casa.png · 16×16 · RGBA";

fn informe(found: &[AuxChunk], appearance: Option<AuxChunk>) -> AuxReport {
    AuxReport {
        found: found.to_vec(),
        appearance,
    }
}

fn aviso(report: &[Conversion], aux: &AuxReport) -> String {
    describe_open_aux("casa.png", &casa(), report, aux)
}

fn veces(texto: &str, buscado: &str) -> usize {
    texto.matches(buscado).count()
}

fn en_el_juego_de_caracteres(texto: &str) -> bool {
    texto.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

// ---------------------------------------------------------------------------------------
// `aux_cause`.
// ---------------------------------------------------------------------------------------

#[test]
fn aux_cause_da_las_cinco_causas_y_chunk_ignorado_para_el_resto() {
    for (chunk, causa) in CAUSAS {
        assert_eq!(aux_cause(chunk), causa, "{chunk:?}");
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
fn fila_8_informe_vacio_es_describe_open_de_hoy() {
    let vacio = AuxReport::default();
    assert_eq!(aviso(&[], &vacio), ABIERTO);
    assert_eq!(aviso(&[], &vacio), describe_open("casa.png", &casa(), &[]));
    let conversion = [Conversion::RgbToRgba];
    assert_eq!(
        aviso(&conversion, &vacio),
        describe_open("casa.png", &casa(), &conversion)
    );
    // La forma de hoy: la coletilla va al final cuando solo hay conversiones.
    assert_eq!(
        aviso(&conversion, &vacio),
        format!("{ABIERTO} · convertido: RGB convertido a RGBA · Ctrl+S guardará una copia")
    );
}

#[test]
fn fila_8_iccp_pone_la_causa_delante_con_su_coletilla() {
    let aux = informe(&[AuxChunk::Icc], Some(AuxChunk::Icc));
    assert_eq!(
        aviso(&[], &aux),
        "perfil de color ICC ignorado; Ctrl+S guardará una copia · abierto casa.png · 16×16 · RGBA"
    );
}

#[test]
fn fila_8_iccp_y_conversion_no_repiten_la_coletilla() {
    let aux = informe(&[AuxChunk::Icc], Some(AuxChunk::Icc));
    let text = aviso(&[Conversion::DepthExpanded { from_bits: 4 }], &aux);
    assert_eq!(
        text,
        "perfil de color ICC ignorado; Ctrl+S guardará una copia · abierto casa.png · 16×16 · RGBA · convertido: gris de 4 bits ampliado a 8"
    );
    assert_eq!(veces(&text, "Ctrl+S guardará una copia"), 1);
}

#[test]
fn fila_8_solo_texto_y_fecha_nombra_solo_texto() {
    let aux = informe(&[AuxChunk::Text, AuxChunk::Time], None);
    assert_eq!(aviso(&[], &aux), format!("{ABIERTO} · ignorados: texto"));
}

#[test]
fn fila_8_resolucion_fecha_y_chunks_de_color_sin_causa_no_dicen_nada() {
    let sin_nada = aviso(&[], &AuxReport::default());
    for (nombre, aux) in [
        (
            "pHYs + tIME",
            informe(&[AuxChunk::Physical, AuxChunk::Time], None),
        ),
        (
            "sRGB + gAMA 45455 + cHRM sRGB",
            informe(
                &[AuxChunk::Srgb, AuxChunk::Gamma, AuxChunk::Chromaticities],
                None,
            ),
        ),
        ("solo cICP sRGB", informe(&[AuxChunk::CodePoints], None)),
    ] {
        assert_eq!(aviso(&[], &aux), sin_nada, "{nombre}");
    }
}

#[test]
fn fila_8_texto_exif_fctl_y_desconocido_dan_los_cuatro_rotulos() {
    let aux = informe(
        &[
            AuxChunk::Text,
            AuxChunk::Exif,
            AuxChunk::Animation,
            AuxChunk::Other,
        ],
        None,
    );
    assert_eq!(
        aviso(&[], &aux),
        format!("{ABIERTO} · ignorados: texto, Exif, animación, otros")
    );
}

#[test]
fn fila_8_la_clausula_va_tras_las_conversiones_cuando_no_hay_causa() {
    // Tras el aviso de siempre, que con conversiones ya lleva su coletilla.
    let aux = informe(&[AuxChunk::Text], None);
    let conversion = [Conversion::RgbToRgba];
    let text = aviso(&conversion, &aux);
    assert!(
        text.starts_with(&describe_open("casa.png", &casa(), &conversion)),
        "{text}"
    );
    assert!(text.ends_with(" · ignorados: texto"), "{text}");
    assert_eq!(veces(&text, "Ctrl+S guardará una copia"), 1);
}

#[test]
fn fila_8_las_otras_cuatro_causas() {
    for (chunk, causa) in CAUSAS.into_iter().skip(1) {
        let aux = informe(&[chunk], Some(chunk));
        assert_eq!(
            aviso(&[], &aux),
            format!("{causa}; Ctrl+S guardará una copia · {ABIERTO}"),
            "{chunk:?}"
        );
    }
    assert_eq!(
        aviso(
            &[],
            &informe(&[AuxChunk::Animation], Some(AuxChunk::Animation))
        ),
        "APNG: solo el 1.er fotograma; Ctrl+S guardará una copia · abierto casa.png · 16×16 · RGBA"
    );
}

// ---------------------------------------------------------------------------------------
// Fila 12: anchos.
// ---------------------------------------------------------------------------------------

const ESCALAS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];
/// Columnas del texto de un aviso en la barra a la ventana mínima (66 menos `aviso: `).
const COLUMNAS_DE_TEXTO: usize = 59;

fn largo(texto: &str) -> usize {
    texto.chars().count()
}

#[test]
fn fila_12_la_barra_a_la_ventana_minima_tiene_66_columnas_en_las_cinco_escalas() {
    for ppp in ESCALAS {
        let ancho_px = min_window_width_pt(ppp) * ppp;
        assert_eq!(columns(ancho_px, ppp), 66, "ppp {ppp}");
    }
}

#[test]
fn fila_12_cada_causa_con_su_coletilla_cabe_entera() {
    for (chunk, causa) in CAUSAS {
        let frase = format!("{causa}{COLETILLA}");
        assert!(
            largo(&frase) <= COLUMNAS_DE_TEXTO,
            "{chunk:?}: {} columnas",
            largo(&frase)
        );
        assert!(en_el_juego_de_caracteres(&frase), "{frase}");
        // Y es lo que `describe_open_aux` pone al principio, que es lo que no se recorta.
        let text = aviso(&[], &informe(&[chunk], Some(chunk)));
        assert!(text.starts_with(&frase), "{text}");
    }
    // Las medidas de la spec.
    assert_eq!(largo(&format!("{}{COLETILLA}", CAUSAS[0].1)), 55);
    assert_eq!(largo(&format!("{}{COLETILLA}", CAUSAS[1].1)), 57);
    assert_eq!(largo(&format!("{}{COLETILLA}", CAUSAS[2].1)), 49);
    assert_eq!(largo(&format!("{}{COLETILLA}", CAUSAS[3].1)), 54);
    assert_eq!(largo(&format!("{}{COLETILLA}", CAUSAS[4].1)), 55);
}

#[test]
fn fila_12_la_frase_de_guardado_con_nombre_de_15_caracteres_cabe_entera() {
    let nombre = "casas-copia.png";
    assert_eq!(largo(nombre), 15);
    for (chunk, causa) in [CAUSAS[0], CAUSAS[4]] {
        let saved = Saved {
            path: PathBuf::from("/algun/directorio").join(nombre),
            copy_reason: Some(causa.to_string()),
        };
        let frase = describe_saved(&saved);
        assert_eq!(
            frase,
            format!("{causa}; guardado como {nombre}"),
            "{chunk:?}"
        );
        assert!(
            largo(&frase) <= COLUMNAS_DE_TEXTO,
            "{chunk:?}: {}",
            largo(&frase)
        );
        // Salvo el nombre, todo es texto fijo de la interfaz.
        let fijo = frase.replace(nombre, "");
        assert!(en_el_juego_de_caracteres(&fijo), "{fijo}");
    }
}

#[test]
fn fila_12_la_clausula_con_los_cuatro_rotulos_mide_40_o_menos() {
    let aux = informe(
        &[
            AuxChunk::Text,
            AuxChunk::Exif,
            AuxChunk::Animation,
            AuxChunk::Other,
        ],
        None,
    );
    let text = aviso(&[], &aux);
    let clausula = text.rsplit(" · ").next().expect("cláusula");
    assert_eq!(clausula, "ignorados: texto, Exif, animación, otros");
    assert!(largo(clausula) <= 40, "{} columnas", largo(clausula));
    assert!(en_el_juego_de_caracteres(&text), "{text}");
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: textos.
// ---------------------------------------------------------------------------------------

fn aux_arbitrario() -> impl Strategy<Value = AuxReport> {
    (
        subsequence(TODOS.to_vec(), 0..=TODOS.len()),
        proptest::option::of(proptest::sample::select(
            CAUSAS.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        )),
    )
        .prop_map(|(found, appearance)| AuxReport { found, appearance })
}

fn conversiones_arbitrarias() -> impl Strategy<Value = Vec<Conversion>> {
    let todas = vec![
        Conversion::DepthExpanded { from_bits: 1 },
        Conversion::DepthExpanded { from_bits: 2 },
        Conversion::DepthExpanded { from_bits: 4 },
        Conversion::Depth16Reduced,
        Conversion::RgbToRgba,
        Conversion::ColorKeyToAlpha,
    ];
    subsequence(todas, 0..=6)
}

proptest! {
    #[test]
    fn propiedad_4_textos(aux in aux_arbitrario(), report in conversiones_arbitrarias()) {
        let image = casa();
        let text = describe_open_aux("casa.png", &image, &report, &aux);

        // Con el informe vacío es `describe_open`.
        prop_assert_eq!(
            describe_open_aux("casa.png", &image, &report, &AuxReport::default()),
            describe_open("casa.png", &image, &report)
        );

        // Empieza por la causa si y solo si hay `appearance`.
        match aux.appearance {
            Some(causa) => prop_assert!(
                text.starts_with(&format!("{}{COLETILLA} · abierto casa.png", aux_cause(causa))),
                "{}", text
            ),
            None => prop_assert!(text.starts_with("abierto casa.png"), "{}", text),
        }

        // La coletilla, una sola vez si hay causa o conversiones y ninguna si no.
        let esperadas = usize::from(aux.appearance.is_some() || !report.is_empty());
        prop_assert_eq!(veces(&text, "Ctrl+S guardará una copia"), esperadas, "{}", text);

        // La cláusula: solo los cuatro rótulos, en orden, y ≤ 40 columnas.
        if let Some(inicio) = text.find("ignorados:") {
            let clausula = &text[inicio..];
            prop_assert!(largo(clausula) <= 40, "{}", clausula);
            prop_assert!(text[..inicio].ends_with(" · "), "{}", text);
            let rotulos: Vec<&str> = clausula["ignorados:".len()..].split(',').map(str::trim).collect();
            let permitidos = ["texto", "Exif", "animación", "otros"];
            prop_assert!(rotulos.iter().all(|r| permitidos.contains(r)), "{:?}", rotulos);
            let posiciones: Vec<usize> = rotulos
                .iter()
                .filter_map(|r| permitidos.iter().position(|p| p == r))
                .collect();
            prop_assert!(posiciones.windows(2).all(|w| w[0] < w[1]), "{:?}", rotulos);
        }
        // Sin causa, la cláusula es exactamente lo que haya de texto, Exif, animación y otros.
        if aux.appearance.is_none() {
            let rotulos: Vec<&str> = [
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
            if rotulos.is_empty() {
                prop_assert_eq!(&text, &base);
            } else {
                prop_assert_eq!(&text, &format!("{base} · ignorados: {}", rotulos.join(", ")));
            }
        }

        // Ni resolución, ni fecha, ni chunks de color en el aviso, y todo el texto fijo en el juego.
        prop_assert!(!text.contains("resolución") && !text.contains("fecha"), "{}", text);
        prop_assert!(en_el_juego_de_caracteres(&text), "{}", text);
    }
}
