//! Tests de `convert_mode` en su parte sin egui (specs/hito-2-convertir-modo.md, unidad 20): filas 1 a
//! 6 y 9, propiedades 1, 2 y 4 y la medición `#[ignore]` de la fila 12. En ROJO hasta la
//! implementación: `convert_mode` solo declara estos módulos de prueba.
//!
//! - Fila 1: `targets`, `convert_form` por origen y `open_convert_mode`.
//! - Fila 2: `analyze` y `classify` (el informe sale de `convert_image(.., NOTHING)`).
//! - Fila 3: `needs` y `block_reason` (cobertura y motivo), con un oráculo escrito aparte.
//! - Fila 4: `allowed_of`, `plan_from_values` y el `check` del formulario.
//! - Fila 5: `report_text` y `history_warning`.
//! - Fila 6: `sync_convert` con un `Analyzer` que cuenta (cuándo se calcula).
//! - Fila 9: `start_convert`, `PendingConvert::tick` y `CONVERTING_NOTICE`.
//! - Propiedades: 1 (el informe y `Aceptar` dicen la verdad), 2 (el estado del formulario no se
//!   corrompe) y 4 (calcular solo al cambiar de destino).
//! - Fila 12 (medición): `#[ignore]`, `cargo test --release -p px-app -- --ignored --nocapture`.
//!
//! **Suposiciones y ambigüedades** (la spec no fija un detalle; se elige lo más literal y se prueba
//! lo demás por propiedades):
//! - Rótulos de las opciones de `destino`: la spec solo dice «los modos en el orden Indexado · RGBA ·
//!   Gris con alfa · Gris». Se supone que cada opción se llama como el modo, igual que en la 19
//!   (`Indexado`, `RGBA`, `Gris con alfa`, `Gris`). Es lo que el banco de egui pulsa.
//! - `#ffff` es el 4 dígitos `rgba` con `a = f` → `Rgba::from_hex` lo lee como `(255,255,255,255)`:
//!   **opaco** (la spec lo aclara); el de alfa menor es `#fff8`.
//! - Derives no listados: `AlphaChoice`, `Permissions`, `Needs`, `PaletteInfo` e `History` solo se
//!   construyen y se les leen los campos (nunca se comparan ni se imprimen enteros, salvo
//!   `PaletteInfo` dentro de `Analysis`, que es `PartialEq` y `Debug`). `SystemAnalyzer` es una
//!   estructura sin campos (`SystemAnalyzer`), como `SystemAlloc` de la 19.
//! - «Líneas» del informe: `report_text` devuelve texto con saltos de línea; se mide con
//!   `wrap_text(texto, 42)` (lo que pinta la zona de solo lectura: ≤ 7 filas de ≤ 42 caracteres) y
//!   los textos literales de D3 y D4 se buscan en el texto con los saltos pasados a espacios (un
//!   envoltorio previo de `report_text` o el de la zona no los parte).
//! - Singular y plural: la spec dice que existen y solo da el plural. Se exige que con N = 1 no
//!   salga «1 píxeles» ni «1 colores» y que salga «1 píxel» y «1 color» (español correcto).
//! - Aviso de tamaño: solo depende de `size` (no de las dimensiones de `image`); el de historia,
//!   solo de `image` y `history`. Así se prueban 8192² y 4097 × 4096 sin reservarlas.
//! - Con el informe pendiente (`analysis = None`) el texto empieza por `Calculando el informe…`; si
//!   sale además el aviso de tamaño no se dice (la spec no lo fija) y el de historia no sale.
//! - Los textos de `TooLarge` en el informe no están en la spec: solo se exige que haya una primera
//!   línea distinta de las demás y que respete juego y longitud.
//! - Un fondo hexadecimal o activo con alfa < 255 y con alfa en juego: `block_reason` no tiene texto
//!   fijado para el hexadecimal (el `check` del formulario lo rechaza antes): no se prueba.
//! - Un fondo «color activo» no opaco o sin color se rechaza siempre que esté elegido, se pierda el
//!   alfa o no (la spec lo aclara: no se deja `Aceptar` activo sin `Plan`).
//! - Durante la espera de un cálculo, el valor que devuelve `sync_convert` en los fotogramas
//!   intermedios no se prueba (solo el del fotograma del cambio y el del cálculo).

#![allow(clippy::needless_range_loop)]

use super::*;

use std::sync::OnceLock;
use std::time::Instant;

use proptest::prelude::*;
use px_core::{
    Allowed, Command, ConversionReport, ConvertError, Document, Image, Mode, Palette, Pixel, Rgba,
    convert_image,
};
use px_ui::actions::Locks;
use px_ui::color_edit::parse_hex;
use px_ui::dialog::{
    Dialog, DialogKind, Event, FieldKind, FieldSpec, Modal, Reaction, Rows, ShowWhen, Values,
};
use px_ui::dialog_keys::{FocusTarget, initial_focus};
use px_ui::licenses_view::wrap_text;
use px_ui::text::INTERFACE_CHARSET;

use crate::new_image::WARN_PIXELS;

// ---------------------------------------------------------------------------------------
// Ayudantes: modos, imágenes de la spec 9, textos y formulario.
// ---------------------------------------------------------------------------------------

const MODOS: [Mode; 4] = [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray];
const MIB: usize = 1 << 20;

/// Los destinos de `from` en el orden de D2: Indexado · RGBA · Gris con alfa · Gris sin el actual.
fn destinos(from: Mode) -> [Mode; 3] {
    let v: Vec<Mode> = MODOS.into_iter().filter(|m| *m != from).collect();
    [v[0], v[1], v[2]]
}

fn indice_de_destino(from: Mode, target: Mode) -> usize {
    destinos(from)
        .iter()
        .position(|m| *m == target)
        .expect("destino de ese origen")
}

/// Nombre del modo en las opciones de `destino` (suposición: el de la 19).
fn nombre_de_modo(mode: Mode) -> &'static str {
    match mode {
        Mode::Indexed => "Indexado",
        Mode::Rgba => "RGBA",
        Mode::GrayAlpha => "Gris con alfa",
        Mode::Gray => "Gris",
    }
}

const TODOS_LOS_CAMPOS: [&str; 6] = [
    "destino",
    "informe",
    "luma_gris_alfa",
    "luma_gris",
    "alfa_gris",
    "fondo_hex",
];

/// Los ids que tiene el formulario de cada origen (tabla de D2).
fn ids_de_formulario(from: Mode) -> Vec<&'static str> {
    match from {
        Mode::Rgba | Mode::Indexed => TODOS_LOS_CAMPOS.to_vec(),
        Mode::GrayAlpha => vec!["destino", "informe", "alfa_gris", "fondo_hex"],
        Mode::Gray => vec!["destino", "informe"],
    }
}

/// Los ids visibles que corresponden a un destino (índice) y, con destino Gris y alfa, a la opción
/// del grupo del alfa.
fn ids_visibles(from: Mode, destino: usize, alfa: usize) -> Vec<&'static str> {
    let target = destinos(from)[destino];
    let mut v = vec!["destino", "informe"];
    match (from, target) {
        (Mode::Rgba | Mode::Indexed, Mode::GrayAlpha) => v.push("luma_gris_alfa"),
        (Mode::Rgba | Mode::Indexed, Mode::Gray) => {
            v.push("luma_gris");
            v.push("alfa_gris");
            if alfa == 4 {
                v.push("fondo_hex");
            }
        }
        (Mode::GrayAlpha, Mode::Gray) => {
            v.push("alfa_gris");
            if alfa == 4 {
                v.push("fondo_hex");
            }
        }
        _ => {}
    }
    v
}

fn dialogo(from: Mode) -> Dialog {
    Dialog::new(DialogKind::ConvertMode, convert_form(from)).expect("formulario válido")
}

fn elige(d: &mut Dialog, field: &'static str, index: usize) {
    assert_eq!(d.event(Event::Choice { field, index }), Reaction::None);
}

fn marca(d: &mut Dialog, field: &'static str, on: bool) {
    assert_eq!(d.event(Event::Check { field, on }), Reaction::None);
}

fn escribe_hex(d: &mut Dialog, text: &str) {
    assert_eq!(
        d.event(Event::Hex {
            field: "fondo_hex",
            text: text.to_string()
        }),
        Reaction::None
    );
}

fn confirma_hex(d: &mut Dialog) {
    assert_eq!(
        d.event(Event::HexConfirm {
            field: "fondo_hex",
            blur: false
        }),
        Reaction::None
    );
}

fn en_el_juego(texto: &str) -> bool {
    texto.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

fn campo_de<'a>(d: &'a Dialog, id: &str) -> &'a FieldSpec {
    d.spec()
        .fields
        .iter()
        .find(|f| f.id == id)
        .unwrap_or_else(|| panic!("falta el campo {id}"))
}

fn tiene_campo(d: &Dialog, id: &str) -> bool {
    d.spec().fields.iter().any(|f| f.id == id)
}

/// El texto del informe del diálogo, leído de su spec (el `ReadOnly` que cambia `set_report`).
fn informe_de(d: &Dialog) -> String {
    match &campo_de(d, "informe").kind {
        FieldKind::ReadOnly { text, .. } => text.clone(),
        otro => panic!("el informe no es de solo lectura: {otro:?}"),
    }
}

/// Un texto con los saltos de línea pasados a espacios: para buscar un literal aunque el informe
/// se envuelva.
fn normalizado(texto: &str) -> String {
    texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn lineas_de_zona(texto: &str) -> Vec<String> {
    wrap_text(texto, 42)
}

fn comprueba_informe(texto: &str) {
    let lineas = lineas_de_zona(texto);
    assert!(
        !lineas.is_empty() && lineas.len() <= 7,
        "{} líneas en {texto:?}",
        lineas.len()
    );
    for linea in &lineas {
        assert!(linea.chars().count() <= 42, "{linea:?}");
    }
    assert!(
        en_el_juego(&texto.replace('\n', " ")),
        "fuera del juego: {texto:?}"
    );
}

// --- Imágenes (las de la spec 9, «Casos límite») ---

fn rgba_de(pixels: &[Rgba]) -> Image {
    let mut image = Image::new(pixels.len() as u32, 1, Mode::Rgba).expect("imagen RGBA");
    for (x, p) in pixels.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Rgba(*p))
            .expect("píxel");
    }
    image
}

fn gris_de(values: &[u8]) -> Image {
    let mut image = Image::new(values.len() as u32, 1, Mode::Gray).expect("imagen Gray");
    for (x, v) in values.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Gray(*v))
            .expect("píxel");
    }
    image
}

fn gris_alfa_de(values: &[(u8, u8)]) -> Image {
    let mut image = Image::new(values.len() as u32, 1, Mode::GrayAlpha).expect("imagen GrayAlpha");
    for (x, (gray, alpha)) in values.iter().enumerate() {
        image
            .set_pixel(
                x as u32,
                0,
                Pixel::GrayAlpha {
                    gray: *gray,
                    alpha: *alpha,
                },
            )
            .expect("píxel");
    }
    image
}

fn indexada_de(colors: &[Rgba], transparent: Option<u8>, pixels: &[u8]) -> Image {
    let mut palette = Palette::from_colors(colors.iter().copied()).expect("paleta");
    palette
        .set_transparent_index(transparent)
        .expect("índice transparente");
    let mut image = Image::new_indexed(pixels.len() as u32, 1, palette).expect("imagen indexada");
    for (x, i) in pixels.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Indexed(*i))
            .expect("píxel");
    }
    image
}

/// Fila 1: Gray 3×1 `[0, 127, 255]`.
fn gris_fila_1() -> Image {
    gris_de(&[0, 127, 255])
}

/// Fila 2: GrayAlpha 3×1 `[(10,0), (10,255), (200,128)]`.
fn gris_alfa_fila_2() -> Image {
    gris_alfa_de(&[(10, 0), (10, 255), (200, 128)])
}

/// Fila 7: Rgba 5×1.
fn rgba_fila_7() -> Image {
    rgba_de(&[
        Rgba::opaque(5, 5, 5),
        Rgba::new(9, 9, 9, 0),
        Rgba::opaque(5, 5, 5),
        Rgba::new(1, 2, 3, 0),
        Rgba::opaque(7, 7, 7),
    ])
}

/// Fila 9: `c(0)..c(255)` y un `(0,0,0,0)` más (257×1): 257 colores con el transparente.
fn rgba_257_colores() -> Image {
    let mut colores: Vec<Rgba> = (0..256u32)
        .map(|i| Rgba::opaque((i % 256) as u8, (i / 256) as u8, 0))
        .collect();
    colores.push(Rgba::TRANSPARENT);
    rgba_de(&colores)
}

/// Fila 11 de la 9: paleta `[(10,10,10), (200,0,0), (5,5,5)]`, transparente el índice 2.
fn indexada_fila_11(pixels: &[u8]) -> Image {
    indexada_de(
        &[
            Rgba::opaque(10, 10, 10),
            Rgba::opaque(200, 0, 0),
            Rgba::opaque(5, 5, 5),
        ],
        Some(2),
        pixels,
    )
}

fn tres_rojos() -> Image {
    rgba_de(&[Rgba::opaque(255, 0, 0); 3])
}

/// Una imagen cualquiera, pequeña, de cada modo.
fn imagen_de_modo(mode: Mode) -> Image {
    match mode {
        Mode::Rgba => rgba_de(&[
            Rgba::opaque(255, 0, 0),
            Rgba::new(0, 255, 0, 128),
            Rgba::opaque(7, 7, 7),
            Rgba::new(0, 0, 255, 128),
        ]),
        Mode::Indexed => indexada_fila_11(&[0, 1, 1, 2, 2]),
        Mode::GrayAlpha => gris_alfa_de(&[(10, 0), (10, 255), (200, 128), (30, 0)]),
        Mode::Gray => gris_fila_1(),
    }
}

/// Una imagen gris de `w × h` a ceros (1 byte por píxel).
fn gris_vacia(w: u32, h: u32) -> Image {
    Image::new(w, h, Mode::Gray).expect("imagen Gray")
}

fn perdida(color: u64, alpha: u64) -> Analysis {
    Analysis::Lossy(ConversionReport {
        color_lost: color,
        alpha_lost: alpha,
    })
}

fn exacta() -> Analysis {
    Analysis::Exact { palette: None }
}

fn exacta_con_paleta(colors: usize, transparent: bool) -> Analysis {
    Analysis::Exact {
        palette: Some(PaletteInfo {
            colors,
            transparent,
        }),
    }
}

// --- Permisos ---

const NOMBRES_DE_ALFA: [&str; 5] = ["Keep", "White", "Black", "Active", "Hex"];

/// Las cinco opciones del grupo del alfa, en el orden del formulario; `hex` es el color de `Hex`.
fn alfa_de(index: usize, hex: Rgba) -> AlphaChoice {
    match index {
        0 => AlphaChoice::Keep,
        1 => AlphaChoice::White,
        2 => AlphaChoice::Black,
        3 => AlphaChoice::Active,
        _ => AlphaChoice::Hex(hex),
    }
}

fn permisos(luma: bool, alfa: usize, hex: Rgba) -> Permissions {
    Permissions {
        luma,
        alpha: alfa_de(alfa, hex),
    }
}

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const ACTIVO_OPACO: Rgba = Rgba::opaque(0x12, 0x34, 0x56);
const ACTIVO_CON_ALFA: Rgba = Rgba::new(0xff, 0, 0, 0x80);
const ACTIVO_TRANSPARENTE: Rgba = Rgba::TRANSPARENT;

/// Los colores activos de la fila 3: opaco, con alfa, transparente y sin color.
fn activos() -> [Option<Rgba>; 4] {
    [
        Some(ACTIVO_OPACO),
        Some(ACTIVO_CON_ALFA),
        Some(ACTIVO_TRANSPARENTE),
        None,
    ]
}

// --- Textos literales de la spec ---

const PENDIENTE: &str = "calculando el informe";
const SIN_CUANTIZADOR: &str = "más de 256 colores: sin cuantizador";
const NO_CABE: &str = "la imagen convertida no cabe";
const FALTA_COLOR: &str = "falta: permitir perder el color";
const ELIGE_FONDO: &str = "falta: elegir el fondo del alfa";
const FALTA_AMBOS: &str = "falta: perder el color; fondo del alfa";
const FALTA_COLOR_Y_VALOR: &str = "falta: perder el color; fondo válido";

const INFORME_CALCULANDO: &str = "Calculando el informe…";
const INFORME_EXACTO: &str = "Sin pérdidas: la conversión es exacta.";
const INFORME_CUANTIZADOR: &str = "Esta imagen tiene más de 256 colores y una paleta indexada no los admite. Más de 256 colores: el cuantizador llega en el hito 4; hasta entonces no se puede pasar a Indexado.";
const AVISO_TAMANO: &str = "aviso: más de 16777216 px; puede tardar";
const AVISO_HISTORIA_TODO: &str = "aviso: convertir descarta el historial anterior; solo se podrá deshacer hasta tu próxima edición";
const AVISO_HISTORIA_PARTE: &str = "aviso: convertir descarta parte del historial anterior";

fn historia(limit: usize, used: usize) -> History {
    History { limit, used }
}

/// Bytes de los píxeles de una imagen (lo que cuesta un paso de conversión, sin el margen).
fn bytes_de_pixeles(image: &Image) -> usize {
    Image::byte_len(image.width(), image.height(), image.mode()).expect("tamaño válido")
}

/// El aviso de historia que da la spec (D7), calculado aparte.
fn aviso_de_historia_esperado(image: &Image, limit: usize, used: usize) -> Option<&'static str> {
    let c = bytes_de_pixeles(image) as u64 + HISTORY_MARGIN;
    let (limit, used) = (limit as u64, used as u64);
    if c > limit {
        Some(AVISO_HISTORIA_TODO)
    } else if 2 * c > limit && used + c > limit {
        Some(AVISO_HISTORIA_PARTE)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------
// Constantes.
// ---------------------------------------------------------------------------------------

#[test]
fn las_constantes_de_la_spec() {
    assert_eq!(SYNC_PIXELS, 1 << 22);
    assert_eq!(SYNC_PIXELS, 4_194_304);
    assert_eq!(WAIT_FRAMES, 2);
    assert_eq!(HISTORY_MARGIN, 2048);
    assert_eq!(WARN_PIXELS, 1 << 24, "el aviso de tamaño es el de la 19");
    assert_eq!(FIELD_TARGET, "destino");
    assert_eq!(FIELD_REPORT, "informe");
    assert_eq!(FIELD_LUMA_GRAY_ALPHA, "luma_gris_alfa");
    assert_eq!(FIELD_LUMA_GRAY, "luma_gris");
    assert_eq!(FIELD_ALPHA, "alfa_gris");
    assert_eq!(FIELD_ALPHA_HEX, "fondo_hex");
}

// ---------------------------------------------------------------------------------------
// Fila 1: formulario por origen.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_1_targets_da_los_otros_tres_modos_en_el_orden_de_la_decision_2_y_nunca_el_actual() {
    assert_eq!(
        targets(Mode::Indexed),
        [Mode::Rgba, Mode::GrayAlpha, Mode::Gray]
    );
    assert_eq!(
        targets(Mode::Rgba),
        [Mode::Indexed, Mode::GrayAlpha, Mode::Gray]
    );
    assert_eq!(
        targets(Mode::GrayAlpha),
        [Mode::Indexed, Mode::Rgba, Mode::Gray]
    );
    assert_eq!(
        targets(Mode::Gray),
        [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha]
    );
    for from in MODOS {
        assert_eq!(targets(from), destinos(from), "{from:?}");
        assert!(!targets(from).contains(&from), "{from:?}");
    }
}

#[test]
fn fila_1_convert_form_es_valido_con_su_titulo_y_botones_para_los_cuatro_origenes() {
    for from in MODOS {
        let spec = convert_form(from);
        assert_eq!(spec.title, "Convertir modo", "{from:?}");
        assert_eq!(spec.accept, "Aceptar");
        assert_eq!(spec.cancel, Some("Cancelar"));
        spec.validate()
            .unwrap_or_else(|e| panic!("{from:?}: {e:?}"));
        assert!(spec.check.is_some(), "{from:?}: lleva el check del fondo");
        assert!(
            Dialog::new(DialogKind::ConvertMode, spec).is_ok(),
            "{from:?}"
        );
    }
}

#[test]
fn fila_1_el_destino_es_el_primer_campo_un_grupo_de_tres_modos_distintos_del_actual() {
    for from in MODOS {
        let d = dialogo(from);
        let primero = &d.spec().fields[0];
        assert_eq!(primero.id, FIELD_TARGET, "{from:?}");
        assert_eq!(primero.label, "Modo de destino", "{from:?}");
        assert!(primero.shown_when.is_none());
        let FieldKind::Choice { options, default } = &primero.kind else {
            panic!("{from:?}: destino no es un grupo");
        };
        assert_eq!(options.len(), 3, "{from:?}: solo los tres modos distintos");
        assert_eq!(
            *default, 0,
            "{from:?}: la primera opción es la de por defecto"
        );
        let esperado: Vec<&str> = destinos(from).into_iter().map(nombre_de_modo).collect();
        assert_eq!(options.to_vec(), esperado, "{from:?}");
        assert!(
            !options.contains(&nombre_de_modo(from)),
            "{from:?}: el mismo modo no se ofrece"
        );
    }
}

#[test]
fn fila_1_el_informe_es_de_solo_lectura_con_siete_filas_y_va_el_segundo() {
    for from in MODOS {
        let d = dialogo(from);
        let segundo = &d.spec().fields[1];
        assert_eq!(segundo.id, FIELD_REPORT, "{from:?}");
        assert!(segundo.shown_when.is_none());
        match &segundo.kind {
            FieldKind::ReadOnly { text, rows } => {
                assert_eq!(*rows, Rows::Fixed(7), "{from:?}");
                comprueba_informe(text);
            }
            otro => panic!("{from:?}: el informe es {otro:?}"),
        }
    }
}

#[test]
fn fila_1_solo_hay_los_campos_de_la_tabla_de_la_decision_2_por_origen() {
    for from in MODOS {
        let d = dialogo(from);
        let mut reales: Vec<&str> = d.spec().fields.iter().map(|f| f.id).collect();
        let mut esperados = ids_de_formulario(from);
        reales.sort_unstable();
        esperados.sort_unstable();
        assert_eq!(reales, esperados, "{from:?}");
    }
}

#[test]
fn fila_1_cada_campo_se_ve_solo_con_el_destino_y_la_opcion_que_le_tocan() {
    for from in MODOS {
        for destino in 0..3 {
            for alfa in 0..5 {
                let mut d = dialogo(from);
                elige(&mut d, "destino", destino);
                if tiene_campo(&d, "alfa_gris") {
                    // El grupo del alfa puede cambiarse mientras es visible (destino Gris) o,
                    // como estado conservado, estando oculto: se fija desde el destino Gris.
                    let gris = indice_de_destino(from, Mode::Gray);
                    elige(&mut d, "destino", gris);
                    elige(&mut d, "alfa_gris", alfa);
                    elige(&mut d, "destino", destino);
                }
                let esperados = ids_visibles(from, destino, alfa);
                for id in TODOS_LOS_CAMPOS {
                    assert_eq!(
                        d.is_shown(id),
                        esperados.contains(&id),
                        "{from:?}, destino {destino}, alfa {alfa}: {id}"
                    );
                }
            }
        }
    }
}

#[test]
fn fila_1_fondo_hex_es_transitivo_si_el_grupo_del_alfa_se_oculta_tambien_lo_hace_el_hexadecimal() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        let mut d = dialogo(from);
        let gris = indice_de_destino(from, Mode::Gray);
        elige(&mut d, "destino", gris);
        assert!(d.is_shown("alfa_gris"), "{from:?}");
        assert!(
            !d.is_shown("fondo_hex"),
            "{from:?}: con «No perder el alfa»"
        );
        elige(&mut d, "alfa_gris", 4);
        assert!(d.is_shown("fondo_hex"), "{from:?}");
        // Se oculta el grupo (otro destino): el hexadecimal también, aunque conserve la opción 4.
        elige(&mut d, "destino", 0);
        assert!(!d.is_shown("alfa_gris"), "{from:?}");
        assert!(!d.is_shown("fondo_hex"), "{from:?}: el control está oculto");
        // Y vuelven juntos.
        elige(&mut d, "destino", gris);
        assert!(
            d.is_shown("alfa_gris") && d.is_shown("fondo_hex"),
            "{from:?}"
        );
    }
}

#[test]
fn fila_1_los_shown_when_solo_apuntan_atras_y_a_las_opciones_de_la_decision_2() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        let d = dialogo(from);
        let gris = indice_de_destino(from, Mode::Gray);
        let ShowWhen { field, choice } = campo_de(&d, "alfa_gris")
            .shown_when
            .expect("depende del destino");
        assert_eq!((field, choice), ("destino", gris), "{from:?}");
        let ShowWhen { field, choice } = campo_de(&d, "fondo_hex")
            .shown_when
            .expect("depende del alfa");
        assert_eq!((field, choice), ("alfa_gris", 4), "{from:?}");
    }
    for from in [Mode::Rgba, Mode::Indexed] {
        let d = dialogo(from);
        let ShowWhen { field, choice } = campo_de(&d, "luma_gris_alfa")
            .shown_when
            .expect("depende del destino");
        assert_eq!(
            (field, choice),
            ("destino", indice_de_destino(from, Mode::GrayAlpha)),
            "{from:?}"
        );
        let ShowWhen { field, choice } = campo_de(&d, "luma_gris")
            .shown_when
            .expect("depende del destino");
        assert_eq!(
            (field, choice),
            ("destino", indice_de_destino(from, Mode::Gray)),
            "{from:?}"
        );
    }
}

#[test]
fn fila_1_los_rotulos_y_opciones_son_los_literales_de_la_decision_2() {
    let d = dialogo(Mode::Rgba);
    for id in ["luma_gris_alfa", "luma_gris"] {
        let campo = campo_de(&d, id);
        assert_eq!(
            campo.label, "Permitir perder el color (pasar a gris)",
            "{id}"
        );
        assert_eq!(campo.kind, FieldKind::Check { default: false }, "{id}");
    }
    let alfa = campo_de(&d, "alfa_gris");
    assert_eq!(alfa.label, "Si hay alfa, componer sobre:");
    match &alfa.kind {
        FieldKind::Choice { options, default } => {
            assert_eq!(
                options.to_vec(),
                [
                    "No perder el alfa",
                    "Fondo blanco",
                    "Fondo negro",
                    "Fondo: el color activo",
                    "Fondo: un color hexadecimal",
                ]
            );
            assert_eq!(*default, 0, "«No perder el alfa» es la de por defecto");
        }
        otro => panic!("el alfa no es un grupo: {otro:?}"),
    }
    let hex = campo_de(&d, "fondo_hex");
    assert_eq!(hex.label, "Color");
    assert_eq!(
        hex.kind,
        FieldKind::Hex {
            mode: Mode::Rgba,
            default: Rgba::new(0x80, 0x80, 0x80, 0xff)
        }
    );
}

#[test]
fn fila_1_todo_texto_fijo_esta_en_el_juego_y_los_rotulos_de_casilla_y_opcion_caben() {
    for from in MODOS {
        let spec = convert_form(from);
        let mut textos: Vec<&str> = vec![spec.title, spec.accept];
        textos.extend(spec.cancel);
        for f in &spec.fields {
            textos.push(f.label);
            match &f.kind {
                FieldKind::Choice { options, .. } => {
                    for o in options.iter().copied() {
                        assert!(o.chars().count() <= 42, "{from:?}: {o:?}");
                        textos.push(o);
                    }
                }
                FieldKind::Check { .. } => {
                    assert!(f.label.chars().count() <= 42, "{from:?}: {:?}", f.label);
                }
                _ => {}
            }
        }
        for texto in textos {
            assert!(en_el_juego(texto), "{from:?}: {texto:?} fuera del juego");
        }
    }
}

#[test]
fn fila_1_al_abrir_no_hay_motivo_ninguna_casilla_marcada_y_el_alfa_en_no_perder() {
    for from in MODOS {
        let mut d = dialogo(from);
        assert_eq!(d.reason(), None, "{from:?}");
        assert!(d.is_valid(), "{from:?}");
        assert!(d.values().is_some(), "{from:?}");
        assert_eq!(d.chosen("destino"), Some(0), "{from:?}");
        // Los valores por defecto de la spec: casillas sin marcar y el alfa en la opción 0.
        for f in &d.spec().fields {
            match &f.kind {
                FieldKind::Check { default } => assert!(!default, "{from:?}: {}", f.id),
                FieldKind::Choice { default, .. } => assert_eq!(*default, 0, "{from:?}: {}", f.id),
                _ => {}
            }
        }
        // Y las casillas visibles lo leen sin marcar.
        for destino in 0..3 {
            elige(&mut d, "destino", destino);
            for id in ["luma_gris_alfa", "luma_gris"] {
                if d.is_shown(id) {
                    assert_eq!(d.checked(id), Some(false), "{from:?}: {id}");
                }
            }
            if d.is_shown("alfa_gris") {
                assert_eq!(d.chosen("alfa_gris"), Some(0), "{from:?}");
            }
        }
    }
}

#[test]
fn fila_1_open_convert_mode_abre_el_dialogo_con_el_motivo_de_calculando_y_el_foco_en_el_destino() {
    for from in MODOS {
        let mut modal = Modal::default();
        assert!(
            open_convert_mode(&mut modal, from, Locks::default()),
            "{from:?}"
        );
        assert_eq!(modal.kind(), Some(DialogKind::ConvertMode));
        let Modal::Open(d) = &modal else {
            panic!("{from:?}: cerrado");
        };
        assert_eq!(d.spec().title, "Convertir modo");
        assert_eq!(d.block(), Some(PENDIENTE), "{from:?}");
        assert_eq!(d.reason(), Some(PENDIENTE.to_string()), "{from:?}");
        assert!(!d.is_valid(), "{from:?}: Aceptar atenuado hasta calcular");
        assert_eq!(
            initial_focus(d),
            FocusTarget::Option("destino", 0),
            "{from:?}"
        );
    }
}

#[test]
fn fila_1_open_convert_mode_da_false_con_un_bloqueo_o_con_otro_dialogo_abierto() {
    for locks in [
        Locks {
            gesture: true,
            ..Locks::default()
        },
        Locks {
            dialog: true,
            ..Locks::default()
        },
        Locks {
            text_focus: true,
            ..Locks::default()
        },
        Locks {
            removal: true,
            ..Locks::default()
        },
        Locks {
            modal: true,
            ..Locks::default()
        },
    ] {
        let mut modal = Modal::default();
        assert!(
            !open_convert_mode(&mut modal, Mode::Rgba, locks),
            "{locks:?}"
        );
        assert!(!modal.is_open(), "{locks:?}");
    }
    let mut modal = Modal::default();
    assert!(open_convert_mode(&mut modal, Mode::Rgba, Locks::default()));
    let antes = modal.clone();
    assert!(!open_convert_mode(&mut modal, Mode::Gray, Locks::default()));
    assert_eq!(modal, antes, "no cambia el diálogo abierto");
}

// ---------------------------------------------------------------------------------------
// Fila 2: del informe de `convert_image` a `Analysis`.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_2_tres_rojos_a_gris_pierde_el_color_en_tres_pixeles() {
    assert_eq!(analyze(&tres_rojos(), Mode::Gray), perdida(3, 0));
}

#[test]
fn fila_2_rojo_semitransparente_a_gris_pierde_color_y_alfa_en_un_pixel() {
    let image = rgba_de(&[Rgba::new(255, 0, 0, 128)]);
    assert_eq!(analyze(&image, Mode::Gray), perdida(1, 1));
}

#[test]
fn fila_2_gris_con_alfa_de_la_fila_2_a_gris_pierde_el_alfa_en_dos_pixeles() {
    assert_eq!(analyze(&gris_alfa_fila_2(), Mode::Gray), perdida(0, 2));
}

#[test]
fn fila_2_gris_a_rgba_y_a_gris_con_alfa_es_exacta_sin_paleta() {
    for target in [Mode::Rgba, Mode::GrayAlpha] {
        assert_eq!(analyze(&gris_fila_1(), target), exacta(), "{target:?}");
    }
}

#[test]
fn fila_2_gris_a_indexado_es_exacta_con_una_paleta_nueva_de_tres_colores_sin_transparente() {
    assert_eq!(
        analyze(&gris_fila_1(), Mode::Indexed),
        exacta_con_paleta(3, false)
    );
}

#[test]
fn fila_2_rgba_de_la_fila_7_a_indexado_da_tres_colores_con_el_cero_transparente() {
    assert_eq!(
        analyze(&rgba_fila_7(), Mode::Indexed),
        exacta_con_paleta(3, true)
    );
}

#[test]
fn fila_2_rgba_de_257_colores_a_indexado_es_too_many_colors() {
    assert_eq!(
        analyze(&rgba_257_colores(), Mode::Indexed),
        Analysis::TooManyColors
    );
}

#[test]
fn fila_2_indexada_de_la_fila_11_con_unos_y_cero_a_gris_pierde_el_color_en_dos_pixeles() {
    assert_eq!(
        analyze(&indexada_fila_11(&[0, 1, 1]), Mode::Gray),
        perdida(2, 0)
    );
}

#[test]
fn fila_2_classify_de_cada_resultado() {
    let report = ConversionReport {
        color_lost: 4,
        alpha_lost: 5,
    };
    assert_eq!(
        classify(Err(ConvertError::LossNotAllowed { report })),
        Analysis::Lossy(report)
    );
    assert_eq!(
        classify(Err(ConvertError::TooManyColors { at_least: 257 })),
        Analysis::TooManyColors
    );
    assert_eq!(
        classify(Err(ConvertError::TooLarge {
            width: 16385,
            height: 16384
        })),
        Analysis::TooLarge
    );
    // `Ok`: exacta; a indexado se lee la paleta de la conversión.
    let a_gris_alfa = convert_image(&gris_fila_1(), Mode::GrayAlpha, Allowed::NOTHING);
    assert_eq!(classify(a_gris_alfa), exacta());
    let a_indexado = convert_image(&rgba_fila_7(), Mode::Indexed, Allowed::NOTHING);
    assert_eq!(classify(a_indexado), exacta_con_paleta(3, true));
    let a_indexado = convert_image(&gris_fila_1(), Mode::Indexed, Allowed::NOTHING);
    assert_eq!(classify(a_indexado), exacta_con_paleta(3, false));
}

#[test]
fn fila_2_classify_de_too_large_no_reserva_memoria_para_ninguna_dimension() {
    for (width, height) in [(1u32, 1u32), (16385, 16384), (u32::MAX, u32::MAX)] {
        assert_eq!(
            classify(Err(ConvertError::TooLarge { width, height })),
            Analysis::TooLarge
        );
    }
}

#[test]
fn fila_2_analyze_coincide_con_classify_de_convert_image_sin_permisos() {
    let casos: Vec<(Image, Mode)> = vec![
        (tres_rojos(), Mode::Gray),
        (gris_alfa_fila_2(), Mode::Gray),
        (gris_alfa_fila_2(), Mode::Indexed),
        (gris_fila_1(), Mode::Indexed),
        (rgba_fila_7(), Mode::Indexed),
        (rgba_257_colores(), Mode::Indexed),
        (indexada_fila_11(&[0, 1, 1]), Mode::Gray),
        (indexada_fila_11(&[0, 1, 1]), Mode::Rgba),
    ];
    for (image, target) in casos {
        assert_eq!(
            analyze(&image, target),
            classify(convert_image(&image, target, Allowed::NOTHING)),
            "{:?} a {target:?}",
            image.mode()
        );
    }
}

#[test]
fn fila_2_analyze_no_toca_el_documento() {
    let documento = Document::new(rgba_fila_7());
    let id = documento.main_image();
    let antes = documento.image(id).expect("imagen").clone();
    let state_id = documento.state_id();
    for target in [Mode::Indexed, Mode::Gray, Mode::GrayAlpha] {
        let _ = analyze(documento.image(id).expect("imagen"), target);
    }
    assert_eq!(documento.state_id(), state_id);
    assert!(!documento.can_undo());
    assert!(!documento.can_redo());
    assert_eq!(documento.image(id), Some(&antes));
}

#[test]
fn fila_2_el_informe_no_depende_de_los_permisos_con_nothing_sale_entero() {
    // `LossNotAllowed` lleva los dos contadores completos: con cualquier `Allowed` el informe de
    // `analyze` es el mismo (analyze usa siempre NOTHING).
    let image = rgba_de(&[
        Rgba::opaque(255, 0, 0),
        Rgba::new(0, 255, 0, 128),
        Rgba::new(1, 2, 3, 0),
        Rgba::opaque(9, 9, 9),
    ]);
    assert_eq!(analyze(&image, Mode::Gray), perdida(2, 2));
    assert_eq!(analyze(&image, Mode::GrayAlpha), perdida(2, 0));
}

// ---------------------------------------------------------------------------------------
// Fila 3: cobertura y motivo.
// ---------------------------------------------------------------------------------------

/// Las siete formas de `Analysis` que interesan: sin pérdida, con paleta, solo color, solo alfa,
/// las dos cosas y los dos imposibles.
fn analisis_de_la_fila_3() -> Vec<(&'static str, Analysis)> {
    vec![
        ("Exact", exacta()),
        ("Exact con paleta", exacta_con_paleta(5, true)),
        ("Lossy 2/0", perdida(2, 0)),
        ("Lossy 0/3", perdida(0, 3)),
        ("Lossy 2/3", perdida(2, 3)),
        ("TooManyColors", Analysis::TooManyColors),
        ("TooLarge", Analysis::TooLarge),
    ]
}

/// El motivo que da la spec (D4), calculado aparte y en el orden de D4.
fn motivo_esperado(
    analysis: Option<&Analysis>,
    luma: bool,
    alfa: usize,
    activo: Option<Rgba>,
) -> Option<String> {
    let Some(analysis) = analysis else {
        return Some(PENDIENTE.to_string());
    };
    let report = match analysis {
        Analysis::TooManyColors => return Some(SIN_CUANTIZADOR.to_string()),
        Analysis::TooLarge => return Some(NO_CABE.to_string()),
        Analysis::Exact { .. } => ConversionReport::default(),
        Analysis::Lossy(report) => *report,
    };
    let falta_color = report.color_lost > 0 && !luma;
    let falta_fondo = report.alpha_lost > 0 && alfa == 0;
    // Un fondo «color activo» sin color o con alfa es un valor inválido.
    let valor_malo = alfa == 3 && activo.is_none_or(|c| c.a < 255);
    if falta_color && falta_fondo {
        return Some(FALTA_AMBOS.to_string());
    }
    if falta_color && valor_malo {
        return Some(FALTA_COLOR_Y_VALOR.to_string());
    }
    if falta_color {
        return Some(FALTA_COLOR.to_string());
    }
    if falta_fondo {
        return Some(ELIGE_FONDO.to_string());
    }
    // Un fondo «color activo» no opaco se rechaza siempre que esté elegido, haga falta el alfa o no.
    if alfa == 3 {
        match activo {
            None => return Some("no hay color activo".to_string()),
            Some(c) if c.a < 255 => {
                return Some(format!(
                    "el color activo #{:02x}{:02x}{:02x}{:02x} tiene alfa",
                    c.r, c.g, c.b, c.a
                ));
            }
            Some(_) => {}
        }
    }
    None
}

#[test]
fn fila_3_needs_pide_color_y_alfa_solo_cuando_hay_pixeles_perdidos() {
    for (nombre, analysis, color, alfa) in [
        ("Exact", exacta(), false, false),
        (
            "Exact con paleta",
            exacta_con_paleta(3, false),
            false,
            false,
        ),
        ("Lossy 3/0", perdida(3, 0), true, false),
        ("Lossy 0/2", perdida(0, 2), false, true),
        ("Lossy 1/1", perdida(1, 1), true, true),
        ("TooManyColors", Analysis::TooManyColors, false, false),
        ("TooLarge", Analysis::TooLarge, false, false),
    ] {
        let n = needs(&analysis);
        assert_eq!((n.luma, n.alpha), (color, alfa), "{nombre}");
    }
}

#[test]
fn fila_3_exact_no_tiene_motivo_con_cualquier_permiso_marcado_salvo_un_activo_no_opaco() {
    for (nombre, analysis) in analisis_de_la_fila_3().into_iter().take(2) {
        for luma in [false, true] {
            for alfa in 0..5 {
                for activo in activos() {
                    assert_eq!(
                        block_reason(Some(&analysis), &permisos(luma, alfa, ROJO), activo),
                        // Solo un fondo «color activo» no opaco estorba, haga falta el alfa o no.
                        motivo_esperado(Some(&analysis), luma, alfa, activo),
                        "{nombre}, luma {luma}, alfa {}, activo {activo:?}",
                        NOMBRES_DE_ALFA[alfa]
                    );
                }
            }
        }
    }
}

#[test]
fn fila_3_lossy_de_solo_color_pide_la_casilla_y_nada_mas() {
    let analysis = perdida(2, 0);
    for alfa in 0..5 {
        for activo in activos() {
            // Un fondo «color activo» sin color o con alfa se suma como valor inválido.
            let esperado = if alfa == 3 && activo.is_none_or(|c| c.a < 255) {
                FALTA_COLOR_Y_VALOR
            } else {
                FALTA_COLOR
            };
            assert_eq!(
                block_reason(Some(&analysis), &permisos(false, alfa, ROJO), activo),
                Some(esperado.to_string()),
                "alfa {}",
                NOMBRES_DE_ALFA[alfa]
            );
            assert_eq!(
                block_reason(Some(&analysis), &permisos(true, alfa, ROJO), activo),
                // El fondo no hace falta, salvo que sea un «color activo» no opaco.
                motivo_esperado(Some(&analysis), true, alfa, activo),
                "alfa {}: el fondo no hace falta",
                NOMBRES_DE_ALFA[alfa]
            );
        }
    }
}

#[test]
fn fila_3_lossy_de_solo_alfa_pide_el_fondo_y_un_fondo_opaco_lo_cubre() {
    let analysis = perdida(0, 3);
    for luma in [false, true] {
        // Sin fondo: «No perder el alfa».
        assert_eq!(
            block_reason(
                Some(&analysis),
                &permisos(luma, 0, ROJO),
                Some(ACTIVO_OPACO)
            ),
            Some(ELIGE_FONDO.to_string()),
            "luma {luma}"
        );
        // Blanco, negro, hexadecimal opaco y activo opaco cubren el alfa.
        for alfa in 1..5 {
            assert_eq!(
                block_reason(
                    Some(&analysis),
                    &permisos(luma, alfa, ROJO),
                    Some(ACTIVO_OPACO)
                ),
                None,
                "luma {luma}, alfa {}",
                NOMBRES_DE_ALFA[alfa]
            );
        }
    }
}

#[test]
fn fila_3_lossy_de_color_y_alfa_lista_los_dos_y_cada_permiso_quita_el_suyo() {
    let analysis = perdida(2, 3);
    // Ni casilla ni fondo: salen los dos, en el orden color y fondo.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permisos(false, 0, ROJO),
            Some(ACTIVO_OPACO)
        ),
        Some(FALTA_AMBOS.to_string())
    );
    // Con la casilla y sin fondo: el fondo.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permisos(true, 0, ROJO),
            Some(ACTIVO_OPACO)
        ),
        Some(ELIGE_FONDO.to_string())
    );
    // Con fondo y sin casilla: el color.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permisos(false, 1, ROJO),
            Some(ACTIVO_OPACO)
        ),
        Some(FALTA_COLOR.to_string())
    );
    // Con las dos cosas: ninguno.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permisos(true, 2, ROJO),
            Some(ACTIVO_OPACO)
        ),
        None
    );
}

#[test]
fn fila_3_el_color_activo_con_alfa_da_su_valor_resuelto_y_sin_color_dice_que_no_hay() {
    let analysis = perdida(0, 1);
    let motivo = block_reason(
        Some(&analysis),
        &permisos(false, 3, ROJO),
        Some(Rgba::new(0xff, 0, 0, 0x80)),
    )
    .expect("hay motivo");
    assert_eq!(motivo, "el color activo #ff000080 tiene alfa");
    assert_eq!(motivo.chars().count(), 36);
    // La entrada transparente: `#00000000`.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permisos(false, 3, ROJO),
            Some(Rgba::TRANSPARENT)
        ),
        Some("el color activo #00000000 tiene alfa".to_string())
    );
    assert_eq!(
        block_reason(Some(&analysis), &permisos(false, 3, ROJO), None),
        Some("no hay color activo".to_string())
    );
    // Con el color activo opaco, sin motivo.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permisos(false, 3, ROJO),
            Some(ACTIVO_OPACO)
        ),
        None
    );
}

#[test]
fn fila_3_un_color_activo_no_opaco_bloquea_aunque_el_alfa_no_se_pierda() {
    // Elegido «color activo» como fondo, un color sin alfa opaco se rechaza siempre (no se deja
    // `Aceptar` activo sin `Plan`); con un color opaco no estorba.
    for analysis in [exacta(), perdida(2, 0)] {
        for activo in activos() {
            assert_eq!(
                block_reason(Some(&analysis), &permisos(true, 3, ROJO), activo),
                motivo_esperado(Some(&analysis), true, 3, activo),
                "activo {activo:?}"
            );
        }
        assert_eq!(
            block_reason(
                Some(&analysis),
                &permisos(true, 3, ROJO),
                Some(ACTIVO_OPACO)
            ),
            None
        );
        assert_eq!(
            block_reason(
                Some(&analysis),
                &permisos(true, 3, ROJO),
                Some(ACTIVO_CON_ALFA)
            ),
            Some("el color activo #ff000080 tiene alfa".to_string())
        );
        // Con otra opción del grupo, el color activo no cuenta.
        assert_eq!(
            block_reason(
                Some(&analysis),
                &permisos(true, 1, ROJO),
                Some(ACTIVO_CON_ALFA)
            ),
            None
        );
    }
}

#[test]
fn fila_3_los_imposibles_tienen_siempre_motivo_con_cualquier_permiso() {
    for (analysis, esperado) in [
        (Analysis::TooManyColors, SIN_CUANTIZADOR),
        (Analysis::TooLarge, NO_CABE),
    ] {
        for luma in [false, true] {
            for alfa in 0..5 {
                for activo in activos() {
                    assert_eq!(
                        block_reason(Some(&analysis), &permisos(luma, alfa, ROJO), activo),
                        Some(esperado.to_string()),
                        "luma {luma}, alfa {}, activo {activo:?}",
                        NOMBRES_DE_ALFA[alfa]
                    );
                }
            }
        }
    }
}

#[test]
fn fila_3_pendiente_dice_calculando_el_informe_con_cualquier_permiso() {
    for luma in [false, true] {
        for alfa in 0..5 {
            for activo in activos() {
                assert_eq!(
                    block_reason(None, &permisos(luma, alfa, ROJO), activo),
                    Some(PENDIENTE.to_string()),
                    "luma {luma}, alfa {}",
                    NOMBRES_DE_ALFA[alfa]
                );
            }
        }
    }
}

#[test]
fn fila_3_los_textos_de_los_motivos_son_los_literales_y_miden_como_mucho_37() {
    assert_eq!(PENDIENTE.chars().count(), 21);
    assert_eq!(SIN_CUANTIZADOR.chars().count(), 35);
    for texto in [
        PENDIENTE,
        SIN_CUANTIZADOR,
        NO_CABE,
        FALTA_COLOR,
        ELIGE_FONDO,
    ] {
        assert!(texto.chars().count() <= 37, "{texto:?}");
        assert!(en_el_juego(texto), "{texto:?}");
    }
}

#[test]
fn fila_3_matriz_completa_contra_el_oraculo_del_orden_de_d4() {
    let mut comprobados = 0;
    for (nombre, analysis) in analisis_de_la_fila_3() {
        for luma in [false, true] {
            for alfa in 0..5 {
                for activo in activos() {
                    let real = block_reason(Some(&analysis), &permisos(luma, alfa, ROJO), activo);
                    let esperado = motivo_esperado(Some(&analysis), luma, alfa, activo);
                    assert_eq!(
                        real, esperado,
                        "{nombre}, luma {luma}, alfa {}, activo {activo:?}",
                        NOMBRES_DE_ALFA[alfa]
                    );
                    if let Some(texto) = real {
                        let limite = if texto.starts_with("falta: ") { 44 } else { 37 };
                        assert!(texto.chars().count() <= limite, "{texto:?}");
                        assert!(en_el_juego(&texto), "{texto:?}");
                    }
                    comprobados += 1;
                }
            }
        }
    }
    // Y con el informe pendiente.
    for luma in [false, true] {
        for alfa in 0..5 {
            for activo in activos() {
                assert_eq!(
                    block_reason(None, &permisos(luma, alfa, ROJO), activo),
                    motivo_esperado(None, luma, alfa, activo)
                );
                comprobados += 1;
            }
        }
    }
    assert_eq!(comprobados, 7 * 2 * 5 * 4 + 2 * 5 * 4);
}

// ---------------------------------------------------------------------------------------
// Fila 4: del formulario a `Allowed`.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_4_allowed_of_da_el_fondo_de_cada_opcion_y_la_casilla_de_color() {
    let activo = Some(Rgba::opaque(0x12, 0x34, 0x56));
    for luma in [false, true] {
        let casos: [(AlphaChoice, Option<Allowed>); 5] = [
            (
                AlphaChoice::Keep,
                Some(Allowed {
                    luma,
                    drop_alpha: None,
                }),
            ),
            (
                AlphaChoice::White,
                Some(Allowed {
                    luma,
                    drop_alpha: Some([255, 255, 255]),
                }),
            ),
            (
                AlphaChoice::Black,
                Some(Allowed {
                    luma,
                    drop_alpha: Some([0, 0, 0]),
                }),
            ),
            (
                AlphaChoice::Active,
                Some(Allowed {
                    luma,
                    drop_alpha: Some([0x12, 0x34, 0x56]),
                }),
            ),
            (
                AlphaChoice::Hex(Rgba::opaque(0xff, 0x00, 0x00)),
                Some(Allowed {
                    luma,
                    drop_alpha: Some([255, 0, 0]),
                }),
            ),
        ];
        for (alpha, esperado) in casos {
            let perms = Permissions { luma, alpha };
            assert_eq!(allowed_of(&perms, activo), esperado, "luma {luma}");
        }
    }
}

#[test]
fn fila_4_allowed_of_da_none_si_el_fondo_no_es_opaco() {
    let activo_con_alfa = Some(ACTIVO_CON_ALFA);
    for luma in [false, true] {
        for (nombre, perms, activo) in [
            (
                "hex con alfa",
                Permissions {
                    luma,
                    alpha: AlphaChoice::Hex(Rgba::new(0xff, 0, 0, 0x80)),
                },
                None,
            ),
            (
                "activo con alfa",
                Permissions {
                    luma,
                    alpha: AlphaChoice::Active,
                },
                activo_con_alfa,
            ),
            (
                "activo transparente",
                Permissions {
                    luma,
                    alpha: AlphaChoice::Active,
                },
                Some(Rgba::TRANSPARENT),
            ),
            (
                "sin activo",
                Permissions {
                    luma,
                    alpha: AlphaChoice::Active,
                },
                None,
            ),
        ] {
            assert_eq!(allowed_of(&perms, activo), None, "{nombre}, luma {luma}");
        }
    }
}

/// Un formulario de `from` en el que se han marcado las casillas y el grupo del alfa **mientras
/// eran visibles** y se deja en el destino final (así una casilla de otro destino queda oculta
/// pero marcada).
fn preparado(
    from: Mode,
    destino_final: usize,
    luma_gris_alfa: bool,
    luma_gris: bool,
    alfa: usize,
    hex: Option<&str>,
) -> Dialog {
    let mut d = dialogo(from);
    if tiene_campo(&d, "luma_gris_alfa") {
        elige(&mut d, "destino", indice_de_destino(from, Mode::GrayAlpha));
        marca(&mut d, "luma_gris_alfa", luma_gris_alfa);
    }
    if tiene_campo(&d, "luma_gris") {
        elige(&mut d, "destino", indice_de_destino(from, Mode::Gray));
        marca(&mut d, "luma_gris", luma_gris);
    }
    if tiene_campo(&d, "alfa_gris") {
        elige(&mut d, "destino", indice_de_destino(from, Mode::Gray));
        elige(&mut d, "alfa_gris", alfa);
        if let Some(texto) = hex.filter(|_| alfa == 4) {
            escribe_hex(&mut d, texto);
        }
    }
    elige(&mut d, "destino", destino_final);
    d
}

/// El `Allowed` esperado para un destino, con el modelo de la spec escrito aparte. `None` si el
/// fondo elegido no vale (activo sin color opaco).
fn allowed_esperado(
    from: Mode,
    destino: usize,
    luma_gris_alfa: bool,
    luma_gris: bool,
    alfa: usize,
    hex_opaco: Option<Rgba>,
    activo: Option<Rgba>,
) -> Option<Allowed> {
    let target = destinos(from)[destino];
    let luma = match (from, target) {
        (Mode::Rgba | Mode::Indexed, Mode::GrayAlpha) => luma_gris_alfa,
        (Mode::Rgba | Mode::Indexed, Mode::Gray) => luma_gris,
        _ => false,
    };
    let con_grupo = target == Mode::Gray && from != Mode::Gray;
    let drop_alpha = if con_grupo {
        match alfa {
            0 => None,
            1 => Some([255, 255, 255]),
            2 => Some([0, 0, 0]),
            3 => {
                let c = activo.filter(|c| c.a == 255)?;
                Some([c.r, c.g, c.b])
            }
            _ => {
                let c = hex_opaco?;
                Some([c.r, c.g, c.b])
            }
        }
    } else {
        None
    };
    Some(Allowed { luma, drop_alpha })
}

#[test]
fn fila_4_plan_from_values_en_todas_las_combinaciones_de_casillas_y_alfa() {
    let activo = Some(Rgba::opaque(0x0a, 0x0b, 0x0c));
    let mut comprobados = 0;
    for from in MODOS {
        for destino in 0..3 {
            for luma_ga in [false, true] {
                for luma_g in [false, true] {
                    for alfa in 0..4 {
                        let d = preparado(from, destino, luma_ga, luma_g, alfa, None);
                        let values = d.values().expect("todo válido");
                        let esperado =
                            allowed_esperado(from, destino, luma_ga, luma_g, alfa, None, activo)
                                .map(|allowed| Plan {
                                    target: destinos(from)[destino],
                                    allowed,
                                });
                        let plan = plan_from_values(&values, from, activo);
                        assert_eq!(
                            plan, esperado,
                            "{from:?}, destino {destino}, ga {luma_ga}, g {luma_g}, alfa {alfa}"
                        );
                        if let Some(plan) = plan {
                            assert_ne!(plan.target, from, "nunca el modo actual");
                            assert_eq!(plan.target, targets(from)[destino]);
                        }
                        comprobados += 1;
                    }
                }
            }
        }
    }
    assert_eq!(comprobados, 4 * 3 * 2 * 2 * 4);
}

#[test]
fn fila_4_la_casilla_de_un_destino_oculto_marcada_no_entra_en_allowed() {
    for from in [Mode::Rgba, Mode::Indexed] {
        // Marcada la de «Gris» y elegido «Gris con alfa» (su propia casilla sin marcar).
        let d = preparado(
            from,
            indice_de_destino(from, Mode::GrayAlpha),
            false,
            true,
            0,
            None,
        );
        assert!(!d.is_shown("luma_gris"));
        assert_eq!(d.checked("luma_gris"), Some(true));
        let plan = plan_from_values(&d.values().expect("válido"), from, None).expect("plan");
        assert_eq!(plan.target, Mode::GrayAlpha);
        assert!(!plan.allowed.luma, "{from:?}: la casilla oculta no cuenta");
        assert_eq!(plan.allowed.drop_alpha, None);

        // Y al revés.
        let d = preparado(
            from,
            indice_de_destino(from, Mode::Gray),
            true,
            false,
            0,
            None,
        );
        assert!(!d.is_shown("luma_gris_alfa"));
        assert_eq!(d.checked("luma_gris_alfa"), Some(true));
        let plan = plan_from_values(&d.values().expect("válido"), from, None).expect("plan");
        assert_eq!(plan.target, Mode::Gray);
        assert!(!plan.allowed.luma, "{from:?}: la casilla oculta no cuenta");

        // A un destino sin casilla (Indexado, RGBA): nunca hay `luma`, aunque las dos estén marcadas.
        for target in [Mode::Indexed, Mode::Rgba] {
            let destino = destinos(from).iter().position(|m| *m == target);
            let Some(destino) = destino else { continue };
            let d = preparado(from, destino, true, true, 4, Some("#ff0000"));
            let plan = plan_from_values(&d.values().expect("válido"), from, None).expect("plan");
            assert_eq!(plan.target, target);
            assert_eq!(
                plan.allowed,
                Allowed {
                    luma: false,
                    drop_alpha: None
                },
                "{from:?} a {target:?}"
            );
        }
    }
}

#[test]
fn fila_4_el_alfa_del_grupo_oculto_no_entra_en_allowed_a_un_destino_que_conserva_el_alfa() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        // Fondo blanco elegido con destino Gris y luego destino Indexado.
        let d = preparado(from, 0, false, false, 1, None);
        let plan = plan_from_values(&d.values().expect("válido"), from, None).expect("plan");
        assert_eq!(plan.allowed.drop_alpha, None, "{from:?}");
    }
}

#[test]
fn fila_4_los_hexadecimales_de_la_spec_dan_el_fondo_o_el_error_del_alfa() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        let gris = indice_de_destino(from, Mode::Gray);
        // (texto, RGB del fondo si vale)
        for (texto, fondo) in [
            ("#ff0000", Some([255u8, 0, 0])),
            ("#fff", Some([255, 255, 255])),
            // `#ffff` son cuatro dígitos `rgba` con `a = f`: opaco.
            ("#ffff", Some([255, 255, 255])),
            ("#ff000080", None),
            ("#fff8", None),
        ] {
            let d = preparado(from, gris, true, true, 4, Some(texto));
            match fondo {
                Some(rgb) => {
                    assert_eq!(d.reason(), None, "{from:?}: {texto}");
                    let plan = plan_from_values(&d.values().expect("válido"), from, None)
                        .expect("hay plan");
                    assert_eq!(plan.target, Mode::Gray);
                    assert_eq!(plan.allowed.drop_alpha, Some(rgb), "{from:?}: {texto}");
                    assert_eq!(
                        plan.allowed.luma,
                        from != Mode::GrayAlpha,
                        "{from:?}: {texto}"
                    );
                }
                None => {
                    assert_eq!(
                        d.reason(),
                        Some("Color: el fondo no lleva alfa".to_string()),
                        "{from:?}: {texto}"
                    );
                    assert_eq!(d.values(), None, "{from:?}: {texto}: no hay Plan");
                }
            }
        }
    }
}

#[test]
fn fila_4_el_hexadecimal_con_alfa_oculto_no_estorba() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        // Con el destino Gris y el hexadecimal con alfa: error. Cambiando de destino, desaparece.
        let gris = indice_de_destino(from, Mode::Gray);
        let mut d = preparado(from, gris, false, false, 4, Some("#ff000080"));
        assert_eq!(
            d.reason(),
            Some("Color: el fondo no lleva alfa".to_string()),
            "{from:?}"
        );
        elige(&mut d, "destino", 0);
        assert_eq!(d.reason(), None, "{from:?}: el hexadecimal está oculto");
        assert!(d.values().is_some());
    }
}

#[test]
fn fila_4_un_hexadecimal_ilegible_invalida_el_formulario_con_el_motivo_del_campo() {
    for texto in ["zz", "#12", "", "#gg0000"] {
        let d = preparado(Mode::Rgba, 2, true, true, 4, Some(texto));
        let motivo = d.reason().expect("hay motivo");
        assert!(motivo.starts_with("Color: "), "{texto:?}: {motivo}");
        assert_eq!(d.values(), None, "{texto:?}");
    }
}

#[test]
fn fila_4_el_color_activo_sin_valor_o_con_alfa_no_da_plan_y_el_opaco_si() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        let d = preparado(
            from,
            indice_de_destino(from, Mode::Gray),
            true,
            true,
            3,
            None,
        );
        let values = d.values().expect("el formulario es válido");
        for (activo, esperado) in [
            (Some(Rgba::opaque(1, 2, 3)), Some([1u8, 2, 3])),
            (Some(ACTIVO_CON_ALFA), None),
            (Some(Rgba::TRANSPARENT), None),
            (None, None),
        ] {
            let plan = plan_from_values(&values, from, activo);
            match esperado {
                Some(rgb) => {
                    let plan = plan.expect("hay plan");
                    assert_eq!(plan.allowed.drop_alpha, Some(rgb), "{from:?}");
                }
                None => assert!(plan.is_none(), "{from:?}: activo {activo:?}"),
            }
        }
    }
}

#[test]
fn fila_4_sin_campos_visibles_que_falten_el_plan_es_none() {
    // Unos `Values` sin ningún campo (no hay destino): sin plan, para los cuatro orígenes.
    for from in MODOS {
        assert_eq!(
            plan_from_values(&Values::default(), from, None),
            None,
            "{from:?}"
        );
        assert_eq!(
            plan_from_values(&Values::default(), from, Some(ACTIVO_OPACO)),
            None,
            "{from:?}"
        );
    }
}

#[test]
fn fila_4_el_destino_del_plan_es_targets_de_from_en_la_posicion_elegida_nunca_el_actual() {
    for from in MODOS {
        for i in 0..3 {
            let d = preparado(from, i, false, false, 0, None);
            let plan = plan_from_values(&d.values().expect("válido"), from, None).expect("plan");
            assert_eq!(plan.target, targets(from)[i], "{from:?}, {i}");
            assert_ne!(plan.target, from, "{from:?}, {i}");
        }
    }
}

#[test]
fn fila_4_intro_en_el_dialogo_entrega_los_valores_con_el_hexadecimal_confirmado() {
    // `Accept` confirma el texto pendiente antes de entregar: el fondo sale en `Accepted`.
    let mut d = preparado(Mode::Rgba, 2, true, true, 4, Some("#102030"));
    match d.event(Event::Accept) {
        Reaction::Accepted(values) => {
            let plan = plan_from_values(&values, Mode::Rgba, None).expect("plan");
            assert_eq!(
                plan,
                Plan {
                    target: Mode::Gray,
                    allowed: Allowed {
                        luma: true,
                        drop_alpha: Some([0x10, 0x20, 0x30])
                    }
                }
            );
        }
        otra => panic!("se esperaba Accepted y salió {otra:?}"),
    }
}

#[test]
fn fila_4_confirmar_el_hexadecimal_de_un_color_valido_lo_deja_como_fondo() {
    let mut d = preparado(Mode::Indexed, 2, false, false, 4, Some("#336699"));
    confirma_hex(&mut d);
    assert!(!d.hex_pending("fondo_hex"));
    let plan = plan_from_values(&d.values().expect("válido"), Mode::Indexed, None).expect("plan");
    assert_eq!(plan.allowed.drop_alpha, Some([0x33, 0x66, 0x99]));
}

// ---------------------------------------------------------------------------------------
// Fila 5: texto del informe y aviso de historia.
// ---------------------------------------------------------------------------------------

/// El límite por defecto (64 MiB) y nada usado.
fn sin_historia() -> History {
    History {
        limit: 64 * MIB,
        used: 0,
    }
}

/// `report_text` con permisos y color activo por defecto (sin fondo) y sin aviso de historia.
fn informe(analysis: Option<&Analysis>, size: (u32, u32)) -> String {
    let image = gris_vacia(4, 4);
    report_text(
        analysis,
        size,
        &permisos(false, 0, ROJO),
        None,
        &image,
        sin_historia(),
    )
}

fn informe_con(analysis: Option<&Analysis>, perms: &Permissions, activo: Option<Rgba>) -> String {
    let image = gris_vacia(4, 4);
    report_text(analysis, (4, 4), perms, activo, &image, sin_historia())
}

fn contiene(texto: &str, buscado: &str) -> bool {
    normalizado(texto).contains(buscado)
}

#[test]
fn fila_5_pendiente_dice_calculando_el_informe_y_no_trae_aviso_de_historia() {
    let image = gris_vacia(100, 100);
    let texto = report_text(
        None,
        (100, 100),
        &permisos(false, 0, ROJO),
        None,
        &image,
        historia(0, 0),
    );
    assert!(
        normalizado(&texto).starts_with(INFORME_CALCULANDO),
        "{texto:?}"
    );
    assert!(!contiene(&texto, "historial"), "{texto:?}");
    comprueba_informe(&texto);
}

#[test]
fn fila_5_exacta_sin_paleta_dice_que_la_conversion_es_exacta() {
    let texto = informe(Some(&exacta()), (4, 4));
    assert!(contiene(&texto, INFORME_EXACTO), "{texto:?}");
    assert!(!contiene(&texto, "Pierde"), "{texto:?}");
    comprueba_informe(&texto);
}

#[test]
fn fila_5_exacta_a_indexado_dice_las_entradas_de_la_paleta_nueva() {
    let sin_transparente = informe(Some(&exacta_con_paleta(3, false)), (4, 4));
    assert!(
        contiene(
            &sin_transparente,
            "Sin pérdidas. Paleta nueva de 3 colores."
        ),
        "{sin_transparente:?}"
    );
    assert!(
        !contiene(&sin_transparente, "transparente"),
        "{sin_transparente:?}"
    );
    let con_transparente = informe(Some(&exacta_con_paleta(3, true)), (4, 4));
    assert!(
        contiene(
            &con_transparente,
            "Sin pérdidas. Paleta nueva de 3 colores, el 0 transparente."
        ),
        "{con_transparente:?}"
    );
    let grande = informe(Some(&exacta_con_paleta(256, true)), (4, 4));
    assert!(
        contiene(
            &grande,
            "Sin pérdidas. Paleta nueva de 256 colores, el 0 transparente."
        ),
        "{grande:?}"
    );
    for texto in [&sin_transparente, &con_transparente, &grande] {
        comprueba_informe(texto);
    }
}

#[test]
fn fila_5_la_paleta_de_un_color_va_en_singular() {
    let texto = informe(Some(&exacta_con_paleta(1, false)), (4, 4));
    assert!(contiene(&texto, "Paleta nueva de 1 color"), "{texto:?}");
    assert!(!contiene(&texto, "1 colores"), "{texto:?}");
    let texto = informe(Some(&exacta_con_paleta(1, true)), (4, 4));
    assert!(!contiene(&texto, "1 colores"), "{texto:?}");
}

#[test]
fn fila_5_con_perdida_dice_solo_las_perdidas_que_son_mayores_que_cero() {
    let solo_color = informe(Some(&perdida(3, 0)), (4, 4));
    assert!(
        contiene(&solo_color, "Pierde el color en 3 píxeles."),
        "{solo_color:?}"
    );
    assert!(!contiene(&solo_color, "alfa"), "{solo_color:?}");
    assert!(!contiene(&solo_color, "Sin pérdidas"), "{solo_color:?}");

    let solo_alfa = informe(Some(&perdida(0, 2)), (4, 4));
    assert!(
        contiene(&solo_alfa, "Pierde el alfa en 2 píxeles."),
        "{solo_alfa:?}"
    );
    assert!(!contiene(&solo_alfa, "Pierde el color"), "{solo_alfa:?}");

    let las_dos = informe(Some(&perdida(3, 2)), (4, 4));
    let plano = normalizado(&las_dos);
    let color = plano.find("Pierde el color en 3 píxeles.").expect("color");
    let alfa = plano.find("Pierde el alfa en 2 píxeles.").expect("alfa");
    assert!(color < alfa, "primero el color: {las_dos:?}");

    for texto in [&solo_color, &solo_alfa, &las_dos] {
        comprueba_informe(texto);
    }
}

#[test]
fn fila_5_con_un_pixel_perdido_va_en_singular() {
    let texto = informe(Some(&perdida(1, 1)), (4, 4));
    assert!(contiene(&texto, "Pierde el color en 1 píxel"), "{texto:?}");
    assert!(contiene(&texto, "Pierde el alfa en 1 píxel"), "{texto:?}");
    assert!(!contiene(&texto, "1 píxeles"), "{texto:?}");
}

#[test]
fn fila_5_el_texto_de_too_many_colors_explica_el_cuantizador_y_el_hito_4_sin_culpar() {
    let texto = informe(Some(&Analysis::TooManyColors), (4, 4));
    assert!(contiene(&texto, INFORME_CUANTIZADOR), "{texto:?}");
    assert!(contiene(&texto, "cuantizador"), "{texto:?}");
    assert!(contiene(&texto, "hito 4"), "{texto:?}");
    assert!(contiene(&texto, "más de 256 colores"), "{texto:?}");
    comprueba_informe(&texto);
}

#[test]
fn fila_5_too_large_tiene_su_primera_linea_y_respeta_juego_y_longitud() {
    let texto = informe(Some(&Analysis::TooLarge), (4, 4));
    assert!(!texto.trim().is_empty());
    for otro in [INFORME_EXACTO, INFORME_CALCULANDO, "Pierde el"] {
        assert!(!contiene(&texto, otro), "{texto:?}");
    }
    comprueba_informe(&texto);
}

#[test]
fn fila_5_el_fondo_del_alfa_sale_solo_con_fondo_opaco_elegido_y_alfa_en_juego() {
    // Alfa en juego: Lossy con `alpha_lost > 0`.
    let con_alfa = perdida(1, 1);
    let solo_color = perdida(2, 0);
    let amarillo = Rgba::opaque(0xff, 0xcc, 0x00);
    for (nombre, perms, activo, esperado) in [
        (
            "blanco",
            permisos(true, 1, ROJO),
            None,
            Some("Fondo del alfa: #ffffff"),
        ),
        (
            "negro",
            permisos(true, 2, ROJO),
            None,
            Some("Fondo del alfa: #000000"),
        ),
        (
            "hexadecimal",
            permisos(true, 4, amarillo),
            None,
            Some("Fondo del alfa: #ffcc00"),
        ),
        (
            "activo opaco",
            permisos(true, 3, ROJO),
            Some(Rgba::opaque(1, 2, 3)),
            Some("Fondo del alfa: #010203"),
        ),
        (
            "activo con alfa",
            permisos(true, 3, ROJO),
            Some(ACTIVO_CON_ALFA),
            None,
        ),
        ("activo sin color", permisos(true, 3, ROJO), None, None),
        (
            "no perder el alfa",
            permisos(true, 0, ROJO),
            Some(ACTIVO_OPACO),
            None,
        ),
    ] {
        let texto = informe_con(Some(&con_alfa), &perms, activo);
        match esperado {
            Some(linea) => assert!(contiene(&texto, linea), "{nombre}: {texto:?}"),
            None => assert!(!contiene(&texto, "Fondo del alfa"), "{nombre}: {texto:?}"),
        }
        comprueba_informe(&texto);
    }
    // Sin alfa en juego (solo color, exacta, pendiente): no sale aunque haya fondo elegido.
    for (nombre, analysis) in [
        ("solo color", Some(&solo_color)),
        ("exacta", Some(&exacta())),
        ("pendiente", None),
    ] {
        let texto = informe_con(analysis, &permisos(true, 1, ROJO), Some(ACTIVO_OPACO));
        assert!(!contiene(&texto, "Fondo del alfa"), "{nombre}: {texto:?}");
    }
}

#[test]
fn fila_5_el_orden_es_estado_fondo_tamano_e_historia() {
    let image = gris_vacia(8, 8);
    let perms = permisos(true, 2, ROJO);
    let texto = report_text(
        Some(&perdida(2, 3)),
        (4097, 4096),
        &perms,
        None,
        &image,
        historia(100, 0),
    );
    let plano = normalizado(&texto);
    let posiciones: Vec<usize> = [
        "Pierde el color en 2 píxeles.",
        "Pierde el alfa en 3 píxeles.",
        "Fondo del alfa: #000000",
        AVISO_TAMANO,
        AVISO_HISTORIA_TODO,
    ]
    .iter()
    .map(|t| {
        plano
            .find(t)
            .unwrap_or_else(|| panic!("falta {t:?} en {plano:?}"))
    })
    .collect();
    assert!(posiciones.windows(2).all(|w| w[0] < w[1]), "{posiciones:?}");
}

#[test]
fn fila_5_el_aviso_de_tamano_sale_solo_con_mas_de_2_a_la_24_pixeles() {
    for (size, avisa) in [
        ((2048u32, 2048u32), false),
        ((4096, 4096), false),
        ((4097, 4096), true),
        ((4096, 4097), true),
        ((8192, 8192), true),
        ((1, 1), false),
        ((16_777_216, 1), false),
        ((16_777_217, 1), true),
    ] {
        for analysis in [
            exacta(),
            exacta_con_paleta(3, false),
            perdida(2, 0),
            perdida(0, 1),
        ] {
            let texto = informe(Some(&analysis), size);
            assert_eq!(contiene(&texto, AVISO_TAMANO), avisa, "{size:?}: {texto:?}");
            comprueba_informe(&texto);
        }
        // Los imposibles y el pendiente nunca lo traen por debajo del umbral.
        if !avisa {
            for analysis in [
                Some(&Analysis::TooManyColors),
                Some(&Analysis::TooLarge),
                None,
            ] {
                let texto = informe(analysis, size);
                assert!(!contiene(&texto, "puede tardar"), "{size:?}: {texto:?}");
            }
        }
    }
    // El texto es el de la spec y cabe.
    assert_eq!(AVISO_TAMANO.chars().count(), 39);
}

#[test]
fn fila_5_el_aviso_de_historia_sigue_la_regla_de_d7_con_limites_pequenos() {
    // Gray 100 × 100: 10 000 bytes de píxeles, un paso cuesta c = 12 048.
    let image = gris_vacia(100, 100);
    assert_eq!(bytes_de_pixeles(&image), 10_000);
    let c = 10_000 + HISTORY_MARGIN as usize;
    for (limite, usado, esperado) in [
        // c > límite: el primero (también con el límite 0).
        (0, 0, Some(AVISO_HISTORIA_TODO)),
        (c - 1, 0, Some(AVISO_HISTORIA_TODO)),
        (c - 1, 5_000, Some(AVISO_HISTORIA_TODO)),
        // c == límite: no es el primero; el segundo solo si hay historia que no cabe.
        (c, 0, None),
        (c, 1, Some(AVISO_HISTORIA_PARTE)),
        // límite/2 < c ≤ límite.
        (2 * c - 1, 0, None),
        (2 * c - 1, c, Some(AVISO_HISTORIA_PARTE)),
        (20_000, 0, None),
        (20_000, 7_952, None), // h + c == límite: cabe
        (20_000, 7_953, Some(AVISO_HISTORIA_PARTE)),
        (20_000, 20_000, Some(AVISO_HISTORIA_PARTE)),
        // c == límite/2: no pasa de la mitad (estrictamente): ninguno, con cualquier `h`.
        (2 * c, 0, None),
        (2 * c, 2 * c, None),
        (2 * c, 100_000, None),
        // c < límite/2.
        (64 * MIB, 0, None),
        (64 * MIB, 63 * MIB, None),
    ] {
        let texto = report_text(
            Some(&exacta()),
            (100, 100),
            &permisos(false, 0, ROJO),
            None,
            &image,
            historia(limite, usado),
        );
        match esperado {
            Some(aviso) => assert!(
                contiene(&texto, aviso),
                "límite {limite}, usado {usado}: falta {aviso:?} en {texto:?}"
            ),
            None => assert!(
                !contiene(&texto, "historial"),
                "límite {limite}, usado {usado}: {texto:?}"
            ),
        }
        // Y el que da `history_warning` es el mismo.
        assert_eq!(
            history_warning(&image, historia(limite, usado)).as_deref(),
            esperado,
            "límite {limite}, usado {usado}"
        );
        assert_eq!(
            aviso_de_historia_esperado(&image, limite, usado),
            esperado,
            "el oráculo de la prueba"
        );
        comprueba_informe(&texto);
    }
}

#[test]
fn fila_5_los_dos_textos_de_historia_son_los_de_d7_y_no_salen_a_la_vez() {
    let image = gris_vacia(100, 100);
    let todo = history_warning(&image, historia(0, 0)).expect("primer aviso");
    assert_eq!(todo, AVISO_HISTORIA_TODO);
    let parte = history_warning(&image, historia(20_000, 20_000)).expect("segundo aviso");
    assert_eq!(parte, AVISO_HISTORIA_PARTE);
    for texto in [&todo, &parte] {
        assert!(en_el_juego(texto), "{texto:?}");
    }
    assert_ne!(todo, parte);
    // Cada texto cabe en la zona de siete filas (con el resto del informe se mide en el peor caso).
    assert!(lineas_de_zona(&todo).len() <= 4);
    assert!(lineas_de_zona(&parte).len() <= 2);
}

#[test]
fn fila_5_el_aviso_de_historia_solo_sale_con_exact_o_lossy() {
    let image = gris_vacia(100, 100);
    for (nombre, analysis, sale) in [
        ("Exact", Some(exacta()), true),
        ("Exact con paleta", Some(exacta_con_paleta(3, true)), true),
        ("Lossy", Some(perdida(2, 0)), true),
        ("TooManyColors", Some(Analysis::TooManyColors), false),
        ("TooLarge", Some(Analysis::TooLarge), false),
        ("pendiente", None, false),
    ] {
        // Límite 0: el primer aviso saldría con cualquier informe posible.
        let texto = report_text(
            analysis.as_ref(),
            (100, 100),
            &permisos(true, 1, ROJO),
            Some(ACTIVO_OPACO),
            &image,
            historia(0, 0),
        );
        assert_eq!(contiene(&texto, "historial"), sale, "{nombre}: {texto:?}");
        comprueba_informe(&texto);
    }
}

#[test]
fn fila_5_gris_de_4096_al_cuadrado_con_el_limite_por_defecto_no_avisa_con_ninguna_historia() {
    // 16 MiB de píxeles: c = 16 MiB + 2048 ≤ 32 MiB (la mitad del límite): ninguno, con cualquier `h`.
    let image = gris_vacia(4096, 4096);
    for usado in [0, 32 * MIB, 63 * MIB, 64 * MIB] {
        assert_eq!(
            history_warning(&image, historia(64 * MIB, usado)),
            None,
            "h = {usado}"
        );
    }
}

#[test]
#[ignore = "reserva 64 MiB y 34 MiB: cargo test -p px-app -- --ignored"]
fn fila_5_rgba_de_4096_y_de_3000_con_el_limite_por_defecto_y_h_de_0_y_de_63_mib() {
    // RGBA 4096²: 64 MiB de píxeles > límite → el primer aviso, con cualquier `h`.
    let grande = Image::new(4096, 4096, Mode::Rgba).expect("RGBA 4096²");
    for usado in [0, 63 * MIB] {
        assert_eq!(
            history_warning(&grande, historia(64 * MIB, usado)).as_deref(),
            Some(AVISO_HISTORIA_TODO),
            "h = {usado}"
        );
    }
    drop(grande);
    // RGBA 3000²: 34,3 MiB (más de la mitad del límite y menos que el límite).
    let mediana = Image::new(3000, 3000, Mode::Rgba).expect("RGBA 3000²");
    assert_eq!(
        history_warning(&mediana, historia(64 * MIB, 63 * MIB)).as_deref(),
        Some(AVISO_HISTORIA_PARTE)
    );
    assert_eq!(history_warning(&mediana, historia(64 * MIB, 0)), None);
}

#[test]
fn fila_5_con_limite_cero_el_aviso_es_el_primero_con_cualquier_imagen() {
    for image in [
        gris_vacia(1, 1),
        imagen_de_modo(Mode::Rgba),
        imagen_de_modo(Mode::Indexed),
    ] {
        for usado in [0, 1, 1000] {
            assert_eq!(
                history_warning(&image, historia(0, usado)).as_deref(),
                Some(AVISO_HISTORIA_TODO),
                "{:?}, h {usado}",
                image.mode()
            );
        }
    }
}

#[test]
fn fila_5_el_coste_de_un_paso_cuenta_los_bytes_de_pixeles_de_cada_modo() {
    // 10 × 10 en cada modo: 100, 100, 200 y 400 bytes. Con límite = c − 1 avisa y con c no.
    for (mode, bytes) in [
        (Mode::Gray, 100usize),
        (Mode::GrayAlpha, 200),
        (Mode::Rgba, 400),
    ] {
        let image = Image::new(10, 10, mode).expect("imagen");
        let c = bytes + HISTORY_MARGIN as usize;
        assert_eq!(
            history_warning(&image, historia(c - 1, 0)).as_deref(),
            Some(AVISO_HISTORIA_TODO),
            "{mode:?}"
        );
        assert_eq!(history_warning(&image, historia(c, 0)), None, "{mode:?}");
    }
    let palette =
        Palette::from_colors([Rgba::opaque(1, 2, 3), Rgba::opaque(4, 5, 6)]).expect("paleta");
    let indexada = Image::new_indexed(10, 10, palette).expect("indexada");
    let c = 100 + HISTORY_MARGIN as usize;
    assert_eq!(
        history_warning(&indexada, historia(c - 1, 0)).as_deref(),
        Some(AVISO_HISTORIA_TODO)
    );
    assert_eq!(history_warning(&indexada, historia(c, 0)), None);
}

#[test]
fn fila_5_el_caso_peor_cabe_en_siete_lineas_de_42_caracteres_y_en_el_juego() {
    // RGBA a Gris con color, alfa, fondo, tamaño e historia (primera forma).
    let image = gris_vacia(8, 8);
    for perms in [
        permisos(true, 1, ROJO),
        permisos(true, 2, ROJO),
        permisos(true, 4, ROJO),
    ] {
        let texto = report_text(
            Some(&perdida(123_456_789, 987_654_321)),
            (8192, 8192),
            &perms,
            None,
            &image,
            historia(0, 0),
        );
        comprueba_informe(&texto);
        let plano = normalizado(&texto);
        for parte in [
            "Pierde el color en 123456789 píxeles.",
            "Pierde el alfa en 987654321 píxeles.",
            "Fondo del alfa: #",
            AVISO_TAMANO,
            AVISO_HISTORIA_TODO,
        ] {
            assert!(plano.contains(parte), "falta {parte:?} en {plano:?}");
        }
        assert!(lineas_de_zona(&texto).len() <= 7);
    }
    // El mismo caso con el segundo aviso de historia.
    let texto = report_text(
        Some(&perdida(2, 3)),
        (4097, 4096),
        &permisos(true, 1, ROJO),
        None,
        &gris_vacia(100, 100),
        historia(20_000, 20_000),
    );
    comprueba_informe(&texto);
    assert!(contiene(&texto, AVISO_HISTORIA_PARTE));
}

#[test]
fn fila_5_todo_informe_posible_respeta_juego_y_siete_lineas() {
    let image = gris_vacia(100, 100);
    let mut comprobados = 0;
    for (_, analysis) in analisis_de_la_fila_3() {
        for pendiente in [false, true] {
            for size in [(4u32, 4u32), (4097, 4096), (8192, 8192)] {
                for (limite, usado) in [(0, 0), (20_000, 20_000), (64 * MIB, 0)] {
                    for alfa in 0..5 {
                        for activo in activos() {
                            let texto = report_text(
                                if pendiente { None } else { Some(&analysis) },
                                size,
                                &permisos(true, alfa, ROJO),
                                activo,
                                &image,
                                historia(limite, usado),
                            );
                            comprueba_informe(&texto);
                            comprobados += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(comprobados > 1000);
}

#[test]
fn fila_5_describe_converted_dice_el_modo_y_solo_las_perdidas_mayores_que_cero() {
    for (target, modo) in [
        (Mode::Indexed, "indexado"),
        (Mode::Rgba, "RGBA"),
        (Mode::GrayAlpha, "gris con alfa"),
        (Mode::Gray, "gris"),
    ] {
        let sin = format!("imagen convertida a {modo}");
        assert_eq!(describe_converted(target, None), sin);
        assert_eq!(describe_converted(target, Some(&exacta())), sin);
        assert_eq!(
            describe_converted(target, Some(&exacta_con_paleta(3, false))),
            sin
        );
        assert_eq!(
            describe_converted(target, Some(&perdida(3, 0))),
            format!("{sin} · perdido: color 3 px")
        );
        assert_eq!(
            describe_converted(target, Some(&perdida(0, 2))),
            format!("{sin} · perdido: alfa 2 px")
        );
        assert_eq!(
            describe_converted(target, Some(&perdida(3, 2))),
            format!("{sin} · perdido: color 3 px, alfa 2 px")
        );
    }
    // El de la spec.
    assert_eq!(
        describe_converted(Mode::GrayAlpha, Some(&perdida(3, 0))),
        "imagen convertida a gris con alfa · perdido: color 3 px"
    );
    assert!(en_el_juego(&describe_converted(
        Mode::GrayAlpha,
        Some(&perdida(3, 2))
    )));
}

// ---------------------------------------------------------------------------------------
// Fila 6: cuándo se calcula (`sync_convert` con un `Analyzer` que cuenta).
// ---------------------------------------------------------------------------------------

/// El informe que da el analizador falso, según el destino (sin tocar píxeles).
fn analisis_falso(target: Mode) -> Analysis {
    match target {
        Mode::Gray => perdida(2, 1),
        Mode::GrayAlpha => perdida(2, 0),
        Mode::Indexed => exacta_con_paleta(3, false),
        Mode::Rgba => exacta(),
    }
}

/// Un `Analyzer` falso: anota cada llamada (ancho, alto y modo de la imagen, y el destino) y da
/// `analisis_falso`.
#[derive(Default)]
struct Contador {
    llamadas: Vec<(u32, u32, Mode, Mode)>,
}

impl Analyzer for Contador {
    fn analyze(&mut self, image: &Image, target: Mode) -> Analysis {
        self.llamadas
            .push((image.width(), image.height(), image.mode(), target));
        analisis_falso(target)
    }
}

/// Un diálogo abierto con `open_convert_mode`, su `ConvertSync` y un analizador que cuenta.
struct Montaje {
    modal: Modal,
    sync: ConvertSync,
    contador: Contador,
    image: Image,
    state_id: u64,
    limit: usize,
    used: usize,
}

impl Montaje {
    fn nuevo(from: Mode, image: Image) -> Montaje {
        let mut modal = Modal::default();
        assert!(open_convert_mode(&mut modal, from, Locks::default()));
        Montaje {
            modal,
            sync: ConvertSync::new(from, 0, Some(ACTIVO_OPACO)),
            contador: Contador::default(),
            image,
            state_id: 0,
            limit: 64 * MIB,
            used: 0,
        }
    }

    fn dialogo(&self) -> &Dialog {
        match &self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("el diálogo está cerrado"),
        }
    }

    fn dialogo_mut(&mut self) -> &mut Dialog {
        match &mut self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("el diálogo está cerrado"),
        }
    }

    /// Un fotograma de `sync_convert`: `true` si hay algo que repintar.
    fn sincroniza(&mut self) -> bool {
        let Montaje {
            modal,
            sync,
            contador,
            image,
            state_id,
            limit,
            used,
            ..
        } = self;
        let Modal::Open(dialogo) = modal else {
            panic!("el diálogo está cerrado");
        };
        sync_convert(
            dialogo,
            sync,
            image,
            *state_id,
            History {
                limit: *limit,
                used: *used,
            },
            contador,
        )
    }

    fn llamadas(&self) -> usize {
        self.contador.llamadas.len()
    }

    fn destino_de_la_ultima_llamada(&self) -> Mode {
        self.contador.llamadas.last().expect("hubo una llamada").3
    }

    fn informe(&self) -> String {
        informe_de(self.dialogo())
    }

    /// Cambia el destino (índice) con un evento del diálogo.
    fn elige_destino(&mut self, index: usize) {
        elige(self.dialogo_mut(), "destino", index);
    }

    /// `true` si el diálogo está en espera de un cálculo (motivo de «calculando»).
    fn pendiente(&self) -> bool {
        self.dialogo().block() == Some(PENDIENTE)
    }

    /// `Accept` sobre una copia: `Rejected` mientras haya motivo.
    fn accept_rechazado(&self) -> bool {
        matches!(
            self.dialogo().clone().event(Event::Accept),
            Reaction::Rejected(_)
        )
    }
}

#[test]
fn fila_6_antes_del_primer_fotograma_no_hay_informe_vigente() {
    let sync = ConvertSync::new(Mode::Rgba, 0, None);
    assert!(sync.analysis().is_none());
}

#[test]
fn fila_6_al_abrir_con_una_imagen_pequena_hay_una_llamada_con_el_primer_destino() {
    for from in MODOS {
        let image = imagen_de_modo(from);
        let (w, h) = (image.width(), image.height());
        let mut m = Montaje::nuevo(from, image);
        assert!(m.pendiente(), "{from:?}: al abrir, calculando");
        assert_eq!(m.llamadas(), 0, "{from:?}: abrir no calcula");
        assert!(m.sincroniza(), "{from:?}: el primer fotograma repinta");
        assert_eq!(m.llamadas(), 1, "{from:?}");
        assert_eq!(
            m.contador.llamadas[0],
            (w, h, from, targets(from)[0]),
            "{from:?}: la imagen y el destino por defecto"
        );
        assert_eq!(
            m.sync.analysis(),
            Some(&analisis_falso(targets(from)[0])),
            "{from:?}"
        );
        assert!(!m.pendiente(), "{from:?}");
        // Sin más eventos no hay nada que hacer.
        for _ in 0..3 {
            assert!(!m.sincroniza(), "{from:?}: sin cambios no repinta");
        }
        assert_eq!(m.llamadas(), 1, "{from:?}: ni se vuelve a calcular");
    }
}

#[test]
fn fila_6_cambiar_de_destino_hace_una_llamada_con_el_destino_elegido() {
    for from in MODOS {
        let mut m = Montaje::nuevo(from, imagen_de_modo(from));
        m.sincroniza();
        let mut esperadas = 1;
        for index in [1, 2, 0, 2, 1, 0] {
            m.elige_destino(index);
            assert!(m.sincroniza(), "{from:?}: el cambio repinta");
            esperadas += 1;
            assert_eq!(m.llamadas(), esperadas, "{from:?}, destino {index}");
            assert_eq!(
                m.destino_de_la_ultima_llamada(),
                targets(from)[index],
                "{from:?}"
            );
            assert_eq!(
                m.sync.analysis(),
                Some(&analisis_falso(targets(from)[index])),
                "{from:?}, destino {index}"
            );
            // Sin más eventos, nada.
            assert!(!m.sincroniza());
            assert_eq!(m.llamadas(), esperadas);
        }
    }
}

#[test]
fn fila_6_elegir_el_mismo_destino_no_llama() {
    let mut m = Montaje::nuevo(Mode::Rgba, imagen_de_modo(Mode::Rgba));
    m.sincroniza();
    m.elige_destino(0);
    assert!(!m.sincroniza());
    assert_eq!(m.llamadas(), 1);
}

#[test]
fn fila_6_volver_al_destino_anterior_es_otro_cambio_y_otra_llamada() {
    let mut m = Montaje::nuevo(Mode::Rgba, imagen_de_modo(Mode::Rgba));
    m.sincroniza();
    for (index, esperadas) in [(1, 2), (0, 3), (1, 4)] {
        m.elige_destino(index);
        m.sincroniza();
        assert_eq!(m.llamadas(), esperadas, "destino {index}");
    }
}

#[test]
fn fila_6_cambiar_casillas_fondo_o_hexadecimal_no_llama_al_analizador() {
    let mut m = Montaje::nuevo(Mode::Rgba, imagen_de_modo(Mode::Rgba));
    m.sincroniza();

    // Destino «Gris con alfa» (perdida 2/0): la casilla cubre el color.
    m.elige_destino(1);
    m.sincroniza();
    let base = m.llamadas();
    assert_eq!(m.dialogo().block(), Some(FALTA_COLOR));
    assert!(contiene(&m.informe(), "Pierde el color en 2 píxeles."));
    marca(m.dialogo_mut(), "luma_gris_alfa", true);
    assert!(m.sincroniza(), "la casilla cambia el motivo");
    assert_eq!(m.llamadas(), base, "marcar la casilla no calcula");
    assert_eq!(m.dialogo().block(), None);
    assert!(m.dialogo().is_valid());
    marca(m.dialogo_mut(), "luma_gris_alfa", false);
    assert!(m.sincroniza());
    assert_eq!(m.llamadas(), base, "desmarcarla tampoco");
    assert_eq!(m.dialogo().block(), Some(FALTA_COLOR));

    // Destino «Gris» (perdida 2/1): color y fondo.
    m.elige_destino(2);
    m.sincroniza();
    let base = m.llamadas();
    assert_eq!(base, 3);
    assert_eq!(m.dialogo().block(), Some(FALTA_AMBOS));
    marca(m.dialogo_mut(), "luma_gris", true);
    assert!(m.sincroniza());
    assert_eq!(m.dialogo().block(), Some(ELIGE_FONDO));
    for (opcion, fondo) in [
        (1, "Fondo del alfa: #ffffff"),
        (2, "Fondo del alfa: #000000"),
        (3, "Fondo del alfa: #123456"),
        (4, "Fondo del alfa: #808080"),
    ] {
        elige(m.dialogo_mut(), "alfa_gris", opcion);
        assert!(m.sincroniza(), "opción {opcion}");
        assert_eq!(m.llamadas(), base, "opción {opcion}: no calcula");
        assert_eq!(m.dialogo().block(), None, "opción {opcion}");
        assert!(
            contiene(&m.informe(), fondo),
            "opción {opcion}: {:?}",
            m.informe()
        );
    }
    // Un hexadecimal confirmado, válido y con alfa, y uno ilegible.
    escribe_hex(m.dialogo_mut(), "#ff0000");
    confirma_hex(m.dialogo_mut());
    m.sincroniza();
    assert_eq!(m.llamadas(), base);
    assert!(
        contiene(&m.informe(), "Fondo del alfa: #ff0000"),
        "{:?}",
        m.informe()
    );
    escribe_hex(m.dialogo_mut(), "zz");
    m.sincroniza();
    assert_eq!(m.llamadas(), base);
    assert!(!m.dialogo().is_valid());
    escribe_hex(m.dialogo_mut(), "#ff000080");
    confirma_hex(m.dialogo_mut());
    m.sincroniza();
    assert_eq!(
        m.llamadas(),
        base,
        "ni con el alfa que el formulario rechaza"
    );
    assert_eq!(
        m.dialogo().reason(),
        Some("Color: el fondo no lleva alfa".to_string())
    );
    assert!(
        !contiene(&m.informe(), "Fondo del alfa"),
        "{:?}",
        m.informe()
    );
    // «No perder el alfa» vuelve a pedir el fondo, sin llamar.
    elige(m.dialogo_mut(), "alfa_gris", 0);
    m.sincroniza();
    assert_eq!(m.llamadas(), base);
    assert_eq!(m.dialogo().block(), Some(ELIGE_FONDO));
}

#[test]
fn fila_6_con_2_a_la_22_pixeles_se_calcula_en_el_mismo_fotograma() {
    let mut m = Montaje::nuevo(Mode::Gray, gris_vacia(2048, 2048));
    assert_eq!(
        2048u64 * 2048,
        SYNC_PIXELS,
        "la imagen de la prueba mide justo SYNC_PIXELS"
    );
    assert!(m.sincroniza());
    assert_eq!(m.llamadas(), 1, "hasta 2²² píxeles: inmediato");
    assert!(!m.pendiente());
    assert!(m.sync.analysis().is_some());
}

#[test]
fn fila_6_por_encima_de_2_a_la_22_la_llamada_llega_wait_frames_fotogramas_despues() {
    let mut m = Montaje::nuevo(Mode::Gray, gris_vacia(2048, 2049));
    const { assert!(2048u64 * 2049 > SYNC_PIXELS) };
    let mut fotogramas = 0usize;
    // Del fotograma del cambio (el primero) al anterior al cálculo: «Calculando».
    for fotograma in 0..WAIT_FRAMES {
        m.sincroniza();
        fotogramas += 1;
        assert_eq!(m.llamadas(), 0, "fotograma {fotograma}: aún sin llamar");
        assert!(m.pendiente(), "fotograma {fotograma}");
        assert_eq!(m.dialogo().reason(), Some(PENDIENTE.to_string()));
        assert!(
            !m.dialogo().is_valid(),
            "fotograma {fotograma}: Aceptar atenuado"
        );
        assert!(
            m.accept_rechazado(),
            "fotograma {fotograma}: Accept da Rejected"
        );
        assert!(
            contiene(&m.informe(), INFORME_CALCULANDO),
            "fotograma {fotograma}: {:?}",
            m.informe()
        );
    }
    // Exactamente `WAIT_FRAMES` fotogramas después del primero: la llamada.
    let repinta = m.sincroniza();
    fotogramas += 1;
    assert_eq!(fotogramas, usize::from(WAIT_FRAMES) + 1);
    assert!(repinta, "el informe ya está: hay que repintar");
    assert_eq!(
        m.llamadas(),
        1,
        "la llamada llega WAIT_FRAMES fotogramas después"
    );
    assert_eq!(m.destino_de_la_ultima_llamada(), Mode::Indexed);
    assert!(!m.pendiente());
    assert_eq!(m.dialogo().block(), None);
    assert!(m.dialogo().is_valid());
    assert!(
        contiene(&m.informe(), "Sin pérdidas. Paleta nueva de 3 colores."),
        "{:?}",
        m.informe()
    );
    // Y después no se repite ni repinta.
    for _ in 0..3 {
        assert!(!m.sincroniza());
        assert_eq!(m.llamadas(), 1);
    }
}

#[test]
fn fila_6_con_2048_por_2049_cambiar_de_destino_vuelve_a_esperar() {
    let mut m = Montaje::nuevo(Mode::Gray, gris_vacia(2048, 2049));
    for _ in 0..=WAIT_FRAMES {
        m.sincroniza();
    }
    assert_eq!(m.llamadas(), 1);
    m.elige_destino(1);
    assert!(m.sincroniza(), "el cambio repinta con el aviso");
    assert_eq!(m.llamadas(), 1, "el fotograma del cambio no calcula");
    assert!(m.pendiente());
    assert!(contiene(&m.informe(), INFORME_CALCULANDO));
    assert!(m.accept_rechazado());
    for _ in 1..WAIT_FRAMES {
        m.sincroniza();
        assert_eq!(m.llamadas(), 1);
        assert!(m.pendiente());
    }
    assert!(m.sincroniza());
    assert_eq!(m.llamadas(), 2);
    assert_eq!(m.destino_de_la_ultima_llamada(), Mode::Rgba);
    assert!(!m.pendiente());
}

#[test]
fn fila_6_cambiar_de_destino_dos_veces_durante_la_espera_la_reinicia_sin_llamar() {
    let mut m = Montaje::nuevo(Mode::Gray, gris_vacia(2048, 2049));
    for _ in 0..=WAIT_FRAMES {
        m.sincroniza();
    }
    assert_eq!(m.llamadas(), 1);

    // Primer cambio y espera hasta justo antes del fotograma del cálculo.
    m.elige_destino(1);
    m.sincroniza();
    for _ in 1..WAIT_FRAMES {
        m.sincroniza();
        assert_eq!(m.llamadas(), 1);
    }
    // En el fotograma en que habría llegado la llamada, el usuario cambia otra vez: se reinicia.
    m.elige_destino(2);
    m.sincroniza();
    assert_eq!(m.llamadas(), 1, "el cambio reinicia la espera sin calcular");
    assert!(m.pendiente(), "sigue calculando");
    for _ in 1..WAIT_FRAMES {
        m.sincroniza();
        assert_eq!(m.llamadas(), 1, "la espera nueva aún no ha terminado");
        assert!(m.pendiente());
    }
    m.sincroniza();
    assert_eq!(
        m.llamadas(),
        2,
        "WAIT_FRAMES fotogramas después del último cambio"
    );
    // Nunca se calculó el destino intermedio.
    let destinos_calculados: Vec<Mode> = m.contador.llamadas.iter().map(|l| l.3).collect();
    assert_eq!(destinos_calculados, [Mode::Indexed, Mode::GrayAlpha]);
    assert!(!m.pendiente());
}

#[test]
fn fila_6_un_state_id_distinto_descarta_el_informe_y_recalcula() {
    // Imagen pequeña: se recalcula en el mismo fotograma.
    let mut m = Montaje::nuevo(Mode::Rgba, imagen_de_modo(Mode::Rgba));
    m.sincroniza();
    assert_eq!(m.llamadas(), 1);
    m.state_id = 7;
    // El informe sale igual: puede que no haya nada que repintar (no se asegura el valor).
    m.sincroniza();
    assert_eq!(m.llamadas(), 2);
    assert_eq!(m.destino_de_la_ultima_llamada(), Mode::Indexed);
    assert!(!m.sincroniza());
    assert_eq!(
        m.llamadas(),
        2,
        "con el mismo state_id no vuelve a calcular"
    );

    // Imagen grande: pasa por la espera de siempre.
    let mut m = Montaje::nuevo(Mode::Gray, gris_vacia(2048, 2049));
    for _ in 0..=WAIT_FRAMES {
        m.sincroniza();
    }
    assert_eq!(m.llamadas(), 1);
    m.state_id = 7;
    m.sincroniza();
    assert_eq!(m.llamadas(), 1);
    assert!(m.pendiente(), "el informe vigente se descartó");
    for _ in 1..WAIT_FRAMES {
        m.sincroniza();
        assert_eq!(m.llamadas(), 1);
    }
    m.sincroniza();
    assert_eq!(m.llamadas(), 2);
    assert!(!m.pendiente());
}

#[test]
fn fila_6_sync_convert_devuelve_true_solo_si_hay_algo_que_repintar() {
    let mut m = Montaje::nuevo(Mode::Rgba, imagen_de_modo(Mode::Rgba));
    assert!(m.sincroniza(), "primer fotograma");
    for _ in 0..3 {
        assert!(!m.sincroniza(), "reposo");
    }
    m.elige_destino(1);
    assert!(m.sincroniza(), "otro destino");
    assert!(!m.sincroniza());
    marca(m.dialogo_mut(), "luma_gris_alfa", true);
    assert!(m.sincroniza(), "la casilla cambia el motivo");
    assert!(!m.sincroniza());
    // La misma casilla marcada otra vez: sin cambios.
    marca(m.dialogo_mut(), "luma_gris_alfa", true);
    assert!(!m.sincroniza(), "nada cambia");
    marca(m.dialogo_mut(), "luma_gris_alfa", false);
    assert!(m.sincroniza(), "desmarcar vuelve a pedir el permiso");
    assert!(!m.sincroniza());
}

#[test]
fn fila_6_el_analizador_recibe_la_imagen_y_nunca_una_copia_modificada() {
    let image = imagen_de_modo(Mode::Indexed);
    let copia = image.clone();
    let mut m = Montaje::nuevo(Mode::Indexed, image);
    m.sincroniza();
    m.elige_destino(2);
    m.sincroniza();
    assert_eq!(m.image, copia, "el diálogo no modifica la imagen");
    for llamada in &m.contador.llamadas {
        assert_eq!((llamada.0, llamada.1, llamada.2), (5, 1, Mode::Indexed));
    }
}

// ---------------------------------------------------------------------------------------
// Fila 9: aceptar en dos tiempos (funciones puras).
// ---------------------------------------------------------------------------------------

fn plan_de_prueba() -> Plan {
    Plan {
        target: Mode::Gray,
        allowed: Allowed {
            luma: true,
            drop_alpha: Some([1, 2, 3]),
        },
    }
}

#[test]
fn fila_9_hasta_2_a_la_22_pixeles_se_convierte_en_el_mismo_fotograma() {
    let plan = plan_de_prueba();
    for pixels in [1u64, 100, SYNC_PIXELS - 1, SYNC_PIXELS, 2048 * 2048] {
        assert_eq!(start_convert(plan, pixels), Start::Now(plan), "{pixels}");
    }
}

#[test]
fn fila_9_por_encima_de_2_a_la_22_es_later_y_tick_da_el_plan_exactamente_en_el_wait_frames_esimo() {
    let plan = plan_de_prueba();
    for pixels in [
        SYNC_PIXELS + 1,
        2048 * 2048 + 1,
        2048 * 2049,
        4096 * 4096,
        4097 * 4096,
        8192 * 8192,
        u64::MAX,
    ] {
        let Start::Later(mut pendiente) = start_convert(plan, pixels) else {
            panic!("{pixels}: debía esperar");
        };
        assert_eq!(pendiente.plan(), plan, "{pixels}");
        let mut entregas = 0;
        for tick in 1..=usize::from(WAIT_FRAMES) + 6 {
            let resultado = pendiente.tick();
            if tick == usize::from(WAIT_FRAMES) {
                assert_eq!(resultado, Some(plan), "{pixels}: tick {tick}");
                entregas += 1;
            } else {
                assert_eq!(resultado, None, "{pixels}: tick {tick}");
            }
        }
        assert_eq!(entregas, 1, "{pixels}: el plan se entrega una sola vez");
    }
}

#[test]
fn fila_9_el_plan_que_sale_es_el_que_entro_con_su_allowed_incluido() {
    for allowed in [
        Allowed::NOTHING,
        Allowed {
            luma: true,
            drop_alpha: None,
        },
        Allowed {
            luma: false,
            drop_alpha: Some([255, 255, 255]),
        },
        Allowed {
            luma: true,
            drop_alpha: Some([0, 0, 0]),
        },
    ] {
        for target in MODOS {
            let plan = Plan { target, allowed };
            let Start::Later(mut pendiente) = start_convert(plan, 8192 * 8192) else {
                panic!("debía esperar");
            };
            let mut entregado = None;
            for _ in 0..WAIT_FRAMES {
                entregado = pendiente.tick().or(entregado);
            }
            assert_eq!(entregado, Some(plan));
        }
    }
}

#[test]
fn fila_9_pasado_el_wait_frames_esimo_tick_no_repite() {
    let plan = plan_de_prueba();
    let Start::Later(mut pendiente) = start_convert(plan, 4096 * 4096) else {
        panic!("debía esperar");
    };
    for _ in 0..WAIT_FRAMES {
        pendiente.tick();
    }
    for _ in 0..20 {
        assert_eq!(pendiente.tick(), None);
    }
}

#[test]
fn fila_9_start_convert_es_determinista_y_pending_convert_se_clona_con_su_cuenta() {
    let plan = plan_de_prueba();
    assert_eq!(
        start_convert(plan, 8192 * 8192),
        start_convert(plan, 8192 * 8192)
    );
    let Start::Later(original) = start_convert(plan, 8192 * 8192) else {
        panic!("debía esperar");
    };
    let mut copia = original.clone();
    assert_eq!(copia, original);
    copia.tick();
    assert_ne!(
        copia, original,
        "la copia lleva su propia cuenta de fotogramas"
    );
    let mut a = original.clone();
    let mut b = original;
    for _ in 0..WAIT_FRAMES {
        assert_eq!(a.tick(), b.tick());
    }
}

#[test]
fn fila_9_el_aviso_es_convirtiendo_y_esta_en_el_juego() {
    assert_eq!(CONVERTING_NOTICE, "Convirtiendo…");
    assert!(en_el_juego(CONVERTING_NOTICE));
    assert!(CONVERTING_NOTICE.chars().count() <= 44);
}

#[test]
fn fila_9_el_umbral_de_aceptar_es_el_mismo_que_el_del_informe() {
    // 2048² + 1 px: el informe espera y el aceptar espera; 2048²: los dos son inmediatos.
    let plan = plan_de_prueba();
    assert!(matches!(start_convert(plan, SYNC_PIXELS), Start::Now(_)));
    assert!(matches!(
        start_convert(plan, SYNC_PIXELS + 1),
        Start::Later(_)
    ));
    let mut inmediato = Montaje::nuevo(Mode::Gray, gris_vacia(2048, 2048));
    inmediato.sincroniza();
    assert_eq!(inmediato.llamadas(), 1);
    let mut con_espera = Montaje::nuevo(Mode::Gray, gris_vacia(2048, 2049));
    con_espera.sincroniza();
    assert_eq!(con_espera.llamadas(), 0);
    // Y la espera es de `WAIT_FRAMES` en los dos.
    let Start::Later(mut pendiente) = start_convert(plan, SYNC_PIXELS + 1) else {
        panic!("debía esperar");
    };
    let mut ticks = 0;
    while pendiente.tick().is_none() {
        ticks += 1;
        assert!(ticks < 100);
    }
    let mut fotogramas = 0;
    while con_espera.llamadas() == 0 {
        con_espera.sincroniza();
        fotogramas += 1;
        assert!(fotogramas < 100);
    }
    assert_eq!(ticks + 1, usize::from(WAIT_FRAMES));
    assert_eq!(fotogramas, usize::from(WAIT_FRAMES));
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: el informe y `Aceptar` dicen la verdad.
// ---------------------------------------------------------------------------------------

/// Los valores de canal con los que se generan los colores: fuerzan grises, alfa 0, alfa 255 y
/// repeticiones.
const VALORES: [u8; 5] = [0, 1, 127, 128, 255];

/// Todo lo que hace falta para construir una imagen de hasta 6 × 6 en cualquier modo.
#[derive(Clone, Debug)]
struct Semilla {
    modo: usize,
    ancho: u32,
    alto: u32,
    /// 36 píxeles, cada uno con cuatro índices en `VALORES`.
    pixeles: Vec<[usize; 4]>,
    /// 6 colores de paleta, cada uno con cuatro índices en `VALORES`.
    colores: Vec<[usize; 4]>,
    /// Entradas de la paleta (1 a 6).
    n: usize,
    /// Índice transparente: `6` es «ninguno».
    transparente: usize,
}

fn estrategia_de_semilla() -> impl Strategy<Value = Semilla> {
    (
        0usize..4,
        1u32..=6,
        1u32..=6,
        prop::collection::vec(prop::array::uniform4(0usize..5), 36),
        prop::collection::vec(prop::array::uniform4(0usize..5), 6),
        1usize..=6,
        0usize..=6,
    )
        .prop_map(
            |(modo, ancho, alto, pixeles, colores, n, transparente)| Semilla {
                modo,
                ancho,
                alto,
                pixeles,
                colores,
                n,
                transparente,
            },
        )
}

fn rgba_de_indices(v: &[usize; 4]) -> Rgba {
    Rgba::new(VALORES[v[0]], VALORES[v[1]], VALORES[v[2]], VALORES[v[3]])
}

fn imagen_de_semilla(s: &Semilla) -> Image {
    let (w, h) = (s.ancho, s.alto);
    let total = (w * h) as usize;
    let mode = MODOS[s.modo];
    let mut image = match mode {
        Mode::Indexed => {
            let colores: Vec<Rgba> = s.colores.iter().take(s.n).map(rgba_de_indices).collect();
            let mut palette = Palette::from_colors(colores).expect("paleta");
            if s.transparente < s.n {
                palette
                    .set_transparent_index(Some(s.transparente as u8))
                    .expect("índice transparente");
            }
            Image::new_indexed(w, h, palette).expect("imagen indexada")
        }
        otro => Image::new(w, h, otro).expect("imagen"),
    };
    for (i, v) in s.pixeles.iter().take(total).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let pixel = match mode {
            Mode::Rgba => Pixel::Rgba(rgba_de_indices(v)),
            Mode::Gray => Pixel::Gray(VALORES[v[0]]),
            Mode::GrayAlpha => Pixel::GrayAlpha {
                gray: VALORES[v[0]],
                alpha: VALORES[v[3]],
            },
            Mode::Indexed => Pixel::Indexed((v[0] % s.n) as u8),
        };
        image.set_pixel(x, y, pixel).expect("píxel");
    }
    image
}

const HEXES_OPACOS: [Rgba; 4] = [
    Rgba::opaque(255, 0, 0),
    Rgba::opaque(0, 0, 0),
    Rgba::opaque(255, 255, 255),
    Rgba::opaque(10, 20, 30),
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para imágenes de hasta 6 × 6 en cualquier modo, cualquier destino distinto del origen y
    /// cualquier `Allowed` (por los permisos): `analyze` coincide con `convert_image(.., NOTHING)`
    /// y `block_reason` es `None` si y solo si `Command::ConvertMode` con ese `Allowed` da `Ok`,
    /// salvo los fondos no opacos, que no producen `Allowed`.
    #[test]
    fn propiedad_1_el_informe_y_aceptar_dicen_la_verdad(
        semilla in estrategia_de_semilla(),
        destino in 0usize..3,
        luma in any::<bool>(),
        alfa in 0usize..5,
        hex in 0usize..4,
        activo in 0usize..4,
    ) {
        let image = imagen_de_semilla(&semilla);
        let from = image.mode();
        let target = destinos(from)[destino];

        // `analyze` es `convert_image(.., NOTHING)` clasificada.
        let analysis = analyze(&image, target);
        let directo = convert_image(&image, target, Allowed::NOTHING);
        match (&analysis, &directo) {
            (Analysis::Exact { palette }, Ok(conversion)) => {
                let esperada = if target == Mode::Indexed {
                    let p = conversion.image.palette();
                    prop_assert!(p.is_some(), "una conversión a indexado lleva paleta");
                    p.map(|p| (p.len(), p.transparent_index().is_some()))
                } else {
                    None
                };
                prop_assert_eq!(
                    palette.as_ref().map(|p| (p.colors, p.transparent)),
                    esperada
                );
            }
            (Analysis::Lossy(r), Err(ConvertError::LossNotAllowed { report })) => {
                prop_assert_eq!(r, report);
            }
            (Analysis::TooManyColors, Err(ConvertError::TooManyColors { .. })) => {}
            (Analysis::TooLarge, Err(ConvertError::TooLarge { .. })) => {}
            (analisis, directo) => {
                return Err(TestCaseError::fail(format!(
                    "analyze {analisis:?} no coincide con convert_image {:?}",
                    directo.as_ref().map(|_| "Ok").map_err(|e| e.clone())
                )));
            }
        }

        // `Aceptar` activo ⇔ `apply` acepta.
        let activo = activos()[activo];
        let perms = permisos(luma, alfa, HEXES_OPACOS[hex]);
        let Some(allowed) = allowed_of(&perms, activo) else {
            return Ok(());
        };
        let motivo = block_reason(Some(&analysis), &perms, activo);
        let mut documento = Document::new(image.clone());
        let id = documento.main_image();
        let resultado = documento.apply(Command::ConvertMode { image: id, target, allowed });
        prop_assert_eq!(
            motivo.is_none(),
            resultado.is_ok(),
            "{:?} a {:?} con {:?}: motivo {:?}, apply {:?}",
            from, target, allowed, motivo, resultado
        );
        if let Some(texto) = motivo {
            // El pie entero mide 44 columnas: 37 tras `error: ` y 44 con `falta: ` en su lugar.
            let limite = if texto.starts_with("falta: ") { 44 } else { 37 };
            prop_assert!(texto.chars().count() <= limite, "{}", texto);
            prop_assert!(en_el_juego(&texto), "{}", texto);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: el estado del formulario no se corrompe.
// ---------------------------------------------------------------------------------------

const TEXTOS_HEX: [&str; 10] = [
    "#ff0000",
    "#ff000080",
    "#fff",
    "#ffff",
    "#fff8",
    "zz",
    "#12",
    "",
    "#80808080",
    "#00ff00",
];

#[derive(Clone, Debug)]
enum Gesto {
    Destino(usize),
    /// 0: la de «Gris con alfa»; 1: la de «Gris».
    Casilla(usize, bool),
    Alfa(usize),
    Hex(usize),
    Confirma,
    Revierte,
}

fn gesto() -> impl Strategy<Value = Gesto> {
    prop_oneof![
        (0usize..3).prop_map(Gesto::Destino),
        (0usize..2, any::<bool>()).prop_map(|(k, on)| Gesto::Casilla(k, on)),
        (0usize..5).prop_map(Gesto::Alfa),
        (0usize..TEXTOS_HEX.len()).prop_map(Gesto::Hex),
        Just(Gesto::Confirma),
        Just(Gesto::Revierte),
    ]
}

/// Lo que debería tener el formulario tras los gestos, escrito aparte del motor.
#[derive(Clone, Debug)]
struct Modelo {
    from: Mode,
    destino: usize,
    luma: [bool; 2],
    alfa: usize,
    confirmado: Rgba,
    pendiente: Option<String>,
}

impl Modelo {
    fn nuevo(from: Mode) -> Modelo {
        Modelo {
            from,
            destino: 0,
            luma: [false; 2],
            alfa: 0,
            confirmado: Rgba::new(0x80, 0x80, 0x80, 0xff),
            pendiente: None,
        }
    }

    fn target(&self) -> Mode {
        destinos(self.from)[self.destino]
    }

    fn grupo_visible(&self) -> bool {
        self.target() == Mode::Gray && self.from != Mode::Gray
    }

    fn hex_visible(&self) -> bool {
        self.grupo_visible() && self.alfa == 4
    }

    /// El color del hexadecimal con su texto pendiente: `Err` si el texto no vale.
    fn efectivo(&self) -> Result<Rgba, ()> {
        match &self.pendiente {
            Some(texto) => parse_hex(texto, Mode::Rgba, self.confirmado).map_err(|_| ()),
            None => Ok(self.confirmado),
        }
    }

    fn luma_efectiva(&self) -> bool {
        match (self.from, self.target()) {
            (Mode::Rgba | Mode::Indexed, Mode::GrayAlpha) => self.luma[0],
            (Mode::Rgba | Mode::Indexed, Mode::Gray) => self.luma[1],
            _ => false,
        }
    }

    fn invalido_de_campo(&self) -> bool {
        self.hex_visible() && self.efectivo().is_err()
    }

    fn falla_el_check(&self) -> bool {
        self.hex_visible() && matches!(self.efectivo(), Ok(c) if c.a < 255)
    }

    fn permisos(&self) -> Permissions {
        let alpha = if !self.grupo_visible() {
            AlphaChoice::Keep
        } else if self.alfa == 4 {
            AlphaChoice::Hex(self.efectivo().unwrap_or(self.confirmado))
        } else {
            alfa_de(self.alfa, self.confirmado)
        };
        Permissions {
            luma: self.luma_efectiva(),
            alpha,
        }
    }

    fn plan(&self, activo: Option<Rgba>) -> Option<Plan> {
        let hex_opaco = self.efectivo().ok().filter(|c| c.a == 255);
        let allowed = allowed_esperado(
            self.from,
            self.destino,
            self.luma[0],
            self.luma[1],
            self.alfa,
            hex_opaco,
            activo,
        )?;
        Some(Plan {
            target: self.target(),
            allowed,
        })
    }
}

/// Aplica un gesto al diálogo y al modelo, solo sobre campos visibles (como el usuario).
fn aplica(d: &mut Dialog, m: &mut Modelo, g: &Gesto) {
    match g {
        Gesto::Destino(i) => {
            d.event(Event::Choice {
                field: "destino",
                index: *i,
            });
            m.destino = *i;
        }
        Gesto::Casilla(k, on) => {
            let id = ["luma_gris_alfa", "luma_gris"][*k];
            if d.is_shown(id) {
                d.event(Event::Check { field: id, on: *on });
                m.luma[*k] = *on;
            }
        }
        Gesto::Alfa(i) => {
            if d.is_shown("alfa_gris") {
                d.event(Event::Choice {
                    field: "alfa_gris",
                    index: *i,
                });
                m.alfa = *i;
            }
        }
        Gesto::Hex(i) => {
            if d.is_shown("fondo_hex") {
                let text = TEXTOS_HEX[*i].to_string();
                d.event(Event::Hex {
                    field: "fondo_hex",
                    text: text.clone(),
                });
                m.pendiente = Some(text);
            }
        }
        Gesto::Confirma => {
            if d.is_shown("fondo_hex") {
                d.event(Event::HexConfirm {
                    field: "fondo_hex",
                    blur: false,
                });
                if let Some(texto) = m.pendiente.clone()
                    && let Ok(color) = parse_hex(&texto, Mode::Rgba, m.confirmado)
                {
                    m.confirmado = color;
                    m.pendiente = None;
                }
            }
        }
        Gesto::Revierte => {
            if d.is_shown("fondo_hex") {
                d.event(Event::HexRevert { field: "fondo_hex" });
                m.pendiente = None;
            }
        }
    }
}

/// Las invariantes de la propiedad 2 tras un gesto y su `sync_convert`.
fn comprueba_formulario(
    d: &Dialog,
    m: &Modelo,
    sync: &ConvertSync,
    activo: Option<Rgba>,
    ctx: &str,
) -> Result<(), TestCaseError> {
    // Solo son visibles los campos de D2 para este destino (y la opción del alfa).
    let esperados = ids_visibles(m.from, m.destino, m.alfa);
    for id in TODOS_LOS_CAMPOS {
        let debe = tiene_campo(d, id) && esperados.contains(&id);
        prop_assert_eq!(d.is_shown(id), debe, "{}: {}", ctx, id);
    }

    // `values()` es `Some` si y solo si no hay campo inválido, ni `check`, ni motivo externo.
    let valido = !m.invalido_de_campo() && !m.falla_el_check() && d.block().is_none();
    let valores = d.values();
    prop_assert_eq!(valores.is_some(), valido, "{}: values", ctx);
    prop_assert_eq!(d.reason().is_none(), valido, "{}: reason", ctx);
    prop_assert_eq!(d.is_valid(), valido, "{}: is_valid", ctx);

    if let Some(values) = &valores {
        // Los campos ocultos no salen en `Values`.
        for id in TODOS_LOS_CAMPOS {
            if !esperados.contains(&id) {
                prop_assert!(
                    values.get(id).is_none(),
                    "{}: {} sale en los valores",
                    ctx,
                    id
                );
            }
        }
        prop_assert_eq!(values.choice("destino"), Some(m.destino), "{}", ctx);
        // El `Allowed` no se contamina con una casilla de otro destino.
        let plan = plan_from_values(values, m.from, activo);
        prop_assert_eq!(plan, m.plan(activo), "{}: plan", ctx);
        if let Some(plan) = plan {
            prop_assert!(
                plan.target != m.from,
                "{}: el destino es el modo actual",
                ctx
            );
            prop_assert_eq!(plan.target, targets(m.from)[m.destino], "{}", ctx);
        }
    }

    // El motivo externo es el de `block_reason` con los permisos visibles (salvo el hexadecimal que
    // el propio formulario rechaza).
    if !(m.hex_visible() && (m.invalido_de_campo() || m.falla_el_check())) {
        prop_assert_eq!(
            d.block().map(str::to_string),
            block_reason(sync.analysis(), &m.permisos(), activo),
            "{}: motivo",
            ctx
        );
    }

    // El informe cabe y está en el juego.
    let texto = informe_de(d);
    let lineas = lineas_de_zona(&texto);
    prop_assert!(
        !lineas.is_empty() && lineas.len() <= 7,
        "{}: {:?}",
        ctx,
        texto
    );
    for linea in &lineas {
        prop_assert!(linea.chars().count() <= 42, "{}: {:?}", ctx, linea);
    }
    prop_assert!(
        en_el_juego(&texto.replace('\n', " ")),
        "{}: {:?}",
        ctx,
        texto
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para los cuatro orígenes y secuencias de 0 a 48 eventos (destinos, casillas, grupo del alfa,
    /// hexadecimales válidos e inválidos) con `sync_convert` tras cada una: sin pánico, solo son
    /// visibles los campos de D2, `values()` es `Some` si y solo si no hay campo inválido, ni
    /// `check`, ni motivo externo, los campos ocultos no salen en `Values`, el destino del plan nunca
    /// es el modo actual y el informe cabe.
    #[test]
    fn propiedad_2_el_estado_del_formulario_no_se_corrompe(
        gestos in prop::collection::vec(gesto(), 0..=48),
        activo in 0usize..4,
    ) {
        let activo = activos()[activo];
        for from in MODOS {
            let image = imagen_de_modo(from);
            let mut modal = Modal::default();
            prop_assert!(open_convert_mode(&mut modal, from, Locks::default()));
            let Modal::Open(d) = &mut modal else {
                return Err(TestCaseError::fail("cerrado"));
            };
            let mut sync = ConvertSync::new(from, 0, activo);
            let mut analizador = SystemAnalyzer;
            let mut m = Modelo::nuevo(from);
            let historial = History { limit: 64 * MIB, used: 0 };
            sync_convert(d, &mut sync, &image, 0, historial, &mut analizador);
            comprueba_formulario(d, &m, &sync, activo, &format!("{from:?}, al abrir"))?;

            for (i, g) in gestos.iter().enumerate() {
                let ctx = format!("{from:?}, paso {i}: {g:?}");
                aplica(d, &mut m, g);
                let historial = History { limit: 64 * MIB, used: 0 };
                sync_convert(d, &mut sync, &image, 0, historial, &mut analizador);
                comprueba_formulario(d, &m, &sync, activo, &ctx)?;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: calcular solo al cambiar de destino.
// ---------------------------------------------------------------------------------------

/// Una imagen por encima de `SYNC_PIXELS` (2048 × 2049) de cada modo, construida una sola vez.
fn imagen_grande(mode: Mode) -> &'static Image {
    static GRIS: OnceLock<Image> = OnceLock::new();
    static GRIS_ALFA: OnceLock<Image> = OnceLock::new();
    static RGBA: OnceLock<Image> = OnceLock::new();
    static INDEXADA: OnceLock<Image> = OnceLock::new();
    match mode {
        Mode::Gray => GRIS.get_or_init(|| gris_vacia(2048, 2049)),
        Mode::GrayAlpha => {
            GRIS_ALFA.get_or_init(|| Image::new(2048, 2049, Mode::GrayAlpha).expect("imagen"))
        }
        Mode::Rgba => RGBA.get_or_init(|| Image::new(2048, 2049, Mode::Rgba).expect("imagen")),
        Mode::Indexed => INDEXADA.get_or_init(|| {
            let palette = Palette::from_colors([Rgba::opaque(1, 2, 3), Rgba::opaque(4, 5, 6)])
                .expect("paleta");
            Image::new_indexed(2048, 2049, palette).expect("imagen")
        }),
    }
}

#[derive(Clone, Debug)]
enum Paso {
    Evento(Gesto),
    Fotograma,
}

fn paso() -> impl Strategy<Value = Paso> {
    prop_oneof![
        3 => gesto().prop_map(Paso::Evento),
        2 => Just(Paso::Fotograma),
    ]
}

/// Repite una secuencia de eventos y fotogramas sobre una imagen y comprueba la propiedad 4.
fn reproduce(
    from: Mode,
    image: &Image,
    pasos: &[Paso],
    activo: Option<Rgba>,
) -> Result<(), TestCaseError> {
    let grande = u64::from(image.width()) * u64::from(image.height()) > SYNC_PIXELS;
    let mut modal = Modal::default();
    prop_assert!(open_convert_mode(&mut modal, from, Locks::default()));
    let Modal::Open(d) = &mut modal else {
        return Err(TestCaseError::fail("cerrado"));
    };
    let mut sync = ConvertSync::new(from, 0, activo);
    let mut contador = Contador::default();
    let mut m = Modelo::nuevo(from);

    // Estado del modelo de fotogramas: el destino visto en el último `sync_convert`, cuántos
    // cambios se han visto y en qué fotograma fue el último (la apertura cuenta como fotograma 0).
    let mut observado: Option<usize> = None;
    let mut cambios = 0usize;
    let mut inicio = 0usize;
    let mut fotograma = 0usize;

    let mut sincroniza = |d: &mut Dialog,
                          sync: &mut ConvertSync,
                          contador: &mut Contador,
                          m: &Modelo,
                          ctx: &str|
     -> Result<(), TestCaseError> {
        if observado != Some(m.destino) {
            if observado.is_some() {
                cambios += 1;
            }
            observado = Some(m.destino);
            inicio = fotograma;
        }
        let historial = History {
            limit: 64 * MIB,
            used: 0,
        };
        sync_convert(d, sync, image, 0, historial, contador);
        let pendiente = grande && fotograma < inicio + usize::from(WAIT_FRAMES);
        fotograma += 1;
        prop_assert!(
            contador.llamadas.len() <= 1 + cambios,
            "{}: {} llamadas con {} cambios de destino",
            ctx,
            contador.llamadas.len(),
            cambios
        );
        if pendiente {
            prop_assert_eq!(d.block(), Some(PENDIENTE), "{}: pendiente", ctx);
            prop_assert!(
                !d.is_valid(),
                "{}: Aceptar está bloqueado mientras se calcula",
                ctx
            );
            prop_assert!(
                matches!(d.clone().event(Event::Accept), Reaction::Rejected(_)),
                "{}: Accept con un cálculo pendiente",
                ctx
            );
        } else {
            prop_assert!(
                d.block() != Some(PENDIENTE),
                "{}: no hay nada pendiente y el motivo lo dice",
                ctx
            );
            prop_assert!(sync.analysis().is_some(), "{}: hay informe vigente", ctx);
        }
        Ok(())
    };

    sincroniza(d, &mut sync, &mut contador, &m, "apertura")?;
    for (i, paso) in pasos.iter().enumerate() {
        match paso {
            Paso::Evento(g) => aplica(d, &mut m, g),
            Paso::Fotograma => {
                sincroniza(
                    d,
                    &mut sync,
                    &mut contador,
                    &m,
                    &format!("paso {i}: {paso:?}"),
                )?;
            }
        }
    }
    // Tras `WAIT_FRAMES` fotogramas ociosos no queda ningún cálculo pendiente. Si la secuencia acabó
    // en un evento, el primer fotograma ocioso es el que ve el cambio: hacen falta `WAIT_FRAMES + 1`.
    for k in 0..=WAIT_FRAMES {
        sincroniza(d, &mut sync, &mut contador, &m, &format!("ocioso {k}"))?;
    }
    prop_assert!(d.block() != Some(PENDIENTE), "queda un cálculo pendiente");
    prop_assert!(sync.analysis().is_some());
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para secuencias aleatorias de eventos y fotogramas sobre imágenes de cualquier tamaño (por
    /// debajo y por encima de `SYNC_PIXELS`) y de los cuatro modos: las llamadas a
    /// `Analyzer::analyze` son como mucho las de cambios de destino más una por apertura (nunca
    /// crecen con casillas, fondo, hexadecimal ni fotogramas ociosos); tras `WAIT_FRAMES` fotogramas
    /// ociosos no queda ningún cálculo pendiente; y mientras haya uno pendiente `Aceptar` está
    /// bloqueado.
    #[test]
    fn propiedad_4_calcular_solo_al_cambiar_de_destino(
        pasos in prop::collection::vec(paso(), 0..=60),
        activo in 0usize..4,
    ) {
        let activo = activos()[activo];
        for from in MODOS {
            reproduce(from, &imagen_de_modo(from), &pasos, activo)?;
            reproduce(from, imagen_grande(from), &pasos, activo)?;
        }
    }

    /// `PendingConvert` entrega su plan una sola vez, siempre en el `WAIT_FRAMES`-ésimo `tick`, con
    /// cualquier plan y cualquier número de píxeles por encima de `SYNC_PIXELS`; por debajo (o
    /// igual) `start_convert` da `Now` con el mismo plan.
    #[test]
    fn propiedad_4_pending_convert_entrega_su_plan_una_sola_vez(
        destino in 0usize..4,
        luma in any::<bool>(),
        fondo in prop::option::of(any::<[u8; 3]>()),
        pixeles in prop_oneof![1u64..=SYNC_PIXELS, (SYNC_PIXELS + 1)..=(1u64 << 40)],
        ticks in 0usize..=12,
    ) {
        let plan = Plan {
            target: MODOS[destino],
            allowed: Allowed { luma, drop_alpha: fondo },
        };
        match start_convert(plan, pixeles) {
            Start::Now(entregado) => {
                prop_assert!(pixeles <= SYNC_PIXELS, "{} píxeles dan Now", pixeles);
                prop_assert_eq!(entregado, plan);
            }
            Start::Later(mut pendiente) => {
                prop_assert!(pixeles > SYNC_PIXELS, "{} píxeles dan Later", pixeles);
                prop_assert_eq!(pendiente.plan(), plan);
                let mut entregas = Vec::new();
                for tick in 1..=ticks {
                    if let Some(p) = pendiente.tick() {
                        entregas.push((tick, p));
                    }
                }
                if ticks >= usize::from(WAIT_FRAMES) {
                    prop_assert_eq!(entregas, vec![(usize::from(WAIT_FRAMES), plan)]);
                } else {
                    prop_assert!(entregas.is_empty());
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 12 (medición, ignorada): informe, `apply` y refresco a 4096² y 8192².
// ---------------------------------------------------------------------------------------

/// RGBA de `lado × lado` con 16 grises opacos (exacta a gris, a RGBA y a indexado) y, en una
/// esquina, un píxel de color y otro semitransparente (para la conversión con pérdida).
fn imagen_de_medicion(lado: u32) -> Image {
    let mut image = Image::new(lado, lado, Mode::Rgba).expect("imagen de medición");
    for y in 0..lado {
        for x in 0..lado {
            let g = (((x ^ y) & 15) * 17) as u8;
            image
                .set_pixel(x, y, Pixel::Rgba(Rgba::opaque(g, g, g)))
                .expect("píxel");
        }
    }
    image
}

#[test]
#[ignore = "medición: cargo test --release -p px-app -- --ignored --nocapture"]
fn fila_12_medicion_de_informe_apply_y_refresco_a_4096_y_8192() {
    for lado in [4096u32, 8192] {
        let t = Instant::now();
        let mut image = imagen_de_medicion(lado);
        println!("{lado}²: imagen de prueba construida en {:?}", t.elapsed());

        // Informes: exacta a indexado (la peor), exacta a gris, y con pérdida.
        let t = Instant::now();
        let a_indexado = analyze(&image, Mode::Indexed);
        let informe_indexado = t.elapsed();
        assert_eq!(a_indexado, exacta_con_paleta(16, false));

        let t = Instant::now();
        let a_gris = analyze(&image, Mode::Gray);
        let informe_gris = t.elapsed();
        assert_eq!(a_gris, exacta());

        image
            .set_pixel(0, 0, Pixel::Rgba(Rgba::opaque(255, 0, 0)))
            .expect("píxel");
        let t = Instant::now();
        let con_perdida = analyze(&image, Mode::Gray);
        let informe_con_perdida = t.elapsed();
        assert_eq!(con_perdida, perdida(1, 0));

        println!(
            "{lado}²: informe a indexado {informe_indexado:?}, a gris exacta {informe_gris:?}, con pérdida {informe_con_perdida:?}"
        );
        // Red de seguridad, no un objetivo: el informe de un 4096² no pasa de 2 s.
        if lado == 4096 {
            for (nombre, t) in [
                ("a indexado", informe_indexado),
                ("a gris", informe_gris),
                ("con pérdida", informe_con_perdida),
            ] {
                assert!(
                    t.as_secs_f64() <= 2.0,
                    "el informe {nombre} de un 4096² tardó {t:?}"
                );
            }
        }

        // `apply` y refresco de la textura.
        let mut documento = Document::new(image);
        let id = documento.main_image();
        let t = Instant::now();
        let aplicado = documento
            .apply(Command::ConvertMode {
                image: id,
                target: Mode::GrayAlpha,
                allowed: Allowed {
                    luma: true,
                    drop_alpha: None,
                },
            })
            .expect("convertir");
        let apply = t.elapsed();
        assert!(aplicado);
        let t = Instant::now();
        let rgba8 = px_ui::texture::to_rgba8(documento.image(id).expect("imagen"));
        let refresco = t.elapsed();
        assert_eq!(rgba8.len(), 4 * lado as usize * lado as usize);
        drop(rgba8);
        let t = Instant::now();
        documento.undo().expect("deshacer");
        let deshacer = t.elapsed();
        println!("{lado}²: apply {apply:?}, refresco (to_rgba8) {refresco:?}, undo {deshacer:?}");
    }
}
