# Hito 2 · Unidad 11a: texto dentro de la ventana (fuente, escala, tema y barra de estado)

Estado: **aprobada** (2026-10-18; decisiones 1 a 9 como se recomendaban, con los matices del visto bueno incorporados al texto, que **prevalecen sobre las secciones anteriores**: todo texto externo pasa por la sustitución y se prueba con japonés, emoji, controles, marcas bidireccionales, caracteres combinados y nombres larguísimos; el truncado es propio y usa `…` solo si la fuente lo tiene, y `...` si no; los ficheros de la fuente los pone el usuario en `assets/fonts/` (no se descargan ni se modifican) y un test fija su SHA-256; la licencia es la OFL de `public/assets/LICENSE` y el MIT del repositorio de la fuente no se atribuye; fondo neutro, tonos solo para acentos y avisos, contraste ≥ 4,5:1 con una función pura, y avisos con prefijo de texto; fila nueva del lienzo desplazado por la barra; coste real en `docs/COSTES.md`; regla de capturas en `CLAUDE.md`). Lo medido sale de un banco en el scratchpad, fuera del repo (copia de `HEAD` y una crate de pruebas con `egui` 0.37.0 sin ventana). Esta página **resuelve la decisión «fuente» que dejó pendiente `specs/hito-2-ventana.md`** (§2, «Resolución»). El número «11a» es mío: va antes de la unidad 11 (panel de paletas y diálogos).

## Objetivo

Que la app pueda dibujar texto dentro de la ventana, con aspecto de píxeles y **sin desenfoque a ninguna escala de pantalla**, y estrenarlo con una **barra de estado** (herramienta, color, zoom, posición del cursor, aviso) que repite lo que hoy va en el título; el título se mantiene. Cambios: `px-ui` (fuente, escala, tema, barra) y `px-app` (instalarlos y pintar la barra). Sin dependencias nuevas, sin `unsafe`, sin cambios en `deny.toml`.

## Lo medido

Banco: `egui` 0.37.0 con el mismo código de texto que usará la app (`epaint` 0.37 rasteriza con `skrifa` y `vello_cpu`). Un rasterizador por software mío convierte las mallas en píxeles como la GPU (triángulos, filtro bilineal del atlas, alfa premultiplicado) y mide los píxeles de tinta que no son ni fondo ni primer plano («intermedios», alfa entre 8 y 247 de 255), y compara con la ampliación entera del texto nativo. «n» = múltiplo entero de píxeles físicos por píxel de la fuente. **No he podido ver una ventana real a 1,5× ni 2×**: la nitidez en la GPU real queda para la prueba manual.

### Opción 1: `default_fonts` de egui

- **Crate única**: `epaint_default_fonts` 0.37.0, licencia declarada `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`; llega por `epaint` ← `egui` ← `eframe`/`egui_glow`/`px-ui`.
- **Fuentes que entran con `default_fonts`**:

| Fichero | Bytes | Licencia | Texto |
| --- | --- | --- | --- |
| `Ubuntu-Light.ttf` | 361 676 | Ubuntu Font Licence 1.0 | `fonts/UFL.txt` (4 673 B); copiado en `specs/hito-2-ventana-licencias.md` §A2 |
| `Hack-Regular.ttf` | 309 408 | MIT © 2018 Source Foundry Authors, **más** Bitstream Vera License © 2003 Bitstream Inc. (nombres reservados «Bitstream» y «Vera») y cambios de DejaVu en dominio público | `fonts/Hack-Regular.txt` (3 734 B); §A3a |
| `egui-icons.ttf` | 3 484 | MIT (subconjunto de `emoji-icon-font`) | `fonts/emoji-icon-font-mit-license.txt`; §A3b y A3c |

  `NotoEmoji-Regular.ttf` (418 804 B, OFL-1.1, §A1) y `emoji-icon-font.ttf` (324 132 B) vienen en el paquete pero solo se compilan con `monochrome_emoji_fonts`, que no se activa. La licencia de Bitstream Vera **no figura en la expresión SPDX** de la crate, así que `cargo deny` no avisaría de ella: habría que atribuirla a mano.
- **`deny.toml`** (no se añade; comprobado en la copia: sin esto `licenses` da `rejected` para `OFL-1.1` y `Ubuntu-font-1.0`, con esto da `licenses ok`):

```toml
[[licenses.exceptions]]
crate = "epaint_default_fonts"
allow = ["OFL-1.1", "Ubuntu-font-1.0"]
```

- **Binario** (release, `strip`, `px-app`; base actual 7 694 104 B): con `default_fonts` y sin código de texto, 8 377 680 B (+683 576). **Con el mismo código de texto que la opción 2** (una etiqueta y los estilos), 8 624 944 B: **+576 576 B más que la opción 2** (8 048 368 B).
- **Nitidez**: Ubuntu-Light es una fuente vectorial suavizada: de cada 100 píxeles de tinta, 95 son intermedios a 14 px y 65 a 28 px, igual con escala 1,0, 1,5 y 2,0 (hinting por defecto). Es texto legible, pero **no tiene aspecto de píxeles** y la escala entera no lo cambia.

### Opción 2: fuente de píxeles con `FontDefinitions`

Descargadas de sus repositorios oficiales en carpetas separadas; licencia leída del fichero del repositorio. Cobertura por tabla `cmap`, cuadrícula por los contornos (todas sin curvas), nitidez con el banco a escala 1,0, 1,5 y 2,0.

| | **Departure Mono 1.500** | **Tiny5 2.007** | **Cozette 1.30.0** (`CozetteVector.ttf`) |
| --- | --- | --- | --- |
| Origen | `rektdeckard/departure-mono`, release `v1.500` (zip) | `Gissio/font_tiny5`, commit `1f124caa…`, `fonts/ttf/` | `slavfox/Cozette`, release `v.1.30.0` |
| Licencia exacta | SIL OFL 1.1, © 2022–2024 Helena Zhang (`public/assets/LICENSE`; el MIT del repositorio es el del sitio web, no el de la fuente) | SIL OFL 1.1, © 2022–2024 The Tiny5 Project Authors (los metadatos de la fuente dicen 2026) | MIT, © 2020 Ines |
| Nombre reservado | **No** | **No** | no aplica |
| Formato y bytes | OTF (CFF) 84 480 | TTF 235 484 | TTF 649 328 |
| Cobertura | `ñÑáéíóúÁÉÍÓÚüÜ¿¡`, `×·—…`, `→`, ASCII, `€`: sí | todo lo anterior | todo lo anterior |
| Cuadrícula | 11 px por em (50 de 550 unidades); monoespaciada, 7 px de avance, mayúsculas 8, minúsculas 6; alto de línea 14 | 8 px por em (128 de 1024); proporcional (avance 4 a 5), mayúsculas 5, minúsculas 4; alto de línea 9 | 13 px por em; unidad de 157,54 (no entera); monoespaciada, 6 px de avance; alto 13 |
| Tamaños nítidos (px físicos) | 11, 22, 33… | 8, 16, 24… | 13, 26 (a 39, el 13,5 % de intermedios por redondeo de unidades) |
| Intermedios a 1,0 / 1,5 / 2,0 con n = 1, 2, 3 | 0 en las nueve y **idéntico píxel a píxel** a la ampliación entera del texto nativo | 0 en las nueve, idéntico | 0 con n = 1 y 2; 1 138 de 8 445 con n = 3 |
| Tamaño «ingenuo» (11 pt a 1,5×, o ×1,5) | 58 % de intermedios | 62 % | 57,5 % |

La imagen mostraba los trazos del tamaño ingenuo con grosores desiguales y los de n entero uniformes. Con `hinting` y `subpixel_binning` activados (valores por defecto de egui) los contornos de Departure siguen cayendo en la rejilla, pero el atlas bajo los glifos gana texels con alfa casi nulo (12 con n = 1, 2 055 con n = 2, menos de 32 sobre 255): invisibles, pero rompen una comprobación estricta de 0 y 255. **Se desactivan los dos por fuente** (`FontTweak`); con ellos desactivados hay 0 texels intermedios bajo los glifos a todas las escalas probadas.

Descartadas con medida: **Pixelify Sans** (OFL; 79 % de intermedios: no está en una rejilla), **Silkscreen** (OFL; 10 % de intermedios con n = 1, coordenadas fuera de la rejilla de 125 unidades, minúsculas en versalitas), **Press Start 2P** (rejilla perfecta de 8 px, pero declara nombre reservado «Press Start 2P» y es muy ancha y pesada), **Jersey 10** (OFL; nítida a 18,67·n px pero con mayúsculas de 10 px: reserva para títulos).

**Qué hay que incluir** (OFL §2: copyright y texto de la licencia con cada copia; no vender la fuente sola; sin nombre reservado, y sin tocar la fuente no hay nada que renombrar): la fuente **sin modificar ni recortar** (el OTF ya lleva el copyright y la licencia en su tabla `name`, comprobado); `assets/fonts/LICENSE` con el texto literal (es el fichero `public/assets/LICENSE` del repositorio oficial; **el MIT de la raíz de ese repositorio es el del sitio web: no se incluye ni se atribuye a la fuente**); `docs/THIRD-PARTY.md` (nombre, versión, origen oficial, SPDX `OFL-1.1`, el copyright tal como aparece en el fichero de licencia, si declara o no *Reserved Font Name* —no declara ninguno—, los SHA-256 de la fuente `4d53f663155cf8bf7ffc8e688776e719625f7bbb80a8d90073438b249261a2e0` y de la licencia `b65e42750f3cb65437a97f4dfd781c58f199259949db2341012be60e32500e83`, y el texto completo); y un aviso en la app: `px-app --licencias` imprime ese documento (`include_str!`) y el cuadro «Acerca de» llega con los diálogos (unidad 11). Esto es lectura de los textos, no asesoramiento legal.

### Opción 3: fuente de mapa de bits propia

- **3a, pintor propio sin el texto de egui**: tabla de unos 115 glifos de 5×7 (95 ASCII, 14 letras, `¿¡`, `×·—…`; ≈130 líneas), medición (≈20), dibujado (≈60) y **widgets propios** de etiqueta, botón, casilla, campo de texto, deslizador y menú (≈400 a 600 líneas). Se pierden `TextEdit`, `DragValue`, `ComboBox`, menús, etiquetas de ventana y todo widget de egui que mide texto. Estimación mía: 60 000 a 100 000 tokens y mantenimiento permanente.
- **3b, hallazgo: `GlyphRasterizer` de egui 0.37** (gancho para glifos propios dentro del texto de egui). Probado en el banco **sin ninguna fuente instalada**: egui maqueta (`Añ A` mide 24×8 px a tamaño 7 y escala 1,0; 48×17 a 2,0), los widgets seguirían funcionando. Límites medidos: `has_glyph` da `false` para los glifos del rasterizador (la prueba de cobertura iría contra la tabla propia), el alto de línea sale de egui (8 px para glifos de 7, ×1,2) y no hay *kerning*. Estimación mía: 15 000 a 20 000 tokens más el diseño de 115 glifos (lo caro es el diseño, no el código); cada carácter nuevo pide un glifo. Es compatible con la opción 2 (un rasterizador con prioridad `Highest` puede sustituir solo algunos glifos).

### Recomendación

**Opción 2 con Departure Mono 1.500.** La nitidez **no falla** en las pruebas (0 píxeles intermedios y igualdad exacta con la ampliación entera a 1,0, 1,5 y 2,0), la licencia es OFL sin nombre reservado, pesa 84 KB, cubre todo el español y su rejilla de 11 px da un texto de 14 px de alto a 1×, legible en una barra. Es monoespaciada: los números de la barra no bailan y medir es multiplicar. Coste de esta unidad, estimado: unos 20 000 a 25 000 tokens (tests ≈ 8 000, código ≈ 150 líneas ≈ 7 000, `THIRD-PARTY.md` y revisión el resto); mantenimiento bajo (cambiar de fuente es cambiar un fichero, su licencia y una constante). **Tiny5** es la alternativa si se quiere una interfaz más compacta (a n = 2 da 18 px de línea, proporcional); **Cozette** (MIT) queda tercera por tamaño (649 KB), unidades no enteras y procedencia sin comprobar de sus 6 047 glifos. La opción 3b queda como pulido posterior, como pedías.

## Diseño

### Carga

`assets/fonts/DepartureMono-Regular.otf` y `assets/fonts/LICENSE` (en la raíz del repositorio; **los ha puesto el usuario, descargados del origen oficial: no se descargan ni se modifican**; `.gitattributes` los marca como `binary` para que ningún sistema cambie sus finales de línea y el SHA-256 sea el mismo en los tres). En `px-ui/src/text.rs`: `pub const FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/DepartureMono-Regular.otf");` y `FONT_LICENSE: &str = include_str!("../../../assets/fonts/LICENSE");` e `install_fonts(&egui::Context)`: `FontDefinitions::empty()` con **una sola fuente** (`"DepartureMono"`, `FontData::from_static(FONT_BYTES)` con `FontTweak { hinting: Some(false), subpixel_binning: Some(false), .. }`) como única de `Proportional` y de `Monospace`, y `ctx.set_fonts(..)`. Sin fuentes de reserva: un carácter ausente sale como cuadrado.

### Tamaño base y escala sin desenfoque

El em nativo es **11 px físicos**. `text_scale(ppp)` = `round(ppp)` con el medio hacia arriba, mínimo 1, máximo 8 (no finito o ≤ 0: 1). El tamaño de fuente en puntos es **`11·n / ppp`**, de modo que en píxeles físicos es siempre múltiplo de 11. Todos los estilos de texto (`Small`, `Body`, `Button`, `Monospace`) usan 11·n px y `Heading` 22·n px; no hay más tamaños. `apply_scale(ctx)` se llama en cada fotograma y solo escribe el estilo si cambió (al mover la ventana a otro monitor se reajusta sin reiniciar). Consecuencia a confirmar: con 1,25× el texto queda en 11 px (menor que el 13,75 que «pide» el sistema) y con 1,5× en 22 px (mayor que 16,5); el tamaño exacto no se puede tener con nitidez. El «píxel de interfaz» es `u = n/ppp` puntos (n píxeles físicos): márgenes y trazos del tema van en múltiplos de `u`.

### Glifo ausente y texto que viene de fuera

`INTERFACE_CHARSET` = ASCII imprimible (95) + `ñÑáéíóúÁÉÍÓÚüÜ` (14) + `¿¡` + `×·—…` = 115 caracteres. Todo texto de la interfaz **fija** debe caer dentro. **Todo texto que venga de fuera** (nombres de fichero, errores del sistema, informes de conversión, avisos) pasa por `replace_missing(text, has_glyph)` antes de llegar a egui, sin excepción. Por cada carácter:

- de control (`char::is_control`, o U+2028 y U+2029) ⇒ un espacio;
- invisible o combinante ⇒ `?`: marcas bidireccionales y de formato (U+00AD, U+061C, U+200B a U+200F, U+202A a U+202E, U+2060 a U+206F, U+FEFF, U+FFF9 a U+FFFB), selectores de variación (U+FE00 a U+FE0F, U+E0100 a U+E01EF) y marcas combinantes (U+0300 a U+036F, U+1AB0 a U+1AFF, U+1DC0 a U+1DFF, U+20D0 a U+20FF, U+FE20 a U+FE2F). Así un nombre con `U+202E` no se puede leer al revés y el ancho sigue siendo «7·n por carácter» (la fuente es monoespaciada y una marca combinante no ocuparía sitio);
- sin glifo en la fuente ⇒ `?` (japonés, emoji, escrituras sin cubrir: cada carácter da un `?`, también las secuencias de emoji con U+200D);
- el resto, igual.

La salida tiene siempre **el mismo número de caracteres** que la entrada. El **truncado es propio** (no el `truncate()` de egui): `fit_text(text, max_chars, has_glyph)` deja el texto si cabe y, si no, recorta y termina en `…` **solo si la fuente tiene ese glifo** y en `...` si no; con menos sitio que el puntos suspensivos, los primeros `max_chars` caracteres de él; con 0, vacío. Funciona en tiempo lineal y sin pánico con nombres de 100 000 caracteres o más. Política de egui: `MissingGlyphPolicy::Panic` en `debug_assertions` y en los tests, `Tofu` en release; como todo texto externo pasa por `replace_missing`, el `Panic` solo puede saltar por un texto fijo fuera de `INTERFACE_CHARSET`.

### Tema mínimo (fondo neutro; color solo para acentos y avisos)

Los grises son neutros (sin tono): `gris(p) = round(p·255/100)` para las superficies y el texto. Los dos tonos de color salen de la misma regla en HSL, elegida aquí y no copiada de ninguna paleta: acento 168° (62 %, 52 %) y aviso 28° (85 %, 58 %). Se usan **solo** para la selección y el aviso. Contrastes medidos (WCAG, sobre superficie 0 / 1 / 2 / 3):

| Constante | Valor | Contraste |
| --- | --- | --- |
| `SURFACE_0` (campos de texto) | `#1a1a1a` (gris 10 %) | – |
| `SURFACE_1` (paneles, barra de estado) | `#242424` (14 %) | – |
| `SURFACE_2` (ventanas, botones) | `#303030` (19 %) | – |
| `SURFACE_3` (botón con el ratón encima) | `#424242` (26 %) | – |
| `TEXT` | `#e0e0e0` (88 %) | 13,2 / 11,8 / 10,0 / 7,6 |
| `TEXT_DIM` | `#a3a3a3` (64 %) | 6,9 / 6,2 / 5,2 / 4,0 |
| `ACCENT` | `#39d0b2` | 9,0 / 8,0 / 6,8 / 5,2 |
| `WARNING` | `#ef8e39` | 7,1 / 6,4 / 5,4 / 4,1 |

`contrast_ratio(a, b)` es una **función pura** (WCAG 2.x: `(L1 + 0,05) / (L2 + 0,05)`, con `L1 ≥ L2` la luminancia relativa de cada color) y un test exige **al menos 4,5:1 entre cada texto y cada fondo con el que se usa**: `TEXT` sobre las cuatro superficies (aquí se exige 7); `TEXT_DIM`, `ACCENT` y `WARNING` sobre las superficies 0 a 2 (sobre la 3 no se usan). **Los avisos no dependen del color**: llevan el prefijo de texto `aviso: ` (`NOTICE_PREFIX`), y el color `WARNING` solo lo refuerza.

`visuals(ppp) -> egui::Visuals`: modo oscuro, `panel_fill = SURFACE_1`, `window_fill = SURFACE_2`, `extreme_bg_color = SURFACE_0`, `faint_bg_color = SURFACE_2`, selección con `ACCENT`, **esquinas a 0** y **sin sombras** (aspecto de píxeles), trazos de `u = text_scale(ppp)/ppp` puntos (n píxeles físicos). El fondo del lienzo (gris 40) no cambia en esta unidad.

### Barra de estado

`px_ui::status::show(ui, text, is_notice)`: un `egui::Panel::bottom("estado")` con marco `SURFACE_1`, trazo superior de `u`, margen interior de 3·`u` y una etiqueta de **una línea sin ajuste ni truncado de egui**: el texto ya viene de `bar_text` (`NOTICE_PREFIX` si es aviso, luego `replace_missing` y `fit_text` con `columns(ancho_útil_px, ppp)` = `floor(ancho / (7·n))` columnas). **Altura = (14 + 2·3)·n = 20·n píxeles físicos**, también con textos larguísimos. Se pinta **antes** del lienzo, que ya usa `available_rect_before_wrap`, se queda con el resto y calcula la conversión pantalla-imagen y el ajuste de vista (`Home`) con **su** rectángulo, no con el de la ventana. Color `TEXT`; con aviso, `WARNING` y el prefijo. El texto sale de `status_line(tool, &TitleInfo)` en `px-app/src/title.rs`: es el título **sin** el prefijo `Pixoko: ` ni el nombre de fichero (`64×48 · ×8 · (3, 4) · línea · color 3 #ff0000 · deshacer`; con error o aviso, solo ese texto). `window_title` pasa a ser `format!("Pixoko: {}", status_line(..))` y `title_with_file` no cambia: **el título sale idéntico** y los tests de las unidades 6 y 7 siguen igual. `px-app --licencias` (`wants_licenses(args)`) imprime `docs/THIRD-PARTY.md` (`include_str!`) y sale con 0, antes de abrir ventana.

## Piezas (firmas)

```rust
// px-ui/src/text.rs
pub const FONT_BYTES: &[u8];            // assets/fonts/DepartureMono-Regular.otf
pub const FONT_LICENSE: &str;           // assets/fonts/LICENSE
pub const FONT_EM_PX: f32 = 11.0;
pub const MAX_TEXT_SCALE: u32 = 8;
pub const INTERFACE_CHARSET: &str;      // los 115 caracteres
pub fn text_scale(ppp: f32) -> u32;
pub fn font_size_points(ppp: f32) -> f32;     // 11·n/ppp
pub fn heading_size_points(ppp: f32) -> f32;  // 22·n/ppp
pub fn ui_unit(ppp: f32) -> f32;              // n/ppp
pub fn install_fonts(ctx: &egui::Context);
pub fn apply_scale(ctx: &egui::Context);      // estilos `Small`, `Body`, `Button`, `Monospace` y `Heading`
pub fn replace_missing(text: &str, has_glyph: impl Fn(char) -> bool) -> String;
pub fn fit_text(text: &str, max_chars: usize, has_glyph: impl Fn(char) -> bool) -> String;
pub fn printable(ctx: &egui::Context, text: &str) -> String;   // replace_missing con las fuentes de `ctx`

// px-ui/src/theme.rs
pub const SURFACE_0: Color32; /* … SURFACE_3, TEXT, TEXT_DIM, ACCENT, WARNING */
pub const NOTICE_PREFIX: &str = "aviso: ";
pub fn contrast_ratio(a: Color32, b: Color32) -> f64;
pub fn visuals(ppp: f32) -> egui::Visuals;

// px-ui/src/status.rs
pub fn bar_height_px(n: u32) -> u32;                   // 20·n
pub fn columns(width_px: f32, ppp: f32) -> usize;       // floor(width / (7·n)), 0 si no es finito o < 0
pub fn bar_text(ctx: &egui::Context, text: &str, is_notice: bool, columns: usize) -> String;
pub fn show(ui: &mut egui::Ui, text: &str, is_notice: bool);

// px-app/src/title.rs
pub fn status_line(tool: px_ui::tool::Tool, info: &TitleInfo) -> String;
// px-app/src/licenses.rs (módulo nuevo: `files.rs` y sus tests no se tocan)
pub const THIRD_PARTY: &str;            // docs/THIRD-PARTY.md
pub fn wants_licenses(args: &[std::ffi::OsString]) -> bool;
```

## Casos límite

`has_glyph`, `layout_no_wrap` y `tessellate` de un `egui::Context` sin ventana, con `install_fonts` y `apply_scale` aplicados; `ppp` como se indique.

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | `text_scale` con 0,5; 1,0; 1,25; 1,49; 1,5; 1,75; 2,0; 2,5; 3,0; 8,0; 20,0; y NaN, 0, −1, ∞ | 1, 1, 1, 1, 2, 2, 2, 3, 3, 8, 8; y 1, 1, 1, 1 |
| 2 | `font_size_points(ppp) · ppp` con ppp 1, 1,25, 1,5, 2, 2,5, 3 (y el de `Heading`) | 11, 11, 22, 22, 33, 33 (±0,001); `Heading` el doble |
| 3 | `has_glyph` para cada uno de los 115 caracteres de `INTERFACE_CHARSET` | `true` en todos |
| 4 | `has_glyph` de `日`, `✓` (U+2713), `😀`, y `has_glyph('…')`; `replace_missing("a日b✓c", ..)`; `"a\0b\x1bc\r\n\u{2028}"`; `""`; `"straße €"` | `false` los tres y `true`; `"a?b?c"`; `"a b c   "` (cada control, un espacio); `""`; `"straße €"` igual (`ß` y `€` sí tienen glifo) |
| 5 | Con `MissingGlyphPolicy::Panic`: maquetar el texto fijo (las etiquetas de `Tool`, `status_line` con cada herramienta y con y sin cursor, color, deshacer y rehacer, y el «Pixoko») y, aparte, maquetar `"a日b"` | lo primero no entra en pánico; lo segundo, sí (la guardia funciona) |
| 6 | Tamaño de `"Pixoko"` ×ppp con ppp 1, 1,5, 2, 3; y de los 115 caracteres seguidos con ppp 1,5 | (42, 14), (84, 28), (84, 28), (126, 42) px (±0,05 hasta ppp 3; el tamaño maquetado se cuantiza a ppp/64 px, y la propiedad 3 usa `max(0,05; ppp/64 + 0,005)`); 805·2 = 1 610 px de ancho |
| 7 | `"Pixoko: 64×48 · ×8 · (12, 7) · lápiz ñÑ ¿Qué?"` en el origen (3,37; 5,11) pt, ppp 1; 1,25; 1,5; 1,75; 2; 2,5; 3: vértices de cada glifo ×ppp, uv ×tamaño del atlas, y alfa del atlas bajo cada glifo | enteros (±0,001 y ±0,01) y alfa ∈ {0, 255}: **0 glifos fuera de píxel, 0 texels intermedios** (medido) |
| 8 | `install_fonts`; `FONT_BYTES`; `FONT_LICENSE`; `docs/THIRD-PARTY.md`; `.gitattributes` | una sola fuente `DepartureMono` en `Proportional` y `Monospace`, con `hinting` y `subpixel_binning` en `Some(false)`. Una función `sha256` escrita en el propio test (sin dependencias) pasa antes sus vectores (`""` ⇒ `e3b0c442…b855` y `"abc"` ⇒ `ba7816bf…15ad`). `sha256(FONT_BYTES)` = `4d53f663155cf8bf7ffc8e688776e719625f7bbb80a8d90073438b249261a2e0` (84 480 B, empieza por `OTTO`) y `sha256(FONT_LICENSE)` = `b65e42750f3cb65437a97f4dfd781c58f199259949db2341012be60e32500e83`; `FONT_LICENSE` empieza por `Copyright 2022–2024 Helena Zhang (helenazhang.com).`. `docs/THIRD-PARTY.md` contiene `Departure Mono`, `1.500`, `https://github.com/rektdeckard/departure-mono`, `public/assets/LICENSE`, `OFL-1.1`, el copyright literal, los dos SHA-256, la declaración de que no hay *Reserved Font Name* y el texto completo de `FONT_LICENSE`; **no** contiene `MIT License` ni `Tobias Fried`. `.gitattributes` contiene la línea `assets/fonts/** binary` |
| 9 | `apply_scale` a ppp 1,5; luego a 2,0; dos veces seguidas | `Body` 14,667 pt y `Heading` 29,333 pt; después 11 y 22 pt; la segunda llamada no cambia el estilo |
| 10 | `status_line(Line, info completo)`; sin color; sin cursor; sin tamaño (`Pencil`); con error; y `window_title` y `title_with_file` en los cuatro | `64×48 · ×8 · (3, 4) · línea · color 3 #ff0000 · deshacer · rehacer`; sin ese campo; sin ese campo; ` · ×1 · lápiz` (queda el hueco del tamaño, como en el título de hoy: `Pixoko:  · ×1 · lápiz`); solo el error. `window_title == "Pixoko: " + status_line`; `title_with_file` igual que hoy (`casa.png* — Pixoko: …`) |
| 11 | Banco con `Canvas` y la barra, ppp 1, 1,5, 2; `bar_text` de 400 caracteres con `columns` de 40 | alto de la barra 20, 40, 40 px físicos (±0,05 px: egui cuantiza el rectángulo del panel a 1/32 pt y a 1,5× sale 39,984; además el alto del lienzo en píxeles enteros es exactamente pantalla menos barra); el rectángulo del lienzo es la pantalla menos la barra; esquina de la barra en píxel entero; el texto largo da 40 caracteres y termina en `…`, y el alto no cambia |
| 12 | `contrast_ratio`: blanco con negro, un color consigo mismo, `#767676` con blanco, y con los argumentos intercambiados. Después, `TEXT` sobre las cuatro superficies; `TEXT_DIM`, `ACCENT` y `WARNING` sobre las tres primeras; `visuals(ppp)`; `NOTICE_PREFIX` | 21; 1; 4,54 (±0,01); el mismo valor. Contraste ≥ 7 para `TEXT` y ≥ 4,5 para los demás; las cuatro superficies y el texto son grises (r = g = b) y valen `#1a1a1a`, `#242424`, `#303030`, `#424242` y `#e0e0e0`; todos los colores son opacos y distintos; `panel_fill`, `window_fill` y `extreme_bg_color` son `SURFACE_1`, `SURFACE_2` y `SURFACE_0`; esquinas y sombras a 0; el prefijo es `aviso: ` |
| 13 | **Lienzo desplazado por la barra.** Imagen RGBA 64×48, pantalla de 400×300 pt, la barra a ppp 1 (20 px) y a ppp 2 (40 px físicos, 20 pt); `Home`, y el puntero sobre el píxel (2, 3) y sobre la barra | ppp 1: rectángulo del lienzo (0, 0)–(400, 280); `Home` da **×5** (con el rectángulo de la ventana saldría ×6), la imagen ocupa (40, 20)–(360, 260) y el puntero en (52,5; 37,5), el centro del píxel (2, 3), lo da. ppp 2: lienzo de 800×560 px físicos; `Home` da **×11** (con el de la ventana, ×12), la imagen ocupa 704×528 px desde (48, 16) px y el puntero en el centro del píxel (2, 3) lo da. En los dos, el puntero sobre la barra da `hover` ninguno y ningún evento de trazo |
| 14 | **Texto externo** por `replace_missing` con el `has_glyph` de la fuente: `"日本語.png"`; `"😀 ok"`; `"👨\u{200D}👩\u{200D}👧"`; `"a\u{202E}b\u{202C}c"`; `"\u{200B}x\u{FEFF}"`; `"e\u{301}"`; `"n\u{303}"` y `"ñ"`; y un informe de conversión de ejemplo con `ñ`, `×` y `…` | `"???.png"`; `"? ok"`; `"?????"`; `"a?b?c"`; `"?x?"`; `"e?"`; `"n?"` y `"ñ"` (precompuesta, igual); el informe igual. Siempre mismo número de caracteres, y todos con glifo; con `MissingGlyphPolicy::Panic`, maquetar cada resultado no entra en pánico |
| 15 | `fit_text("abcdef", n, ..)` con `n` = 6, 5, 3, 2, 1, 0, con `…` en la fuente y sin él; `""`; un nombre de 100 000 caracteres con `n` = 40 | `"abcdef"`; con `…`: `"abcd…"`, `"ab…"`, `"a…"`, `"…"`, `""`; sin `…`: `"abcdef"`, `"ab..."`, `"..."`, `".."`, `"."`, `""`; vacío da vacío; 39 caracteres y `…` (40) sin pánico |
| 16 | `bar_text(ctx, "se ignora el fichero", true, 100)`; con `false`; con `"no se pudo leer 日本.png"` y aviso; el primero con `columns` = 10; `columns(400, 1,0)`, `columns(800, 1,5)`, `columns(800, 2,0)`, y `columns` con −1, NaN e ∞ | `"aviso: se ignora el fichero"`; sin prefijo; `"aviso: no se pudo leer ??.png"`; `"aviso: se…"`; 57, 57, 57; 0, 0, 0 |
| 17 | `wants_licenses` con `[]`, `["--licencias"]`, `["a.png"]`, `["a.png", "--licencias"]`, `["--licencia"]`, `["--LICENCIAS"]` y un `OsString` que no es UTF-8 | `false`, `true`, `false`, `true`, `false`, `false`, `false` y sin pánico; `parse_args` no cambia |

## Propiedades (proptest)

1. **Escala.** Para cualquier `f32`: `text_scale` está en 1 a 8 y no decrece con ppp; con ppp en [0,5; 8) difiere de ppp en no más de 0,5; y para ppp en [0,25; 16], `font_size_points(ppp) · ppp` = 11·`text_scale(ppp)` (±0,001, y **nunca por debajo** en `f32`: a 1,3 daría 10,999999 y el rasterizador alfa 254; `font_size_points` sube un ulp hasta cumplirlo).
2. **Nitidez geométrica.** Para ppp múltiplo de 0,05 entre 1 y 4, un origen cualquiera y una cadena de 1 a 60 caracteres de `INTERFACE_CHARSET`: se cumple la fila 7 (esquinas en píxel entero, uv en texel entero, alfa 0 o 255).
3. **Medida.** Con la misma entrada: ancho = 7·n·(número de caracteres) y alto = 14·n píxeles (±0,05).
4. **Texto externo.** Para cualquier `String` (con sesgo hacia controles, marcas bidireccionales, combinantes, emoji y japonés) y el `has_glyph` de la fuente: `replace_missing` da el mismo número de caracteres, solo caracteres con glifo y no invisibles o combinantes (más `?` y espacios), es idempotente y es la identidad sobre cualquier cadena de `INTERFACE_CHARSET`. `fit_text` (con y sin `…`) sobre cualquier cadena y `max_chars`: nunca más de `max_chars` caracteres; igual que la entrada si cabe; si no, termina en `…` o en `...` y tiene exactamente `max_chars` (si `max_chars` ≥ longitud del sufijo).
5. **Título intacto.** Para cualquier herramienta y `TitleInfo` sin error: `window_title == "Pixoko: " + status_line`, `status_line` solo usa caracteres de `INTERFACE_CHARSET` y no lleva saltos de línea.

## Dónde van las pruebas

`px-ui/tests/text.rs` (filas 1 a 9, 14 y 15; propiedades 1 a 4; el `sha256` propio del test y su prueba con vectores), `px-ui/tests/theme.rs` (fila 12) y `px-ui/tests/status.rs` (filas 11, 13 y 16; el banco de egui y `Canvas` de `tests/canvas.rs`, copiado y no modificado). En `px-app`: un módulo nuevo de tests en `src/title.rs` (fila 10 y propiedad 5; no se tocan los que ya tiene) y `src/licenses.rs` con sus tests (fila 17). Las filas 8 y 12 leen `docs/THIRD-PARTY.md` y `.gitattributes` con `CARGO_MANIFEST_DIR`. No se tocan los tests de las unidades anteriores. Orden: spec aprobada y regla de `CLAUDE.md`; fuente, licencia, `.gitattributes` y `THIRD-PARTY.md`; `text.rs`; tema y barra; `title.rs`, `licenses.rs` y `main.rs`; `docs/COSTES.md`. Un commit por pieza, cada uno con fmt, clippy, tests con `--locked` y `cargo deny` en verde, y revisión de `license-auditor`; no se toca `deny.toml`.

## Prueba manual

`cargo run --release -p px-app`, en Linux, Windows y macOS, y a **1,0, 1,5 y 2,0** (Windows y macOS: ajustes de pantalla, 100 %, 150 % y 200 %; Linux con X11: `WINIT_X11_SCALE_FACTOR=1.5`; con Wayland, la escala del compositor). También 1,25 y 3,0 si se puede. Qué ver, con una lupa o una captura ampliada al 800 %:

1. La barra de estado abajo, con `lápiz · color 1 #…` y `×N` y las coordenadas al mover el ratón sobre la imagen; **el título de la ventana dice lo mismo que antes**.
2. **Nitidez:** cada trazo del texto mide un número entero de píxeles físicos y el mismo grosor en toda la línea; sin bordes grises ni trazos más gruesos unos que otros. A 1,5× el texto es el doble de grande que a 1,0× (n = 2): es lo decidido, no un fallo.
3. Texto externo: con las herramientas `lápiz` y `línea` (`P` y `L`) se ven la `á` y la `í` de la barra. **Arrastrar ficheros que no se puedan abrir** con estos nombres, uno por uno: `piñata ¿qué?.txt` (se ven `ñ`, `¿` y `?` sin cuadrados), `日本語 😀.txt` (se ven `?` en su lugar), uno con una marca bidireccional (`U+202E`, el nombre no se lee al revés: sale un `?`), `e` con acento combinado (`e` + `U+0301`: sale `e?`) y uno de 300 caracteres (termina en `…`, la barra no crece). Siempre con el prefijo `aviso: `, y en color de aviso. Si el error no nombra el fichero, probar con el aviso de cambios sin guardar al soltar un PNG con ese nombre.
4. Con cambios sin guardar, cerrar la ventana: el aviso aparece en la barra en color de aviso y desaparece a los 5 s.
5. Estrechar la ventana: el texto termina en `…` y la barra no crece. El lienzo no se solapa con la barra y el zoom, el desplazamiento y el lápiz funcionan como antes.
6. Mover la ventana entre dos monitores de distinta escala (si hay): el texto se reajusta sin reiniciar y sigue nítido.
7. `px-app --licencias` imprime `docs/THIRD-PARTY.md`, con la licencia OFL completa de Departure Mono y su copyright, y sale sin abrir ventana.
8. En macOS y Windows, además, que OpenGL no cambie la nitidez (riesgo ya anotado en la unidad 2).

## Fuera de alcance

Barra de herramientas, panel de paletas y diálogos (unidad 11), cuadro «Acerca de» dentro de la ventana, nombres de fichero en la barra (siguen en el título), selector de escala de interfaz, texto en varios idiomas o con caracteres fuera de `INTERFACE_CHARSET` (CJK, emoji), la opción 3b, tema claro, cambiar el fondo del lienzo.

## Resolución del visto bueno (2026-10-18)

Decisiones 1 a 10 aprobadas como se recomendaban (opción 2; Departure Mono sin recortar; escala por redondeo; tamaño 11·n y `Heading` doble; glifo ausente; aviso de licencia; colores; barra; numeración 11a; opción 3b aplazada), con estos matices, ya incorporados arriba: **(5)** texto externo siempre por `replace_missing`, con las pruebas de la fila 14 y la propiedad 4, y truncado propio con `…` solo si la fuente lo tiene (`fit_text`, fila 15); **(6)** licencia OFL de `public/assets/LICENSE`, ficheros puestos por el usuario en `assets/fonts/` y no tocados, SHA-256 fijado en la fila 8, `THIRD-PARTY.md` con el copyright literal y el *Reserved Font Name* (ninguno), `.gitattributes` con `assets/fonts/** binary`; **(7)** fondo neutro, `contrast_ratio` puro con mínimo 4,5:1 (fila 12) y avisos con el prefijo `aviso: `; **(8)** fila 13, el lienzo desplazado por la barra, con `Home` sobre el rectángulo del lienzo; **(10)** el coste real en tokens de la unidad, medido, va a `docs/COSTES.md` junto a la estimación de 20 000 a 25 000; **(11)** regla nueva en `CLAUDE.md`: no se hacen capturas de pantalla del escritorio y la comprobación visual la hace el usuario.
