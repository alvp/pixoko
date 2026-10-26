# Hito 4 · Unidad 4.2: cuantizador de color

Estado: **aprobada** y **implementada** (2026-10-26; a falta del cierre completo, el `license-auditor` y la prueba manual; el usuario delegó todas las decisiones: se confirman las de «Decisiones» tal como están, ver «Visto bueno»). Cambios previstos: `px-core` y `px-raster` (**solo los listados abajo, aprobados de antemano por el usuario para esa lista y no más**, D02), `px-ui`, `px-app` y `px-golden`. Sin dependencias nuevas ni `unsafe`; `docs/DEPENDENCIES.md` no cambia. Código, comentarios y nombres de pruebas nuevos en inglés; textos visibles en español en `quantize_es.rs` (revisión en [`hito-4-cuantizador-textos.md`](hito-4-cuantizador-textos.md)). Sin subagentes (`CLAUDE.md`, presupuesto): las pruebas las escribe la sesión principal antes del código; `license-auditor` queda para el usuario (hay cambios en `px-core` y `px-raster`).

Referencias: fila 4.2 y decisiones D02 y D13 de [`mapa-funciones.md`](mapa-funciones.md); [`hito-2-convertir-modo.md`](hito-2-convertir-modo.md) (diálogo, informe, dos tiempos, `TooManyColors`); [`hito-4-imagen.md`](hito-4-imagen.md) (patrón aditivo: `px-raster` calcula, `px-core` valida y mueve, `based_on`); `convert_cels`/`ConvertAll` (paleta común, el 0 transparente).

## Objetivo

`Imagen → Reducir colores…` lleva un documento **RGBA o gris con alfa** (todas sus capas) a **indexado con N ≤ 256 colores** en **un solo paso de deshacer**, con paleta calculada o ajena, tramado (ninguno, difusión de error u ordenado) y umbral de alfa. `Convertir modo…` a Indexado, cuando la imagen tiene más de 256 colores, deja de ser un callejón: ofrece el botón `Reducir colores…` (como `Aplanar` en 3.5). El cuantizador es determinista (aritmética entera, sin `HashMap`, empates por índice).

## Lo comprobado (en el código)

1. `convert_image`/`convert_cels` ya hacen la conversión **exacta** (hasta 256 colores, paleta por primera aparición, el 0 transparente si hay más de una capa, una celda vacía o un píxel `a = 0`) y dan `TooManyColors` al pasar. El cuantizador no la sustituye: cuando los colores caben, **debe dar lo mismo** (D1).
2. `Command::ConvertAll` y `ReplaceImages` (4.1) intercambian imágenes y plantilla del lienzo en un paso (`Converted`). Ninguno sirve tal cual: `ReplaceImages` exige el modo y la paleta del lienzo, y `ConvertAll` calcula él mismo la imagen.
3. El diálogo de `Convertir modo…` ya tiene el patrón de botón que lanza otra acción (`FIELD_FLATTEN`, `flatten_offer`), y la reserva previa, «Convirtiendo…» y el informe en dos tiempos (`SYNC_PIXELS`, `WAIT_FRAMES`).
4. Indexado exige opacidad 255 y mezcla Normal en todas las capas (`IndexedOpacity`, `IndexedBlend`) y, con más de una capa, un índice transparente.
5. **No comprobado:** que Aseprite tenga las opciones con los nombres que uso en el script de Lua (`ColorQuantization`, `ChangePixelFormat`, tramados). El script `tools/corpus-h4/quantize.lua` está **escrito sin ejecutar**; las dudas van marcadas con «comprobar». Mientras no existan los PNG en `golden/aseprite-quant/`, la prueba contra la referencia (fila 12) está `#[ignore]`, y lo demás no depende de ellos.

## Decisiones (⚠ = decido algo que no me diste; las confirmo abajo)

**D1. Exacto si cabe.** Si, tras el umbral de alfa, la imagen (todas las capas) tiene ≤ N colores distintos (contando el transparente como uno), la paleta es la de `convert_cels` (el 0 transparente si hace falta, el resto por primera aparición recorriendo las celdas de abajo arriba) y el resultado es **idéntico píxel a píxel** al de `ConvertAll` (con `alpha ≥ umbral` igual a `a > 0` cuando el umbral es 1). El tramado no actúa: no hay error que repartir. Si no caben: algoritmo de D2.
**D2. Paleta calculada.** (1) Histograma de 6 bits por canal (262 144 casillas con cuenta y suma de r, g, b en `u64`; solo píxeles opacos). (2) *Median cut* ponderado: se parte la caja de mayor error cuadrático (en `i128`), por el eje de mayor varianza, en la mediana ponderada (empate: la casilla menor), hasta N cajas. (3) Cada caja da la media de sus colores reales (redondeada). (4) **3 iteraciones de Lloyd** sobre las casillas ocupadas (asignar por distancia al cuadrado, recalcular medias; un grupo vacío conserva su centro). (5) El orden de la paleta es el de las cajas por su menor casilla, para que sea estable. N cuenta el transparente si lo hay (si hace falta transparente quedan N − 1 colores y N ≥ 2).
**D3. Paleta ajena.** `PaletteSource::Fixed(Palette)`: la paleta resultante **es** esa paleta (incluido su índice transparente) y N no se usa. Si hace falta transparencia y la paleta no tiene índice transparente: error `FixedNeedsTransparent`. Quién la da: **la última paleta indexada de la sesión** (⚠ la de un documento indexado al abrirlo y la del documento indexado del que se sale al convertir de modo); cargar una paleta desde fichero llega con 4.6 y usará el mismo campo.
**D4. Transparencia y umbral.** `alpha_threshold` de 1 a 255 (por defecto 128): un píxel con `a < umbral` es transparente (el índice transparente) y uno con `a ≥ umbral` es opaco con su RGB; el alfa parcial **no** se conserva (indexado no lo tiene). Hace falta índice transparente si hay más de una capa, una celda vacía o algún píxel por debajo del umbral. Gris con alfa se trata como RGB `(v, v, v)`.
**D5. Mapeo de píxeles.** Primero coincidencia exacta con una entrada (búsqueda binaria sobre una tabla ordenada) y, si no, la entrada más cercana por distancia euclídea al cuadrado en RGB sin ponderar, vía una **tabla de 6 bits por canal** (262 144 entradas de `u8`, calculada una vez por fuerza bruta sobre el centro de cada casilla; error máximo de 2 niveles por canal respecto de la búsqueda exacta; empate: el índice menor). Es la misma regla con tramado o sin él.
**D6. Tramado.** `Dither::None`; `Dither::FloydSteinberg` (de izquierda a derecha, error en 1/16, `i16` por canal, recortado a 0..=255, el error **no** pasa a un píxel transparente ni sale de uno); `Dither::Ordered` (matriz de Bayer 8 × 8 generada por entrelazado de bits, desplazamiento `((2m + 1 − 64)·s) / 128` con `s = 256 / ⌈∛N⌉`, sumado a r, g, b y recortado). Las capas se tramán cada una por su cuenta. Sin matrices de otros tamaños ni dirección serpenteante (fuera de alcance).
**D7. Varias capas y celdas vacías.** Una sola paleta para todas las celdas (el histograma suma todas). Las celdas vacías siguen vacías; capa oculta o bloqueada incluidas (como `ConvertAll`). Capas con opacidad ≠ 255 o mezcla ≠ Normal: no se puede (`IndexedOpacity`/`IndexedBlend`), con el mismo motivo que `Convertir modo…`.
**D8. Calidad.** Error medio medido sobre los píxeles opacos como media del valor absoluto de la diferencia por canal (MAE). **Oráculo interno independiente:** la rejilla uniforme de ≤ N celdas (reparto 6 × 7 × 6 para 252 y análogos), contra la que el cuantizador sin tramado **no puede ser peor** en las imágenes de prueba. **Referencia externa:** el cuantizador de Aseprite (fila 12): MAE propio ≤ MAE de la referencia × 1,10 + 0,5 niveles, con tramado y sin él; el margen se fija aquí y no se toca después. El tramado se mide además por MAE de bloque (media 4 × 4), que debe bajar respecto de sin tramado en degradados.
**D9. Un paso, `Command::QuantizeAll`.** `px-raster` calcula las imágenes y la paleta; `px-core` valida y mueve, con `based_on`. Reserva previa: `px-raster` comprueba el tope de celdas (como D10 de 4.1: rechaza solo si el resultado supera `cells_limit()` **y** crece; indexado ocupa ¼ de RGBA, así que no crece) y que se puede reservar la salida antes de empezar.
**D10. Coste.** Tres barridos: histograma, mapeo (con tramado) y, si procede, el recuento exacto previo (se corta al pasar de N colores). Memoria extra: 1 byte por píxel de celda más ≈ 10 MiB de tablas. Medido a 8192² RGBA (1 y 4 capas) en `px-raster/tests/quantize_perf.rs` (`#[ignore]`, no es puerta de CI) y anotado en la corrección posterior. Por encima de `SYNC_PIXELS` el aceptar va en dos tiempos con «Reduciendo colores…» (el mecanismo de «Convirtiendo…»).
**D11. Interfaz.** Entrada nueva `Reducir colores…` en `Imagen` (tras `Convertir modo…`; sin tecla por defecto, editable; ayuda nueva: 46 → 47). Solo con RGBA o gris con alfa (con otro modo: aviso `Reducir colores: solo RGBA o gris con alfa`). Diálogo `Reducir colores`: `Número de colores` (2 a 256, 256), `Paleta` (`Calculada de la imagen` y, si la sesión tiene una, `Guardada de la sesión (N colores)`), `Tramado` (`Ninguno`, `Difusión de error`, `Ordenado`), `Alfa mínimo para ser opaco` (1 a 255, 128) y un informe de solo lectura (tamaño, capas, aviso de tiempo por encima de 2²⁴ píxeles). El botón `Reducir colores…` de `Convertir modo…` se ve con destino Indexado y origen RGBA o gris con alfa; cierra ese diálogo y abre este. El texto de `TooManyColors` y su motivo pierden la mención al hito 4 y mandan a ese botón. Tras aplicar: aviso de 5 s `colores reducidos a N (error medio X,X)`, la vista y las texturas se rehacen como en `Convertir modo…`.
**D12. Cuantizar un indexado a menos colores** (reducir una paleta) y **elegir la paleta de un fichero**: fuera (4.6 y posteriores).

## Cambios exactos en `px-core` (todos aditivos; el visto bueno vale solo para esta lista)

- **Q-01** `Command::QuantizeAll { palette: Palette, images: Vec<Option<Image>>, based_on: u64 }`: `images[i]` es la celda de la capa `i` (de abajo arriba), indexada con `palette`. Solo como comando suelto (en un `Batch` es `NotBatchable`).
- **Q-02** Validación, sin tocar nada hasta que todo pasa: el lienzo es RGBA o gris con alfa (`QuantizeSource`); `images.len() == capas`; cada `Some` es indexada, del tamaño del lienzo y con `palette` (`ReplaceImageMismatch`); `Some` exactamente donde la celda tenía imagen (`ReplaceEmptiness`); opacidad 255 y mezcla Normal (`IndexedOpacity`, `IndexedBlend`); con más de una capa, `palette` tiene índice transparente (`NoTransparency { mode: Indexed }`); `based_on == state_id()` (`StaleImage`); tope de celdas sobre el resultado y solo si crece (`CelsTooLarge`); reserva previa (`OutOfMemory`).
- **Q-03** `DocumentError::QuantizeSource { mode: Mode }` (variante nueva; el resto se reutiliza).
- **Q-04** Paso de historia que intercambia las imágenes, la plantilla y el modo del lienzo (el mecanismo de `Converted`, `size: None`), con su `cost` contado; deshacer y rehacer exactos, `state_id` incluido.
- Nada no aditivo: ninguna firma pública existente cambia.

## Cambios exactos en `px-raster` (aditivos)

- **Z-01** `Dither`, `PaletteSource`, `QuantizeOptions { colors: u16, dither: Dither, alpha_threshold: u8, palette: PaletteSource }`, `QuantizeError`, `QuantizeStats { colors: usize, exact: bool, transparent_pixels: u64 }`, `Quantized { images, palette, stats }` y `quantize_cels(cels, size, source, &options) -> Result<Quantized, QuantizeError>` (pura; no reserva antes de comprobar opciones ni tamaño).
- **Z-02** `quantize_command(doc, &options) -> Result<(Command, QuantizeStats), QuantizeError>`: aplica `quantize_cels` a todas las celdas con imagen, comprueba D7 y el tope (D9) **antes** de reservar y rellena `based_on`.
- **Z-03** `mean_abs_error(original: &Image, indexed: &Image, alpha_threshold: u8) -> Option<ErrorStats>` con `ErrorStats { pixels: u64, sum_abs: u64, sum_sq: u64 }` y `mae_milli()` (MAE por canal en milésimas de nivel): la medida de D8, pública para `px-golden`.

## Piezas de `px-ui` y `px-app`

`px-ui`: `AppAction::Quantize`, entrada de `Imagen`, clave de ayuda, `DialogKind::Quantize`, textos en `quantize_es.rs`. `px-app`: `quantize_flow.rs` (formulario, plan desde los valores, informe, dos tiempos, aviso final), `Session::quantize`, `Session.saved_palette`, `Dispatched::Quantize`, el botón de `Convertir modo…` (`FIELD_QUANTIZE`, `quantize_offer`) y el cableado en `lib.rs`. `px-golden`: `quantize.rs` con la prueba contra Aseprite. `tools/corpus-h4/` (script de Lua y `generate.sh`) y `specs/corpus-hito-4-cuantizador.md` (casos y pasos exactos para el usuario).

## Casos (12; 1 a 7 y 12 en `px-raster`/`px-golden`, 8 y 9 en `px-core`, 10 a 11 en `px-ui`/`px-app`)

| # | Entrada | Esperado |
| --- | --- | --- |
| 1 | **Exacto si cabe.** RGBA de 5 × 4 con 6 colores y N = 16, 6 y 8; con un píxel `a = 0`; gris con alfa | Idéntico a `ConvertAll` (imagen y paleta); `stats.exact`; MAE 0; con N = 5 ya no es exacto |
| 2 | **Reducir.** Degradado RGB de 64 × 64 (4096 colores) con N = 2, 4, 16 y 256; imagen de 3 manchas de color | Paleta de ≤ N entradas; todo índice < `len`; MAE ≤ el de la rejilla uniforme del oráculo; las manchas conservan su color medio (±8); repetible bit a bit |
| 3 | **Umbral de alfa.** Alfas 0, 1, 127, 128, 255 con umbral 1, 128 y 255; gris con alfa | Transparente ⇔ `a < umbral`; los demás opacos con su RGB; índice transparente solo si hace falta; el alfa parcial no sobrevive |
| 4 | **Tramado.** Gris plano 50 % con paleta fija {0, 255}; degradado horizontal con 4 colores | Ordenado: fracción de píxeles claros = 32/64 por baldosa de 8 × 8 y patrón de Bayer exacto; difusión: la media 4 × 4 baja el error de bloque respecto de «ninguno»; ambos deterministas; el error no cruza un píxel transparente |
| 5 | **Paleta ajena.** Paleta de 4 entradas (con y sin transparente); píxeles que coinciden y que no | La paleta del resultado es la dada; coincidencia exacta → su índice; el resto, la más cercana (empate: la menor); sin transparente y haciendo falta: `FixedNeedsTransparent` |
| 6 | **Varias capas.** 3 capas (una vacía, una oculta, una bloqueada) con colores distintos | Una sola paleta; celdas vacías `None`; índice transparente presente; el color de cada píxel de cada capa está a la distancia mínima posible de su entrada |
| 7 | **Errores.** N = 0 y 257; N = 1 con transparencia; opacidad 128 y mezcla Multiply; modo Indexado o Gris | `InvalidColorCount`, `NoColorSlots`, `LayerOpacity`, `LayerBlend`, `SourceMode`; sin reservar ni cambiar nada |
| 8 | **`QuantizeAll` aplica y deshace.** Documento de 3 capas | Un solo paso; modo, paleta y celdas nuevos; vacías siguen vacías; propiedades de capa intactas; `undo`/`redo` exactos con `state_id` |
| 9 | **`QuantizeAll` rechaza.** `based_on` viejo; cantidad errónea; imagen no indexada, de otro tamaño o con otra paleta; celda que cambia de vacía; opacidad ≠ 255; sin transparente con 2 capas; dentro de `Batch`; fuente indexada | `StaleImage`, `ReplaceCountMismatch`, `ReplaceImageMismatch`, `ReplaceEmptiness`, `IndexedOpacity`, `NoTransparency`, `NotBatchable`, `QuantizeSource`; documento, historia y `state_id` idénticos |
| 10 | **Menú, ayuda, atajos y botón.** `Imagen` con 11 entradas; ayuda 47; acciones editables 40; `Convertir modo…` de RGBA a Indexado con más de 256 colores | `Reducir colores…` en su sitio, sin tecla y editable; atenuada con gesto o modal; el botón se ve solo con destino Indexado y origen RGBA o gris con alfa; el motivo ya no dice «cuantizador» ni «hito 4» y manda al botón |
| 11 | **Diálogo y sesión.** Abrir con un RGBA de 5000 colores; elegir N, tramado y umbral; `Aceptar`; `Esc`; deshacer; con paleta guardada y sin ella; con capa de opacidad 128 | Valores por defecto de D11; 0 y 300 no se aceptan con motivo; un paso y `modified`; zoom y capa activa como en `Convertir modo…`; texturas rehechas; aviso con N y error; sin efecto con gesto o modal; capa con opacidad ≠ 255: aviso y no abre |
| 12 | **Referencia (Aseprite).** Los 6 casos de `specs/corpus-hito-4-cuantizador.md` con N = 16 y 64, sin tramado, con difusión y ordenado | MAE propio ≤ MAE de la referencia × 1,10 + 0,5; `#[ignore]` mientras falten los PNG; con ellos presentes, estricta |

## Propiedades (proptest; 256 casos, 500 como máximo)

1. **Validez y determinismo.** Para imágenes de hasta 12 × 9 en RGBA o gris con alfa, N de 1 a 256, cualquier tramado y umbral: la paleta tiene ≤ N entradas, todo índice < `len`, la salida es idéntica en dos ejecuciones y, si la imagen tiene ≤ N colores, el resultado es exacto (MAE 0).
2. **Transparencia.** Para esas mismas imágenes: un píxel sale en el índice transparente ⇔ su alfa < umbral; el resto nunca lo usa.
3. **No peor que la rejilla.** Para imágenes de hasta 16 × 16 con muchos colores y sin tramado: MAE del cuantizador ≤ MAE de la rejilla uniforme de ≤ N celdas (más 1 nivel de margen).
4. **Documento.** Para documentos de 1 a 3 capas (vacías, ocultas, bloqueadas) y secuencias de `quantize_command` con `undo`/`redo`: cada aplicación es **un** paso, `state_id` cambia una vez, deshacerla devuelve el documento inicial exacto, rehacerla el final, y una que falla deja capas, historia y `state_id` idénticos.

## Pruebas nuevas

12 filas y 4 propiedades: mínimo 16, hasta unas 30 con varios casos por fila; tope de 40. Reparto: `px-raster/tests/quantize.rs` (1 a 7, propiedades 1 a 3), `px-raster/tests/quantize_perf.rs` (`#[ignore]`), `px-core/tests/quantize_all.rs` (8 y 9), `px-raster/tests/quantize_doc.rs` (propiedad 4), `px-ui/tests/quantize.rs` (parte de 10), `px-app/src/quantize_flow/tests.rs` (11 y el resto de 10) y `px-golden/tests/quantize.rs` (12).

## Pruebas anteriores que cambian (por lectura; el compilador da los números)

**A. Solo cifras, listas y literales:** la lista de `Imagen` en `px-ui/tests/menu.rs` y `background_menu.rs` (una entrada más tras `Convertir modo…`), el recuento de ayuda (46 → 47) y de acciones editables (39 → 40) y los de `keymap.rs`/`keymap_file.rs` si cuentan acciones; en `px-app` las pruebas de `convert_mode` que esperan el texto de `TooManyColors` («cuantizador», «hito 4»), su motivo `más de 256 colores: sin cuantizador` y la lista de campos de `convert_form`. **No cambian:** `Command`, `Document` ni ninguna firma pública existente. Si alguna necesita más que cifras, listas, literales o firmas: **me paro y te lo digo**.

## Orden de trabajo

Commits separados: (1) esta spec con su fila del `INDEX`; (2) adaptación de pruebas anteriores (el mensaje explica qué comprobaba cada una); (3) pruebas, en rojo, con los textos para revisión; (4) lógica de `px-core` y `px-raster`; (5) `px-ui` y `px-app`; (6) script de Lua, instrucciones y prueba de referencia. Cierre (lo lanza el usuario): `cargo fmt`, `clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`; después, el `license-auditor` una sola vez sobre el rango. Luego, la corrección única de esta spec y las notas en `PLAN-RESUMEN.md` y `mapa-funciones.md`.

## Prueba manual (la haces tú, sin capturas)

1. Abrir un PNG RGBA de fotografía: `Imagen → Convertir modo… → Indexado` dice que tiene más de 256 colores y muestra `Reducir colores…`.
2. Pulsarlo: se abre `Reducir colores` con 256 colores, paleta calculada, sin tramado y alfa mínimo 128. `Aceptar`: imagen indexada; el título gana asterisco; un `Ctrl+Z` la devuelve.
3. Repetir con 16 colores y los tres tramados; comparar a ojo el aspecto (difusión y ordenado suavizan los degradados).
4. Un PNG con transparencia parcial: subir y bajar el alfa mínimo cambia qué píxeles quedan transparentes.
5. Documento con 3 capas (una oculta, una bloqueada, una vacía): reducir las afecta a todas con una sola paleta.
6. Una capa con opacidad 128: avisa y no abre.
7. Un indexado: `Reducir colores…` avisa que solo vale con RGBA o gris con alfa. Convertir ese indexado a RGBA y reducir con «Guardada de la sesión»: la paleta es la de antes.
8. Con 8192² RGBA: «Reduciendo colores…» antes de congelarse y termina en pocos segundos.
9. `Ayuda → Atajos de teclado…`: `Reducir colores…` sin tecla; `F1` sobre su entrada abre su ayuda.

## Fuera de alcance

Reducir un indexado a menos colores, cargar la paleta de un fichero (4.6), matrices de otros tamaños y tramado serpenteante, cuantización perceptual (Lab), tramado de patrón y suave (4.7), hilos, y cualquier otro cambio en `px-core` o `px-raster`.

## Decisiones que necesito que confirmes

1. **D1:** exacto si los colores caben, idéntico a `ConvertAll`.
2. **D2:** *median cut* ponderado con histograma de 6 bits y 3 iteraciones de Lloyd.
3. **D3:** paleta ajena = la última paleta indexada de la sesión, hasta que 4.6 cargue ficheros.
4. **D4:** umbral de alfa de 1 a 255 (128 por defecto); el alfa parcial no se conserva.
5. **D5 y D6:** mapeo por tabla de 6 bits tras coincidencia exacta; tramado de difusión de Floyd–Steinberg y ordenado de Bayer 8 × 8.
6. **D8:** criterio de calidad MAE ≤ referencia × 1,10 + 0,5 niveles y la rejilla uniforme como oráculo interno.
7. **D11:** entrada propia `Imagen → Reducir colores…` más el botón en `Convertir modo…`.
8. **Q-01 a Q-04 y Z-01 a Z-03:** tu aprobación expresa para tocar `px-core` y `px-raster` solo con esa lista (dada de antemano).

## Visto bueno (2026-10-26; el usuario delegó en mi criterio)

Confirmadas tal como están las ocho decisiones del final. Aprobación expresa (de antemano) de `px-core` y `px-raster` **solo para Q-01 a Q-04 y Z-01 a Z-03**; si hiciera falta algo más, parar y decirlo. Modo ahorro de `CLAUDE.md`: una sola corrección posterior de esta spec, al final; informe final de 20 líneas como máximo.

## Corrección posterior (al cerrar la unidad; la única)

Lo que se hizo distinto de lo escrito arriba, y por qué:

- **Rótulos de los campos numéricos.** El formulario admite 12 caracteres en un campo numérico: son `Colores` y `Alfa mínimo` (no «Número de colores» ni «Alfa mínimo para ser opaco»); el alfa mínimo se explica en la ayuda. Textos en [`hito-4-cuantizador-textos.md`](hito-4-cuantizador-textos.md).
- **Tramado ordenado (D6).** El desplazamiento usa `s = 256 / ⌈∛k⌉` con `k` = entradas **opacas** de la paleta (en una calculada, N menos la transparente; en una ajena, las suyas), no N: con una paleta ajena N no se usa.
- **`QuantizeStats`** lleva además `opaque_pixels` y `abs_error` (el error medio sale del mismo barrido del mapeo; `mae_milli()`), y `QuantizeError` tiene `SizeMismatch` y `TooLarge { needed, limit }`. Es el mismo tipo de Z-01, con más campos; `mean_abs_error` (Z-03) sigue siendo la medida independiente que usan las pruebas.
- **`QuantizeAll` (Q-02).** Se comprueba `based_on` antes del modo del documento: un comando viejo sobre un documento ya indexado da `StaleImage`, no `QuantizeSource`.
- **Mapeo (D5).** La tabla de 6 bits se rellena de forma perezosa (solo las casillas que aparecen), para no pagar 262 144 búsquedas con una imagen de 12 × 9; el resultado es el mismo.
- **Paleta guardada (D3).** Se guarda al abrir un documento indexado y al salir de uno indexado con `Convertir modo…` (también con varias capas). `Nueva imagen` indexada cuenta como abrir.
- **Medidas** (release, 8192² RGBA, 64 colores, máquina virtual de la nube; `px-raster/tests/quantize_perf.rs`, `#[ignore]`, no es puerta de CI): 1 capa 1,1 s sin tramado, 1,4 s con difusión y 1,1 s con ordenado; 4 capas 5,0 / 6,1 / 4,7 s. La memoria extra es 1 byte por píxel de celda más unos 10 MiB de tablas (calculado, no medido). Con más de 2²² píxeles el aceptar va en dos tiempos con «Reduciendo colores…».
- **Referencia (fila 12).** El script de Lua y la prueba existen pero **no se han ejecutado**: faltan los 42 ficheros de `golden/aseprite-quant/` (pasos en [`corpus-hito-4-cuantizador.md`](corpus-hito-4-cuantizador.md)); la prueba está `#[ignore]` hasta entonces. Hasta tenerlos, la calidad solo está respaldada por el oráculo interno (la rejilla uniforme, propiedad 3).
- **Subagentes.** `test-writer` y `license-auditor` no se usaron (presupuesto de `CLAUDE.md`): las pruebas las escribió la sesión principal antes del código y el `license-auditor` queda para el usuario (no hay dependencias nuevas; sí cambios en `px-core` y `px-raster`).
- **Pruebas anteriores adaptadas:** además de las de la lista, `px-ui/tests/image_ops.rs` (fila 9 de 4.1: 11 entradas en `Imagen`) y una cifra de `convert_mode/tests.rs` (el motivo medía 35 caracteres y ahora 34). Ninguna cambió de intención.
