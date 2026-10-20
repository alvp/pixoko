//! px-ui: modelo de vista del lienzo, conversión de imágenes, lienzo de egui, texto, tema y
//! barra de estado, el panel de paletas, el editor de color, la capa de acciones (`actions`), el
//! menú, la columna de herramientas, el panel de licencias y el damero de transparencia del lienzo
//! con su fondo elegible (`checker`, `background`) y el teclado como tabla de datos (`keymap`,
//! `key_events`).

pub mod actions;
pub mod background;
pub mod canvas;
pub mod checker;
pub mod chrome;
pub mod color;
pub mod color_edit;
pub mod color_editor;
pub mod dialog;
pub mod dialog_keys;
pub mod dialog_layout;
pub mod dialog_view;
pub mod dirty;
pub mod edit_layout;
pub mod key_events;
pub mod keymap;
pub mod licenses_view;
pub mod menu;
pub mod notice;
pub mod number;
pub mod palette_grid;
pub mod palette_ops;
pub mod palette_panel;
pub mod pick;
pub mod shortcuts;
pub mod status;
pub mod stroke;
pub mod text;
pub mod texture;
pub mod theme;
pub mod tiles;
pub mod tool;
pub mod toolbar;
pub mod view;
