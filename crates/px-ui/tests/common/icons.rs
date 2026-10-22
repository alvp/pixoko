//! Iconos de prueba para los bancos de egui (specs/interfaz-iconos.md): 14 texturas blancas de
//! 16 × 16, una por `Icon`, cada una con su `TextureId`. Los bancos buscan en lo pintado la malla
//! cuya textura es la de un icono y leen su tinta y su rectángulo; no hace falta el dibujo real
//! (de eso se ocupan las pruebas de `px-app`).

#![allow(dead_code)]

use egui::{ColorImage, Context, TextureHandle, TextureId, TextureOptions};
use px_ui::icons::{Icon, IconSet};

/// Las texturas de prueba (se mantienen vivas mientras viva el banco) y el `IconSet` que las usa.
pub struct IconsOfTest {
    pub set: IconSet,
    handles: Vec<TextureHandle>,
}

impl IconsOfTest {
    pub fn new_2_2(ctx: &Context) -> IconsOfTest {
        let handles: Vec<TextureHandle> = Icon::ALL
            .iter()
            .map(|icon| {
                ctx.load_texture(
                    format!("icono-de-prueba-{}", icon.file_stem()),
                    ColorImage::filled([16, 16], egui::Color32::WHITE),
                    TextureOptions::NEAREST,
                )
            })
            .collect();
        let set = IconSet::new(|icon| handles[icon.index()].id());
        IconsOfTest { set, handles }
    }

    /// El icono cuya textura es `id`.
    pub fn icon_of(&self, id: TextureId) -> Option<Icon> {
        Icon::ALL
            .into_iter()
            .find(|icon| self.handles[icon.index()].id() == id)
    }
}
