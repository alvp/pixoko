//! Propiedades 1 a 4 de la puerta del hito 2 (specs/hito-2-puerta.md), con `proptest`.
//!
//! Casos fijados por la spec (D6): 64 en las propiedades 1 y 2 y 32 en la 3 y la 4 (tocan el
//! disco). `PROPTEST_CASES` solo puede **bajarlos** al desarrollar (`casos`): sin él, valen los de la
//! spec.
//!
//! Los gestos se generan como números crudos que se interpretan contra el modelo actual (`accion`):
//! así toda secuencia es válida para el modo y la paleta del momento y se puede cambiar de modo a
//! mitad de secuencia con las conversiones exactas. El modelo restringe la línea a H, V y 45°.
//!
//! **Suposiciones y ambigüedades:**
//! - Propiedad 2: `modified` se comprueba contra el estado de apertura (sin guardar, para no tocar el
//!   disco; los guardados están en las propiedades 1 y 3 y en las filas 5 a 8). Además de recorrer la
//!   historia entera, se prueba una rama nueva tras deshacer unos pasos: el estado no se reutiliza y
//!   `redo` falla.
//! - Propiedad 4: con `Ok`, el modelo se siembra con lo que abrió la sesión (el fichero mutado no tiene
//!   lectura independiente) y se comprueba que editar, guardar y reabrir lo conserva.

use px_core::ImageId;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use proptest::prelude::*;
use px_core::{Document, Mode};
use px_ui::canvas::Canvas;

use crate::files::FileError;
use crate::session::{Saved, Session};

use super::comun::*;

/// Casos de una propiedad: los de la spec, o menos si `PROPTEST_CASES` los baja.
fn casos(de_la_spec: u32) -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|valor| valor.parse::<u32>().ok())
        .map_or(de_la_spec, |pedidos| pedidos.min(de_la_spec))
}

/// Un gesto o una operación en bruto: un tipo y seis números.
type Crudo = (u8, [u16; 6]);

fn estrategia_crudos(max: usize) -> impl Strategy<Value = Vec<Crudo>> {
    proptest::collection::vec(
        (any::<u8>(), proptest::array::uniform6(any::<u16>())),
        1..=max,
    )
}

fn modo_de(seleccion: u8) -> Mode {
    match seleccion % 4 {
        0 => Mode::Rgba,
        1 => Mode::Indexed,
        2 => Mode::Gray,
        _ => Mode::GrayAlpha,
    }
}

/// Imagen aleatoria repetible: RGBA con alfa 0, 128 y 255 y 3 a 300 colores; indexada con 1 a 40
/// entradas con alfa propio y, a veces, índice transparente; gris; gris con alfa.
fn modelo_aleatorio(modo: Mode, ancho: usize, alto: usize, semilla: u64) -> Modelo {
    let mut r = Lcg::new(semilla);
    let mut m = modelo_de(modo, ancho, alto, vec![[0; 4]; ancho * alto]);
    const ALFAS: [u8; 4] = [0, 128, 255, 255];
    match modo {
        Mode::Rgba => {
            let pool = [3u32, 8, 40, 300][r.hasta(4) as usize];
            let colores: Vec<[u8; 4]> = (0..pool)
                .map(|_| {
                    [
                        r.hasta(256) as u8,
                        r.hasta(256) as u8,
                        r.hasta(256) as u8,
                        ALFAS[r.hasta(4) as usize],
                    ]
                })
                .collect();
            for celda in &mut m.celdas {
                *celda = colores[r.hasta(pool) as usize];
            }
        }
        Mode::Indexed => {
            let tope = if r.hasta(4) == 0 { 40 } else { 8 };
            let largo = 1 + r.hasta(tope);
            m.paleta = (0..largo)
                .map(|_| {
                    [
                        r.hasta(256) as u8,
                        r.hasta(256) as u8,
                        r.hasta(256) as u8,
                        ALFAS[r.hasta(4) as usize],
                    ]
                })
                .collect();
            m.transparente = (r.hasta(2) == 0).then(|| r.hasta(largo) as u8);
            for celda in &mut m.celdas {
                *celda = [r.hasta(largo) as u8, 0, 0, 0];
            }
        }
        Mode::Gray => {
            for celda in &mut m.celdas {
                *celda = [r.hasta(256) as u8, 0, 0, 0];
            }
        }
        Mode::GrayAlpha => {
            for celda in &mut m.celdas {
                *celda = [r.hasta(256) as u8, ALFAS[r.hasta(4) as usize], 0, 0];
            }
        }
    }
    m
}

enum Accion {
    Paso(Paso),
    Guardar,
}

const DIRECCIONES: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// Un punto en el lienzo o a un par de píxeles de él (para ejercitar el recorte).
fn coord(m: &Modelo, v: &[u16; 6]) -> (i32, i32) {
    (
        i32::from(v[0]) % (m.ancho as i32 + 4) - 2,
        i32::from(v[1]) % (m.alto as i32 + 4) - 2,
    )
}

fn largo(m: &Modelo, bits: u16) -> i32 {
    1 + i32::from(bits) % (m.ancho.max(m.alto) as i32 + 3)
}

fn eleccion_de(m: &Modelo, v: u16) -> Eleccion {
    if m.modo == Mode::Indexed {
        Eleccion::Entrada((usize::from(v) % m.paleta.len()) as u8)
    } else {
        Eleccion::Ranura(1 + (v % 9) as u8)
    }
}

/// Un trayecto de 1 a 3 tramos, cada uno horizontal, vertical o a 45°.
fn ruta_de(m: &Modelo, v: &[u16; 6]) -> Vec<(i32, i32)> {
    let mut punto = coord(m, v);
    let mut ruta = vec![punto];
    for i in 0..=usize::from(v[5] % 3) {
        let (dx, dy) = DIRECCIONES[usize::from((v[2] >> (3 * i)) % 8)];
        let n = largo(m, v[3] >> (4 * i));
        punto = (punto.0 + dx * n, punto.1 + dy * n);
        ruta.push(punto);
    }
    ruta
}

/// Interpreta un gesto crudo contra el modelo actual. Las operaciones de paleta solo valen en
/// indexadas y, en las demás, se cambian por un gesto.
fn accion(m: &Modelo, tipo: u8, v: &[u16; 6], con_guardar: bool) -> Accion {
    let indexada = m.modo == Mode::Indexed;
    let largo_paleta = m.paleta.len().max(1);
    let lapiz = || Paso::Lapiz(ruta_de(m, v), eleccion_de(m, v[4]));
    let linea = || {
        let a = coord(m, v);
        let (dx, dy) = DIRECCIONES[usize::from(v[2] % 8)];
        let n = largo(m, v[3]);
        Paso::Linea(a, (a.0 + dx * n, a.1 + dy * n), eleccion_de(m, v[4]))
    };
    let relleno = || Paso::Relleno(coord(m, v), eleccion_de(m, v[4]));
    let borrador = || Paso::Borrador(ruta_de(m, v));
    Accion::Paso(match tipo % 12 {
        0 | 1 => lapiz(),
        2 => linea(),
        3 => relleno(),
        4 => borrador(),
        5 if indexada => Paso::ColorEntrada(
            (usize::from(v[0]) % largo_paleta) as u8,
            [
                v[1] as u8,
                (v[1] >> 8) as u8,
                v[2] as u8,
                [0, 128, 255][usize::from(v[3] % 3)],
            ],
        ),
        5 => lapiz(),
        6 if indexada => {
            let inicio = usize::from(v[0]) % largo_paleta;
            Paso::Rotar {
                inicio: inicio as u8,
                largo: (1 + usize::from(v[1]) % (largo_paleta - inicio)) as u16,
                pasos: i32::from(v[2] % 7) - 3,
            }
        }
        6 => linea(),
        7 if indexada => Paso::Mover {
            de: (usize::from(v[0]) % largo_paleta) as u8,
            a: (usize::from(v[1]) % largo_paleta) as u8,
        },
        7 => relleno(),
        8 if indexada && largo_paleta < 256 => Paso::Anadir {
            activa: (usize::from(v[0]) % largo_paleta) as u8,
        },
        8 => borrador(),
        9 if indexada => Paso::Transparente(if v[0].is_multiple_of(4) {
            None
        } else {
            Some((usize::from(v[1]) % largo_paleta) as u8)
        }),
        9 => lapiz(),
        10 => Paso::Convertir,
        _ if con_guardar => return Accion::Guardar,
        _ => lapiz(),
    })
}

/// `save` y relectura: lo escrito es el modelo normalizado, sin conversiones.
fn guardar_y_releer(w: &mut Mundo, t: &Trabajo) {
    let sin_ruta = w.s.path().is_none();
    let guardado = guardar(&mut w.s, t.ruta()).unwrap_or_else(|e| panic!("save dio {e}"));
    if sin_ruta {
        assert_eq!(guardado.path, t.ruta().join("sin-titulo.png"));
    }
    w.marcar_guardado();
    let leido = leer(&guardado.path);
    assert!(leido.report.is_empty(), "relectura con conversiones");
    assert_igual(&leido.image, &w.m.normalizada(), "relectura tras save");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(casos(64)))]

    /// Propiedad 1: tras cada paso de un guion aleatorio, `Session` es el modelo; al final, la
    /// relectura es el modelo normalizado.
    #[test]
    fn propiedad_1_guion_aleatorio_contra_el_modelo(
        seleccion in any::<u8>(),
        ancho in 1usize..=24,
        alto in 1usize..=24,
        semilla in any::<u64>(),
        crudos in estrategia_crudos(30),
    ) {
        let t = Trabajo::nuevo();
        let modelo = modelo_aleatorio(modo_de(seleccion), ancho, alto, semilla);
        let sesion = Session::sample(Document::new(modelo.a_imagen()));
        let mut w = Mundo::de_sesion(sesion, "propiedad 1");
        w.comprobar();
        for (tipo, v) in &crudos {
            match accion(&w.m, *tipo, v, true) {
                Accion::Paso(paso) => {
                    w.aplicar(&paso);
                }
                Accion::Guardar => guardar_y_releer(&mut w, &t),
            }
        }
        guardar_y_releer(&mut w, &t);
        prop_assert_eq!(t.nombres(), vec!["sin-titulo.png".to_string()]);
    }

    /// Propiedad 2: `undo` k veces da la instantánea del paso `n − k` y `redo` la vuelve a dar;
    /// `modified` es falso si y solo si el `state_id` es el guardado, y nunca con una imagen distinta.
    #[test]
    fn propiedad_2_historia(
        seleccion in any::<u8>(),
        ancho in 1usize..=24,
        alto in 1usize..=24,
        semilla in any::<u64>(),
        crudos in estrategia_crudos(30),
        rama in (any::<u8>(), proptest::array::uniform6(any::<u16>())),
        corte in any::<u8>(),
    ) {
        let modelo = modelo_aleatorio(modo_de(seleccion), ancho, alto, semilla);
        let sesion = Session::sample(Document::new(modelo.a_imagen()));
        let mut w = Mundo::de_sesion(sesion, "propiedad 2");
        for (tipo, v) in &crudos {
            if let Accion::Paso(paso) = accion(&w.m, *tipo, v, false) {
                w.aplicar(&paso);
            }
        }
        let n = w.pasos();
        prop_assert_eq!(w.pos, n);

        // `undo` k veces: la instantánea anotada tras el paso n − k (lo comprueba el mundo).
        for k in 1..=n {
            w.deshacer();
            prop_assert_eq!(w.pos, n - k);
        }
        prop_assert_eq!(w.s.document.state_id(), 0);
        prop_assert!(!w.s.modified());
        w.deshacer();
        // `redo` las vuelve a dar.
        for k in 1..=n {
            w.rehacer();
            prop_assert_eq!(w.pos, k);
        }
        w.rehacer();
        // Para todo k: k pasos atrás y k adelante.
        for k in 1..=n {
            for _ in 0..k {
                w.deshacer();
            }
            for _ in 0..k {
                w.rehacer();
            }
            prop_assert_eq!(w.pos, n);
        }

        // Una rama nueva tras deshacer unos pasos: `redo` falla y no se reutiliza ningún estado.
        let atras = usize::from(corte) % (n + 1);
        for _ in 0..atras {
            w.deshacer();
        }
        let (tipo, v) = rama;
        let cambio = match accion(&w.m, tipo, &v, false) {
            Accion::Paso(paso) => w.aplicar(&paso),
            Accion::Guardar => false,
        };
        if cambio {
            prop_assert_eq!(w.pasos(), n - atras + 1);
        }
        w.rehacer();
        w.deshacer_todo();
        w.rehacer_todo();
    }

    /// Propiedad 3: con secuencias aleatorias de `save`, `save_as` y `save_to` desde una copia, el
    /// directorio de trabajo contiene exactamente los ficheros de los `Saved` devueltos (y el de
    /// origen), sin temporales; cada uno es la relectura esperada y `golden/png/` queda igual.
    #[test]
    fn propiedad_3_solo_se_escribe_lo_que_se_pide(
        fuente in 0usize..6,
        operaciones in proptest::collection::vec(
            (0u8..4, proptest::array::uniform6(any::<u16>()), 0usize..NOMBRES.len()),
            1..=8,
        ),
    ) {
        let t = Trabajo::nuevo();
        let mut fuentes = reales();
        fuentes.push(fuente_rgb8());
        fuentes.push(fuente_con_iccp());
        let mut w = abrir(&t, &fuentes[fuente]);
        let origen = w.s.path().expect("ruta").to_path_buf();
        let bytes_origen = fs::read(&origen).expect("leer el origen");
        let mut escritos: BTreeMap<PathBuf, Modelo> = BTreeMap::new();

        for (tipo, v, nombre) in &operaciones {
            let actual = w.s.path().expect("ruta").to_path_buf();
            let convertida = w.s.converted();
            match tipo {
                0 => {
                    let paso = Paso::Lapiz(ruta_de(&w.m, v), eleccion_de(&w.m, v[4]));
                    w.aplicar(&paso);
                }
                1 => {
                    let destino = if convertida { libre_copia(&actual) } else { actual.clone() };
                    let guardado = guardar(&mut w.s, t.ruta()).expect("save");
                    registrar(&mut w, &mut escritos, &guardado, &destino, convertida);
                }
                2 => {
                    let destino = libre_copia(&actual);
                    let guardado = guardar_como(&mut w.s, t.ruta()).expect("save_as");
                    registrar(&mut w, &mut escritos, &guardado, &destino, false);
                }
                _ => {
                    let elegido = t.ruta().join(NOMBRES[*nombre]);
                    let destino = con_png(&elegido);
                    let existe = destino != elegido && fs::symlink_metadata(&destino).is_ok();
                    let modificado = w.s.modified();
                    let resultado = guardar_en(&mut w.s, &elegido);
                    if existe {
                        prop_assert_eq!(resultado, Err(FileError::Exists(destino)));
                        prop_assert_eq!(w.s.path(), Some(actual.as_path()));
                        prop_assert_eq!(w.s.modified(), modificado);
                    } else {
                        let copia = convertida && destino == actual;
                        let esperado = if copia { libre_copia(&actual) } else { destino };
                        let guardado = resultado.expect("save_to");
                        registrar(&mut w, &mut escritos, &guardado, &esperado, copia);
                    }
                }
            }
        }

        // Exactamente los ficheros de los `Saved` y el de origen, sin temporales.
        let mut esperados: BTreeSet<String> = escritos
            .keys()
            .map(|ruta| ruta.file_name().expect("nombre").to_string_lossy().into_owned())
            .collect();
        esperados.insert(origen.file_name().expect("nombre").to_string_lossy().into_owned());
        let hay = t.nombres();
        sin_temporales(&hay);
        prop_assert_eq!(hay, esperados.into_iter().collect::<Vec<_>>());
        for (ruta, modelo) in &escritos {
            assert_igual(&leer(ruta).image, modelo, &format!("{}", ruta.display()));
        }
        if !escritos.contains_key(&origen) {
            prop_assert_eq!(fs::read(&origen).expect("leer el origen"), bytes_origen);
        }
        // `golden/png/` queda igual: lo comprueba el guardián del trabajo al soltarse.
    }

    /// Propiedad 4: un real con 1 a 4 bytes cambiados da `Ok` o `Err` sin pánico; con `Err` la
    /// sesión queda igual; con `Ok`, editar, guardar y reabrir da el modelo.
    #[test]
    fn propiedad_4_reales_mutados(
        real in 0usize..4,
        cambios in proptest::collection::vec((any::<usize>(), 1u8..=255), 1..=4),
    ) {
        let t = Trabajo::nuevo();
        let fuente = reales().remove(real);
        let mut mutado = fuente.bytes.clone();
        for (posicion, xor) in &cambios {
            let en = posicion % mutado.len();
            mutado[en] ^= xor;
        }
        let ruta_mutada = t.escribir("mutado.png", &mutado);

        // Una sesión con cambios sin guardar, que abre el fichero mutado descartándolos.
        let mut w = abrir(&t, &fuente);
        w.aplicar(&edicion_que_cambia(&w.m));
        let documento = w.s.document.clone();
        let (id, ruta, modificado) = (
            w.s.document.state_id(),
            w.s.path().map(|p| p.to_path_buf()),
            w.s.modified(),
        );
        match w.s.open(&ruta_mutada, Canvas::MAX_SIDE, true) {
            Err(_) => {
                prop_assert!(w.s.document == documento, "un open fallido cambió el documento");
                prop_assert_eq!(w.s.document.state_id(), id);
                prop_assert_eq!(w.s.path().map(|p| p.to_path_buf()), ruta);
                prop_assert_eq!(w.s.modified(), modificado);
                w.comprobar();
            }
            Ok(_) => {
                let mut w = Mundo::de_sesion(w.s, "mutado");
                prop_assert_eq!(w.s.document.state_id(), 0);
                prop_assert!(!w.s.modified());
                if let Some(paso) = edicion_que_cambia_opt(&w.m) {
                    prop_assert!(w.aplicar(&paso));
                }
                let abierta = w.s.path().expect("ruta").to_path_buf();
                let destino = if w.s.converted() { libre_copia(&abierta) } else { abierta };
                let guardado = guardar(&mut w.s, t.ruta()).expect("save");
                prop_assert_eq!(&guardado.path, &destino);
                let mut otra = sesion_vacia();
                otra.open(&guardado.path, Canvas::MAX_SIDE, false).expect("reabrir");
                let imagen = otra.document.image(ImageId::from_raw(0)).expect("imagen");
                assert_igual(imagen, &w.m.normalizada(), "mutado reabierto");
            }
        }
    }
}

/// Nombres como los que da un diálogo: sin extensión, con ella en minúsculas o mayúsculas, con
/// espacios y varios puntos.
const NOMBRES: [&str; 6] = ["a", "b.png", "C.PNG", "con espacios", "d.e", "otra.png"];

/// Tras un guardado que salió bien: destino y motivo de copia esperados, `modified` falso y
/// relectura igual al modelo normalizado.
fn registrar(
    w: &mut Mundo,
    escritos: &mut BTreeMap<PathBuf, Modelo>,
    guardado: &Saved,
    esperado: &std::path::Path,
    con_razon: bool,
) {
    assert_eq!(guardado.path, esperado, "destino del guardado");
    assert_eq!(guardado.copy_reason.is_some(), con_razon, "copy_reason");
    w.marcar_guardado();
    assert_eq!(w.s.path(), Some(esperado), "ruta tras guardar");
    assert!(!w.s.converted(), "sigue convertida tras guardar");
    assert_igual(
        &leer(esperado).image,
        &w.m.normalizada(),
        "relectura tras guardar",
    );
    escritos.insert(esperado.to_path_buf(), w.m.normalizada());
}
