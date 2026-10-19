# Hito 2 · Unidad 13b: chunks auxiliares del PNG en el informe de lectura

Estado: **aprobada** (2026-10-19; con las respuestas del visto bueno ya incorporadas al texto, que **prevalecen** sobre cualquier versión anterior: D1 con una aserción más en la prueba adaptada; `cICP` por primarios y transferencia; `iCCP` siempre, sin heurísticas por el nombre del perfil; sin tope de chunks ni CRC, con el coste medido; el aviso lista solo texto, Exif, animación y otros; APNG activa la copia). Cambios: `px-formats` y `px-app` (nada en `main.rs`). **No se toca `px-core` ni `px-raster`; `write_png` no cambia.** Sin dependencias nuevas ni `unsafe`. Base: [`hito-1-png.md`](hito-1-png.md) (lectura, «Limitaciones»), [`hito-2-abrir-guardar.md`](hito-2-abrir-guardar.md) (copia por conversión, avisos), [`hito-2-dialogos-archivo.md`](hito-2-dialogos-archivo.md) (`save_to`, `describe_saved_dialog`) y D9 de [`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md).

## Objetivo

`read_png` lista los chunks auxiliares que ignora, sin cambiar un solo valor de píxel. Si el fichero trae información de color que Pixoko descarta (con `iCCP` siempre) o es un APNG (se edita solo el primer fotograma), `Ctrl+S` guarda una copia, como con las conversiones, y el aviso dice por qué. El resto solo se informa.

## Lo comprobado

Con un programa suelto fuera del repo (borrado) sobre `read_png` actual, PNG 1×1 hechos a mano:

- **CRC erróneo o longitud imposible en un auxiliar** (`gAMA` de 3 bytes, `iCCP` y `tEXt` con CRC malo): `Ok`, sin aviso. El crate los omite por defecto (`skip_ancillary_crc_failures`).
- **Chunk que declara 2³¹−1 bytes con el fichero corto** (`eXIf`, `iCCP`, `tEXt`, desconocido, tras el IDAT): `Err(Decode("PNG truncado"))` en microsegundos, sin reservar: lo corta `scan_chunks` (aritmética comprobada, solo trabaja con trozos del búfer).
- **Un `eXIf` completo de 70 MiB da `Err(Decode("limits are exceeded"))`**: el crate lo lee entero y no se puede saltar (su propio `TODO`). Un `tEXt`, `iCCP` o desconocido de 70 MiB da `Ok` (los salta). Es de hoy y no cambia; ver «Riesgos».
- **200 000 chunks `tEXt`** seguidos: `Ok` en 5 ms.
- **`cICP` en el crate `png` 0.18.1** (leído en su fuente al escribir las pruebas): rechaza matriz ≠ 0, rango fuera de 0 y 1, duplicado y uno después de PLTE o IDAT (`Err(Decode)`). Es de hoy y conforme a la especificación (la matriz debe ser 0).
- **Coste del recorrido con el fichero más grande permitido** (`MAX_FILE_BYTES`, 256 MiB; `--release`): el peor caso son chunks vacíos, 22 369 615 en 255 MiB. `read_png` de hoy tarda **544 ms** (casi todo, del crate, que procesa cada chunk). El bucle propuesto (leer la cabecera de cada chunk y clasificar el tipo, sin CRC ni reservas) cuesta **39 ms** por sí solo, un 7 % de eso. Con un único `tEXt` de 256 MiB, `read_png` tarda 27 ms y el bucle, 60 ns. Con el código real (mejor de 3, mismo equipo): **556 ms frente a 536 ms** sin el recorrido nuevo, es decir, unos 20 ms más (un 4 %); con el único `tEXt` de 256 MiB, 27 ms en los dos.
- **Corpus de `golden/png`** (recorrido de chunks): los dos de Grafx2 llevan `tEXt` y `crNg` (desconocido); los dos de LibreSprite, ninguno. O sea que el informe nuevo tendrá contenido real.
- Anchura de la barra, calculada con `min_window_width_u` (468 u): a la ventana mínima la barra tiene **66 columnas en las cinco escalas** (1,0, 1,25, 1,5, 2,0 y 3,0). Sin el prefijo `aviso: ` (7) quedan **59** para el texto. El pie de los diálogos (44 columnas) no interviene: esta unidad no abre ningún diálogo.

## Decisiones

**D1. Forma del informe: campo nuevo `aux` en `PngRead`** (no una lista aparte). `PngRead { image, report, aux }`; `report` y `Conversion` no cambian, y `is_exact()` sigue siendo `report.is_empty()` (un PNG con `pHYs` sigue siendo exacto en valores). Es aditivo para quien lee; **una prueba anterior cambia** (ver más abajo) porque comparaba el `PngRead` entero. Alternativa: una función `read_png_aux(bytes) -> (PngRead, AuxReport)` y `read_png` intacto: cero pruebas tocadas, pero dos puertas de entrada y la información se pierde con solo llamar a la equivocada. **Recomiendo el campo.** Meter los chunks en `report` (`Conversion`) queda descartado: `is_exact` pasaría a falso con cualquier `pHYs` y el `!report.is_empty()` de la app forzaría copia con un `tEXt`.

**D2. Qué activa la copia (`appearance`).** Regla única, evaluada en el orden de la lista: `iCCP` va primero porque «siempre» activa la copia (aunque haya un `cICP` sRGB, que según la especificación lo desplazaría); entre los demás rige el orden de la especificación (cICP > sRGB > gAMA/cHRM). Modelo de «¿se vería distinto en un visor que respete el chunk?»:

1. `iCCP` presente (cualquier longitud) ⇒ `Icc`. Siempre.
2. Si no, `cICP` de 4 bytes: con primarios (primer byte) distintos de 1 o transferencia (segundo) distinta de 13, es decir, no sRGB ⇒ `CodePoints`; con 1 y 13 ⇒ nada y se acaba. Matriz y rango no cuentan: el crate `png` ya rechaza un `cICP` con matriz distinta de 0 o rango distinto de 0 y 1, un duplicado o uno tras PLTE o IDAT (`Err(Decode)`, de hoy), así que nunca llegan al informe.
3. Si no, `sRGB` de 1 byte ⇒ nada y se acaba (el fichero declara sRGB; `gAMA` y `cHRM` quedan subordinados).
4. Si no, `gAMA` de 4 bytes con |valor − 45455| > **1000** ⇒ `Gamma` (cubre 1/2,2 y excluye 1,0 y 1/1,8); si no, `cHRM` de 32 bytes con algún valor a más de **1000** (0,01) de blanco D65 y primarios de sRGB (31270, 32900; 64000, 33000; 30000, 60000; 15000, 6000) ⇒ `Chromaticities`. Si ambos difieren, `Gamma`.

Por qué: `sRGB` y un `gAMA`/`cHRM` de sRGB no llevan nada que se pierda (sin etiqueta, todo visor asume sRGB), y copiar por ellos sería ruido en casi cualquier PNG de una herramienta. Una gamma lineal o unos primarios Adobe RGB o Display P3 sí cambian lo que se ve. `cICP` no estaba en tu lista; lo trato como `iCCP` porque la Tercera Edición (la referencia de la unidad 5) lo pone por delante. La tolerancia de 1000 es elección nuestra, no de la especificación: 45455 ± 1000 equivale a una gamma de 2,15 a 2,25. **`iCCP` activa siempre la copia, aunque el perfil sea sRGB** (falso positivo aceptado): no se mira el nombre ni el contenido del perfil.

Además, **`acTL` presente (APNG) ⇒ `Animation`**, sea cual sea el color (D6). Si coinciden, manda `Animation` (perder la animación es más grave que perder el color); la copia es una sola y el texto nombra esa causa. Un chunk de tamaño imposible no se interpreta: cuenta como `Other` y no activa nada. Eso vale para los cuatro cuyo valor se lee (`gAMA` ≠ 4 bytes, `cHRM` ≠ 32, `sRGB` ≠ 1, `cICP` ≠ 4); los de solo presencia (`iCCP`, `pHYs`, `tIME`, texto, `eXIf`, `acTL`, `fcTL`, `fdAT`) no comprueban la longitud (un `acTL` con longitud rara activa igualmente la copia: va hacia lo seguro). Cada chunk se clasifica por separado: en repetidos, el valor lo da el primero **con longitud válida**, y los de longitud mala cuentan como `Other`.

**D3. Seguridad.**
- **Ninguna reserva sale de una longitud declarada.** El escaneo trabaja con trozos del búfer (`scan_chunks`), como hoy. Se leen como mucho **32 bytes** de un chunk (`cHRM`); `iCCP`, texto y `eXIf` solo se miran por su tipo, sin descomprimir nada.
- **El informe tiene tamaño constante:** un tipo aparece una vez (`found` como mucho 11 entradas), sin contadores por chunk.
- **Sin tope al número de chunks inspeccionados** (aprobado): inspeccionar cada uno cuesta O(1) de tiempo y nada de memoria, y el recorrido hasta IEND ya existe para detectar `TrailingData`, así que un tope no ahorra nada. Y abriría un agujero: 1025 chunks de texto delante de un `iCCP` lo esconderían y se sobrescribiría el original sin avisar. Medido: 22,4 millones de chunks en 255 MiB, 39 ms (ver «Lo comprobado»).
- **Truncados, repetidos, CRC erróneo:** truncado ⇒ el mismo `Err(Decode)` de hoy (antes de clasificar nada); repetidos ⇒ vale el primero y el tipo se lista una vez; **CRC erróneo ⇒ no se verifica** (sin CRC propio: ni coste ni pánico posible); el chunk se lista igual. Error posible: un `iCCP` dañado fuerza una copia que no hacía falta. Va siempre hacia la copia, nunca hacia pisar el original.

**D4. Cómo se muestra** (`Notice` de 5 s de siempre, mismo texto en la barra, con `aviso: `, y en el título; sin marcador permanente):
- **Lo accionable va primero** y cabe entero: `perfil de color ICC ignorado; Ctrl+S guardará una copia` (55 columnas ≤ 59), seguido de ` · ` y el aviso de siempre (`abierto casa.png · 16×16 · RGBA`), que es lo que se recorta por la cola si la ventana es mínima. Las otras causas: `espacio de color cICP ignorado` (57 con la coletilla), `gamma no sRGB ignorada` (49), `primarios no sRGB ignorados` (54) y, para APNG, **`APNG: solo el 1.er fotograma; Ctrl+S guardará una copia`** (55; el texto que proponías, `animación ignorada: solo se edita el primer fotograma; …`, mide 80 y no cabe en 59). Se muestra el primer fotograma: es la imagen por defecto del IDAT (unidad 5).
- **Con conversiones y causa:** `{causa}; Ctrl+S guardará una copia · abierto … · convertido: …` (sin repetir la coletilla).
- **Solo informar:** al final del texto, también cuando hay causa y, tras el aviso de siempre, ` · ignorados: texto, Exif, animación, otros` (los que haya, en ese orden; como mucho 4 rótulos: 40 columnas, 43 con el separador ` · `). **El aviso lista solo `Text`, `Exif`, `Animation` y `Other`**: ni los chunks de color (con causa ya van en la causa; sin ella no llevan nada), ni `Physical` (`pHYs`) ni `Time` (`tIME`), que se quedan en `aux`. `Animation` sale aquí solo con `fcTL` o `fdAT` sin `acTL` (un APNG roto), que no activa la copia.
- **Al guardar:** con causa **única** y sin conversiones, `perfil de color ICC ignorado; guardado como casa-copia.png` (44 + nombre; y para APNG, `APNG: solo el 1.er fotograma; guardado como casa-copia.png`, 44 + nombre; el nombre entero cabe hasta 15 caracteres, y si no, se recorta el nombre, no la causa), en `Ctrl+S` (`describe_saved`) y en el diálogo (`describe_saved_dialog`). Con conversiones, la forma larga de hoy con la causa en la lista: `guardado como X-copia.png: el original se convierte al abrir (RGB convertido a RGBA, perfil de color ICC ignorado) y no se sobrescribe` (se recorta en la barra y sale entero en el título, como ya pasa). `Saved` no cambia (sus literales están en pruebas anteriores): la forma corta se elige cuando `copy_reason` es exactamente una de las cinco causas.

**D5. Pruebas.** PNG hechos a mano en el test con el ayudante `trozo` (CRC válido) y con basura (CRC erróneo, tipos con minúscula inicial y bytes no alfabéticos, longitudes 0 o 2³¹−1 con el fichero corto, repetidos, antes y después del IDAT); sin ficheros nuevos en el repo.

**D6. APNG activa la copia** (decidido en el visto bueno): `acTL` presente ⇒ `appearance = Animation` y `Ctrl+S` guarda una copia, porque sobrescribir dejaría un único fotograma. `AuxChunk::Animation` cubre `acTL`, `fcTL` y `fdAT`; solo `acTL` activa la copia.

## Piezas

```rust
// px-formats (aditivo; PngRead gana un campo, Conversion y write_png no cambian)
/// Orden de declaración = orden de `found`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)] #[non_exhaustive]
pub enum AuxChunk { Icc, CodePoints, Srgb, Gamma, Chromaticities, Physical, Text, Time, Exif, Animation, Other }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuxReport { pub found: Vec<AuxChunk>, pub appearance: Option<AuxChunk> }  // appearance: Animation | Icc | CodePoints | Gamma | Chromaticities
pub struct PngRead { pub image: Image, pub report: Vec<Conversion>, pub aux: AuxReport }
// iCCP→Icc; cICP→CodePoints; sRGB→Srgb; gAMA→Gamma; cHRM→Chromaticities; pHYs→Physical; tEXt/zTXt/iTXt→Text;
// tIME→Time; eXIf→Exif; acTL/fcTL/fdAT→Animation; cualquier otro auxiliar (minúscula inicial, salvo tRNS) y los
// de tamaño imposible →Other. Los críticos desconocidos ya son Err(Decode).

// px-app/files.rs
pub fn describe_open_aux(name: &str, image: &Image, report: &[Conversion], aux: &AuxReport) -> String;
//   describe_open(..) = describe_open_aux(.., &AuxReport::default()); describe_report no cambia.
pub fn aux_cause(chunk: AuxChunk) -> &'static str;     // las cinco causas; otro ⇒ "chunk ignorado"
// px-app/session.rs: Session::open usa describe_open_aux; la razón de copia = conversiones + causa, separadas por «, »;
// describe_saved y describe_saved_dialog: forma corta si copy_reason es una causa exacta.
```

## Pruebas anteriores que cambian (medido en una copia; commit propio antes de las pruebas nuevas)

1. **`px-formats/tests/png.rs::chunks_auxiliares_no_cambian_imagen_ni_informe`**: comparaba `lee(con) == lee(sin)` (el `PngRead` entero) para comprobar que los auxiliares no cambian ni imagen ni informe. Con `aux` el entero ya no coincide (es justo lo que se añade). Pasa a `assert_eq!` de `image` y de `report` por separado, y **gana una aserción**: `aux.found` refleja los siete chunks que inserta el test (`Icc, Srgb, Gamma, Physical, Text, Time, Other`, en el orden de declaración). Conserva su intención (los auxiliares no cambian ni imagen ni informe) y no se afloja.
2. **`px-app/src/session/tests.rs::fila_2_abrir_el_corpus`** (línea 303, por lectura del código; no medido): el texto esperado era `describe_open(name, image, &report)`; con Grafx2 (`tEXt` y `crNg`) ahora lleva ` · ignorados: texto, otros`. Pasa a `describe_open_aux(.., &expected.aux)`. Es el mismo aserto con la forma completa; la fila 9 de la unidad 15 (`describe_open` con tres argumentos) no cambia.

Ninguna necesita más que eso. Medido: el resto de `px-formats` y `px-golden` pasa con `aux` relleno (el fallo de `structure` en la copia es de la copia, sin `docs/`). Si al implementar sale otra, me paro.

## Cambios en otras specs

`hito-1-png.md` («Limitaciones» y decisión 1: el informe no lista los chunks ignorados; APNG «sin aviso») y `hito-2-abrir-guardar.md` («Limitaciones»: «no hay aviso») dejan de ser ciertas para `aux`: `report` sigue contando solo conversiones de valores. Un APNG ya no se abre «sin aviso»: activa la copia.

## Limitaciones y riesgos

- **Falso positivo frecuente de `iCCP`:** GIMP, Krita o Photoshop guardan a menudo un perfil sRGB dentro de `iCCP`; no se distingue sin descomprimir el perfil (no hay `flate2` directo, y es más superficie sobre datos del fichero). Cuesta una copia de más, nunca una pérdida.
- **`eXIf` de más de 64 MiB da error** (límite del crate, de hoy): un PNG válido con Exif enorme no se abre. No se toca aquí.
- **CRC no verificado** en los auxiliares del informe (D3).
- Tras los 5 s el aviso desaparece y no queda marcador: quien no lo vea encontrará `X-copia.png` al lado; el aviso de guardar lo repite.
- Los chunks siguen perdiéndose al guardar (`write_png` no cambia); esto solo avisa y protege el original frente a la pérdida de color.
- Los textos con nombre de fichero largo se recortan por la cola, como los de hoy.

## Casos límite

`png(...)` se construye a mano con `trozo`; `A` es el `AuxReport`. Una fila agrupa casos parecidos.

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | PNG sin auxiliares (RGBA, indexado); corpus `golden/png` (Grafx2 ×2; LibreSprite ×2) | `A` vacío y `appearance None`; Grafx2: `found [Text, Other]`, `appearance None`; LibreSprite: vacío |
| 2 | Cada tipo válido por separado: `iCCP`; `cICP` (1,13,0,1); `sRGB`; `gAMA` 45455; `cHRM` de sRGB; `pHYs`; `tEXt`, `zTXt`, `iTXt`; `tIME`; `eXIf`; `acTL`; `teSt` | `found` con solo su variante (`Text` para los tres de texto, `Animation` para `acTL`); `appearance` solo con `iCCP` (`Icc`) y `acTL` (`Animation`) |
| 3 | `tEXt, iCCP, tEXt, gAMA, iCCP`, antes o después del IDAT; y 2000 `tEXt` con un `iCCP` al final (antes del IDAT) | `found [Icc, Gamma, Text]` en los dos sitios, sin repetidos y en el orden de declaración; `found [Icc, Text]` con `appearance Icc` en el segundo (ningún tope lo esconde) |
| 4 | Causa, sin `sRGB`: `iCCP`; `iCCP` + `sRGB`; `iCCP` + `cICP` sRGB; `cICP` (9,16,0,1), (1,16,0,1) y (9,13,0,1); `cICP` (1,13,0,0) + `gAMA` 100000; `cICP` no sRGB + `sRGB`; `sRGB` + `gAMA` 100000 + `cHRM` Adobe; `gAMA` 100000; `gAMA` 44455, 46455, 46456; `cHRM` Adobe RGB; `cHRM` de sRGB ±1000 y ±1001 en un valor; `gAMA` y `cHRM` raros | `Icc`; `Icc`; `Icc` (manda `iCCP`); `CodePoints` las tres; `None` (el rango no cuenta); `CodePoints` (el `cICP` va antes que `sRGB`); `None`; `Gamma`; `None`, `None`, `Gamma`; `Chromaticities`; `None` y `Chromaticities`; `Gamma` |
| 5 | Longitud imposible: `gAMA` de 3, `cHRM` de 31, `sRGB` de 2, `cICP` de 5; `iCCP` de 0 bytes | `found [Other]` y `appearance None`; `iCCP` vacío ⇒ `Icc` |
| 6 | Basura: `CRC` erróneo en `iCCP`, `gAMA` y `tEXt`; tipos `zzZz` y de bytes no alfabéticos con minúscula inicial (`\x61\x01\x02\x03`); longitud 0; chunk que declara 2³¹−1 con el fichero corto | las de CRC erróneo, `Ok` y listados como si fueran buenos; las de longitud gigante, `Err(Decode)` sin reservar (como hoy); nunca pánico |
| 7 | APNG (`acTL`, `fcTL` y `fdAT` como en la unidad 5) solo; APNG + `iCCP`; `fcTL` sin `acTL`; APNG de 2 cuadros con `Session::open` y `save` | `found [Animation]` y `appearance Animation`; `found [Icc, Animation]` y `Animation` (manda); `found [Animation]` y `appearance None`; la imagen es la del IDAT (primer fotograma), `converted()` cierto, el texto empieza por `APNG: solo el 1.er fotograma; Ctrl+S guardará una copia`, `save` escribe `x-copia.png` con `copy_reason Some("APNG: solo el 1.er fotograma")` y `describe_saved` = `APNG: solo el 1.er fotograma; guardado como x-copia.png` |
| 8 | `describe_open_aux` (con `casa.png`, 16×16, RGBA): `A` vacío; `iCCP`; `iCCP` + conversión de gris de 4 bits; solo `tEXt` y `tIME`; `pHYs`+`tIME`; `sRGB`+`gAMA` 45455+`cHRM` sRGB; `tEXt`, `eXIf`, `fcTL` y un desconocido; las otras cuatro causas | igual que `describe_open` de hoy; `perfil de color ICC ignorado; Ctrl+S guardará una copia · abierto casa.png · 16×16 · RGBA`; con la causa delante y `· convertido: gris de 4 bits ampliado a 8` detrás **sin** segunda coletilla; `… · ignorados: texto`; igual que sin ellos (ni resolución ni fecha); igual que sin ellos; `… · ignorados: texto, Exif, animación, otros`; los textos de D4 |
| 9 | `Session::open` de un RGBA 1×1 con `iCCP` y `save` (y luego otro `save`; y `save_to` con la misma ruta, y con una equivalente) | `converted()` cierto; el texto empieza por la causa; escribe `x-copia.png` con el original intacto byte a byte; `copy_reason == Some("perfil de color ICC ignorado")`; `describe_saved` y `describe_saved_dialog` = `perfil de color ICC ignorado; guardado como x-copia.png`; el segundo `save` sobrescribe la copia con `copy_reason None` |
| 10 | `Session::open` solo con `tEXt`, `pHYs`, `tIME`, `eXIf`; y de los dos Grafx2 | `converted()` falso; `save` sobrescribe el original (`copy_reason None`); el texto lleva `ignorados: texto, Exif` (sin resolución ni fecha); Grafx2 `ignorados: texto, otros`; el nombre propuesto por `request_for` no es la copia |
| 11 | `Session::open` de un RGB 1×1 con `iCCP` y `save` | razón `RGB convertido a RGBA, perfil de color ICC ignorado`; `describe_saved` en la forma larga de hoy con esa lista; una sola coletilla al abrir |
| 12 | Anchos: `status::columns` a la ventana mínima en 1,0, 1,25, 1,5, 2,0 y 3,0; las cinco causas con su coletilla; la frase de guardado con nombre de 15 caracteres (ICC y APNG); la cláusula `ignorados:` con los cuatro rótulos | 66 columnas en las cinco; causa + coletilla ≤ 59; frase ≤ 59; cláusula ≤ 40; solo caracteres de `INTERFACE_CHARSET` (salvo el nombre) |

## Propiedades (proptest)

1. **Los auxiliares no cambian la imagen ni el informe.** Para cualquier PNG válido de los cuatro modos y cualquier lista de 0 a 8 auxiliares válidos (tipos de D2, orden y repetición cualesquiera, antes o después del IDAT): `image` y `report` iguales a los del fichero sin ellos, `is_exact` igual, y `write_png` de ambas imágenes da los mismos bytes.
2. **Nunca pánico y el informe cumple su forma.** Un PNG válido con 0 a 8 chunks generados (tipo arbitrario con minúscula inicial, longitud real de 0 a 64 bytes, CRC bueno o malo, y alguno que declara una longitud mayor que el fichero) más 1 a 4 bytes cambiados: `Ok` o `Err`; si `Ok`, `found` ordenado y sin repetidos y `appearance` es `None` o un elemento de `found` entre `Animation`, `Icc`, `CodePoints`, `Gamma` y `Chromaticities`.
3. **`appearance` contra un modelo.** Para cualquier combinación de `acTL`, `iCCP`, `cICP`, `sRGB`, `gAMA` y `cHRM` con valores cerca y lejos de los de sRGB (incluidos los bordes ±1000/±1001) y longitudes buenas y malas: coincide con la regla de D2 escrita aparte en el test.
4. **Textos.** Para cualquier `AuxReport` y `report`: `describe_open_aux` con `A` vacío es `describe_open`; empieza por la causa si y solo si hay `appearance`; contiene una sola vez `Ctrl+S guardará una copia` si hay causa o conversiones, y ninguna si no; `ignorados:` solo nombra `texto`, `Exif`, `animación` y `otros`; la cláusula mide ≤ 40 y todo el texto fijo está en `INTERFACE_CHARSET`.

## Dónde van las pruebas

- `px-formats/tests/png.rs` (módulo hijo `aux_chunks`): filas 1 a 7 y propiedades 1 a 3, con el ayudante `trozo` del propio fichero.
- `px-app/src/files/aux_tests.rs` (filas 8 y 12, propiedad 4) y `px-app/src/session/aux_tests.rs` (filas 9 a 11), como módulos de los que ya existen. El cableado de `main.rs` no cambia: el aviso sale de `Session::open` y `describe_saved*`.

## Prueba manual (la haces tú, sin capturas)

1. Un PNG con `iCCP` (por ejemplo, exportado con GIMP): al abrirlo, barra y título dicen `perfil de color ICC ignorado; Ctrl+S guardará una copia · …` 5 s. `Ctrl+S` guarda `nombre-copia.png`; el aviso dice `perfil de color ICC ignorado; guardado como nombre-copia.png`; el original no cambia; un segundo `Ctrl+S` sobrescribe la copia.
2. `Ctrl+Mayús+S` y elegir el nombre del propio fichero: mismo aviso corto.
3. Un PNG de Grafx2 del corpus: `ignorados: texto, otros`; `Ctrl+S` sobrescribe el original.
4. Un PNG con solo `sRGB`: no sale nada de color. Con `gAMA` lineal y sin `sRGB`: `gamma no sRGB ignorada…`. Con `tEXt`: `ignorados: texto`; con solo `pHYs` o `tIME`: nada.
   Un APNG: `APNG: solo el 1.er fotograma; Ctrl+S guardará una copia`, el lienzo muestra el primer fotograma, y `Ctrl+S` guarda `nombre-copia.png` sin tocar el original.
5. Ventana al mínimo y a 1,0, 1,5 y 2,0: la causa y la coletilla se leen enteras; lo que se recorta es el nombre.

## Para más adelante (no se implementa aquí)

- **`iCCP` fuerza copia en casi cualquier PNG de GIMP o Krita** (perfil sRGB incluido, falso positivo aceptado). Con eso, la decisión 4 de la unidad 16 (elegir en el diálogo la ruta del propio documento no sobrescribe, sino que guarda `X-copia.png`) se vuelve molesta.
- Valorar permitir **sobrescribir de forma explícita** cuando la pérdida es solo de metadatos auxiliares (sin conversión de valores y sin APNG) y el usuario elige la ruta original en el diálogo de guardar del sistema, que ya pregunta si se reemplaza. Hay que decidir qué causas cuentan como «solo metadatos» (`iCCP`, `cICP`, `gAMA` y `cHRM` cambian el color que ve otro visor; `Animation` pierde contenido) y cómo se avisa. Con su propia spec y su visto bueno.
