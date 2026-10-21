# Corpus del hito 3: capas, alfa y mezcla

Estado: **preparada, a la espera de que el usuario ejecute el script y pase los errores**. No es una unidad (no cuenta en las 10 del hito; ver `mapa-funciones.md`, sección 1): es la lista única de casos con corpus ajeno, escrita antes de la primera unidad para generarlos de una sentada. No toca código de la aplicación.

**Decisión del usuario (21 de octubre de 2026): la única referencia es Aseprite**, como generador y oráculo de los `.aseprite` y de los PNG aplanados. No se usa LibreSprite en el hito 3. Cambia el flujo de D10: antes los `.aseprite` los escribía Pixoko (3.8) y LibreSprite los convertía; ahora los escribe Aseprite desde un script propio, así que **3.8 empieza leyendo un corpus que ya existe** y su escritura se prueba después (ida y vuelta, y abrir lo que escribe Pixoko con Aseprite por lotes).

- **Propio de arriba abajo.** Cada caso se crea desde cero con un script de Lua del repositorio (`tools/corpus-h3/`, MIT): colores y formas definidos en el script (rectángulos, discos, diagonales y triángulos por aritmética de enteros, sin suavizado), paleta propia en cada sprite (nunca la paleta por defecto) y ningún recurso del programa.
- **Oráculo.** El PNG aplanado sale de abrir el `.aseprite` guardado y exportarlo con el modo por lotes (`--save-as`): es lo que ve el programa al leer el fichero, no lo que el script creía haber dibujado.
- **Mezcla.** Las fórmulas son **propias, de enteros y documentadas** (3.2). El objetivo es **tolerancia 0** contra estos PNG. Si los datos muestran diferencias de redondeo de ±1, se decide con ellos **caso a caso** y se anota en la spec de 3.2 o de 3.9 (qué caso, qué modo, qué fórmula y por qué). Una diferencia mayor que ±1 es un fallo de la fórmula, no de redondeo.
- **El CI no ejecuta Aseprite.** Los resultados se commitean en `golden/aseprite/` y las pruebas solo los leen.

## 0. Antes de nada: versión y modo por lotes

El script se escribió sin ejecutar nada de Aseprite; cada duda sobre su API de Lua está marcada con `comprobar` en el código (lista en la sección 5). `ASEPRITE` es la orden del programa (por defecto `aseprite`).

```sh
# 1. Versión (anótala para el README del corpus; la cadena exacta puede variar)
aseprite --version

# 2. Ejecutar un script de Lua por lotes (escribe humo.aseprite: 4x4, dos capas)
mkdir -p ~/.cache/pixoko-corpus-h3-prueba
aseprite -b --script tools/corpus-h3/comprobar.lua --script-param salida="$HOME/.cache/pixoko-corpus-h3-prueba"

# 3. Exportar un PNG aplanado de un .aseprite
aseprite -b ~/.cache/pixoko-corpus-h3-prueba/humo.aseprite --save-as ~/.cache/pixoko-corpus-h3-prueba/humo.png
```

Las tres juntas, con comprobación del resultado (esquinas `(196, 46, 52, 255)`, centro opaco) y borrado de la carpeta de prueba: `tools/corpus-h3/generar.sh --prueba`. Si algo falla, pásame el error tal cual. Con `-b` no abre la interfaz.

## 1. Cómo se genera

`tools/corpus-h3/generar.sh` (sin argumentos: los 25 casos; con nombres: solo esos). Por caso: ejecuta `generar.lua` con `--script-param caso=<nombre>` y `salida=<carpeta>`, comprueba que existe el `.aseprite`, lo abre y exporta el PNG, y escribe `X.png.sha256` con **Pillow** (herramienta independiente, no Pixoko): SHA-256 de la RGBA de 8 bits, alfa directo, sin cabecera ni dimensiones, el mismo criterio de `golden/png/`. Al final rehace `golden/aseprite/README.md` con la versión de Aseprite y la fecha de esa ejecución, el origen de las imágenes (dibujos propios, ningún recurso del programa) y la tabla de ficheros. `--verificar` vuelca capas y celdas de lo generado (`verificar.lua`) para mirar a ojo. No se commitea nada solo.

Se necesita `python3` con Pillow; si falta, el script lo dice. Las carpetas de prueba van a `~/.cache`, no a `/tmp`.

## 2. Casos

Nombre de fichero: minúsculas, dígitos y guiones. Cada caso son tres ficheros: `X.aseprite`, `X.png` (aplanado) y `X.png.sha256`. **25 casos, el máximo**: si hace falta uno más, se fusionan dos. Todos tienen **un fotograma**, tamaños de 16×16 a 64×64 y paleta propia. «Opacidad» es la de la capa; la de celda queda para el hito 6. Capas de abajo arriba. Colores: nombres de la paleta propia de 16 entradas de `generar.lua` (`negro`, `blanco`, `rojo`, …); **A** y **B** son dos tablas de 8 colores RGB del script, con blanco, negro y valores sin patrón (37, 201, 143, 211, 17, 99) para que el redondeo se vea.

**Unidades** (de `mapa-funciones.md`): 3.1 modelo de capas, 3.2 composición, 3.5 aplanar y exportar, 3.8 `.aseprite` etapa 1, 3.9 puerta. Todos los casos los usa 3.8 (lectura: estructura y propiedades declaradas; ida y vuelta) y 3.9 (la composición del documento leído da el PNG); la columna dice quién más.

### Capas básicas, opacidad y alfa (RGBA)

| Fichero | Tam. | Capas | Contenido | También |
| --- | --- | --- | --- | --- |
| `rgba-una-capa` | 16×16 | 1 | Disco rojo opaco, rectángulos azul (alfa 128) y verde (alfa 64), dos píxeles con alfa 0 y RGB ≠ 0 (el RGB bajo alfa 0 se anota, no se exige) y una rampa de alfa 0 a 255 en la fila 15 | 3.1, 3.5 |
| `capas-normal-tres` | 24×24 | 3 | `fondo` opaco, `medio` (disco) y `arriba` (triángulo y diagonal) con huecos transparentes; Normal, opacidad 255 | 3.1, 3.2, 3.5 |
| `capas-opacidad` | 32×16 | 6 | `fondo` en damero blanco y negro de bloques de 4 y cinco capas `op-0`, `op-64`, `op-128`, `op-200` y `op-255`, cada una con una tira roja, Normal | 3.2 |
| `capas-alfa-origen` | 32×16 | 2 | Base opaca de cuatro bandas y capa superior con alfa de origen 0, 36, 73, 109, 146, 182, 219 y 255 (naranja arriba, cian abajo); opacidad 255 | 3.2 |
| `capas-alfa-fondo` | 32×16 | 2 | Base verde con alfa 255, 200, 128 y 64 por columnas y capa magenta con los mismos alfa por filas: dos translúcidos a la vez; Normal | 3.2 |

### Los ocho modos de D07 (RGBA)

| Fichero | Tam. | Capas | Contenido | También |
| --- | --- | --- | --- | --- |
| `mezcla-normal`, `mezcla-multiplicar`, `mezcla-trama`, `mezcla-superponer`, `mezcla-oscurecer`, `mezcla-aclarar`, `mezcla-sumar`, `mezcla-diferencia` | 32×32 cada uno | 2 | `base` con los colores A por columnas de 4 px y `fuente` con los B por filas de 4 px, ambas opacas: 64 combinaciones de color; la capa `fuente` usa el modo del nombre (Normal, Multiplicar, Trama = *Screen*, Superponer, Oscurecer, Aclarar, Sumar = *Addition*, Diferencia) con opacidad 255 | 3.2 (vectores y dorados) |
| `mezcla-ocho-opacidad` | 64×16 | 9 | `base` opaca con A por columna (x mod 8) y ocho capas `m-<modo>`, cada una en su tira de 8 columnas, origen opaco con B por fila, **opacidad 128** | 3.2 |
| `mezcla-ocho-alfa` | 64×16 | 9 | Igual, opacidad 255 y **alfa de origen 200** (filas 0 a 7) y **77** (filas 8 a 15) | 3.2 |
| `mezcla-ocho-fondo-translucido` | 64×16 | 9 | Igual, opacidad 255, **base con alfa 150**, origen con alfa 255 (filas 0 a 7) y 100 (8 a 15): es donde más se separan las fórmulas | 3.2 |

### Indexado (D06: sin opacidad ni mezcla)

| Fichero | Tam. | Capas | Contenido | También |
| --- | --- | --- | --- | --- |
| `indexado-capas` | 16×16 | 4 | Paleta propia de 16 entradas, índice transparente 0; `fondo`, `oculta` (invisible, cubre todo), `medio` y `arriba`; Normal, opacidad 255 | 3.1, 3.2, 3.5 |
| `indexado-transparente-7` | 16×16 | 2 | Índice transparente **7** y el 0 (negro) opaco: huecos con el 7 dejan ver la capa de abajo | 3.1 |
| `indexado-paleta-256` | 64×64 | 1 | **256 entradas** (fórmula del script) y un bloque de 4×4 por entrada | 3.1 (paleta) |
| `indexado-celdas` | 32×32 | 3 | Indexado con celdas de 12×12 en (5, 7), de 14×14 en (-6, -4) y de 10×10 en (27, 25): desplazada y recortada por ambos lados | 3.1 (D04) |

### Celdas (RGBA, D04: al leer, celda del tamaño del lienzo)

| Fichero | Tam. | Capas | Contenido | También |
| --- | --- | --- | --- | --- |
| `celda-desplazada` | 32×32 | 4 | Celdas de 10×8 en (3, 5), de 12×12 en (16, 4), de 8×8 en (24, 24) pegada al borde y de 1×1 en (31, 31) | 3.1 |
| `celda-recortada` | 32×32 | 6 | Una celda de 48×48 en (-8, -8) que cubre todo, otra de 16×16 en (-8, -8), una en (24, 10) que sale por la derecha, una de 20×10 en (6, 26) que sale por abajo y **dos del todo fuera** del lienzo (en (40, 2) y en (-16, -16)): al leerlas la celda queda vacía | 3.1 |
| `celda-vacia` | 16×16 | 3 | `con-celda`; `sin-celda` (Multiplicar, opacidad 100, **sin celda**: las propiedades se leen aunque no haya imagen) y `transparente` (celda que existe con alfa 0 en todo) | 3.1 (celdas vacías sin memoria) |

### Capas ocultas y caso mixto (RGBA)

| Fichero | Tam. | Capas | Contenido | También |
| --- | --- | --- | --- | --- |
| `capas-ocultas` | 24×24 | 5 | `fondo`, `oculta-1` (invisible, magenta a todo el lienzo), `visible-1`, `oculta-2` (invisible, Multiplicar, 128) y `visible-2`: el PNG aplanado no debe verlas | 3.2, 3.5 |
| `mixto-rgba` | 48×48 | 6 | `fondo` de cuatro bandas, `sombra` (Multiplicar, 192, celda de 24×24 en (10, 12) con alfa 220 y 110), `luz` (Trama, 128, celda de 30×30 en (-8, 20)), `oculta` (invisible), `borde` (diagonal con alfa 128) y `diferencia` (Diferencia, 90, celda de 16×16 en (30, 30)) | 3.2, 3.5, 3.9 (puerta) |

## 3. Prueba del corpus (`px-golden`)

Un módulo nuevo en `png_interop` (`crates/px-golden/tests/png_interop/aseprite_corpus.rs`; sin ejecutable nuevo), con el mismo criterio de `golden/png/`: cada `X.aseprite` tiene su `X.png` y su `X.png.sha256`, y el hash de la RGBA que `read_png` da del PNG es el del `.sha256`. Además, como 3.8 aún no existe, se mira la **cabecera de 128 bytes** de la especificación pública (tamaño igual a la longitud del fichero, magia `0xA5E0`, un fotograma, 16 a 64 de ancho y alto, 32 u 8 bits de color) y que sus dimensiones sean las del PNG.

| Fila | Qué | Resultado |
| --- | --- | --- |
| 1 | Un caso correcto en memoria | `Ok` con su nombre |
| 2 | Cada defecto en memoria: carpeta vacía; sin README; `.aseprite` sin PNG; PNG sin `.aseprite`; sin `.sha256`; hash de otra imagen; magia, tamaño, fotogramas o dimensiones fuera de la cabecera; dimensiones distintas a las del PNG; fichero ajeno; nombre con mayúsculas; más de 25 casos | `Err` con el mensaje del defecto (el control de que el corpus sabe fallar) |
| 3 | La carpeta real `golden/aseprite/` | `Ok`; si la carpeta **no existe** avisa («corpus pendiente») y pasa; si existe y está vacía, falla |

El test no ejecuta Aseprite ni lee las capas.

## 4. Qué queda fuera

- **Hito 5 (fotogramas y etiquetas; 5.5 `.aseprite` etapa 2):** varios fotogramas con duraciones distintas, etiquetas (nombre, rango, dirección), celdas enlazadas entre fotogramas, mesa de luz sobre PNG aplanados, ciclo de color. Los casos de GIF y hojas de sprites (5.6, 5.7) también: este corpus no tiene ninguno. El oráculo de esos casos sigue por decidir (ver «Decisiones»).
- **Hito 6 (`.aseprite` restante, 6.1):** opacidad de celda propia, modo de grises, perfil de color, paleta con alfa en un PNG indexado (si Aseprite escribe `tRNS` por entrada), propiedades de usuario, capas de grupo, capas de mapa de tiles, otros modos de mezcla (los diez que no están en D07), cuadro de referencia de capa y rejilla.
- **Hito 4:** los casos de herramientas (círculo, elipse, degradados, cuantizador) tienen su propia lista, que se escribe al empezar el hito 4.

## 5. Dudas de la API: `comprobar`

El script se escribió de memoria. Lo que hay que mirar si falla (cada una está marcada en el código y centralizada en un ayudante, así que se corrige en un sitio):

1. `app.params` y `--script-param` (`generar.lua`, `comprobar.lua`).
2. `Sprite(ancho, alto, ColorMode.X)` (si no, `Sprite{ width, height, colorMode }`); que un sprite nuevo trae **una capa y ninguna celda** (si trae celda, `newCel` debe reemplazarla).
3. `Palette(n)`, `Palette:setColor` y `Sprite:setPalette`; `Sprite.transparentColor`.
4. `Image(ancho, alto, modo)` e `Image:putPixel(x, y, entero)` (valor de píxel RGBA con `app.pixelColor.rgba`, o índice); si `putPixel` pide `Color`, se cambia en `pon`.
5. `Sprite:newCel(capa, 1, imagen, Point)` reemplaza una celda existente; `Sprite:deleteCel(celda)`.
6. `Layer.opacity` (0 a 255), `Layer.blendMode`, los nombres del enum `BlendMode` (`NORMAL`, `MULTIPLY`, `SCREEN`, `OVERLAY`, `DARKEN`, `LIGHTEN`, `ADDITION`, `DIFFERENCE`) y `Layer.isVisible`.
7. `Sprite:saveAs(ruta.aseprite)` y que **el fichero conserva la parte de las celdas que sale del lienzo** (no la recorta al guardar): se ve con `generar.sh --verificar` (`celda 48x48 en (-8,-8)`). Si las recorta, `celda-recortada` y `indexado-celdas` pierden la mitad de su sentido y se avisa.
8. `--save-as` aplana solo las capas **visibles**; el PNG de un sprite indexado sale indexado (con `PLTE` y, quizá, `tRNS`): se mira qué lee `read_png`.
9. Que el RGB bajo alfa 0 se conserve o no en el PNG (`rgba-una-capa`): se anota lo que pase.
10. `aseprite --version` (formato de la cadena) y `app.version`.
11. `verificar.lua`: `Layer:cel`, `Cel.position`, que con un fichero en la línea de órdenes `app.sprite` sea ese sprite.

## 6. Decisiones que debes confirmar

1. **Aseprite como única referencia del hito 3**, con el flujo nuevo (los ficheros de entrada los escribe Aseprite; 3.8 empieza leyéndolos). `mapa-funciones.md` (D07, D10, el 3.2, el 3.8 y el 3.9), `PLAN-RESUMEN.md` y `REFERENCIAS.md` quedan corregidos. **Lo que dicen del hito 4 y del hito 5 sobre LibreSprite** (cuantizador, formas, GIF, hojas de sprites y etiquetas) **sigue igual hasta que decidas** si también pasan a Aseprite.
2. **25 casos, el máximo.** Sin margen: añadir uno exige quitar otro.
3. **Tolerancia 0** como objetivo y ±1 solo caso a caso con datos (ver arriba).
4. **El test de la carpeta pasa con aviso si `golden/aseprite/` no existe** y es estricto si existe: así el CI sigue en verde hasta que commitees el corpus y no hay que acordarse de quitar un `#[ignore]`.
5. **Capas y celdas fuera de este corpus** (opacidad de celda, grupos, tiles): al leerlas, 3.8 decide si avisa o falla; no hay caso aquí.
