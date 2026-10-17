# Hito 1 · Unidad 6: corpus de ida y vuelta de PNG (`px-golden`)

Estado: **aprobada** (2026-10-17; decisiones 1 a 7 aceptadas, con los matices: SHA-256 en un módulo de ayuda de los tests con más vectores, `.sha256` leídos recortando espacios, README de procedencia sin inspeccionar los PNG, el test de interoperabilidad imprime el informe de cada fichero y todo lo de `golden/` va en el mismo commit)

## Objetivo

Es la puerta de salida del hito 1 («ida y vuelta de PNG sin pérdida en el corpus»). Reúne en `px-golden`, con la API pública de `px-formats` (`specs/hito-1-png.md`), cuatro cosas: ida y vuelta exacta con imágenes sintéticas, conversiones declaradas, errores sin pánico e interoperabilidad con PNG guardados por Grafx2 y LibreSprite. **No añade código de producción**: los tests deberían pasar desde el primer día (ver la fila 14 y los criterios de aceptación: un corpus que no sabe fallar no protege).

Los valores esperados salen de la especificación del PNG (W3C, tercera edición) y de la spec de la unidad 5, no de ningún programa; los programas de referencia solo producen los ficheros del punto 4 (regla de `CLAUDE.md`: se ejecutan y se guarda su salida en `golden/`).

## Entradas y salidas

Ficheros nuevos o cambiados:

| Ruta | Qué |
| --- | --- |
| `crates/px-golden/tests/png_roundtrip.rs` | Puntos 1 a 3 (sintético, conversiones, errores) |
| `crates/px-golden/tests/png_interop.rs` | Punto 4 (corpus de `golden/png/`), SHA-256 y comprobaciones del repo |
| `crates/px-golden/tests/common/sha256.rs` | SHA-256 (FIPS 180-4) para el punto 4, con sus propios tests de vectores |
| `crates/px-golden/tests/common/mod.rs` | Constructor de PNG a mano (chunks, CRC-32, Adler-32, deflate «stored»): copia deliberada del ayudante de `px-formats/tests/png.rs` (decisión 3) |
| `crates/px-golden/Cargo.toml` | `[dev-dependencies]`: `px-core`, `px-formats` (rutas) y `proptest = "1.12.0"` |
| `docs/DEPENDENCIES.md` | La fila de `proptest` pasa a decir «dev-dependency de `px-core`, `px-formats` y `px-golden`». Ninguna crate nueva |
| `.gitattributes` | Punto 5 |
| `golden/png/README.md` | Punto 6 |
| `specs/hito-1-png.md` | Punto 7: nueva sección «Para el hito 2» |

`golden/png/*.png` y `*.png.sha256` ya existen (sin seguimiento en git): los puso el usuario. Se commitean en la misma unidad, junto con `.gitattributes` y el README.

## Comportamiento

### 1. Ida y vuelta sintética (sin ficheros)

Las imágenes se generan en el test. Para cada una, `read_png(&write_png(&img)?)` debe ser `Ok` con **informe vacío** (`is_exact()`) e imagen igual a `normalizada(img)`: `img`, salvo un indexado con `transparent_index = Some(t)`, cuya paleta sale ya resuelta (`resolved(i)`, es decir, la entrada `t` con `a = 0`) y sin índice transparente (spec de PNG, comportamiento 11). `Image` deriva `PartialEq`, así que la igualdad cubre modo, tamaño, píxeles y paleta (longitud, orden, colores duplicados y alfas). Casos de la tabla, filas 1 a 6.

### 2. Conversiones (aparte de las exactas)

Ficheros construidos byte a byte con `common`. Cada caso comprueba **a la vez** la imagen y la lista exacta del informe (contenido y orden: `DepthExpanded`, `Depth16Reduced`, `RgbToRgba`, `ColorKeyToAlpha`) y que `is_exact()` es falso. Valores de la spec de PNG (comportamientos 2 a 4). Filas 7 a 9. No se mezclan con el punto 1: un informe no vacío en la ida y vuelta sintética es un fallo.

### 3. Errores

Ningún caso hace pánico (un pánico falla el test). Cada uno comprueba la variante exacta de `PngError`, no solo `is_err()`. Filas 10 a 12.

### 4. Interoperabilidad con `golden/png/`

- **Qué hay en el corpus.** Para cada `X.png`, un `X.png.sha256` al lado. Hoy: `grafx2-golden1.png`, `grafx2-golden2.png`, `libresprite-golden-indexed.png`, `libresprite-golden-rgba.png`, todos de 16 × 16 y 8 bits (indexado con PLTE de 256 los de Grafx2, con `tEXt` y un chunk auxiliar privado `crNg`; indexado con PLTE y tRNS de 32 entradas y RGBA con alfa 0 y 255, los de LibreSprite).
- **Qué se hashea.** El hash lo calculó el usuario con una herramienta independiente sobre la imagen expandida a RGBA de 8 bits con alfa directo (sin premultiplicar). Concretamente: SHA-256 de `ancho × alto × 4` bytes, fila a fila de arriba abajo y de izquierda a derecha, **sin cabecera ni dimensiones**. *Comprobado al escribir esta spec*: un decodificador independiente (Python, en el scratchpad, fuera del repo) da exactamente los cuatro hashes de los `.sha256` con esta definición, y `read_png` abre los cuatro con informe vacío y la misma vista.
- **La vista RGBA de una `Image`**, con `get_pixel`: `Rgba(c)` → `[r, g, b, a]`; `Indexed(i)` → `palette.resolved(i)` como `[r, g, b, a]`; `Gray(g)` → `[g, g, g, 255]`; `GrayAlpha { gray, alpha }` → `[gray, gray, gray, alpha]`. El RGB bajo alfa 0 se conserva.
- **Formato del `.sha256`.** El contenido, recortados los espacios y saltos de línea (LF o CRLF) de los extremos, son exactamente 64 dígitos hexadecimales (mayúsculas o minúsculas). Cualquier otra cosa (un nombre detrás, otro hash en la segunda línea) es un hash mal formado.
- **SHA-256 sin dependencia nueva.** Se implementa en un módulo de ayuda de los tests, `tests/common/sha256.rs` (FIPS 180-4: unas 40 líneas, solo `std`, sin `unsafe`), y se valida con los vectores del estándar y con longitudes de frontera (fila 15). Alternativa descartada: la crate `sha2` (MIT OR Apache-2.0, pasaría `cargo deny`, pero arrastra varias crates de la familia RustCrypto para ~40 líneas de test; ver decisión 2). Ninguna dependencia nueva, así que no hay cambio en `deny.toml` ni nada que anotar salvo la fila de `proptest`.
- **El verificador es una función pura** `verificar_corpus(entradas)` sobre la lista en memoria de todos los ficheros de la carpeta (nombre y bytes), de la que agrupa PNG y `.sha256` (así detecta también los huérfanos y los ajenos; si la carpeta no existe cuenta como vacía); el test de la carpeta real solo lee el directorio y la llama, y el verificador tiene sus propios tests (fila 14). Por cada fichero, el test imprime su informe de conversiones (`{:?}` de `report`, visible con `--nocapture`), para depurar. Devuelve `Err(String)` con **todos** los problemas juntos si:
  - no hay ningún PNG: «golden/png no contiene ningún PNG: el corpus de interoperabilidad no puede estar vacío (ver golden/png/README.md)»; el test **falla**, nunca pasa en vacío;
  - un PNG no tiene `.sha256` (salvo los de 16 bits, que no lo necesitan) o un `.sha256` no tiene PNG;
  - hay un fichero que no es `README.md`, `*.png` ni `*.png.sha256`;
  - el `.sha256` está mal formado, `read_png` da `Err` o el hash no coincide (el mensaje nombra el fichero y los dos hashes);
  - no queda ningún PNG comprobado (todos de 16 bits).
- **PNG de 16 bits.** Se detectan por el byte de profundidad del IHDR (offset 24). No se hashean (el redondeo puede diferir entre herramientas), pero **sí se leen**: deben dar `Ok` con `Depth16Reduced` en el informe. Se listan como «omitidos del hash» en la salida del test (`--nocapture`), no en silencio. Hoy no hay ninguno.

### 5. Windows: `.gitattributes`

Contenido propuesto (decisión 5):

```
# Los ficheros dorados son datos: git no debe convertir finales de línea ni
# hacer diff de texto, o el CI de Windows los alteraría al hacer checkout.
golden/** binary
golden/**/*.md text diff
```

`binary` equivale a `-diff -merge -text`. La segunda línea va **después** (en `.gitattributes` gana la última regla que casa) y deja el README legible en los diffs. Afecta solo a `golden/`: no es el `* text=auto eol=lf` global que `specs/hito-0-ci.md` dejaba como unidad aparte si `fmt --check` fallara en Windows. Se commitea en la misma unidad que los PNG. Hoy no hay ningún `.gitattributes`. Un test de estructura (fila 16) comprueba que existe y que cubre `golden/`; la comprobación real, local, es `git check-attr` (criterios de aceptación).

### 6. `golden/png/README.md`

Una tabla con cada fichero y el programa que lo guardó, más la procedencia y la licencia. Texto aprobado (las versiones son las de `pacman -Q`; el autor ya comprobó procedencia y metadatos, y no se abren los PNG para inspeccionarlos):

```markdown
# golden/png

Corpus de interoperabilidad de PNG (`specs/hito-1-png-corpus.md`). Cada `X.png` lleva al lado `X.png.sha256`: SHA-256 de la imagen expandida a RGBA de 8 bits (alfa directo, sin cabecera), calculado con una herramienta independiente.

| Fichero | Guardado con |
| --- | --- |
| grafx2-golden1.png | Grafx2 2.9-2 |
| grafx2-golden2.png | Grafx2 2.9-2 |
| libresprite-golden-indexed.png | LibreSprite 1.3-1 |
| libresprite-golden-rgba.png | LibreSprite 1.3-1 |

## Origen y licencia

Son imágenes propias del autor del proyecto, guardadas con Grafx2 y LibreSprite (versiones tal como las da `pacman -Q`) y publicadas bajo la licencia MIT del repositorio.
```

### 7. Spec de PNG

`specs/hito-1-png.md` no tenía una sección «Para el hito 2» (solo un párrafo dentro de la decisión 2). Se añade la sección antes de «Decisiones», con el pico de memoria al escribir y la idea del escritor por flujo, y se deja intacto el párrafo de la decisión 2. **Ya está hecho** en esta revisión (es solo texto de spec). Dato verificado en el código: `write_png` guarda la `Image` (hasta `MAX_BYTES` = 1 GiB), copia todas las muestras a un búfer contiguo (otro tanto) y encodea sobre un `Vec` de salida, así que el pico es de unos 2 GiB más el fichero comprimido.

## Casos

Notación como en la spec de PNG. «Exacto» = `Ok`, informe vacío e imagen igual a `normalizada(img)`.

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | Los cuatro modos en 1 × 1, 3 × 3, 1 × 300 y 300 × 1 con píxeles distintos (patrón determinista); el indexado con paleta de **1** entrada | Exacto |
| 2 | `Indexed` 256 × 1, paleta de **256** entradas, índices `0..=255` | Exacto; `palette().len() == 256` |
| 3 | `Indexed` con colores duplicados: `[(9,9,9,255), (9,9,9,255), (9,9,9,128), (9,9,9,255)]`, píxeles `[0,1,2,3]` | Exacto; la paleta sigue con 4 entradas y los índices no se fusionan |
| 4 | Alfa **0, 1, 254 y 255**, un píxel por valor, en `GrayAlpha`, `Rgba` y en las entradas de una paleta; con RGB y gris distintos de 0 incluso con alfa 0 | Exacto; nada premultiplicado, RGB intacto con `a = 0` |
| 5 | `Indexed` con `transparent_index = Some(0)`, `Some(2)` (última de 3) y `Some(0)` en paleta de 1; una entrada con alfa 200 marcada transparente | `Ok`, informe vacío; imagen = `normalizada(img)`: entrada con `a = 0` y `transparent_index() == None` |
| 6 | **Propiedad**: imágenes aleatorias en los cuatro modos (ver «Tests») | Exacto |
| 7 | Gris de 1, 2 y 4 bits, todos los valores; gris 2 bits `[0,1,2,3]` | `Gray` escalado (`[0,255]`; `[0,85,170,255]`; `v * 17`); informe `[DepthExpanded { from_bits }]` |
| 8 | 16 bits con muestras `[0x0000, 0x0080, 0x0081, 0x01FF, 0xFFFF]` en gris, gris con alfa y RGBA; RGB 16 `(0x0102, 0x0304, 0xFFFF)` | `Gray([0,0,1,2,255])` y la misma regla por canal, informe `[Depth16Reduced]`; el RGB da `Rgba(1,3,255,255)` con `[Depth16Reduced, RgbToRgba]` |
| 9 | RGB 8 `(1,2,3)`; **tRNS**: gris 8 con clave 7 y `[7, 8]`; gris 2 bits con clave 1 y `[1, 2]`; RGB 8 con clave `(1,2,3)` y `(1,2,3)`, `(1,2,4)`; RGB 16 con tRNS | `Rgba(1,2,3,255)` con `[RgbToRgba]`; `[{7,0},{8,255}]` con `[ColorKeyToAlpha]`; `[{85,0},{170,255}]` con `[DepthExpanded{2}, ColorKeyToAlpha]`; `[Rgba(1,2,3,0), Rgba(1,2,4,255)]` con `[RgbToRgba, ColorKeyToAlpha]`; el de 16 bits con `[Depth16Reduced, RgbToRgba, ColorKeyToAlpha]` |
| 10 | Un PNG escrito por `write_png` (indexado 4 × 4): cada prefijo estricto (0 a `len − 1`); CRC erróneo en IHDR, en IDAT y en IEND; más `b"x"`; más un `tEXt` completo | `Err(Decode(_))` en los prefijos y los CRC; `Err(TrailingData)` en los dos últimos. Nunca `Ok` |
| 11 | Indexado 8 bits con PLTE de 2 y un píxel 5; indexado sin PLTE; tRNS de gris de 1 byte; tRNS de RGB de 4 bytes; tRNS de 3 entradas con PLTE de 2 | `Err(PaletteIndexOutOfRange { index: 5, len: 2 })`; `Err(InvalidPalette)`; `Err(InvalidTrns)` (los tres últimos) |
| 12 | Gris 8 de 32769 × 32768 y de 100000 × 100000 con IDAT mínimo (sin reservar nada, sin pánico) | `Err(TooLarge { width, height })` con las dimensiones del fichero |
| 13 | Cada `golden/png/*.png` (salvo 16 bits) | `Ok`; la vista RGBA de la `Image` tiene el SHA-256 del `.sha256` |
| 14 | El verificador con listas en memoria: vacía; PNG sin `.sha256`; `.sha256` sin PNG; fichero ajeno; hash mal formado (63 dígitos; no hex; un nombre detrás del hash); hash con un dígito cambiado; solo PNG de 16 bits; y un PNG de 16 bits sin `.sha256` junto a uno válido | `Err` con el mensaje esperado en cada uno (el vacío, con el texto de arriba); el último, `Ok` y el de 16 bits listado como omitido. Prueba que el corpus **sabe fallar** |
| 15 | SHA-256 de `""`, `"abc"`, los 56 bytes `abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq` (dos bloques de relleno), 1 000 000 × `a`, y de 0, 3, 55, 56, 64 y 1000 bytes `a` | Los resúmenes de los vectores de FIPS 180-4 / NIST; los de las longitudes sueltas calculados con `sha256sum` del sistema y copiados al test (55, 56 y 64 son las fronteras del relleno) |
| 16 | Estructura del repo | `.gitattributes` existe y una línea cubre `golden/` con `binary`; `golden/png/README.md` nombra cada `*.png` del directorio y contiene «MIT» |

## Tests

- **Unitarios**: uno por fila de la tabla, salvo la 6 (una o varias aserciones cada uno).
- **Propiedad mínima** (fila 6, `proptest`, 256 casos por defecto): modo cualquiera de los cuatro; forma elegida entre 1 × 1, 1 × N, N × 1 (N de 2 a 300) y A × B (de 1 a 48 de lado); en indexado, paleta de 1 a 256 entradas con colores sacados a veces de un grupo de 3 (para que haya duplicados) y a veces aleatorios, alfa tomado de `{0, 1, 254, 255}` o cualquiera, y `transparent_index` o no; en `Rgba` y `GrayAlpha` con las mismas reglas de alfa y RGB cualquiera; píxeles cualesquiera. Aserción: la de la fila 1. Si `proptest` genera `*.proptest-regressions` en `px-golden`, se commitea (como en `px-formats`). Solapa a propósito con la propiedad 1 de la unidad 5 (allí, lados de 1 a 8): aquí es la puerta del hito y añade las formas alargadas y las paletas de hasta 256 con duplicados.
- La fila 15 va aparte de la 13 para que, si falla el hash, se sepa si es el SHA-256 propio o el PNG.

## Criterios de aceptación

Desde la raíz, en Linux (el CI cubre los otros dos sistemas, y es donde cuenta el `.gitattributes`):

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` y `cargo deny check` (no cambia nada: sin crates nuevas).
- **El corpus protege.** Una vez en verde, comprobado a mano y sin commitear: (a) cambiar un dígito de un `.sha256` hace fallar `png_interop`, con el nombre del fichero; (b) mover `golden/png/*.png` fuera hace fallar `png_interop` con el mensaje de corpus vacío; (c) alterar un píxel de un PNG hace fallar la comparación. Después se restaura todo (`git status` limpio salvo lo de la unidad).
- `git check-attr -a golden/png/grafx2-golden1.png` muestra `binary` (`-diff`, `-merge`, `-text`); y, con los ficheros ya añadidos al índice, `git ls-files --eol golden/png` los marca `attr/-text`.
- `license-auditor` sobre los ficheros nuevos (`golden/png/*`, README, tests) antes del commit.
- Sin `unsafe`. Commits pequeños: (1) esta spec, `.gitattributes` y `golden/png/` (PNG, `.sha256` y README) en el mismo commit; (2) el SHA-256 y el verificador con sus tests; (3) los puntos 1 a 3. Cada uno con fmt, clippy y test.

## Fuera de alcance

- Más ficheros de terceros o PNG de 16 bits en `golden/` (se pueden añadir sin tocar el código: basta el PNG, su `.sha256` y una fila en el README).
- Escribir con otros programas desde el código; leer los PNG con otro decodificador en el test.
- GIF, BMP, PCX o IFF (hitos 5 y 6).

## Decisiones que necesito que confirmes

1. **Tests en `px-golden`**, con `px-core`, `px-formats` y `proptest` como dev-dependencies de esa crate (hoy no tiene ninguna) y sin código en `src/lib.rs`. Alternativa: dejarlos en `px-formats/tests` (más cerca del código, pero el corpus «dorado» vive en `px-golden` según la arquitectura).
2. **SHA-256 propio en el test** (~40 líneas, validado con vectores NIST y con el `sha256sum` del sistema) en lugar de la crate `sha2`. Recomendado: cero dependencias nuevas. Con `sha2` habría que pasar `cargo deny`, anotar sus transitivas en `docs/DEPENDENCIES.md` y se ganaría solo no mantener 40 líneas de test.
3. **Constructor de PNG duplicado** en `px-golden/tests/common` (copia del de la unidad 5) en vez de compartirlo. Un helper compartido tendría que vivir en `px-golden/src` o en una crate de utilidades; no merece la pena todavía. Si luego hace falta un tercer uso, se unifica.
4. **Qué entra en la comparación.** Hash solo de PNG de 8 bits (los de 16 se leen pero no se hashean) y fallo si no queda ninguno; el informe de los ficheros del corpus **no se exige vacío** (solo el hash), aunque hoy lo está en los cuatro. Y un fichero ajeno en `golden/png/` es un fallo, para que nada quede sin comprobar. ¿Aceptas ser así de estricto?
5. **`.gitattributes`**: `golden/** binary` más la excepción `golden/**/*.md text diff` para que el README se pueda revisar en los diffs. Alternativa: solo la primera línea (el README quedaría como binario).
6. **README: datos de procedencia.** Pongo Grafx2 2.9-2 y LibreSprite 1.3-1 (`pacman -Q` de hoy) y asigno cada fichero al programa por el prefijo de su nombre. Confirma que son las versiones con que los guardaste, que las imágenes son tuyas y que el texto de licencia del README dice lo que quieres.
7. **Alcance de las pruebas de error y conversión**: una muestra representativa por fila (el detalle completo está en `px-formats`, unidad 5), no una réplica de toda su tabla. Si prefieres duplicarla entera aquí, avísame: son unas 40 filas más.

Dos observaciones que no son decisiones: la sección «Para el hito 2» de la spec de PNG ya está añadida (punto 7), y hasta que apruebes no he creado `.gitattributes` ni `golden/png/README.md`.
