//! Puerta del hito 2 (specs/hito-2-puerta.md, unidad 22): un PNG real se abre, se edita y se guarda.
//!
//! No hay código de producción nuevo: estas pruebas ejercen lo que ya existe (`Session`,
//! `ToolState`, `Dialogs`, `write_png` y `read_png`), así que **nacen en verde**; la prueba de que
//! saben fallar son los controles negativos de la fila 9 (aquí) y de la fila 11 (en `px-golden`).
//!
//! Reparto (spec, «Dónde van las pruebas»):
//! - `comun`: directorio de trabajo, fuentes (reales y generadas), modelo independiente,
//!   comparador, instantánea de `golden/png/`, SHA-256 propio, `FakePicker` propio y la ayuda
//!   `fuera_de_golden`.
//! - `filas`: las filas 1 a 10 (la 11 está en `crates/px-golden/tests/png_interop/gate_oraculo.rs`).
//! - `grande`: la fila 12, `#[ignore]` (se lanza a mano, en `--release`).
//! - `propiedades`: las propiedades 1 a 4 (`proptest`).
//!
//! Reglas de la unidad que cumplen todas las pruebas:
//! - **Ninguna escritura sale de `golden/png/`**: los reales se copian al directorio de trabajo
//!   (`<repo>/target/pixoko-gate/<pid>-<n>`, que se borra siempre) y se abre la copia; toda ruta de
//!   escritura pasa por `fuera_de_golden`.
//! - Lo esperado sale de un modelo ingenuo (rejilla de píxeles y paleta), nunca de `Image`,
//!   `Session` ni `px-raster`.

mod capas;
mod comun;
mod filas;
mod grande;
mod propiedades;
