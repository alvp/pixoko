//! Tests de combinar y aplanar de `px-raster` (specs/hito-3-combinar.md, unidad 3.5, cambios
//! `R-01` a `R-05`): `compose_layers`, `MergeError`, `merge_down_command`, `merge_visible_command`
//! y `flatten_command`.
//!
//! Una prueba por cada fila 5 a 7 de «Casos» (`fila_NN_...`; la fila 5 y la 7 se reparten en
//! varias variantes mínimas) y una por cada propiedad 1, 4 y 5 (`propiedad_N_...`; la 5, aquí,
//! con los constructores de `px-raster`), con 256 casos de `proptest`. Las filas 1 a 4 y las
//! propiedades 2 y 3 viven en `crates/px-core/tests/` y la fila 8 en `crates/px-golden`.
//!
//! Los vectores de la fila 6 son los 10 píxeles observados en Aseprite 1.3.18.6-dev ejecutándolo
//! como caja negra (`tools/corpus-h3/merge-probe.lua`), copiados como literales. El oráculo de
//! la propiedad 4 es un modelo independiente escrito aquí (plegado de `composite_pixel` sobre el
//! fondo vacío; en indexado, la regla del índice más alto no transparente).
//!
//! Este fichero es la fase de pruebas en rojo: `compose_layers`, `MergeError`, los tres
//! constructores y `Command::MergeLayers` no existen todavía, así que no compila hasta la lógica
//! de `px-core` (C-01 a C-14) y de `px-raster` (R-01 a R-05). Todo documento mide como mucho 6 × 5.

use proptest::prelude::*;
use px_core::{
    BlendMode, Command, Document, DocumentError, Image, ImageId, Layer, LayerId, Mode, Palette,
    Pixel, PixelEdit, Rgba,
};
use px_raster::{
    ComposeError, MergeError, compose, compose_layers, composite_pixel, flatten_command,
    merge_down_command, merge_visible_command,
};

// ---------------------------------------------------------------------------------------
// Colores, documentos y comprobaciones de apoyo.
// ---------------------------------------------------------------------------------------

const T: Rgba = Rgba::TRANSPARENT;

const fn c(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// `x / 255` al entero más cercano (sin empates posibles), a partir de la definición.
fn rounds_255(x: i128) -> i128 {
    (2 * x + 255).div_euclid(510)
}

/// Una capa de un documento de prueba. `cel_2: None` es una celda vacía (sin imagen); la capa 0
/// siempre tiene celda, así que en ella `None` deja la imagen inicial (toda a ceros).
#[derive(Clone, Debug)]
struct Layer2 {
    visible: bool,
    locked: bool,
    opacity_2: u8,
    mode: BlendMode,
    cel_2: Option<Vec<Pixel>>,
}

impl Layer2 {
    fn new(cel_2: Vec<Pixel>) -> Layer2 {
        Layer2 {
            visible: true,
            locked: false,
            opacity_2: 255,
            mode: BlendMode::Normal,
            cel_2: Some(cel_2),
        }
    }

    fn rgba(cel_2: &[Rgba]) -> Layer2 {
        Layer2::new(cel_2.iter().copied().map(Pixel::Rgba).collect())
    }

    fn gray_2(cel_2: &[(u8, u8)]) -> Layer2 {
        Layer2::new(
            cel_2
                .iter()
                .map(|&(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
                .collect(),
        )
    }

    fn idx(values: &[u8]) -> Layer2 {
        Layer2::new(values.iter().copied().map(Pixel::Indexed).collect())
    }

    fn empty() -> Layer2 {
        Layer2 {
            visible: true,
            locked: false,
            opacity_2: 255,
            mode: BlendMode::Normal,
            cel_2: None,
        }
    }

    fn mode(mut self, mode: BlendMode) -> Layer2 {
        self.mode = mode;
        self
    }

    fn opacity_2(mut self, opacity_2: u8) -> Layer2 {
        self.opacity_2 = opacity_2;
        self
    }

    fn hidden(mut self) -> Layer2 {
        self.visible = false;
        self
    }

    fn locked(mut self) -> Layer2 {
        self.locked = true;
        self
    }
}

/// Documento con `layers` de abajo arriba, a partir de la imagen `template` (que es la celda de
/// la capa 0 y fija el tamaño y el modo). Solo usa los comandos del núcleo ya existentes.
fn builds(template: Image, layers: &[Layer2]) -> Document {
    let mut doc = Document::new(template);
    let (w, h) = (doc.width(), doc.height());
    for i in 1..layers.len() {
        let cmd = Command::AddLayer {
            name: format!("C{i}"),
            at: i,
        };
        assert_eq!(doc.apply(cmd), Ok(true), "añadir la capa {i}");
    }
    for (i, layer_2) in layers.iter().enumerate() {
        let (id, cel) = (doc.layers()[i].id(), doc.layers()[i].cel());
        if let Some(pixels_2) = &layer_2.cel_2 {
            assert_eq!(pixels_2.len(), (w * h) as usize, "píxeles de la capa {i}");
            let pixels = pixels_2
                .iter()
                .enumerate()
                .map(|(k, &pixel)| PixelEdit {
                    x: k as u32 % w,
                    y: k as u32 / w,
                    pixel,
                })
                .collect();
            doc.apply(Command::SetPixels { image: cel, pixels })
                .expect("escribir la celda");
        }
        if layer_2.opacity_2 != 255 {
            let cmd = Command::SetLayerOpacity {
                layer: id,
                opacity: layer_2.opacity_2,
            };
            assert!(doc.apply(cmd).is_ok(), "opacidad de la capa {i}");
        }
        if layer_2.mode != BlendMode::Normal {
            let cmd = Command::SetLayerBlend {
                layer: id,
                blend: layer_2.mode,
            };
            assert!(doc.apply(cmd).is_ok(), "modo de la capa {i}");
        }
        if !layer_2.visible {
            let cmd = Command::SetLayerVisible {
                layer: id,
                visible: false,
            };
            assert!(doc.apply(cmd).is_ok(), "ocultar la capa {i}");
        }
        if layer_2.locked {
            let cmd = Command::SetLayerLocked {
                layer: id,
                locked: true,
            };
            assert!(doc.apply(cmd).is_ok(), "bloquear la capa {i}");
        }
    }
    doc
}

fn image_2(w: u32, h: u32, mode: Mode) -> Image {
    Image::new(w, h, mode).expect("imagen")
}

fn doc_rgba(w: u32, h: u32, layers: &[Layer2]) -> Document {
    builds(image_2(w, h, Mode::Rgba), layers)
}

/// Paleta de `n` colores opacos distintos con el índice transparente `t`.
fn palette(n: u8, t: Option<u8>) -> Palette {
    let mut p = Palette::from_colors(
        (0..n).map(|i| Rgba::new(i.wrapping_mul(30), 255 - i.wrapping_mul(30), 7, 255)),
    )
    .expect("paleta");
    p.set_transparent_index(t).expect("índice transparente");
    p
}

fn template_indexed(w: u32, h: u32, n: u8, t: Option<u8>) -> Image {
    Image::new_indexed(w, h, palette(n, t)).expect("imagen indexada")
}

fn doc_indexed(w: u32, h: u32, t: Option<u8>, layers: &[Layer2]) -> Document {
    builds(template_indexed(w, h, 8, t), layers)
}

/// Los píxeles de una imagen RGBA, fila a fila.
fn rgba_de(img: &Image) -> Vec<Rgba> {
    assert_eq!(img.mode(), Mode::Rgba, "se esperaba una imagen RGBA");
    pixels_of(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Rgba(color) => color,
            other => panic!("se esperaba RGBA, hay {other:?}"),
        })
        .collect()
}

/// Los píxeles de una imagen en cualquier modo, fila a fila.
fn pixels_of(img: &Image) -> Vec<Pixel> {
    (0..img.height())
        .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
        .map(|(x, y)| img.get_pixel(x, y).expect("píxel dentro"))
        .collect()
}

/// Los índices de una imagen indexada, fila a fila.
fn indices_de(img: &Image) -> Vec<u8> {
    pixels_of(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Indexed(i) => i,
            other => panic!("se esperaba indexado, hay {other:?}"),
        })
        .collect()
}

/// `compose` que no debe fallar.
fn compose_2(doc: &Document) -> Image {
    compose(doc).expect("la composición no debe fallar")
}

/// `compose_layers` que no debe fallar.
fn layers_of(doc: &Document, ids: &[LayerId]) -> Image {
    compose_layers(doc, ids).expect("la composición de las capas no debe fallar")
}

fn ids(doc: &Document) -> Vec<LayerId> {
    doc.layers().iter().map(Layer::id).collect()
}

fn ids3(doc: &Document) -> [LayerId; 3] {
    ids(doc).try_into().expect("tres capas")
}

fn ids4(doc: &Document) -> [LayerId; 4] {
    ids(doc).try_into().expect("cuatro capas")
}

/// Las propiedades de una capa que deben volver intactas tras deshacer.
type Signature = (LayerId, ImageId, String, bool, bool, u8, BlendMode);

fn signature(l: &Layer) -> Signature {
    (
        l.id(),
        l.cel(),
        l.name().to_string(),
        l.visible(),
        l.locked(),
        l.opacity(),
        l.blend(),
    )
}

/// Los cuatro campos de `Command::MergeLayers`.
#[derive(Debug)]
struct Fusion {
    target: LayerId,
    remove: Vec<LayerId>,
    image: Option<Image>,
    based_on: u64,
}

fn fusion(cmd: &Command) -> Fusion {
    match cmd {
        Command::MergeLayers {
            target,
            remove,
            image,
            based_on,
        } => Fusion {
            target: *target,
            remove: remove.clone(),
            image: image.clone(),
            based_on: *based_on,
        },
        other => panic!("se esperaba MergeLayers, hay {other:?}"),
    }
}

/// Aplica `cmd` sobre una copia del documento y la devuelve (un solo paso).
fn applied(doc: &Document, cmd: Command) -> Document {
    let mut copy = doc.clone();
    assert_eq!(copy.apply(cmd), Ok(true), "aplicar el comando de combinar");
    assert_ne!(copy.state_id(), doc.state_id(), "estado nuevo");
    copy
}

// ---------------------------------------------------------------------------------------
// Fila 5: compose_layers.
// ---------------------------------------------------------------------------------------

const WHITE: Rgba = c(255, 255, 255, 255);
const GRAY_128: Rgba = c(128, 128, 128, 255);
const A_OPAQUE: Rgba = c(200, 100, 50, 255);

/// `[A, B, C]` de 2 × 1: el fondo opaco `A = (200, 100, 50)`; encima `B`, Normal, con
/// `(10, 20, 30)` solo en el primer píxel; y encima `C`, Multiplicar, con blanco y gris 128.
fn stack_abc() -> Vec<Layer2> {
    vec![
        Layer2::rgba(&[A_OPAQUE, A_OPAQUE]),
        Layer2::rgba(&[c(10, 20, 30, 255), T]),
        Layer2::rgba(&[WHITE, GRAY_128]).mode(BlendMode::Multiply),
    ]
}

#[test]
fn row_05_a_subsets_in_order_and_ignoring_visibility() {
    let doc = doc_rgba(2, 1, &stack_abc());
    let [a, b, c_] = ids3(&doc);

    // {A, C}: B no cuenta. A multiplicado por blanco y por 128: div255(200·128) = 100,
    // div255(100·128) = 50 y div255(50·128) = 25.
    let a_c = [A_OPAQUE, c(100, 50, 25, 255)];
    assert_eq!(rgba_de(&layers_of(&doc, &[a, c_])), a_c);
    // El orden de la lista no manda: manda el del documento.
    assert_eq!(rgba_de(&layers_of(&doc, &[c_, a])), a_c);
    // {B, C}: sin A, contra el vacío (el gris 128 se queda tal cual sobre un fondo sin alfa).
    assert_eq!(
        rgba_de(&layers_of(&doc, &[b, c_])),
        [c(10, 20, 30, 255), GRAY_128]
    );
    // Una sola capa: su celda.
    assert_eq!(rgba_de(&layers_of(&doc, &[b])), [c(10, 20, 30, 255), T]);

    // Todas las visibles: `compose` byte a byte.
    assert_eq!(layers_of(&doc, &[a, b, c_]), compose_2(&doc));
    assert_eq!(
        rgba_de(&layers_of(&doc, &[a, b, c_])),
        [c(10, 20, 30, 255), c(100, 50, 25, 255)]
    );

    // Exactamente las visibles de un documento con una oculta: igual que `compose`.
    let with_hidden = doc_rgba(
        2,
        1,
        &[
            stack_abc()[0].clone(),
            stack_abc()[1].clone(),
            stack_abc()[2].clone().hidden(),
        ],
    );
    let [a, b, c_] = ids3(&with_hidden);
    assert_eq!(layers_of(&with_hidden, &[a, b]), compose_2(&with_hidden));

    // Una oculta incluida cuenta: {A, C} con C oculta sigue multiplicando, y `compose` no.
    assert_eq!(rgba_de(&layers_of(&with_hidden, &[a, c_])), a_c);
    assert_ne!(layers_of(&with_hidden, &[a, c_]), compose_2(&with_hidden));
    // Con todas ocultas, la composición de las tres sigue siendo la de siempre.
    let all: Vec<Layer2> = stack_abc().into_iter().map(Layer2::hidden).collect();
    let hidden_2 = doc_rgba(2, 1, &all);
    assert_eq!(
        rgba_de(&layers_of(&hidden_2, &ids(&hidden_2))),
        [c(10, 20, 30, 255), c(100, 50, 25, 255)]
    );
}

#[test]
fn row_05_b_empty_ones_and_opacity_of_the_bottom_layer() {
    // Una celda vacía no aporta nada, ni siquiera con modo y opacidad raros.
    let with_empty = doc_rgba(
        2,
        1,
        &[
            stack_abc()[0].clone(),
            Layer2::empty().mode(BlendMode::Multiply).opacity_2(100),
        ],
    );
    let [a, d] = <[LayerId; 2]>::try_from(ids(&with_empty)).expect("dos capas");
    assert_eq!(
        rgba_de(&layers_of(&with_empty, &[a, d])),
        [A_OPAQUE, A_OPAQUE]
    );
    assert_eq!(rgba_de(&layers_of(&with_empty, &[d])), [T, T]);

    // La capa de abajo se compone contra el vacío con SU opacidad (el modo no tiene efecto y la
    // opacidad escala el alfa: div255(100·128) = 50).
    let with_opacity = doc_rgba(
        2,
        1,
        &[Layer2::rgba(&[A_OPAQUE, c(10, 20, 30, 100)])
            .mode(BlendMode::Multiply)
            .opacity_2(128)],
    );
    let only = ids(&with_opacity);
    assert_eq!(
        rgba_de(&layers_of(&with_opacity, &only)),
        [c(200, 100, 50, 128), c(10, 20, 30, 50)]
    );
}

#[test]
fn row_05_c_indexed_and_gray_with_alpha() {
    // Indexado, transparente 0: gana el índice más alto no transparente de las capas pedidas.
    let doc = doc_indexed(
        4,
        1,
        Some(0),
        &[
            Layer2::idx(&[1, 2, 3, 0]),
            Layer2::idx(&[0, 4, 0, 0]).hidden(),
            Layer2::idx(&[5, 0, 0, 0]),
            Layer2::empty(),
        ],
    );
    let [l0, l1, l2, empty] = ids4(&doc);
    let output = layers_of(&doc, &[l0, l2]);
    assert_eq!(indices_de(&output), [5, 2, 3, 0]);
    assert_eq!(output.mode(), Mode::Indexed);
    assert_eq!(
        output.palette(),
        doc.palette(),
        "la paleta es la del lienzo"
    );
    // La oculta incluida cuenta; la vacía no aporta.
    assert_eq!(indices_de(&layers_of(&doc, &[l0, l1, empty])), [1, 4, 3, 0]);
    assert_eq!(indices_de(&layers_of(&doc, &[l0, l1, l2])), [5, 4, 3, 0]);
    // Solo la vacía: todo transparente.
    assert_eq!(indices_de(&layers_of(&doc, &[empty])), [0, 0, 0, 0]);

    // Gris con alfa: el canal rojo del RGBA equivalente (R = G = B) y su alfa.
    let data = [
        (vec![(200, 255), (100, 128)], BlendMode::Normal, 255, true),
        (vec![(50, 255), (80, 200)], BlendMode::Multiply, 200, true),
        (vec![(10, 100), (255, 255)], BlendMode::Screen, 128, false),
    ];
    let gray_2: Vec<Layer2> = data
        .iter()
        .map(|(px, mode, op, visible)| {
            let layer_2 = Layer2::gray_2(px).mode(*mode).opacity_2(*op);
            if *visible { layer_2 } else { layer_2.hidden() }
        })
        .collect();
    let rgb: Vec<Layer2> = data
        .iter()
        .map(|(px, mode, op, visible)| {
            let px: Vec<Rgba> = px.iter().map(|&(g, a)| c(g, g, g, a)).collect();
            let layer_2 = Layer2::rgba(&px).mode(*mode).opacity_2(*op);
            if *visible { layer_2 } else { layer_2.hidden() }
        })
        .collect();
    let doc_g = builds(image_2(2, 1, Mode::GrayAlpha), &gray_2);
    let doc_c = doc_rgba(2, 1, &rgb);
    let (ids_g, ids_c) = (ids(&doc_g), ids(&doc_c));
    for sel in [[0usize, 2].as_slice(), &[1, 2], &[0, 1, 2], &[2]] {
        let en_g: Vec<LayerId> = sel.iter().map(|&i| ids_g[i]).collect();
        let en_c: Vec<LayerId> = sel.iter().map(|&i| ids_c[i]).collect();
        let output = layers_of(&doc_g, &en_g);
        assert_eq!(output.mode(), Mode::GrayAlpha);
        let expected: Vec<Pixel> = rgba_de(&layers_of(&doc_c, &en_c))
            .iter()
            .map(|p| Pixel::GrayAlpha {
                gray: p.r,
                alpha: p.a,
            })
            .collect();
        assert_eq!(pixels_of(&output), expected, "capas {sel:?}");
    }
}

#[test]
fn row_05_d_nothing_to_give_in_a_mode_without_transparent() {
    // Indexado sin transparente: con la de abajo llena, esa capa sola es su celda; con solo una
    // vacía no queda ningún píxel que dar.
    let mut doc = doc_indexed(
        4,
        1,
        Some(0),
        &[Layer2::idx(&[1, 2, 3, 0]), Layer2::empty()],
    );
    let removes = Command::SetTransparentIndex {
        image: doc.layers()[0].cel(),
        index: None,
    };
    assert_eq!(doc.apply(removes), Ok(true));
    let [full, empty] = <[LayerId; 2]>::try_from(ids(&doc)).expect("dos capas");
    assert_eq!(indices_de(&layers_of(&doc, &[full])), [1, 2, 3, 0]);
    assert_eq!(indices_de(&layers_of(&doc, &[full, empty])), [1, 2, 3, 0]);
    assert_eq!(
        compose_layers(&doc, &[empty]),
        Err(ComposeError::NoTransparency {
            mode: Mode::Indexed
        })
    );

    // Gris sin alfa con la única capa oculta: `compose` no da nada, pero pedirla a mano da su celda.
    let mut cel_2 = image_2(3, 1, Mode::Gray);
    for (x, v) in [0u8, 10, 250].into_iter().enumerate() {
        cel_2.set_pixel(x as u32, 0, Pixel::Gray(v)).expect("píxel");
    }
    let mut gray_2 = Document::new(cel_2.clone());
    let layer_2 = gray_2.layers()[0].id();
    let hidden = Command::SetLayerVisible {
        layer: layer_2,
        visible: false,
    };
    assert_eq!(gray_2.apply(hidden), Ok(true));
    assert_eq!(
        compose(&gray_2),
        Err(ComposeError::NoTransparency { mode: Mode::Gray })
    );
    assert_eq!(compose_layers(&gray_2, &[layer_2]), Ok(cel_2));
}

// ---------------------------------------------------------------------------------------
// Fila 6: merge_down_command contra los 10 píxeles de Aseprite.
// ---------------------------------------------------------------------------------------

/// Un vector de la sonda: la capa de abajo `L` (capa 0) y la de arriba `U` (capa 1), de ancho
/// `l.len()` y alto 1, y lo que dio Aseprite tras combinar.
struct Vector {
    name: &'static str,
    l_mode: BlendMode,
    l_opacity: u8,
    l: Vec<Rgba>,
    u_mode: BlendMode,
    u_opacity: u8,
    u: Vec<Rgba>,
    aseprite: Vec<Rgba>,
}

const L_COMMON: [Rgba; 2] = [c(200, 100, 50, 255), c(200, 100, 50, 100)];
const U_COMMON: [Rgba; 2] = [c(100, 200, 255, 255), c(100, 200, 255, 150)];

fn vectors() -> [Vector; 4] {
    [
        Vector {
            name: "1: L Normal 128, U Multiplicar 200",
            l_mode: BlendMode::Normal,
            l_opacity: 128,
            l: vec![
                c(200, 100, 50, 255),
                c(200, 100, 50, 128),
                c(0, 0, 0, 0),
                c(10, 20, 30, 255),
            ],
            u_mode: BlendMode::Multiply,
            u_opacity: 200,
            u: vec![
                c(100, 200, 255, 255),
                c(100, 200, 255, 255),
                c(100, 200, 255, 255),
                c(100, 200, 255, 100),
            ],
            aseprite: vec![
                c(99, 113, 104, 228),
                c(99, 148, 165, 214),
                c(100, 200, 255, 200),
                c(27, 55, 74, 167),
            ],
        },
        Vector {
            name: "2: L Multiplicar 255, U Normal 255",
            l_mode: BlendMode::Multiply,
            l_opacity: 255,
            l: L_COMMON.to_vec(),
            u_mode: BlendMode::Normal,
            u_opacity: 255,
            u: U_COMMON.to_vec(),
            aseprite: vec![c(100, 200, 255, 255), c(122, 178, 210, 191)],
        },
        Vector {
            name: "3: L Trama 128, U Normal 255",
            l_mode: BlendMode::Screen,
            l_opacity: 128,
            l: L_COMMON.to_vec(),
            u_mode: BlendMode::Normal,
            u_opacity: 255,
            u: U_COMMON.to_vec(),
            aseprite: vec![c(100, 200, 255, 255), c(113, 187, 229, 171)],
        },
        Vector {
            name: "4: L Normal 255, U Multiplicar 100",
            l_mode: BlendMode::Normal,
            l_opacity: 255,
            l: L_COMMON.to_vec(),
            u_mode: BlendMode::Multiply,
            u_opacity: 100,
            u: U_COMMON.to_vec(),
            aseprite: vec![c(153, 92, 50, 255), c(153, 120, 98, 136)],
        },
    ]
}

#[test]
fn row_06_a_the_ten_pixels_of_aseprite() {
    for v in vectors() {
        let w = v.l.len() as u32;
        let doc = doc_rgba(
            w,
            1,
            &[
                Layer2::rgba(&v.l)
                    .mode(v.l_mode)
                    .opacity_2(v.l_opacity)
                    .locked(),
                Layer2::rgba(&v.u).mode(v.u_mode).opacity_2(v.u_opacity),
            ],
        );
        let [l, u] = <[LayerId; 2]>::try_from(ids(&doc)).expect("dos capas");
        let name_l = doc.layer(l).expect("L").name().to_string();

        let cmd = merge_down_command(&doc, u).unwrap_or_else(|e| panic!("{}: {e:?}", v.name));
        let f = fusion(&cmd);
        assert_eq!(f.target, l, "{}", v.name);
        assert_eq!(f.remove, [u], "{}", v.name);
        assert_eq!(f.based_on, doc.state_id(), "{}", v.name);
        let image_2 = f.image.clone().expect("hay imagen");
        assert_eq!(image_2, layers_of(&doc, &[l, u]), "{}", v.name);

        for (k, (obtained, aseprite)) in rgba_de(&image_2).iter().zip(&v.aseprite).enumerate() {
            // El fondo ya compuesto (L contra el vacío) y el modo de U fijan el dominio empírico
            // de 3.2: solo ahí se admite ±1 en un canal; el alfa, nunca.
            let alpha_background = rounds_255(i128::from(v.l[k].a) * i128::from(v.l_opacity));
            let in_domain =
                0 < alpha_background && alpha_background < 255 && v.u_mode != BlendMode::Normal;
            let tolerance = i32::from(in_domain);
            assert_eq!(
                obtained.a, aseprite.a,
                "{} píxel {k}: el alfa debe coincidir ({obtained:?} frente a {aseprite:?})",
                v.name
            );
            for (channel, o, a) in [
                ("R", obtained.r, aseprite.r),
                ("G", obtained.g, aseprite.g),
                ("B", obtained.b, aseprite.b),
            ] {
                let dif = (i32::from(o) - i32::from(a)).abs();
                assert!(
                    dif <= 2,
                    "{} píxel {k}, canal {channel}: {o} frente a {a} (nunca más de 2)",
                    v.name
                );
                assert!(
                    dif <= tolerance,
                    "{} píxel {k}, canal {channel}: {o} frente a {a}; dominio empírico: {in_domain}",
                    v.name
                );
            }
        }

        // Aplicado: queda L, Normal, 255, desbloqueada, con su nombre y visibilidad.
        let after = applied(&doc, cmd);
        assert_eq!(ids(&after), [l], "{}", v.name);
        let layer_2 = &after.layers()[0];
        assert_eq!(layer_2.opacity(), 255, "{}", v.name);
        assert_eq!(layer_2.blend(), BlendMode::Normal, "{}", v.name);
        assert!(!layer_2.locked(), "{}: queda desbloqueada", v.name);
        assert!(layer_2.visible(), "{}", v.name);
        assert_eq!(layer_2.name(), name_l, "{}", v.name);
        assert_eq!(layer_2.cel(), doc.layer(l).expect("L").cel(), "{}", v.name);
        assert_eq!(after.image(layer_2.cel()), Some(&image_2), "{}", v.name);
    }
}

#[test]
fn row_06_b_errors_empty_ones_and_the_immediate_layer() {
    let doc = doc_rgba(
        2,
        1,
        &[
            Layer2::rgba(&[A_OPAQUE, A_OPAQUE]),
            Layer2::rgba(&[c(10, 20, 30, 255), T]).locked(),
            Layer2::rgba(&[WHITE, GRAY_128]).mode(BlendMode::Multiply),
        ],
    );
    let [a, b, c_] = ids3(&doc);

    // Sin capa debajo.
    assert_eq!(merge_down_command(&doc, a), Err(MergeError::NoLayerBelow));
    // La capa de abajo es la inmediata (aunque esté bloqueada: el bloqueo no se mira).
    let cmd = merge_down_command(&doc, c_).expect("combinar C con B");
    let f = fusion(&cmd);
    assert_eq!((f.target, f.remove.as_slice()), (b, [c_].as_slice()));
    let cmd = merge_down_command(&doc, b).expect("combinar B con A");
    assert_eq!(fusion(&cmd).target, a);
    // Capa desconocida.
    let ghost = LayerId::from_raw(9999);
    assert_eq!(
        merge_down_command(&doc, ghost),
        Err(MergeError::Document(DocumentError::UnknownLayer(ghost)))
    );

    // Ocultas: primero la de arriba, después la de abajo.
    let hidden = |top: bool, below: bool| {
        let mut layers = vec![
            Layer2::rgba(&[A_OPAQUE, A_OPAQUE]),
            Layer2::rgba(&[WHITE, WHITE]),
        ];
        layers[0].visible = !below;
        layers[1].visible = !top;
        doc_rgba(2, 1, &layers)
    };
    let only_top = hidden(true, false);
    let [l, u] = <[LayerId; 2]>::try_from(ids(&only_top)).expect("dos capas");
    assert_eq!(merge_down_command(&only_top, u), Err(MergeError::Hidden(u)));
    let only_below = hidden(false, true);
    assert_eq!(
        merge_down_command(&only_below, u),
        Err(MergeError::Hidden(l))
    );
    let both = hidden(true, true);
    assert_eq!(merge_down_command(&both, u), Err(MergeError::Hidden(u)));

    // Ambas celdas vacías: resultado vacío (la capa 0 siempre tiene imagen, así que va debajo).
    let empty_2 = doc_rgba(
        2,
        1,
        &[
            Layer2::rgba(&[A_OPAQUE, A_OPAQUE]),
            Layer2::empty(),
            Layer2::empty(),
        ],
    );
    let [_, l, u] = ids3(&empty_2);
    let cmd = merge_down_command(&empty_2, u).expect("combinar dos vacías");
    let f = fusion(&cmd);
    assert_eq!((f.target, f.remove.as_slice()), (l, [u].as_slice()));
    assert_eq!(f.image, None);
    assert_eq!(f.based_on, empty_2.state_id());

    // Solo una con imagen: sale esa con su opacidad horneada.
    let a_2 = doc_rgba(
        2,
        1,
        &[
            Layer2::rgba(&[A_OPAQUE, c(10, 20, 30, 100)])
                .mode(BlendMode::Multiply)
                .opacity_2(128),
            Layer2::empty().opacity_2(77),
        ],
    );
    let [l, u] = <[LayerId; 2]>::try_from(ids(&a_2)).expect("dos capas");
    let f = fusion(&merge_down_command(&a_2, u).expect("combinar con una vacía"));
    let image_2 = f.image.expect("hay imagen");
    assert_eq!(rgba_de(&image_2), [c(200, 100, 50, 128), c(10, 20, 30, 50)]);
    assert_eq!(image_2, layers_of(&a_2, &[l, u]));
}

// ---------------------------------------------------------------------------------------
// Fila 7: merge_visible_command y flatten_command.
// ---------------------------------------------------------------------------------------

/// Comprueba el caso `[A, B hidden, C, D con cel_2 vacía]` de un documento de cualquier modo.
fn checks_abcd(doc: &Document) {
    let [a, b, c_, d] = ids4(doc);
    assert!(!doc.layer(b).expect("B").visible());
    assert!(doc.image(doc.layer(d).expect("D").cel()).is_none());
    let before = compose_2(doc);

    // Combinar visibles: A se queda, C y D se quitan, la oculta B no entra en `remove`.
    let cmd = merge_visible_command(doc).expect("combinar visibles");
    let f = fusion(&cmd);
    assert_eq!(f.target, a);
    assert_eq!(f.remove, [c_, d]);
    assert_eq!(f.image.as_ref(), Some(&before));
    assert_eq!(f.based_on, doc.state_id());
    let after = applied(doc, cmd);
    assert_eq!(ids(&after), [a, b], "quedan A y la oculta B");
    assert_eq!(after.layer(b), doc.layer(b), "la oculta no cambia");
    assert_eq!(after.layers().len(), 2);
    assert_eq!(after.image(after.layers()[0].cel()), Some(&before));
    assert_eq!(compose_2(&after), before);

    // Aplanar: además se quita la oculta.
    let cmd = flatten_command(doc).expect("aplanar");
    let f = fusion(&cmd);
    assert_eq!(f.target, a);
    assert_eq!(f.remove, [b, c_, d]);
    assert_eq!(f.image.as_ref(), Some(&before));
    assert_eq!(f.based_on, doc.state_id());
    let after = applied(doc, cmd);
    assert_eq!(ids(&after), [a]);
    assert_eq!(after.layers().len(), 1);
    assert_eq!(after.image(after.layers()[0].cel()), Some(&before));
    assert_eq!(compose_2(&after), before);
}

#[test]
fn row_07_a_merge_visible_and_flatten_with_abcd_rgba() {
    let doc = doc_rgba(
        3,
        1,
        &[
            Layer2::rgba(&[A_OPAQUE, A_OPAQUE, c(20, 40, 60, 90)]),
            Layer2::rgba(&[c(1, 2, 3, 255); 3]).hidden(),
            Layer2::rgba(&[c(10, 20, 30, 255), c(255, 255, 255, 128), T])
                .mode(BlendMode::Multiply)
                .opacity_2(200),
            Layer2::empty().mode(BlendMode::Screen).opacity_2(100),
        ],
    );
    checks_abcd(&doc);
}

#[test]
fn row_07_b_visibility_variants_and_empty_ones() {
    // La visible más baja no es la capa 0: [A oculta, B, C, D oculta].
    let doc = doc_rgba(
        2,
        1,
        &[
            Layer2::rgba(&[A_OPAQUE, A_OPAQUE]).hidden(),
            Layer2::rgba(&[c(10, 20, 30, 255), T]),
            Layer2::rgba(&[c(255, 0, 0, 128), c(0, 0, 255, 255)]).mode(BlendMode::Screen),
            Layer2::rgba(&[WHITE, WHITE]).hidden(),
        ],
    );
    let [a, b, c_, d] = ids4(&doc);
    let before = compose_2(&doc);
    let f = fusion(&merge_visible_command(&doc).expect("combinar visibles"));
    assert_eq!((f.target, f.remove.as_slice()), (b, [c_].as_slice()));
    assert_eq!(f.image.as_ref(), Some(&before));
    let f = fusion(&flatten_command(&doc).expect("aplanar"));
    assert_eq!((f.target, f.remove.as_slice()), (b, [a, c_, d].as_slice()));
    assert_eq!(f.image.as_ref(), Some(&before));
    assert_eq!(f.based_on, doc.state_id());

    // Una sola visible entre varias: combinar visibles no tiene nada que combinar; aplanar deja
    // la visible con la opacidad horneada y quita las ocultas.
    let a_2 = doc_rgba(
        2,
        1,
        &[
            Layer2::rgba(&[WHITE, WHITE]).hidden(),
            Layer2::rgba(&[A_OPAQUE, c(10, 20, 30, 100)]).opacity_2(128),
            Layer2::rgba(&[WHITE, WHITE]).hidden(),
        ],
    );
    let [a, b, c_] = ids3(&a_2);
    assert_eq!(merge_visible_command(&a_2), Err(MergeError::NothingToMerge));
    let cmd = flatten_command(&a_2).expect("aplanar con una visible");
    let f = fusion(&cmd);
    assert_eq!((f.target, f.remove.as_slice()), (b, [a, c_].as_slice()));
    let image_2 = f.image.clone().expect("hay imagen");
    assert_eq!(rgba_de(&image_2), [c(200, 100, 50, 128), c(10, 20, 30, 50)]);
    let after = applied(&a_2, cmd);
    assert_eq!(ids(&after), [b]);
    assert_eq!(after.image(after.layers()[0].cel()), Some(&image_2));

    // Ninguna visible, o una sola capa: `NothingToMerge` en las dos.
    let hidden_2 = doc_rgba(
        2,
        1,
        &[
            Layer2::rgba(&[A_OPAQUE, A_OPAQUE]).hidden(),
            Layer2::rgba(&[WHITE, WHITE]).hidden(),
        ],
    );
    assert_eq!(
        merge_visible_command(&hidden_2),
        Err(MergeError::NothingToMerge)
    );
    assert_eq!(flatten_command(&hidden_2), Err(MergeError::NothingToMerge));
    let single = doc_rgba(2, 1, &[Layer2::rgba(&[A_OPAQUE, A_OPAQUE])]);
    assert_eq!(
        merge_visible_command(&single),
        Err(MergeError::NothingToMerge)
    );
    assert_eq!(flatten_command(&single), Err(MergeError::NothingToMerge));

    // Todas las visibles vacías (la capa 0, oculta, es la única con imagen): resultado vacío.
    let empty_2 = doc_rgba(
        2,
        1,
        &[
            Layer2::rgba(&[A_OPAQUE, A_OPAQUE]).hidden(),
            Layer2::empty(),
            Layer2::empty().opacity_2(100),
        ],
    );
    let [a, b, c_] = ids3(&empty_2);
    let before = compose_2(&empty_2);
    let f = fusion(&merge_visible_command(&empty_2).expect("combinar vacías"));
    assert_eq!((f.target, f.remove.as_slice()), (b, [c_].as_slice()));
    assert_eq!(f.image, None);
    let cmd = flatten_command(&empty_2).expect("aplanar vacías");
    let f = fusion(&cmd);
    assert_eq!((f.target, f.remove.as_slice()), (b, [a, c_].as_slice()));
    assert_eq!(f.image, None);
    assert_eq!(f.based_on, empty_2.state_id());
    let after = applied(&empty_2, cmd);
    assert_eq!(after.image(after.layers()[0].cel()), None);
    assert_eq!(compose_2(&after), before);
}

#[test]
fn row_07_c_indexed_and_gray_with_alpha() {
    // Indexado con transparente 0.
    let indexed = doc_indexed(
        4,
        1,
        Some(0),
        &[
            Layer2::idx(&[1, 2, 3, 0]),
            Layer2::idx(&[0, 4, 0, 0]).hidden(),
            Layer2::idx(&[5, 0, 0, 0]),
            Layer2::empty(),
        ],
    );
    assert_eq!(indices_de(&compose_2(&indexed)), [5, 2, 3, 0]);
    checks_abcd(&indexed);

    // Gris con alfa, con opacidad y mezcla en la tercera capa.
    let gray_2 = builds(
        image_2(2, 1, Mode::GrayAlpha),
        &[
            Layer2::gray_2(&[(200, 255), (100, 128)]),
            Layer2::gray_2(&[(1, 255), (1, 255)]).hidden(),
            Layer2::gray_2(&[(128, 255), (255, 100)])
                .mode(BlendMode::Multiply)
                .opacity_2(128),
            Layer2::empty(),
        ],
    );
    checks_abcd(&gray_2);

    // Indexado sin transparente con todo oculto: ninguna visible, `NothingToMerge`.
    let mut without_t = doc_indexed(
        4,
        1,
        Some(0),
        &[Layer2::idx(&[1, 2, 3, 0]), Layer2::idx(&[1, 1, 1, 1])],
    );
    let removes = Command::SetTransparentIndex {
        image: without_t.layers()[0].cel(),
        index: None,
    };
    assert_eq!(without_t.apply(removes), Ok(true));
    for id in ids(&without_t) {
        let hidden = Command::SetLayerVisible {
            layer: id,
            visible: false,
        };
        assert_eq!(without_t.apply(hidden), Ok(true));
    }
    assert_eq!(
        merge_visible_command(&without_t),
        Err(MergeError::NothingToMerge)
    );
    assert_eq!(flatten_command(&without_t), Err(MergeError::NothingToMerge));
}

// ---------------------------------------------------------------------------------------
// Generadores de documentos para las propiedades.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Type {
    Rgba,
    GrayAlpha,
    /// Indexado con ese índice transparente (paleta de 4 colores).
    Indexed(u8),
}

#[derive(Clone, Debug)]
struct DocGen {
    kind: Type,
    w: u32,
    h: u32,
    layers: Vec<Layer2>,
}

/// Alfa o opacidad con los extremos más frecuentes que en el uniforme.
fn alpha_2() -> impl Strategy<Value = u8> {
    prop_oneof![1 => Just(0u8), 1 => Just(255u8), 2 => any::<u8>()]
}

fn mode_any() -> impl Strategy<Value = BlendMode> {
    proptest::sample::select(BlendMode::ALL.to_vec())
}

fn type_strategy() -> impl Strategy<Value = Type> {
    prop_oneof![
        Just(Type::Rgba),
        Just(Type::GrayAlpha),
        (0u8..4).prop_map(Type::Indexed),
    ]
}

fn pixel_strategy(kind: Type) -> BoxedStrategy<Pixel> {
    match kind {
        Type::Rgba => (any::<u8>(), any::<u8>(), any::<u8>(), alpha_2())
            .prop_map(|(r, g, b, a)| Pixel::Rgba(Rgba::new(r, g, b, a)))
            .boxed(),
        Type::GrayAlpha => (any::<u8>(), alpha_2())
            .prop_map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
            .boxed(),
        Type::Indexed(_) => (0u8..4).prop_map(Pixel::Indexed).boxed(),
    }
}

fn layer_strategy(kind: Type, pixels_2: usize) -> impl Strategy<Value = Layer2> {
    let indexed = matches!(kind, Type::Indexed(_));
    (
        proptest::bool::weighted(0.8),
        proptest::bool::weighted(0.3),
        alpha_2(),
        mode_any(),
        proptest::option::weighted(
            0.8,
            proptest::collection::vec(pixel_strategy(kind), pixels_2),
        ),
    )
        .prop_map(move |(visible, locked, opacity_2, mode, cel_2)| {
            // En indexado el núcleo solo admite opacidad 255 y modo Normal.
            let (opacity_2, mode) = if indexed {
                (255, BlendMode::Normal)
            } else {
                (opacity_2, mode)
            };
            Layer2 {
                visible,
                locked,
                opacity_2,
                mode,
                cel_2,
            }
        })
}

/// Documentos de hasta 6 × 5 con `min` a `max` capas.
fn doc_strategy(min: usize, max: usize) -> impl Strategy<Value = DocGen> {
    (type_strategy(), 1u32..=6, 1u32..=5).prop_flat_map(move |(kind, w, h)| {
        proptest::collection::vec(layer_strategy(kind, (w * h) as usize), min..=max)
            .prop_map(move |layers| DocGen { kind, w, h, layers })
    })
}

fn document(g: &DocGen) -> Document {
    let template = match g.kind {
        Type::Rgba => image_2(g.w, g.h, Mode::Rgba),
        Type::GrayAlpha => image_2(g.w, g.h, Mode::GrayAlpha),
        Type::Indexed(t) => template_indexed(g.w, g.h, 4, Some(t)),
    };
    builds(template, &g.layers)
}

/// Fuerza que las capas `k` y `k + 1` sean visibles.
fn with_visible(g: &DocGen, k: usize) -> DocGen {
    let mut g = g.clone();
    g.layers[k].visible = true;
    g.layers[k + 1].visible = true;
    g
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: aplanar es la composición.
// ---------------------------------------------------------------------------------------

fn checks_composition(g: &DocGen, flatten: bool) -> Result<(), TestCaseError> {
    let before_doc = document(g);
    let before = compose(&before_doc).expect("hay índice transparente: compose no falla");
    let visible: Vec<LayerId> = before_doc
        .layers()
        .iter()
        .filter(|l| l.visible())
        .map(Layer::id)
        .collect();
    let nothing_that_do = if flatten {
        before_doc.layers().len() == 1 || visible.is_empty()
    } else {
        visible.len() < 2
    };
    let cmd = if flatten {
        flatten_command(&before_doc)
    } else {
        merge_visible_command(&before_doc)
    };
    let cmd = match cmd {
        Err(e) => {
            prop_assert_eq!(e, MergeError::NothingToMerge);
            prop_assert!(nothing_that_do, "`NothingToMerge` sin motivo");
            return Ok(());
        }
        Ok(cmd) => cmd,
    };
    prop_assert!(!nothing_that_do, "había que dar `NothingToMerge`");

    let f = fusion(&cmd);
    prop_assert_eq!(f.target, visible[0]);
    let expected: Vec<LayerId> = if flatten {
        ids(&before_doc)
            .into_iter()
            .filter(|&id| id != visible[0])
            .collect()
    } else {
        visible[1..].to_vec()
    };
    prop_assert_eq!(&f.remove, &expected);
    prop_assert_eq!(f.based_on, before_doc.state_id());
    let no_with_image = visible.iter().all(|&id| {
        before_doc
            .image(before_doc.layer(id).unwrap().cel())
            .is_none()
    });
    prop_assert_eq!(f.image.is_none(), no_with_image);
    if let Some(image_2) = &f.image {
        prop_assert_eq!(image_2, &before);
    }

    let mut doc = before_doc.clone();
    prop_assert_eq!(doc.apply(cmd), Ok(true));
    // La vista no cambia: ni un byte.
    prop_assert_eq!(compose(&doc), Ok(before.clone()));
    // La celda resultante es `compose(before)` (o vacía).
    let cel = doc.layer(f.target).unwrap().cel();
    prop_assert_eq!(doc.image(cel), f.image.as_ref());
    let layer_2 = doc.layer(f.target).unwrap();
    prop_assert_eq!(
        (layer_2.opacity(), layer_2.blend(), layer_2.locked()),
        (255, BlendMode::Normal, false)
    );
    if flatten {
        prop_assert_eq!(doc.layers().len(), 1);
    } else {
        prop_assert_eq!(
            doc.layers().len(),
            before_doc.layers().len() - (visible.len() - 1)
        );
        // Las ocultas quedan tal cual.
        for l in before_doc.layers().iter().filter(|l| !l.visible()) {
            prop_assert_eq!(doc.layer(l.id()), Some(l));
        }
    }
    Ok(())
}

fn checks_merge_down_on_the_background(g: &DocGen) -> Result<(), TestCaseError> {
    let g = with_visible(g, 0);
    let before_doc = document(&g);
    let before = compose(&before_doc).expect("compose no falla");
    let upper = before_doc.layers()[1].id();
    let cmd = merge_down_command(&before_doc, upper).expect("las dos son visibles");
    let mut doc = before_doc.clone();
    prop_assert_eq!(doc.apply(cmd), Ok(true));
    prop_assert_eq!(doc.layers().len(), before_doc.layers().len() - 1);
    prop_assert_eq!(compose(&doc), Ok(before));
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // 1. Aplanar es la composición.
    #[test]
    fn property_1_flatten_is_the_composition(g in doc_strategy(1, 5)) {
        checks_composition(&g, true)?;
        checks_composition(&g, false)?;
        if g.layers.len() >= 2 {
            checks_merge_down_on_the_background(&g)?;
        }
    }

    // 4. Combinar abajo contra un modelo independiente.
    #[test]
    fn property_4_merge_down_against_a_model(
        g in doc_strategy(2, 5),
        sel in any::<proptest::sample::Index>(),
    ) {
        let k = sel.index(g.layers.len() - 1);
        checks_model(&with_visible(&g, k), k)?;
    }

    // 5. Deshacer una combinación devuelve las capas tal cual.
    #[test]
    fn property_5_undo_returns_the_layers_as_they_were(
        g in doc_strategy(2, 5),
        operation in 0u8..3,
        sel in any::<proptest::sample::Index>(),
    ) {
        checks_undo(&g, operation, sel.index(g.layers.len() - 1))?;
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: el modelo independiente.
// ---------------------------------------------------------------------------------------

fn a_rgba(p: Pixel) -> Rgba {
    match p {
        Pixel::Rgba(color) => color,
        Pixel::GrayAlpha { gray, alpha } => Rgba::new(gray, gray, gray, alpha),
        other => panic!("el modelo no conoce {other:?}"),
    }
}

fn index(p: Pixel) -> u8 {
    match p {
        Pixel::Indexed(i) => i,
        other => panic!("se esperaba indexado, hay {other:?}"),
    }
}

/// El píxel que debe tener la celda combinada de las capas `k` (abajo) y `k + 1` en `(x, y)`:
/// plegado de `composite_pixel` sobre vacío (primero la de abajo con su opacidad y modo, luego la
/// de arriba con los suyos; las celdas vacías no aportan) y, en indexado, el índice de la de
/// arriba si no es el transparente y si no el de la de abajo (celda vacía = transparente).
fn model(doc: &Document, k: usize, x: u32, y: u32) -> Pixel {
    let par = [&doc.layers()[k], &doc.layers()[k + 1]];
    let pixel_de = |layer_2: &Layer| {
        doc.image(layer_2.cel())
            .map(|img| img.get_pixel(x, y).expect("píxel dentro"))
    };
    match doc.mode() {
        Mode::Indexed => {
            let t = doc
                .palette()
                .and_then(Palette::transparent_index)
                .expect("el generador siempre da índice transparente");
            let below = pixel_de(par[0]).map_or(t, index);
            let top = pixel_de(par[1]).map_or(t, index);
            Pixel::Indexed(if top != t { top } else { below })
        }
        mode => {
            let mut accumulated = T;
            for layer_2 in par {
                if let Some(p) = pixel_de(layer_2) {
                    accumulated =
                        composite_pixel(accumulated, a_rgba(p), layer_2.opacity(), layer_2.blend());
                }
            }
            if mode == Mode::GrayAlpha {
                Pixel::GrayAlpha {
                    gray: accumulated.r,
                    alpha: accumulated.a,
                }
            } else {
                Pixel::Rgba(accumulated)
            }
        }
    }
}

/// El píxel de una celda vacía (nada que dar), en el modo del documento.
fn pixel_empty(doc: &Document) -> Pixel {
    match doc.mode() {
        Mode::Indexed => Pixel::Indexed(
            doc.palette()
                .and_then(Palette::transparent_index)
                .expect("índice transparente"),
        ),
        Mode::GrayAlpha => Pixel::GrayAlpha { gray: 0, alpha: 0 },
        _ => Pixel::Rgba(T),
    }
}

fn checks_model(g: &DocGen, k: usize) -> Result<(), TestCaseError> {
    let before_doc = document(g);
    let before = compose(&before_doc).expect("compose no falla");
    let (l, u) = (before_doc.layers()[k].id(), before_doc.layers()[k + 1].id());
    let cmd = merge_down_command(&before_doc, u).expect("las dos son visibles");
    let f = fusion(&cmd);
    prop_assert_eq!(f.target, l);
    prop_assert_eq!(&f.remove, &vec![u]);

    let mut doc = before_doc.clone();
    prop_assert_eq!(doc.apply(cmd), Ok(true));
    prop_assert!(doc.materialized_bytes() <= before_doc.materialized_bytes());
    prop_assert_eq!(doc.layers().len(), before_doc.layers().len() - 1);

    let result = doc.image(doc.layer(l).unwrap().cel());
    for y in 0..g.h {
        for x in 0..g.w {
            let obtained = result.map_or_else(
                || pixel_empty(&before_doc),
                |img| img.get_pixel(x, y).expect("píxel dentro"),
            );
            prop_assert_eq!(
                obtained,
                model(&before_doc, k, x, y),
                "píxel ({}, {}) de la combinación de las capas {} y {}",
                x,
                y,
                k,
                k + 1
            );
        }
    }
    if k == 0 {
        prop_assert_eq!(compose(&doc), Ok(before));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Propiedad 5: deshacer y rehacer una combinación.
// ---------------------------------------------------------------------------------------

/// `operation`: 0 = combinar abajo (la capa `k + 1` con la `k`), 1 = combinar visibles,
/// 2 = aplanar.
fn checks_undo(g: &DocGen, operation: u8, k: usize) -> Result<(), TestCaseError> {
    let g = if operation == 0 {
        with_visible(g, k)
    } else {
        g.clone()
    };
    let before = document(&g);
    let cmd = match operation {
        0 => merge_down_command(&before, before.layers()[k + 1].id()),
        1 => merge_visible_command(&before),
        _ => flatten_command(&before),
    };
    let cmd = match cmd {
        Err(e) => {
            prop_assert_eq!(e, MergeError::NothingToMerge);
            return Ok(());
        }
        Ok(cmd) => cmd,
    };
    let f = fusion(&cmd);
    let removed: Vec<(usize, Layer)> = f
        .remove
        .iter()
        .map(|id| {
            let pos = before.layers().iter().position(|l| l.id() == *id).unwrap();
            (pos, before.layers()[pos].clone())
        })
        .collect();
    let goal_before = before.layer(f.target).unwrap().clone();

    let mut doc = before.clone();
    prop_assert_eq!(doc.apply(cmd), Ok(true));
    prop_assert_ne!(doc.state_id(), before.state_id());
    let after = doc.clone();

    prop_assert_eq!(doc.undo(), Ok(()));
    prop_assert_eq!(doc.state_id(), before.state_id());
    prop_assert_eq!(ids(&doc), ids(&before));
    for (pos, original) in &removed {
        let ret = &doc.layers()[*pos];
        prop_assert_eq!(signature(ret), signature(original));
        prop_assert_eq!(doc.image(original.cel()), before.image(original.cel()));
        prop_assert!(
            ret == original,
            "la capa quitada {:?} no vuelve igual",
            original.id()
        );
    }
    prop_assert_eq!(doc.layer(f.target), Some(&goal_before));
    prop_assert!(
        doc == before,
        "tras deshacer el documento no es el de antes"
    );

    prop_assert_eq!(doc.redo(), Ok(()));
    prop_assert_eq!(doc.state_id(), after.state_id());
    prop_assert!(
        doc == after,
        "tras rehacer el documento no es el de después"
    );
    Ok(())
}
