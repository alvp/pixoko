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

fn folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons")
}

fn leer(icon: Icon) -> Vec<u8> {
    let path_2 = folder().join(format!("{}.png", icon.file_stem()));
    std::fs::read(&path_2).unwrap_or_else(|e| panic!("{}: {e}", path_2.display()))
}

/// Los píxeles RGBA de un PNG, por filas, como lo hace la carga.
fn rgba_de(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let png = read_png(bytes).expect("PNG legible");
    let (w, h) = (png.image.width(), png.image.height());
    (w, h, to_rgba8(&png.image))
}

#[test]
fn row_1_the_14_pngs_are_16x16_with_alpha_0_or_255_white_neither_empty_nor_full_and_different() {
    let mut seen: HashSet<Vec<bool>> = HashSet::new();
    for icon in Icon::ALL {
        let bytes = leer(icon);
        let name_2 = icon.file_stem();
        // Lo incrustado es lo que hay en el disco.
        assert_eq!(embedded(icon), bytes.as_slice(), "{name_2}: include_bytes");
        let (w, h, rgba) = rgba_de(&bytes);
        assert_eq!((w, h), (16, 16), "{name_2}");
        let mut opaque = 0;
        let mut mascara = Vec::new();
        for px in rgba.as_chunks::<4>().0 {
            assert!(px[3] == 0 || px[3] == 255, "{name_2}: alfa {}", px[3]);
            if px[3] == 255 {
                assert_eq!(
                    &px[..3],
                    &[255, 255, 255],
                    "{name_2}: los opacos son blancos"
                );
                opaque += 1;
            }
            mascara.push(px[3] == 255);
        }
        assert!(
            (1..256).contains(&opaque),
            "{name_2}: ni vacío ni lleno ({opaque} píxeles opacos)"
        );
        assert!(seen.insert(mascara), "{name_2}: es igual que otro icono");
    }
    assert_eq!(seen.len(), 21);
}

#[test]
fn row_1_there_is_one_file_per_icon_none_extra_and_the_readme_says_original_mit_and_new() {
    let mut expected_2_2_2: Vec<String> = Icon::ALL
        .iter()
        .map(|i| format!("{}.png", i.file_stem()))
        .collect();
    expected_2_2_2.push("README.md".to_string());
    expected_2_2_2.sort();
    let mut there_is: Vec<String> = std::fs::read_dir(folder())
        .expect("assets/icons existe")
        .map(|e| {
            e.expect("entrada")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    there_is.sort();
    assert_eq!(
        there_is, expected_2_2_2,
        "ficheros de más o de menos (sin pincel.png)"
    );
    // Los nombres son los de la tabla de la spec.
    let names: Vec<&str> = Icon::ALL.iter().map(|i| i.file_stem()).collect();
    assert_eq!(
        names,
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
    let readme = std::fs::read_to_string(folder().join("README.md")).expect("README.md");
    for sentence in ["originales de Pixoko", "MIT", "dibujados nuevos", "pincel"] {
        assert!(readme.contains(sentence), "el README no dice {sentence:?}");
    }
    let third = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/THIRD-PARTY.md"),
    )
    .expect("THIRD-PARTY.md");
    assert!(
        third.contains("assets/icons") && third.contains("no proceden de terceros"),
        "THIRD-PARTY.md debe decir que los iconos no proceden de terceros"
    );
}

/// Las texturas nuevas del fotograma: `(id, tamaño, options_2, píxeles)`.
fn textures_uploads(
    ctx: &egui::Context,
) -> Vec<(egui::TextureId, [usize; 2], TextureOptions, Vec<Color32>)> {
    let mut output = ctx.run_ui(RawInput::default(), |_| {});
    let uploads = output
        .textures_delta
        .set
        .iter()
        .flat_map(|(id, deltas)| deltas.iter().map(move |delta| (*id, delta)))
        .map(|(id, delta)| match &delta.image {
            egui::ImageData::Color(c) => (id, c.size, delta.options, c.pixels.clone()),
        })
        .collect();
    // egui exige no soltar los cambios de textura sin aplicar.
    output.textures_delta.clear();
    uploads
}

#[test]
fn row_2_load_uploads_14_textures_of_16x16_with_nearest_neighbor_and_different() {
    let ctx = egui::Context::default();
    let icons = load(&ctx).expect("los iconos se cargan");
    let uploads = textures_uploads(&ctx);
    let mut images = HashSet::new();
    for icon in Icon::ALL {
        let id = icons.set.get(icon).expect("cada icono tiene su textura");
        let (_, size, options_2, pixels_2) = uploads
            .iter()
            .find(|(i, ..)| *i == id)
            .unwrap_or_else(|| panic!("{}: no se subió", icon.file_stem()));
        assert_eq!(*size, [16, 16], "{}", icon.file_stem());
        assert_eq!(*options_2, TextureOptions::NEAREST, "{}", icon.file_stem());
        // Premultiplicado: blanco opaco o transparente.
        assert!(
            pixels_2
                .iter()
                .all(|p| *p == Color32::WHITE || *p == Color32::TRANSPARENT),
            "{}",
            icon.file_stem()
        );
        images.insert(pixels_2.clone());
    }
    assert_eq!(images.len(), 21, "las 21 son distintas");
    // Se cargan una vez: otro fotograma no sube nada más.
    assert!(textures_uploads(&ctx).is_empty());
}

#[test]
fn row_2_a_corrupt_png_or_of_another_size_gives_a_clear_error_and_not_a_panic() {
    let ctx = egui::Context::default();
    let broken = load_with(&ctx, &|i| {
        if i == Icon::Line {
            vec![1, 2, 3, 4]
        } else {
            embedded(i).to_vec()
        }
    })
    .err()
    .expect("un PNG corrupto es un error");
    assert!(broken.to_string().contains("linea"), "{broken}");
    assert!(matches!(broken, IconError::Png { .. }), "{broken:?}");

    let small = write_png(&Image::new(8, 8, px_core::Mode::Rgba).expect("imagen")).expect("png");
    let badly = load_with(&ctx, &|i| {
        if i == Icon::Fill {
            small.clone()
        } else {
            embedded(i).to_vec()
        }
    })
    .err()
    .expect("otro tamaño es un error");
    assert!(badly.to_string().contains("relleno"), "{badly}");
    assert!(
        matches!(
            badly,
            IconError::Size {
                width: 8,
                height: 8,
                ..
            }
        ),
        "{badly:?}"
    );
}

/// El mismo dibujo que `icon`, guardado como lo haría Pixoko en modo indexado: dos entradas, la 0
/// transparente y la 1 de `ink`, con la 0 marcada como transparente.
fn in_indexed(icon: Icon, ink: Rgba) -> Vec<u8> {
    let (_, _, rgba) = rgba_de(&leer(icon));
    let mut palette =
        Palette::from_colors([Rgba::new(0, 0, 0, 0), ink]).expect("paleta de dos entradas");
    palette
        .set_transparent_index(Some(0))
        .expect("transparente");
    let mut image_2 = Image::new_indexed(16, 16, palette).expect("imagen indexada");
    for (i, px) in rgba.as_chunks::<4>().0.iter().enumerate() {
        let index_2 = u8::from(px[3] == 255);
        image_2
            .set_pixel((i % 16) as u32, (i / 16) as u32, Pixel::Indexed(index_2))
            .expect("píxel");
    }
    write_png(&image_2).expect("PNG indexado")
}

#[test]
fn row_2_a_retouched_icon_saved_as_indexed_loads_and_tints_the_same() {
    let reference = decode("lapiz", &leer(Icon::Pencil)).expect("RGBA");
    for ink in [
        Rgba::opaque(255, 255, 255),
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(200, 30, 30),
    ] {
        let indexed_2 = in_indexed(Icon::Pencil, ink);
        // Es de verdad un PNG indexado.
        assert_eq!(
            read_png(&indexed_2).expect("legible").image.mode(),
            px_core::Mode::Indexed
        );
        let loaded = decode("lapiz", &indexed_2).expect("el indexado se carga");
        assert_eq!(loaded.size, [16, 16]);
        // Mismos texels que el RGBA: solo cuenta el alfa; el color del retoque no importa.
        assert_eq!(loaded.pixels, reference.pixels, "tinta de dibujo {ink:?}");
    }
    // Tintado: blanco por la tinta da la tinta donde hay alfa y nada donde no.
    for ink in [
        Color32::from_rgb(0x39, 0xd0, 0xb2),
        Color32::from_rgb(0xe0, 0xe0, 0xe0),
    ] {
        for texel in &reference.pixels {
            if *texel == Color32::WHITE {
                assert_eq!(ink * *texel, ink);
            } else {
                assert_eq!(*texel, Color32::TRANSPARENT);
            }
        }
    }
}
