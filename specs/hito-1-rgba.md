# Hito 1 · Unidad 1: color `Rgba` y su notación hexadecimal

Estado: **aprobada** (2026-10-17)

## Objetivo

Definir en `px-core` el tipo de color RGBA de 8 bits por canal y su lectura/escritura como texto hexadecimal (`#rrggbb`, `#rrggbbaa`). Todo lo demás del hito 1 (paletas, imágenes, conversión de modos, PNG) se apoya en este tipo.

## Entradas y salidas

- Tipo: `px_core::Rgba { r: u8, g: u8, b: u8, a: u8 }`.
- `Rgba::new(r, g, b, a) -> Rgba` y `Rgba::opaque(r, g, b) -> Rgba` (`a = 255`).
- `Rgba::TRANSPARENT` = `(0, 0, 0, 0)`. También es el valor de `Default`.
- `Rgba::from_hex(&str) -> Result<Rgba, ParseHexError>`; `FromStr` delega en ella.
- `impl Display for Rgba`: escribe el color en su forma canónica (ver abajo).
- `ParseHexError`: enum con `Length(usize)` (nº de dígitos hexadecimales recibidos) e `InvalidDigit(char)`. Implementa `Display`, `Error`, `Debug`, `Clone`, `PartialEq`, `Eq`.
- `Rgba` deriva `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Debug`.

## Comportamiento

1. **Alfa directo (no premultiplicado).** Los canales `r`, `g`, `b` son independientes de `a`. Un píxel con `a = 0` conserva su color RGB; no se normaliza a negro. La premultiplicación, si hace falta, es cosa del compositor (hito 3).
2. **Lectura.** `from_hex` acepta un `#` inicial opcional y a continuación 3, 4, 6 u 8 dígitos hexadecimales, en mayúsculas o minúsculas:
   - `rgb` → cada dígito se duplica (`#f80` = `#ff8800`), `a = 255`.
   - `rgba` → ídem con alfa (`#f80a` = `#ff8800aa`).
   - `rrggbb` → `a = 255`.
   - `rrggbbaa`.
3. **Escritura canónica.** `Display` produce siempre `#` + minúsculas: `#rrggbb` si `a == 255`, y `#rrggbbaa` en cualquier otro caso. Nunca usa las formas cortas.
4. **Ida y vuelta.** Para todo `c: Rgba`, `Rgba::from_hex(&c.to_string()) == Ok(c)`.
5. **Errores.** `Length(n)` si, tras quitar el `#` inicial, el número de caracteres `n` no es 3, 4, 6 ni 8 (incluye la cadena vacía y `"#"`: `Length(0)`). `InvalidDigit(ch)` con el primer carácter que no sea dígito hexadecimal ASCII. La longitud se comprueba primero.
6. Sin recorte de espacios: `" #fff"` y `"#fff "` son errores.

## Casos límite

| Entrada | Resultado |
| --- | --- |
| `"#000000"` | `Rgba(0, 0, 0, 255)` |
| `"000000"` (sin `#`) | `Rgba(0, 0, 0, 255)` |
| `"#FFF"` y `"#fff"` | `Rgba(255, 255, 255, 255)` |
| `"#f80a"` | `Rgba(255, 136, 0, 170)` |
| `"#ff880000"` | `Rgba(255, 136, 0, 0)` (conserva el RGB con alfa 0) |
| `""` y `"#"` | `Err(Length(0))` |
| `"#ff"` | `Err(Length(2))` |
| `"#fffff"` | `Err(Length(5))` |
| `"#fffffffff"` | `Err(Length(9))` |
| `"#ggg"` | `Err(InvalidDigit('g'))` |
| `"##fff"` | `Err(InvalidDigit('#'))` (se quita un solo `#`; quedan 4 caracteres, longitud válida) |
| `"#ffé"` | `Err(InvalidDigit('é'))` (3 caracteres, aunque sean 4 bytes; `é` no es dígito ASCII) |
| `"+fff"` | `Err(InvalidDigit('+'))` |

La longitud cuenta caracteres Unicode, no bytes.

## Criterios de aceptación

Desde la raíz, en Linux (el CI cubre los otros dos sistemas):

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo deny check` (si se añade `proptest`, ver decisión 2)

## Tests

En `crates/px-core/tests/rgba.rs` (tests de integración sobre la API pública):

- Una prueba por fila de la tabla de casos límite.
- `Default` y `Rgba::TRANSPARENT` valen `(0, 0, 0, 0)`; `opaque` pone `a = 255`.
- Alfa directo: `Rgba::new(10, 20, 30, 0)` conserva `10, 20, 30` tras ida y vuelta por texto.
- Propiedad (comportamiento 4): para cualquier `Rgba` aleatorio, escribir y leer devuelve el mismo color.
- Propiedad: `Display` siempre empieza por `#`, es minúscula y mide 7 o 9 caracteres.
- Propiedad: los errores nunca hacen panic con cadenas arbitrarias (incluida Unicode).

No hay dorado: no existe un comportamiento de programa de referencia que comparar; el formato es una convención textual propia.

## Decisiones

1. **`#` opcional al leer.** Se acepta con o sin `#` (cómodo para CLI y archivos de paleta).
2. **`proptest` como dependencia de desarrollo de `px-core`.** El plan pide pruebas de propiedades con `proptest`; esta es la primera unidad que las necesita. Se añade ahora, debe pasar `cargo deny check`, se revisa con `license-auditor` y se anota en `docs/DEPENDENCIES.md`.
3. **Alfa directo en todo el proyecto** (ver comportamiento 1).
4. **Sin formas CSS adicionales** (`rgb(...)`, nombres de color): fuera de alcance.

## Unidades del hito 1

Hito 1: «Documento, comandos con deshacer, PNG indexado y RGBA». Puerta de salida: ida y vuelta de PNG sin pérdida en el corpus.

1. `Rgba` y su notación hexadecimal (esta página).
2. `Palette` (hasta 256 entradas, índice transparente).
3. `Image` con modos RGBA, indexado y grises (`new`, `get_pixel`, `set_pixel`).
4. `Document` y comandos con deshacer.
5. PNG indexado y RGBA (`px-formats`, dependencia `png`).
6. Corpus de ida y vuelta de PNG.

Movidos al hito 2: conversiones RGB ↔ HSL, reordenar la paleta sin alterar la imagen, conversión entre modos, lienzos grandes y panel de paletas.
