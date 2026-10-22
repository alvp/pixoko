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
fn redondea_255(x: i128) -> i128 {
    (2 * x + 255).div_euclid(510)
}

/// Una capa de un documento de prueba. `celda: None` es una celda vacía (sin imagen); la capa 0
/// siempre tiene celda, así que en ella `None` deja la imagen inicial (toda a ceros).
#[derive(Clone, Debug)]
struct Capa {
    visible: bool,
    locked: bool,
    opacidad: u8,
    modo: BlendMode,
    celda: Option<Vec<Pixel>>,
}

impl Capa {
    fn nueva(celda: Vec<Pixel>) -> Capa {
        Capa {
            visible: true,
            locked: false,
            opacidad: 255,
            modo: BlendMode::Normal,
            celda: Some(celda),
        }
    }

    fn rgba(celda: &[Rgba]) -> Capa {
        Capa::nueva(celda.iter().copied().map(Pixel::Rgba).collect())
    }

    fn gris(celda: &[(u8, u8)]) -> Capa {
        Capa::nueva(
            celda
                .iter()
                .map(|&(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
                .collect(),
        )
    }

    fn idx(valores: &[u8]) -> Capa {
        Capa::nueva(valores.iter().copied().map(Pixel::Indexed).collect())
    }

    fn vacia() -> Capa {
        Capa {
            visible: true,
            locked: false,
            opacidad: 255,
            modo: BlendMode::Normal,
            celda: None,
        }
    }

    fn modo(mut self, modo: BlendMode) -> Capa {
        self.modo = modo;
        self
    }

    fn opacidad(mut self, opacidad: u8) -> Capa {
        self.opacidad = opacidad;
        self
    }

    fn oculta(mut self) -> Capa {
        self.visible = false;
        self
    }

    fn bloqueada(mut self) -> Capa {
        self.locked = true;
        self
    }
}

/// Documento con `capas` de abajo arriba, a partir de la imagen `plantilla` (que es la celda de
/// la capa 0 y fija el tamaño y el modo). Solo usa los comandos del núcleo ya existentes.
fn construye(plantilla: Image, capas: &[Capa]) -> Document {
    let mut doc = Document::new(plantilla);
    let (w, h) = (doc.width(), doc.height());
    for i in 1..capas.len() {
        let cmd = Command::AddLayer {
            name: format!("C{i}"),
            at: i,
        };
        assert_eq!(doc.apply(cmd), Ok(true), "añadir la capa {i}");
    }
    for (i, capa) in capas.iter().enumerate() {
        let (id, cel) = (doc.layers()[i].id(), doc.layers()[i].cel());
        if let Some(pixeles) = &capa.celda {
            assert_eq!(pixeles.len(), (w * h) as usize, "píxeles de la capa {i}");
            let pixels = pixeles
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
        if capa.opacidad != 255 {
            let cmd = Command::SetLayerOpacity {
                layer: id,
                opacity: capa.opacidad,
            };
            assert!(doc.apply(cmd).is_ok(), "opacidad de la capa {i}");
        }
        if capa.modo != BlendMode::Normal {
            let cmd = Command::SetLayerBlend {
                layer: id,
                blend: capa.modo,
            };
            assert!(doc.apply(cmd).is_ok(), "modo de la capa {i}");
        }
        if !capa.visible {
            let cmd = Command::SetLayerVisible {
                layer: id,
                visible: false,
            };
            assert!(doc.apply(cmd).is_ok(), "ocultar la capa {i}");
        }
        if capa.locked {
            let cmd = Command::SetLayerLocked {
                layer: id,
                locked: true,
            };
            assert!(doc.apply(cmd).is_ok(), "bloquear la capa {i}");
        }
    }
    doc
}

fn imagen(w: u32, h: u32, modo: Mode) -> Image {
    Image::new(w, h, modo).expect("imagen")
}

fn doc_rgba(w: u32, h: u32, capas: &[Capa]) -> Document {
    construye(imagen(w, h, Mode::Rgba), capas)
}

/// Paleta de `n` colores opacos distintos con el índice transparente `t`.
fn paleta(n: u8, t: Option<u8>) -> Palette {
    let mut p = Palette::from_colors(
        (0..n).map(|i| Rgba::new(i.wrapping_mul(30), 255 - i.wrapping_mul(30), 7, 255)),
    )
    .expect("paleta");
    p.set_transparent_index(t).expect("índice transparente");
    p
}

fn plantilla_indexada(w: u32, h: u32, n: u8, t: Option<u8>) -> Image {
    Image::new_indexed(w, h, paleta(n, t)).expect("imagen indexada")
}

fn doc_indexado(w: u32, h: u32, t: Option<u8>, capas: &[Capa]) -> Document {
    construye(plantilla_indexada(w, h, 8, t), capas)
}

/// Los píxeles de una imagen RGBA, fila a fila.
fn rgba_de(img: &Image) -> Vec<Rgba> {
    assert_eq!(img.mode(), Mode::Rgba, "se esperaba una imagen RGBA");
    pixeles_de(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Rgba(color) => color,
            otro => panic!("se esperaba RGBA, hay {otro:?}"),
        })
        .collect()
}

/// Los píxeles de una imagen en cualquier modo, fila a fila.
fn pixeles_de(img: &Image) -> Vec<Pixel> {
    (0..img.height())
        .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
        .map(|(x, y)| img.get_pixel(x, y).expect("píxel dentro"))
        .collect()
}

/// Los índices de una imagen indexada, fila a fila.
fn indices_de(img: &Image) -> Vec<u8> {
    pixeles_de(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Indexed(i) => i,
            otro => panic!("se esperaba indexado, hay {otro:?}"),
        })
        .collect()
}

/// `compose` que no debe fallar.
fn componer(doc: &Document) -> Image {
    compose(doc).expect("la composición no debe fallar")
}

/// `compose_layers` que no debe fallar.
fn capas_de(doc: &Document, ids: &[LayerId]) -> Image {
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
type Firma = (LayerId, ImageId, String, bool, bool, u8, BlendMode);

fn firma(l: &Layer) -> Firma {
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
        otro => panic!("se esperaba MergeLayers, hay {otro:?}"),
    }
}

/// Aplica `cmd` sobre una copia del documento y la devuelve (un solo paso).
fn aplicado(doc: &Document, cmd: Command) -> Document {
    let mut copia = doc.clone();
    assert_eq!(copia.apply(cmd), Ok(true), "aplicar el comando de combinar");
    assert_ne!(copia.state_id(), doc.state_id(), "estado nuevo");
    copia
}

// ---------------------------------------------------------------------------------------
// Fila 5: compose_layers.
// ---------------------------------------------------------------------------------------

const BLANCO: Rgba = c(255, 255, 255, 255);
const GRIS_128: Rgba = c(128, 128, 128, 255);
const A_OPACO: Rgba = c(200, 100, 50, 255);

/// `[A, B, C]` de 2 × 1: el fondo opaco `A = (200, 100, 50)`; encima `B`, Normal, con
/// `(10, 20, 30)` solo en el primer píxel; y encima `C`, Multiplicar, con blanco y gris 128.
fn pila_abc() -> Vec<Capa> {
    vec![
        Capa::rgba(&[A_OPACO, A_OPACO]),
        Capa::rgba(&[c(10, 20, 30, 255), T]),
        Capa::rgba(&[BLANCO, GRIS_128]).modo(BlendMode::Multiply),
    ]
}

#[test]
fn fila_05_a_subconjuntos_en_orden_y_sin_mirar_la_visibilidad() {
    let doc = doc_rgba(2, 1, &pila_abc());
    let [a, b, c_] = ids3(&doc);

    // {A, C}: B no cuenta. A multiplicado por blanco y por 128: div255(200·128) = 100,
    // div255(100·128) = 50 y div255(50·128) = 25.
    let a_c = [A_OPACO, c(100, 50, 25, 255)];
    assert_eq!(rgba_de(&capas_de(&doc, &[a, c_])), a_c);
    // El orden de la lista no manda: manda el del documento.
    assert_eq!(rgba_de(&capas_de(&doc, &[c_, a])), a_c);
    // {B, C}: sin A, contra el vacío (el gris 128 se queda tal cual sobre un fondo sin alfa).
    assert_eq!(
        rgba_de(&capas_de(&doc, &[b, c_])),
        [c(10, 20, 30, 255), GRIS_128]
    );
    // Una sola capa: su celda.
    assert_eq!(rgba_de(&capas_de(&doc, &[b])), [c(10, 20, 30, 255), T]);

    // Todas las visibles: `compose` byte a byte.
    assert_eq!(capas_de(&doc, &[a, b, c_]), componer(&doc));
    assert_eq!(
        rgba_de(&capas_de(&doc, &[a, b, c_])),
        [c(10, 20, 30, 255), c(100, 50, 25, 255)]
    );

    // Exactamente las visibles de un documento con una oculta: igual que `compose`.
    let con_oculta = doc_rgba(
        2,
        1,
        &[
            pila_abc()[0].clone(),
            pila_abc()[1].clone(),
            pila_abc()[2].clone().oculta(),
        ],
    );
    let [a, b, c_] = ids3(&con_oculta);
    assert_eq!(capas_de(&con_oculta, &[a, b]), componer(&con_oculta));

    // Una oculta incluida cuenta: {A, C} con C oculta sigue multiplicando, y `compose` no.
    assert_eq!(rgba_de(&capas_de(&con_oculta, &[a, c_])), a_c);
    assert_ne!(capas_de(&con_oculta, &[a, c_]), componer(&con_oculta));
    // Con todas ocultas, la composición de las tres sigue siendo la de siempre.
    let todas: Vec<Capa> = pila_abc().into_iter().map(Capa::oculta).collect();
    let ocultas = doc_rgba(2, 1, &todas);
    assert_eq!(
        rgba_de(&capas_de(&ocultas, &ids(&ocultas))),
        [c(10, 20, 30, 255), c(100, 50, 25, 255)]
    );
}

#[test]
fn fila_05_b_vacias_y_opacidad_de_la_capa_de_abajo() {
    // Una celda vacía no aporta nada, ni siquiera con modo y opacidad raros.
    let con_vacia = doc_rgba(
        2,
        1,
        &[
            pila_abc()[0].clone(),
            Capa::vacia().modo(BlendMode::Multiply).opacidad(100),
        ],
    );
    let [a, d] = <[LayerId; 2]>::try_from(ids(&con_vacia)).expect("dos capas");
    assert_eq!(rgba_de(&capas_de(&con_vacia, &[a, d])), [A_OPACO, A_OPACO]);
    assert_eq!(rgba_de(&capas_de(&con_vacia, &[d])), [T, T]);

    // La capa de abajo se compone contra el vacío con SU opacidad (el modo no tiene efecto y la
    // opacidad escala el alfa: div255(100·128) = 50).
    let con_opacidad = doc_rgba(
        2,
        1,
        &[Capa::rgba(&[A_OPACO, c(10, 20, 30, 100)])
            .modo(BlendMode::Multiply)
            .opacidad(128)],
    );
    let solo = ids(&con_opacidad);
    assert_eq!(
        rgba_de(&capas_de(&con_opacidad, &solo)),
        [c(200, 100, 50, 128), c(10, 20, 30, 50)]
    );
}

#[test]
fn fila_05_c_indexado_y_gris_con_alfa() {
    // Indexado, transparente 0: gana el índice más alto no transparente de las capas pedidas.
    let doc = doc_indexado(
        4,
        1,
        Some(0),
        &[
            Capa::idx(&[1, 2, 3, 0]),
            Capa::idx(&[0, 4, 0, 0]).oculta(),
            Capa::idx(&[5, 0, 0, 0]),
            Capa::vacia(),
        ],
    );
    let [l0, l1, l2, vacia] = ids4(&doc);
    let salida = capas_de(&doc, &[l0, l2]);
    assert_eq!(indices_de(&salida), [5, 2, 3, 0]);
    assert_eq!(salida.mode(), Mode::Indexed);
    assert_eq!(
        salida.palette(),
        doc.palette(),
        "la paleta es la del lienzo"
    );
    // La oculta incluida cuenta; la vacía no aporta.
    assert_eq!(indices_de(&capas_de(&doc, &[l0, l1, vacia])), [1, 4, 3, 0]);
    assert_eq!(indices_de(&capas_de(&doc, &[l0, l1, l2])), [5, 4, 3, 0]);
    // Solo la vacía: todo transparente.
    assert_eq!(indices_de(&capas_de(&doc, &[vacia])), [0, 0, 0, 0]);

    // Gris con alfa: el canal rojo del RGBA equivalente (R = G = B) y su alfa.
    let datos = [
        (vec![(200, 255), (100, 128)], BlendMode::Normal, 255, true),
        (vec![(50, 255), (80, 200)], BlendMode::Multiply, 200, true),
        (vec![(10, 100), (255, 255)], BlendMode::Screen, 128, false),
    ];
    let gris: Vec<Capa> = datos
        .iter()
        .map(|(px, modo, op, visible)| {
            let capa = Capa::gris(px).modo(*modo).opacidad(*op);
            if *visible { capa } else { capa.oculta() }
        })
        .collect();
    let rgb: Vec<Capa> = datos
        .iter()
        .map(|(px, modo, op, visible)| {
            let px: Vec<Rgba> = px.iter().map(|&(g, a)| c(g, g, g, a)).collect();
            let capa = Capa::rgba(&px).modo(*modo).opacidad(*op);
            if *visible { capa } else { capa.oculta() }
        })
        .collect();
    let doc_g = construye(imagen(2, 1, Mode::GrayAlpha), &gris);
    let doc_c = doc_rgba(2, 1, &rgb);
    let (ids_g, ids_c) = (ids(&doc_g), ids(&doc_c));
    for sel in [[0usize, 2].as_slice(), &[1, 2], &[0, 1, 2], &[2]] {
        let en_g: Vec<LayerId> = sel.iter().map(|&i| ids_g[i]).collect();
        let en_c: Vec<LayerId> = sel.iter().map(|&i| ids_c[i]).collect();
        let salida = capas_de(&doc_g, &en_g);
        assert_eq!(salida.mode(), Mode::GrayAlpha);
        let esperado: Vec<Pixel> = rgba_de(&capas_de(&doc_c, &en_c))
            .iter()
            .map(|p| Pixel::GrayAlpha {
                gray: p.r,
                alpha: p.a,
            })
            .collect();
        assert_eq!(pixeles_de(&salida), esperado, "capas {sel:?}");
    }
}

#[test]
fn fila_05_d_sin_nada_que_dar_en_un_modo_sin_transparente() {
    // Indexado sin transparente: con la de abajo llena, esa capa sola es su celda; con solo una
    // vacía no queda ningún píxel que dar.
    let mut doc = doc_indexado(4, 1, Some(0), &[Capa::idx(&[1, 2, 3, 0]), Capa::vacia()]);
    let quita = Command::SetTransparentIndex {
        image: doc.layers()[0].cel(),
        index: None,
    };
    assert_eq!(doc.apply(quita), Ok(true));
    let [llena, vacia] = <[LayerId; 2]>::try_from(ids(&doc)).expect("dos capas");
    assert_eq!(indices_de(&capas_de(&doc, &[llena])), [1, 2, 3, 0]);
    assert_eq!(indices_de(&capas_de(&doc, &[llena, vacia])), [1, 2, 3, 0]);
    assert_eq!(
        compose_layers(&doc, &[vacia]),
        Err(ComposeError::NoTransparency {
            mode: Mode::Indexed
        })
    );

    // Gris sin alfa con la única capa oculta: `compose` no da nada, pero pedirla a mano da su celda.
    let mut celda = imagen(3, 1, Mode::Gray);
    for (x, v) in [0u8, 10, 250].into_iter().enumerate() {
        celda.set_pixel(x as u32, 0, Pixel::Gray(v)).expect("píxel");
    }
    let mut gris = Document::new(celda.clone());
    let capa = gris.layers()[0].id();
    let oculta = Command::SetLayerVisible {
        layer: capa,
        visible: false,
    };
    assert_eq!(gris.apply(oculta), Ok(true));
    assert_eq!(
        compose(&gris),
        Err(ComposeError::NoTransparency { mode: Mode::Gray })
    );
    assert_eq!(compose_layers(&gris, &[capa]), Ok(celda));
}

// ---------------------------------------------------------------------------------------
// Fila 6: merge_down_command contra los 10 píxeles de Aseprite.
// ---------------------------------------------------------------------------------------

/// Un vector de la sonda: la capa de abajo `L` (capa 0) y la de arriba `U` (capa 1), de ancho
/// `l.len()` y alto 1, y lo que dio Aseprite tras combinar.
struct Vector {
    nombre: &'static str,
    l_modo: BlendMode,
    l_opacidad: u8,
    l: Vec<Rgba>,
    u_modo: BlendMode,
    u_opacidad: u8,
    u: Vec<Rgba>,
    aseprite: Vec<Rgba>,
}

const L_COMUN: [Rgba; 2] = [c(200, 100, 50, 255), c(200, 100, 50, 100)];
const U_COMUN: [Rgba; 2] = [c(100, 200, 255, 255), c(100, 200, 255, 150)];

fn vectores() -> [Vector; 4] {
    [
        Vector {
            nombre: "1: L Normal 128, U Multiplicar 200",
            l_modo: BlendMode::Normal,
            l_opacidad: 128,
            l: vec![
                c(200, 100, 50, 255),
                c(200, 100, 50, 128),
                c(0, 0, 0, 0),
                c(10, 20, 30, 255),
            ],
            u_modo: BlendMode::Multiply,
            u_opacidad: 200,
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
            nombre: "2: L Multiplicar 255, U Normal 255",
            l_modo: BlendMode::Multiply,
            l_opacidad: 255,
            l: L_COMUN.to_vec(),
            u_modo: BlendMode::Normal,
            u_opacidad: 255,
            u: U_COMUN.to_vec(),
            aseprite: vec![c(100, 200, 255, 255), c(122, 178, 210, 191)],
        },
        Vector {
            nombre: "3: L Trama 128, U Normal 255",
            l_modo: BlendMode::Screen,
            l_opacidad: 128,
            l: L_COMUN.to_vec(),
            u_modo: BlendMode::Normal,
            u_opacidad: 255,
            u: U_COMUN.to_vec(),
            aseprite: vec![c(100, 200, 255, 255), c(113, 187, 229, 171)],
        },
        Vector {
            nombre: "4: L Normal 255, U Multiplicar 100",
            l_modo: BlendMode::Normal,
            l_opacidad: 255,
            l: L_COMUN.to_vec(),
            u_modo: BlendMode::Multiply,
            u_opacidad: 100,
            u: U_COMUN.to_vec(),
            aseprite: vec![c(153, 92, 50, 255), c(153, 120, 98, 136)],
        },
    ]
}

#[test]
fn fila_06_a_los_diez_pixeles_de_aseprite() {
    for v in vectores() {
        let w = v.l.len() as u32;
        let doc = doc_rgba(
            w,
            1,
            &[
                Capa::rgba(&v.l)
                    .modo(v.l_modo)
                    .opacidad(v.l_opacidad)
                    .bloqueada(),
                Capa::rgba(&v.u).modo(v.u_modo).opacidad(v.u_opacidad),
            ],
        );
        let [l, u] = <[LayerId; 2]>::try_from(ids(&doc)).expect("dos capas");
        let nombre_l = doc.layer(l).expect("L").name().to_string();

        let cmd = merge_down_command(&doc, u).unwrap_or_else(|e| panic!("{}: {e:?}", v.nombre));
        let f = fusion(&cmd);
        assert_eq!(f.target, l, "{}", v.nombre);
        assert_eq!(f.remove, [u], "{}", v.nombre);
        assert_eq!(f.based_on, doc.state_id(), "{}", v.nombre);
        let imagen = f.image.clone().expect("hay imagen");
        assert_eq!(imagen, capas_de(&doc, &[l, u]), "{}", v.nombre);

        for (k, (obtenido, aseprite)) in rgba_de(&imagen).iter().zip(&v.aseprite).enumerate() {
            // El fondo ya compuesto (L contra el vacío) y el modo de U fijan el dominio empírico
            // de 3.2: solo ahí se admite ±1 en un canal; el alfa, nunca.
            let alfa_fondo = redondea_255(i128::from(v.l[k].a) * i128::from(v.l_opacidad));
            let en_dominio = 0 < alfa_fondo && alfa_fondo < 255 && v.u_modo != BlendMode::Normal;
            let tolerancia = i32::from(en_dominio);
            assert_eq!(
                obtenido.a, aseprite.a,
                "{} píxel {k}: el alfa debe coincidir ({obtenido:?} frente a {aseprite:?})",
                v.nombre
            );
            for (canal, o, a) in [
                ("R", obtenido.r, aseprite.r),
                ("G", obtenido.g, aseprite.g),
                ("B", obtenido.b, aseprite.b),
            ] {
                let dif = (i32::from(o) - i32::from(a)).abs();
                assert!(
                    dif <= 2,
                    "{} píxel {k}, canal {canal}: {o} frente a {a} (nunca más de 2)",
                    v.nombre
                );
                assert!(
                    dif <= tolerancia,
                    "{} píxel {k}, canal {canal}: {o} frente a {a}; dominio empírico: {en_dominio}",
                    v.nombre
                );
            }
        }

        // Aplicado: queda L, Normal, 255, desbloqueada, con su nombre y visibilidad.
        let despues = aplicado(&doc, cmd);
        assert_eq!(ids(&despues), [l], "{}", v.nombre);
        let capa = &despues.layers()[0];
        assert_eq!(capa.opacity(), 255, "{}", v.nombre);
        assert_eq!(capa.blend(), BlendMode::Normal, "{}", v.nombre);
        assert!(!capa.locked(), "{}: queda desbloqueada", v.nombre);
        assert!(capa.visible(), "{}", v.nombre);
        assert_eq!(capa.name(), nombre_l, "{}", v.nombre);
        assert_eq!(capa.cel(), doc.layer(l).expect("L").cel(), "{}", v.nombre);
        assert_eq!(despues.image(capa.cel()), Some(&imagen), "{}", v.nombre);
    }
}

#[test]
fn fila_06_b_errores_vacias_y_la_capa_inmediata() {
    let doc = doc_rgba(
        2,
        1,
        &[
            Capa::rgba(&[A_OPACO, A_OPACO]),
            Capa::rgba(&[c(10, 20, 30, 255), T]).bloqueada(),
            Capa::rgba(&[BLANCO, GRIS_128]).modo(BlendMode::Multiply),
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
    let fantasma = LayerId::from_raw(9999);
    assert_eq!(
        merge_down_command(&doc, fantasma),
        Err(MergeError::Document(DocumentError::UnknownLayer(fantasma)))
    );

    // Ocultas: primero la de arriba, después la de abajo.
    let oculta = |arriba: bool, abajo: bool| {
        let mut capas = vec![
            Capa::rgba(&[A_OPACO, A_OPACO]),
            Capa::rgba(&[BLANCO, BLANCO]),
        ];
        capas[0].visible = !abajo;
        capas[1].visible = !arriba;
        doc_rgba(2, 1, &capas)
    };
    let solo_arriba = oculta(true, false);
    let [l, u] = <[LayerId; 2]>::try_from(ids(&solo_arriba)).expect("dos capas");
    assert_eq!(
        merge_down_command(&solo_arriba, u),
        Err(MergeError::Hidden(u))
    );
    let solo_abajo = oculta(false, true);
    assert_eq!(
        merge_down_command(&solo_abajo, u),
        Err(MergeError::Hidden(l))
    );
    let ambas = oculta(true, true);
    assert_eq!(merge_down_command(&ambas, u), Err(MergeError::Hidden(u)));

    // Ambas celdas vacías: resultado vacío (la capa 0 siempre tiene imagen, así que va debajo).
    let vacias = doc_rgba(
        2,
        1,
        &[
            Capa::rgba(&[A_OPACO, A_OPACO]),
            Capa::vacia(),
            Capa::vacia(),
        ],
    );
    let [_, l, u] = ids3(&vacias);
    let cmd = merge_down_command(&vacias, u).expect("combinar dos vacías");
    let f = fusion(&cmd);
    assert_eq!((f.target, f.remove.as_slice()), (l, [u].as_slice()));
    assert_eq!(f.image, None);
    assert_eq!(f.based_on, vacias.state_id());

    // Solo una con imagen: sale esa con su opacidad horneada.
    let una = doc_rgba(
        2,
        1,
        &[
            Capa::rgba(&[A_OPACO, c(10, 20, 30, 100)])
                .modo(BlendMode::Multiply)
                .opacidad(128),
            Capa::vacia().opacidad(77),
        ],
    );
    let [l, u] = <[LayerId; 2]>::try_from(ids(&una)).expect("dos capas");
    let f = fusion(&merge_down_command(&una, u).expect("combinar con una vacía"));
    let imagen = f.image.expect("hay imagen");
    assert_eq!(rgba_de(&imagen), [c(200, 100, 50, 128), c(10, 20, 30, 50)]);
    assert_eq!(imagen, capas_de(&una, &[l, u]));
}

// ---------------------------------------------------------------------------------------
// Fila 7: merge_visible_command y flatten_command.
// ---------------------------------------------------------------------------------------

/// Comprueba el caso `[A, B oculta, C, D con celda vacía]` de un documento de cualquier modo.
fn comprueba_abcd(doc: &Document) {
    let [a, b, c_, d] = ids4(doc);
    assert!(!doc.layer(b).expect("B").visible());
    assert!(doc.image(doc.layer(d).expect("D").cel()).is_none());
    let antes = componer(doc);

    // Combinar visibles: A se queda, C y D se quitan, la oculta B no entra en `remove`.
    let cmd = merge_visible_command(doc).expect("combinar visibles");
    let f = fusion(&cmd);
    assert_eq!(f.target, a);
    assert_eq!(f.remove, [c_, d]);
    assert_eq!(f.image.as_ref(), Some(&antes));
    assert_eq!(f.based_on, doc.state_id());
    let despues = aplicado(doc, cmd);
    assert_eq!(ids(&despues), [a, b], "quedan A y la oculta B");
    assert_eq!(despues.layer(b), doc.layer(b), "la oculta no cambia");
    assert_eq!(despues.layers().len(), 2);
    assert_eq!(despues.image(despues.layers()[0].cel()), Some(&antes));
    assert_eq!(componer(&despues), antes);

    // Aplanar: además se quita la oculta.
    let cmd = flatten_command(doc).expect("aplanar");
    let f = fusion(&cmd);
    assert_eq!(f.target, a);
    assert_eq!(f.remove, [b, c_, d]);
    assert_eq!(f.image.as_ref(), Some(&antes));
    assert_eq!(f.based_on, doc.state_id());
    let despues = aplicado(doc, cmd);
    assert_eq!(ids(&despues), [a]);
    assert_eq!(despues.layers().len(), 1);
    assert_eq!(despues.image(despues.layers()[0].cel()), Some(&antes));
    assert_eq!(componer(&despues), antes);
}

#[test]
fn fila_07_a_combinar_visibles_y_aplanar_con_abcd_rgba() {
    let doc = doc_rgba(
        3,
        1,
        &[
            Capa::rgba(&[A_OPACO, A_OPACO, c(20, 40, 60, 90)]),
            Capa::rgba(&[c(1, 2, 3, 255); 3]).oculta(),
            Capa::rgba(&[c(10, 20, 30, 255), c(255, 255, 255, 128), T])
                .modo(BlendMode::Multiply)
                .opacidad(200),
            Capa::vacia().modo(BlendMode::Screen).opacidad(100),
        ],
    );
    comprueba_abcd(&doc);
}

#[test]
fn fila_07_b_variantes_visibilidad_y_vacias() {
    // La visible más baja no es la capa 0: [A oculta, B, C, D oculta].
    let doc = doc_rgba(
        2,
        1,
        &[
            Capa::rgba(&[A_OPACO, A_OPACO]).oculta(),
            Capa::rgba(&[c(10, 20, 30, 255), T]),
            Capa::rgba(&[c(255, 0, 0, 128), c(0, 0, 255, 255)]).modo(BlendMode::Screen),
            Capa::rgba(&[BLANCO, BLANCO]).oculta(),
        ],
    );
    let [a, b, c_, d] = ids4(&doc);
    let antes = componer(&doc);
    let f = fusion(&merge_visible_command(&doc).expect("combinar visibles"));
    assert_eq!((f.target, f.remove.as_slice()), (b, [c_].as_slice()));
    assert_eq!(f.image.as_ref(), Some(&antes));
    let f = fusion(&flatten_command(&doc).expect("aplanar"));
    assert_eq!((f.target, f.remove.as_slice()), (b, [a, c_, d].as_slice()));
    assert_eq!(f.image.as_ref(), Some(&antes));
    assert_eq!(f.based_on, doc.state_id());

    // Una sola visible entre varias: combinar visibles no tiene nada que combinar; aplanar deja
    // la visible con la opacidad horneada y quita las ocultas.
    let una = doc_rgba(
        2,
        1,
        &[
            Capa::rgba(&[BLANCO, BLANCO]).oculta(),
            Capa::rgba(&[A_OPACO, c(10, 20, 30, 100)]).opacidad(128),
            Capa::rgba(&[BLANCO, BLANCO]).oculta(),
        ],
    );
    let [a, b, c_] = ids3(&una);
    assert_eq!(merge_visible_command(&una), Err(MergeError::NothingToMerge));
    let cmd = flatten_command(&una).expect("aplanar con una visible");
    let f = fusion(&cmd);
    assert_eq!((f.target, f.remove.as_slice()), (b, [a, c_].as_slice()));
    let imagen = f.image.clone().expect("hay imagen");
    assert_eq!(rgba_de(&imagen), [c(200, 100, 50, 128), c(10, 20, 30, 50)]);
    let despues = aplicado(&una, cmd);
    assert_eq!(ids(&despues), [b]);
    assert_eq!(despues.image(despues.layers()[0].cel()), Some(&imagen));

    // Ninguna visible, o una sola capa: `NothingToMerge` en las dos.
    let ocultas = doc_rgba(
        2,
        1,
        &[
            Capa::rgba(&[A_OPACO, A_OPACO]).oculta(),
            Capa::rgba(&[BLANCO, BLANCO]).oculta(),
        ],
    );
    assert_eq!(
        merge_visible_command(&ocultas),
        Err(MergeError::NothingToMerge)
    );
    assert_eq!(flatten_command(&ocultas), Err(MergeError::NothingToMerge));
    let sola = doc_rgba(2, 1, &[Capa::rgba(&[A_OPACO, A_OPACO])]);
    assert_eq!(
        merge_visible_command(&sola),
        Err(MergeError::NothingToMerge)
    );
    assert_eq!(flatten_command(&sola), Err(MergeError::NothingToMerge));

    // Todas las visibles vacías (la capa 0, oculta, es la única con imagen): resultado vacío.
    let vacias = doc_rgba(
        2,
        1,
        &[
            Capa::rgba(&[A_OPACO, A_OPACO]).oculta(),
            Capa::vacia(),
            Capa::vacia().opacidad(100),
        ],
    );
    let [a, b, c_] = ids3(&vacias);
    let antes = componer(&vacias);
    let f = fusion(&merge_visible_command(&vacias).expect("combinar vacías"));
    assert_eq!((f.target, f.remove.as_slice()), (b, [c_].as_slice()));
    assert_eq!(f.image, None);
    let cmd = flatten_command(&vacias).expect("aplanar vacías");
    let f = fusion(&cmd);
    assert_eq!((f.target, f.remove.as_slice()), (b, [a, c_].as_slice()));
    assert_eq!(f.image, None);
    assert_eq!(f.based_on, vacias.state_id());
    let despues = aplicado(&vacias, cmd);
    assert_eq!(despues.image(despues.layers()[0].cel()), None);
    assert_eq!(componer(&despues), antes);
}

#[test]
fn fila_07_c_indexado_y_gris_con_alfa() {
    // Indexado con transparente 0.
    let indexado = doc_indexado(
        4,
        1,
        Some(0),
        &[
            Capa::idx(&[1, 2, 3, 0]),
            Capa::idx(&[0, 4, 0, 0]).oculta(),
            Capa::idx(&[5, 0, 0, 0]),
            Capa::vacia(),
        ],
    );
    assert_eq!(indices_de(&componer(&indexado)), [5, 2, 3, 0]);
    comprueba_abcd(&indexado);

    // Gris con alfa, con opacidad y mezcla en la tercera capa.
    let gris = construye(
        imagen(2, 1, Mode::GrayAlpha),
        &[
            Capa::gris(&[(200, 255), (100, 128)]),
            Capa::gris(&[(1, 255), (1, 255)]).oculta(),
            Capa::gris(&[(128, 255), (255, 100)])
                .modo(BlendMode::Multiply)
                .opacidad(128),
            Capa::vacia(),
        ],
    );
    comprueba_abcd(&gris);

    // Indexado sin transparente con todo oculto: ninguna visible, `NothingToMerge`.
    let mut sin_t = doc_indexado(
        4,
        1,
        Some(0),
        &[Capa::idx(&[1, 2, 3, 0]), Capa::idx(&[1, 1, 1, 1])],
    );
    let quita = Command::SetTransparentIndex {
        image: sin_t.layers()[0].cel(),
        index: None,
    };
    assert_eq!(sin_t.apply(quita), Ok(true));
    for id in ids(&sin_t) {
        let oculta = Command::SetLayerVisible {
            layer: id,
            visible: false,
        };
        assert_eq!(sin_t.apply(oculta), Ok(true));
    }
    assert_eq!(
        merge_visible_command(&sin_t),
        Err(MergeError::NothingToMerge)
    );
    assert_eq!(flatten_command(&sin_t), Err(MergeError::NothingToMerge));
}

// ---------------------------------------------------------------------------------------
// Generadores de documentos para las propiedades.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Tipo {
    Rgba,
    GrisAlfa,
    /// Indexado con ese índice transparente (paleta de 4 colores).
    Indexado(u8),
}

#[derive(Clone, Debug)]
struct DocGen {
    tipo: Tipo,
    w: u32,
    h: u32,
    capas: Vec<Capa>,
}

/// Alfa o opacidad con los extremos más frecuentes que en el uniforme.
fn alfa() -> impl Strategy<Value = u8> {
    prop_oneof![1 => Just(0u8), 1 => Just(255u8), 2 => any::<u8>()]
}

fn modo_cualquiera() -> impl Strategy<Value = BlendMode> {
    proptest::sample::select(BlendMode::ALL.to_vec())
}

fn tipo_strategy() -> impl Strategy<Value = Tipo> {
    prop_oneof![
        Just(Tipo::Rgba),
        Just(Tipo::GrisAlfa),
        (0u8..4).prop_map(Tipo::Indexado),
    ]
}

fn pixel_strategy(tipo: Tipo) -> BoxedStrategy<Pixel> {
    match tipo {
        Tipo::Rgba => (any::<u8>(), any::<u8>(), any::<u8>(), alfa())
            .prop_map(|(r, g, b, a)| Pixel::Rgba(Rgba::new(r, g, b, a)))
            .boxed(),
        Tipo::GrisAlfa => (any::<u8>(), alfa())
            .prop_map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
            .boxed(),
        Tipo::Indexado(_) => (0u8..4).prop_map(Pixel::Indexed).boxed(),
    }
}

fn capa_strategy(tipo: Tipo, pixeles: usize) -> impl Strategy<Value = Capa> {
    let indexado = matches!(tipo, Tipo::Indexado(_));
    (
        proptest::bool::weighted(0.8),
        proptest::bool::weighted(0.3),
        alfa(),
        modo_cualquiera(),
        proptest::option::weighted(
            0.8,
            proptest::collection::vec(pixel_strategy(tipo), pixeles),
        ),
    )
        .prop_map(move |(visible, locked, opacidad, modo, celda)| {
            // En indexado el núcleo solo admite opacidad 255 y modo Normal.
            let (opacidad, modo) = if indexado {
                (255, BlendMode::Normal)
            } else {
                (opacidad, modo)
            };
            Capa {
                visible,
                locked,
                opacidad,
                modo,
                celda,
            }
        })
}

/// Documentos de hasta 6 × 5 con `min` a `max` capas.
fn doc_strategy(min: usize, max: usize) -> impl Strategy<Value = DocGen> {
    (tipo_strategy(), 1u32..=6, 1u32..=5).prop_flat_map(move |(tipo, w, h)| {
        proptest::collection::vec(capa_strategy(tipo, (w * h) as usize), min..=max)
            .prop_map(move |capas| DocGen { tipo, w, h, capas })
    })
}

fn documento(g: &DocGen) -> Document {
    let plantilla = match g.tipo {
        Tipo::Rgba => imagen(g.w, g.h, Mode::Rgba),
        Tipo::GrisAlfa => imagen(g.w, g.h, Mode::GrayAlpha),
        Tipo::Indexado(t) => plantilla_indexada(g.w, g.h, 4, Some(t)),
    };
    construye(plantilla, &g.capas)
}

/// Fuerza que las capas `k` y `k + 1` sean visibles.
fn con_visibles(g: &DocGen, k: usize) -> DocGen {
    let mut g = g.clone();
    g.capas[k].visible = true;
    g.capas[k + 1].visible = true;
    g
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: aplanar es la composición.
// ---------------------------------------------------------------------------------------

fn comprueba_composicion(g: &DocGen, aplanar: bool) -> Result<(), TestCaseError> {
    let antes_doc = documento(g);
    let antes = compose(&antes_doc).expect("hay índice transparente: compose no falla");
    let visibles: Vec<LayerId> = antes_doc
        .layers()
        .iter()
        .filter(|l| l.visible())
        .map(Layer::id)
        .collect();
    let nada_que_hacer = if aplanar {
        antes_doc.layers().len() == 1 || visibles.is_empty()
    } else {
        visibles.len() < 2
    };
    let cmd = if aplanar {
        flatten_command(&antes_doc)
    } else {
        merge_visible_command(&antes_doc)
    };
    let cmd = match cmd {
        Err(e) => {
            prop_assert_eq!(e, MergeError::NothingToMerge);
            prop_assert!(nada_que_hacer, "`NothingToMerge` sin motivo");
            return Ok(());
        }
        Ok(cmd) => cmd,
    };
    prop_assert!(!nada_que_hacer, "había que dar `NothingToMerge`");

    let f = fusion(&cmd);
    prop_assert_eq!(f.target, visibles[0]);
    let esperado: Vec<LayerId> = if aplanar {
        ids(&antes_doc)
            .into_iter()
            .filter(|&id| id != visibles[0])
            .collect()
    } else {
        visibles[1..].to_vec()
    };
    prop_assert_eq!(&f.remove, &esperado);
    prop_assert_eq!(f.based_on, antes_doc.state_id());
    let ninguna_con_imagen = visibles.iter().all(|&id| {
        antes_doc
            .image(antes_doc.layer(id).unwrap().cel())
            .is_none()
    });
    prop_assert_eq!(f.image.is_none(), ninguna_con_imagen);
    if let Some(imagen) = &f.image {
        prop_assert_eq!(imagen, &antes);
    }

    let mut doc = antes_doc.clone();
    prop_assert_eq!(doc.apply(cmd), Ok(true));
    // La vista no cambia: ni un byte.
    prop_assert_eq!(compose(&doc), Ok(antes.clone()));
    // La celda resultante es `compose(antes)` (o vacía).
    let cel = doc.layer(f.target).unwrap().cel();
    prop_assert_eq!(doc.image(cel), f.image.as_ref());
    let capa = doc.layer(f.target).unwrap();
    prop_assert_eq!(
        (capa.opacity(), capa.blend(), capa.locked()),
        (255, BlendMode::Normal, false)
    );
    if aplanar {
        prop_assert_eq!(doc.layers().len(), 1);
    } else {
        prop_assert_eq!(
            doc.layers().len(),
            antes_doc.layers().len() - (visibles.len() - 1)
        );
        // Las ocultas quedan tal cual.
        for l in antes_doc.layers().iter().filter(|l| !l.visible()) {
            prop_assert_eq!(doc.layer(l.id()), Some(l));
        }
    }
    Ok(())
}

fn comprueba_combinar_abajo_en_el_fondo(g: &DocGen) -> Result<(), TestCaseError> {
    let g = con_visibles(g, 0);
    let antes_doc = documento(&g);
    let antes = compose(&antes_doc).expect("compose no falla");
    let upper = antes_doc.layers()[1].id();
    let cmd = merge_down_command(&antes_doc, upper).expect("las dos son visibles");
    let mut doc = antes_doc.clone();
    prop_assert_eq!(doc.apply(cmd), Ok(true));
    prop_assert_eq!(doc.layers().len(), antes_doc.layers().len() - 1);
    prop_assert_eq!(compose(&doc), Ok(antes));
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // 1. Aplanar es la composición.
    #[test]
    fn propiedad_1_aplanar_es_la_composicion(g in doc_strategy(1, 5)) {
        comprueba_composicion(&g, true)?;
        comprueba_composicion(&g, false)?;
        if g.capas.len() >= 2 {
            comprueba_combinar_abajo_en_el_fondo(&g)?;
        }
    }

    // 4. Combinar abajo contra un modelo independiente.
    #[test]
    fn propiedad_4_combinar_abajo_contra_un_modelo(
        g in doc_strategy(2, 5),
        sel in any::<proptest::sample::Index>(),
    ) {
        let k = sel.index(g.capas.len() - 1);
        comprueba_modelo(&con_visibles(&g, k), k)?;
    }

    // 5. Deshacer una combinación devuelve las capas tal cual.
    #[test]
    fn propiedad_5_deshacer_devuelve_las_capas_tal_cual(
        g in doc_strategy(2, 5),
        operacion in 0u8..3,
        sel in any::<proptest::sample::Index>(),
    ) {
        comprueba_deshacer(&g, operacion, sel.index(g.capas.len() - 1))?;
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: el modelo independiente.
// ---------------------------------------------------------------------------------------

fn a_rgba(p: Pixel) -> Rgba {
    match p {
        Pixel::Rgba(color) => color,
        Pixel::GrayAlpha { gray, alpha } => Rgba::new(gray, gray, gray, alpha),
        otro => panic!("el modelo no conoce {otro:?}"),
    }
}

fn indice(p: Pixel) -> u8 {
    match p {
        Pixel::Indexed(i) => i,
        otro => panic!("se esperaba indexado, hay {otro:?}"),
    }
}

/// El píxel que debe tener la celda combinada de las capas `k` (abajo) y `k + 1` en `(x, y)`:
/// plegado de `composite_pixel` sobre vacío (primero la de abajo con su opacidad y modo, luego la
/// de arriba con los suyos; las celdas vacías no aportan) y, en indexado, el índice de la de
/// arriba si no es el transparente y si no el de la de abajo (celda vacía = transparente).
fn modelo(doc: &Document, k: usize, x: u32, y: u32) -> Pixel {
    let par = [&doc.layers()[k], &doc.layers()[k + 1]];
    let pixel_de = |capa: &Layer| {
        doc.image(capa.cel())
            .map(|img| img.get_pixel(x, y).expect("píxel dentro"))
    };
    match doc.mode() {
        Mode::Indexed => {
            let t = doc
                .palette()
                .and_then(Palette::transparent_index)
                .expect("el generador siempre da índice transparente");
            let abajo = pixel_de(par[0]).map_or(t, indice);
            let arriba = pixel_de(par[1]).map_or(t, indice);
            Pixel::Indexed(if arriba != t { arriba } else { abajo })
        }
        modo => {
            let mut acumulado = T;
            for capa in par {
                if let Some(p) = pixel_de(capa) {
                    acumulado = composite_pixel(acumulado, a_rgba(p), capa.opacity(), capa.blend());
                }
            }
            if modo == Mode::GrayAlpha {
                Pixel::GrayAlpha {
                    gray: acumulado.r,
                    alpha: acumulado.a,
                }
            } else {
                Pixel::Rgba(acumulado)
            }
        }
    }
}

/// El píxel de una celda vacía (nada que dar), en el modo del documento.
fn pixel_vacio(doc: &Document) -> Pixel {
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

fn comprueba_modelo(g: &DocGen, k: usize) -> Result<(), TestCaseError> {
    let antes_doc = documento(g);
    let antes = compose(&antes_doc).expect("compose no falla");
    let (l, u) = (antes_doc.layers()[k].id(), antes_doc.layers()[k + 1].id());
    let cmd = merge_down_command(&antes_doc, u).expect("las dos son visibles");
    let f = fusion(&cmd);
    prop_assert_eq!(f.target, l);
    prop_assert_eq!(&f.remove, &vec![u]);

    let mut doc = antes_doc.clone();
    prop_assert_eq!(doc.apply(cmd), Ok(true));
    prop_assert!(doc.materialized_bytes() <= antes_doc.materialized_bytes());
    prop_assert_eq!(doc.layers().len(), antes_doc.layers().len() - 1);

    let resultado = doc.image(doc.layer(l).unwrap().cel());
    for y in 0..g.h {
        for x in 0..g.w {
            let obtenido = resultado.map_or_else(
                || pixel_vacio(&antes_doc),
                |img| img.get_pixel(x, y).expect("píxel dentro"),
            );
            prop_assert_eq!(
                obtenido,
                modelo(&antes_doc, k, x, y),
                "píxel ({}, {}) de la combinación de las capas {} y {}",
                x,
                y,
                k,
                k + 1
            );
        }
    }
    if k == 0 {
        prop_assert_eq!(compose(&doc), Ok(antes));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Propiedad 5: deshacer y rehacer una combinación.
// ---------------------------------------------------------------------------------------

/// `operacion`: 0 = combinar abajo (la capa `k + 1` con la `k`), 1 = combinar visibles,
/// 2 = aplanar.
fn comprueba_deshacer(g: &DocGen, operacion: u8, k: usize) -> Result<(), TestCaseError> {
    let g = if operacion == 0 {
        con_visibles(g, k)
    } else {
        g.clone()
    };
    let antes = documento(&g);
    let cmd = match operacion {
        0 => merge_down_command(&antes, antes.layers()[k + 1].id()),
        1 => merge_visible_command(&antes),
        _ => flatten_command(&antes),
    };
    let cmd = match cmd {
        Err(e) => {
            prop_assert_eq!(e, MergeError::NothingToMerge);
            return Ok(());
        }
        Ok(cmd) => cmd,
    };
    let f = fusion(&cmd);
    let quitadas: Vec<(usize, Layer)> = f
        .remove
        .iter()
        .map(|id| {
            let pos = antes.layers().iter().position(|l| l.id() == *id).unwrap();
            (pos, antes.layers()[pos].clone())
        })
        .collect();
    let objetivo_antes = antes.layer(f.target).unwrap().clone();

    let mut doc = antes.clone();
    prop_assert_eq!(doc.apply(cmd), Ok(true));
    prop_assert_ne!(doc.state_id(), antes.state_id());
    let despues = doc.clone();

    prop_assert_eq!(doc.undo(), Ok(()));
    prop_assert_eq!(doc.state_id(), antes.state_id());
    prop_assert_eq!(ids(&doc), ids(&antes));
    for (pos, original) in &quitadas {
        let vuelta = &doc.layers()[*pos];
        prop_assert_eq!(firma(vuelta), firma(original));
        prop_assert_eq!(doc.image(original.cel()), antes.image(original.cel()));
        prop_assert!(
            vuelta == original,
            "la capa quitada {:?} no vuelve igual",
            original.id()
        );
    }
    prop_assert_eq!(doc.layer(f.target), Some(&objetivo_antes));
    prop_assert!(doc == antes, "tras deshacer el documento no es el de antes");

    prop_assert_eq!(doc.redo(), Ok(()));
    prop_assert_eq!(doc.state_id(), despues.state_id());
    prop_assert!(
        doc == despues,
        "tras rehacer el documento no es el de después"
    );
    Ok(())
}
