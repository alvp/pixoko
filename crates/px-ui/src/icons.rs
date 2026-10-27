//! Los iconos propios de la interfaz (specs/interfaz-iconos.md): sus nombres y el conjunto de
//! texturas con que se pintan. Los PNG (`assets/icons/<name_2>.png`, 16 × 16, alfa 0 o 255,
//! píxeles blancos) los incrusta y los sube `px-app`; aquí no hay dibujos ni dependencia de
//! `px-formats`.
//!
//! El icono del pincel del hito 4 está **reservado** sin dibujar: su nombre (`pincel`) y su fichero
//! se anotan en `assets/icons/README.md`; no hay `Icon::Brush` todavía.

use egui::TextureId;

/// Lado de un icono, en `u` (píxeles de interfaz): 16.
pub const ICON_PX: i64 = 16;

/// Nombre y fichero reservados para el pincel (hito 4); no es un `Icon` ni tiene PNG todavía.
pub const RESERVED_BRUSH_STEM: &str = "pincel";

/// Los 23 iconos: cinco herramientas, cuatro botones de zoom, cinco del editor de paleta, siete
/// del panel de capas, el de intercambiar colores y el de formas (que va al final para no mover
/// el índice de los demás).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Pencil,
    Line,
    Fill,
    Dropper,
    Eraser,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ZoomActualSize,
    Add,
    Remove,
    RotateLeft,
    RotateRight,
    Transparent,
    Eye,
    EyeClosed,
    Lock,
    LockOpen,
    Duplicate,
    Up,
    Down,
    /// Swap the foreground and the background (the button next to the two color bars; C4).
    Swap,
    /// The shapes tool (4.4): a rectangle and a circle overlapped.
    Shapes,
}

impl Icon {
    /// Todos, en el orden de la spec.
    pub const ALL: [Icon; 23] = [
        Icon::Pencil,
        Icon::Line,
        Icon::Fill,
        Icon::Dropper,
        Icon::Eraser,
        Icon::ZoomIn,
        Icon::ZoomOut,
        Icon::ZoomFit,
        Icon::ZoomActualSize,
        Icon::Add,
        Icon::Remove,
        Icon::RotateLeft,
        Icon::RotateRight,
        Icon::Transparent,
        Icon::Eye,
        Icon::EyeClosed,
        Icon::Lock,
        Icon::LockOpen,
        Icon::Duplicate,
        Icon::Up,
        Icon::Down,
        Icon::Swap,
        Icon::Shapes,
    ];

    /// Posición en [`Icon::ALL`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// Nombre del fichero sin extensión (`assets/icons/<name_2>.png`).
    pub fn file_stem(self) -> &'static str {
        match self {
            Icon::Pencil => "lapiz",
            Icon::Line => "linea",
            Icon::Fill => "relleno",
            Icon::Dropper => "cuentagotas",
            Icon::Eraser => "borrador",
            Icon::ZoomIn => "acercar",
            Icon::ZoomOut => "alejar",
            Icon::ZoomFit => "ajustar",
            Icon::ZoomActualSize => "uno-a-uno",
            Icon::Add => "anadir",
            Icon::Remove => "quitar",
            Icon::RotateLeft => "rotar-izquierda",
            Icon::RotateRight => "rotar-derecha",
            Icon::Transparent => "transparente",
            Icon::Eye => "ojo",
            Icon::EyeClosed => "ojo-cerrado",
            Icon::Lock => "candado",
            Icon::LockOpen => "candado-abierto",
            Icon::Duplicate => "duplicar",
            Icon::Up => "subir",
            Icon::Down => "bajar",
            Icon::Swap => "swap",
            Icon::Shapes => "formas",
        }
    }
}

/// Las texturas de los iconos. Vacío por defecto (los botones se pintan sin icono): lo llena
/// `px-app` al arrancar y las texturas las mantiene vivas quien las creó.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IconSet {
    textures: [Option<TextureId>; 23],
}

impl IconSet {
    /// Un conjunto con la textura que da `texture` para cada icono.
    pub fn new(mut texture: impl FnMut(Icon) -> TextureId) -> IconSet {
        let mut textures = [None; 23];
        for icon in Icon::ALL {
            textures[icon.index()] = Some(texture(icon));
        }
        IconSet { textures }
    }

    /// La textura del icono, si el conjunto la tiene.
    pub fn get(&self, icon: Icon) -> Option<TextureId> {
        self.textures[icon.index()]
    }
}
