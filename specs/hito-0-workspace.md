# Hito 0 · Unidad 1: workspace con los 8 crates vacíos

Estado: **aprobada** (2026-10-17)

## Objetivo

Un workspace de Cargo que compila y pasa fmt, clippy y test sin ninguna dependencia externa. Es la base sobre la que se apoyan el resto de unidades y los hitos siguientes. No contiene lógica de pixel art.

## Entradas y salidas

- Entrada: ninguna (no hay función; es estructura).
- Salida: `Cargo.toml` en la raíz y ocho crates en `crates/`.

| Crate | Tipo | Contenido inicial |
| --- | --- | --- |
| px-core | lib | vacío, con documentación del crate |
| px-raster | lib | vacío |
| px-formats | lib | vacío |
| px-render | lib | vacío; único crate que puede usar `unsafe` |
| px-ui | lib | vacío |
| px-golden | lib | vacío; aquí vivirá el comparador de dorados |
| px-app | bin | `main` que no hace nada |
| px-cli | bin | `main` que no hace nada |

## Comportamiento

1. `Cargo.toml` de la raíz declara `[workspace]` con `members = ["crates/*"]` y `resolver = "3"`.
2. `[workspace.package]` fija `edition = "2024"`, `license = "MIT"`, `rust-version = "1.99"` y `publish = false`; cada crate hereda con `.workspace = true`.
3. `[workspace.lints]` aplica a todos los crates: `rust.unsafe_code = "forbid"`, `clippy.all = "warn"`. En px-render se usa `unsafe_code = "deny"` en lugar de `forbid`, para poder permitir `unsafe` en bloques concretos con su comentario (regla de `CLAUDE.md`).
4. Ningún crate declara dependencias, ni entre ellos ni externas. Las aristas entre crates (las tres capas del plan) se añaden cuando una unidad las necesite.
5. Los nombres de los crates llevan el prefijo `px-` y no dependen del nombre del producto.
6. Se añade `.gitignore` con `/target`, y se versiona `Cargo.lock` (hay binarios).

## Casos límite

- Un crate nuevo en `crates/` debe entrar en el workspace sin tocar la raíz (`members = ["crates/*"]`).
- `cargo deny check` debe pasar con la política actual de `deny.toml`: los crates propios son MIT y no hay dependencias.
- Los binarios no deben imprimir nada ni salir con error: `px-app` y `px-cli` terminan con código 0.

## Criterios de aceptación

Todo desde la raíz, en Linux (Windows y macOS los cubrirá la unidad de CI):

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo deny check`
- Los 8 crates aparecen en `cargo metadata --no-deps`.

## Tests

Un único test de estructura en `px-golden` (solo `std`, sin dependencias) que lee `Cargo.toml` de la raíz y los de `crates/*` y comprueba:

- existen exactamente los 8 crates de la tabla;
- cada uno hereda `license.workspace = true` y `lints.workspace = true`, salvo px-render, que declara sus propios lints;
- ningún `Cargo.toml` de crate tiene sección `[dependencies]` con entradas.

Esto evita que la estructura se deshaga sin que nadie lo note, sin inventar tests artificiales dentro de crates vacíos.

## Decisiones

1. **Aristas de dependencia.** Cada unidad añade las que necesite; esta unidad no define ninguna.
2. **`rust-version = "1.99"`.** Se fija al toolchain instalado (1.99.1), el único que se verifica. Se podrá bajar cuando el CI pruebe un mínimo menor.
3. **Test de estructura en px-golden.** Se mantiene.

## Fuera de alcance (unidades siguientes del hito 0)

- Unidad 2: CI en Linux, Windows y macOS con fmt, clippy, test y cargo-deny.
- Unidad 3: instalar Grafx2 y LibreSprite como binarios de referencia (solo ejecutar, nunca leer su código) y anotarlo en la documentación.
- Unidad 4: probar que el hook de guardia bloquea el clonado de Grafx2.
