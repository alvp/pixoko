//! Tests de los iconos propios (specs/interfaz-iconos.md, filas 1 y 2): los 14 PNG de
//! `assets/icons/`, su carga a texturas y un icono retocado y guardado en modo indexado.
//!
//! Sin ventana: un `egui::Context` sin backend recoge las texturas como `TexturesDelta`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::*;
use eframe::egui::{self, Color32, RawInput, TextureOptions};
use px_core::{Image, Palette, Pixel, Rgba};
use px_formats::{read_png, write_png};
use px_ui::icons::Icon;
use px_ui::texture::to_rgba8;

fn carpeta() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons")
}

fn leer(icono: Icon) -> Vec<u8> {
    let ruta = carpeta().join(format!("{}.png", icono.file_stem()));
    std::fs::read(&ruta).unwrap_or_else(|e| panic!("{}: {e}", ruta.display()))
}

/// Los píxeles RGBA de un PNG, por filas, como lo hace la carga.
fn rgba_de(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let png = read_png(bytes).expect("PNG legible");
    let (w, h) = (png.image.width(), png.image.height());
    (w, h, to_rgba8(&png.image))
}

#[test]
fn fila_1_los_14_png_son_de_16x16_con_alfa_0_o_255_blancos_no_vacios_ni_llenos_y_distintos() {
    let mut vistos: HashSet<Vec<bool>> = HashSet::new();
    for icono in Icon::ALL {
        let bytes = leer(icono);
        let nombre = icono.file_stem();
        // Lo incrustado es lo que hay en el disco.
        assert_eq!(embedded(icono), bytes.as_slice(), "{nombre}: include_bytes");
        let (w, h, rgba) = rgba_de(&bytes);
        assert_eq!((w, h), (16, 16), "{nombre}");
        let mut opacos = 0;
        let mut mascara = Vec::new();
        for px in rgba.as_chunks::<4>().0 {
            assert!(px[3] == 0 || px[3] == 255, "{nombre}: alfa {}", px[3]);
            if px[3] == 255 {
                assert_eq!(
                    &px[..3],
                    &[255, 255, 255],
                    "{nombre}: los opacos son blancos"
                );
                opacos += 1;
            }
            mascara.push(px[3] == 255);
        }
        assert!(
            (1..256).contains(&opacos),
            "{nombre}: ni vacío ni lleno ({opacos} píxeles opacos)"
        );
        assert!(vistos.insert(mascara), "{nombre}: es igual que otro icono");
    }
    assert_eq!(vistos.len(), 21);
}

#[test]
fn fila_1_hay_un_fichero_por_icono_ninguno_de_mas_y_el_readme_dice_original_mit_y_nuevo() {
    let mut esperados: Vec<String> = Icon::ALL
        .iter()
        .map(|i| format!("{}.png", i.file_stem()))
        .collect();
    esperados.push("README.md".to_string());
    esperados.sort();
    let mut hay: Vec<String> = std::fs::read_dir(carpeta())
        .expect("assets/icons existe")
        .map(|e| {
            e.expect("entrada")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    hay.sort();
    assert_eq!(
        hay, esperados,
        "ficheros de más o de menos (sin pincel.png)"
    );
    // Los nombres son los de la tabla de la spec.
    let nombres: Vec<&str> = Icon::ALL.iter().map(|i| i.file_stem()).collect();
    assert_eq!(
        nombres,
        [
            "lapiz",
            "linea",
            "relleno",
            "cuentagotas",
            "borrador",
            "acercar",
            "alejar",
            "ajustar",
            "uno-a-uno",
            "anadir",
            "quitar",
            "rotar-izquierda",
            "rotar-derecha",
            "transparente",
            "ojo",
            "ojo-cerrado",
            "candado",
            "candado-abierto",
            "duplicar",
            "subir",
            "bajar",
        ]
    );
    let readme = std::fs::read_to_string(carpeta().join("README.md")).expect("README.md");
    for frase in ["originales de Pixoko", "MIT", "dibujados nuevos", "pincel"] {
        assert!(readme.contains(frase), "el README no dice {frase:?}");
    }
    let terceros = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/THIRD-PARTY.md"),
    )
    .expect("THIRD-PARTY.md");
    assert!(
        terceros.contains("assets/icons") && terceros.contains("no proceden de terceros"),
        "THIRD-PARTY.md debe decir que los iconos no proceden de terceros"
    );
}

/// Las texturas nuevas del fotograma: `(id, tamaño, opciones, píxeles)`.
fn texturas_subidas(
    ctx: &egui::Context,
) -> Vec<(egui::TextureId, [usize; 2], TextureOptions, Vec<Color32>)> {
    let mut salida = ctx.run_ui(RawInput::default(), |_| {});
    let subidas = salida
        .textures_delta
        .set
        .iter()
        .flat_map(|(id, deltas)| deltas.iter().map(move |delta| (*id, delta)))
        .map(|(id, delta)| match &delta.image {
            egui::ImageData::Color(c) => (id, c.size, delta.options, c.pixels.clone()),
        })
        .collect();
    // egui exige no soltar los cambios de textura sin aplicar.
    salida.textures_delta.clear();
    subidas
}

#[test]
fn fila_2_load_sube_14_texturas_de_16x16_con_vecino_mas_cercano_y_distintas() {
    let ctx = egui::Context::default();
    let iconos = load(&ctx).expect("los iconos se cargan");
    let subidas = texturas_subidas(&ctx);
    let mut imagenes = HashSet::new();
    for icono in Icon::ALL {
        let id = iconos.set.get(icono).expect("cada icono tiene su textura");
        let (_, tamano, opciones, pixeles) = subidas
            .iter()
            .find(|(i, ..)| *i == id)
            .unwrap_or_else(|| panic!("{}: no se subió", icono.file_stem()));
        assert_eq!(*tamano, [16, 16], "{}", icono.file_stem());
        assert_eq!(*opciones, TextureOptions::NEAREST, "{}", icono.file_stem());
        // Premultiplicado: blanco opaco o transparente.
        assert!(
            pixeles
                .iter()
                .all(|p| *p == Color32::WHITE || *p == Color32::TRANSPARENT),
            "{}",
            icono.file_stem()
        );
        imagenes.insert(pixeles.clone());
    }
    assert_eq!(imagenes.len(), 21, "las 21 son distintas");
    // Se cargan una vez: otro fotograma no sube nada más.
    assert!(texturas_subidas(&ctx).is_empty());
}

#[test]
fn fila_2_un_png_corrupto_o_de_otro_tamano_da_un_error_claro_y_no_un_panico() {
    let ctx = egui::Context::default();
    let roto = load_with(&ctx, &|i| {
        if i == Icon::Line {
            vec![1, 2, 3, 4]
        } else {
            embedded(i).to_vec()
        }
    })
    .err()
    .expect("un PNG corrupto es un error");
    assert!(roto.to_string().contains("linea"), "{roto}");
    assert!(matches!(roto, IconError::Png { .. }), "{roto:?}");

    let pequeno = write_png(&Image::new(8, 8, px_core::Mode::Rgba).expect("imagen")).expect("png");
    let mal = load_with(&ctx, &|i| {
        if i == Icon::Fill {
            pequeno.clone()
        } else {
            embedded(i).to_vec()
        }
    })
    .err()
    .expect("otro tamaño es un error");
    assert!(mal.to_string().contains("relleno"), "{mal}");
    assert!(
        matches!(
            mal,
            IconError::Size {
                width: 8,
                height: 8,
                ..
            }
        ),
        "{mal:?}"
    );
}

/// El mismo dibujo que `icono`, guardado como lo haría Pixoko en modo indexado: dos entradas, la 0
/// transparente y la 1 de `tinta`, con la 0 marcada como transparente.
fn en_indexado(icono: Icon, tinta: Rgba) -> Vec<u8> {
    let (_, _, rgba) = rgba_de(&leer(icono));
    let mut paleta =
        Palette::from_colors([Rgba::new(0, 0, 0, 0), tinta]).expect("paleta de dos entradas");
    paleta.set_transparent_index(Some(0)).expect("transparente");
    let mut imagen = Image::new_indexed(16, 16, paleta).expect("imagen indexada");
    for (i, px) in rgba.as_chunks::<4>().0.iter().enumerate() {
        let indice = u8::from(px[3] == 255);
        imagen
            .set_pixel((i % 16) as u32, (i / 16) as u32, Pixel::Indexed(indice))
            .expect("píxel");
    }
    write_png(&imagen).expect("PNG indexado")
}

#[test]
fn fila_2_un_icono_retocado_y_guardado_en_indexado_se_carga_y_se_tinta_igual() {
    let referencia = decode("lapiz", &leer(Icon::Pencil)).expect("RGBA");
    for tinta in [
        Rgba::opaque(255, 255, 255),
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(200, 30, 30),
    ] {
        let indexado = en_indexado(Icon::Pencil, tinta);
        // Es de verdad un PNG indexado.
        assert_eq!(
            read_png(&indexado).expect("legible").image.mode(),
            px_core::Mode::Indexed
        );
        let cargado = decode("lapiz", &indexado).expect("el indexado se carga");
        assert_eq!(cargado.size, [16, 16]);
        // Mismos texels que el RGBA: solo cuenta el alfa; el color del retoque no importa.
        assert_eq!(
            cargado.pixels, referencia.pixels,
            "tinta de dibujo {tinta:?}"
        );
    }
    // Tintado: blanco por la tinta da la tinta donde hay alfa y nada donde no.
    for tinta in [
        Color32::from_rgb(0x39, 0xd0, 0xb2),
        Color32::from_rgb(0xe0, 0xe0, 0xe0),
    ] {
        for texel in &referencia.pixels {
            if *texel == Color32::WHITE {
                assert_eq!(tinta * *texel, tinta);
            } else {
                assert_eq!(*texel, Color32::TRANSPARENT);
            }
        }
    }
}
