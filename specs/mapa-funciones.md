# Mapa de funciones y reparto de los hitos 3 a 8

Estado: **aprobada** (2026-10-21), con las decisiones de la sección 8 y cuatro cambios pedidos al dar el visto bueno: el hito 4 baja a 14 unidades, cada unidad lleva su clase (núcleo, deseable u opcional), los casos manuales se piden en una lista única al empezar cada hito y `.aseprite` entra por etapas. Solo reparto: no es una spec de comportamiento ni tiene tests ni toca código. Es el equivalente, para los hitos 3 a 8, de [`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md): cada unidad que sale de aquí tendrá su propia spec (máximo 12 filas y 4 propiedades) y su visto bueno antes de escribir pruebas.

**Cómo se hizo.** Se leyeron `PLAN-RESUMEN.md`, `specs/INDEX.md`, `DISENO-INTERFAZ.md`, `hito-2-resumen.md`, `hito-2-reparto-cierre.md` y `REFERENCIAS.md`, y el código de `px-core` (`Command`, `Document`), `px-raster` y `px-ui` (`Tool`). El manual privado (fuera del repositorio) se leyó por tramos: casi toda la sección Reference (herramientas, menús Imagen, Pincel, Modo, Animación, Color, Efecto, Macro y Preferencias), los tutoriales de colores, rangos y degradado con translucidez, el modelo básico de animación y la mesa de luz. Todo lo de abajo está descrito con palabras propias; las teclas y los nombres de función se citan porque no están protegidos, el texto sí.

**Leyenda.**
- *Hoy*: `sí` (ya existe), `parcial`, `no`, `—` (no aplica).
- *Hito*: 3 a 8 con la unidad entre paréntesis, o `descartada`.
- *Riesgo*: `bajo`, `medio`, `alto`, con el motivo.
- *Corpus*: `—` sin corpus ajeno (propiedades y casos propios); `AS` Aseprite por lotes (**única referencia automática de los hitos 3 a 5**; `aseprite -b` con scripts de Lua propios de `tools/`, que crean los casos, guardan los `.aseprite` y exportan PNG, GIF u hojas; el usuario los ejecuta, los resultados se commitean en `golden/aseprite/` y el CI no los ejecuta; **cada unidad confirma en su spec que Aseprite tiene la función antes de usarlo como oráculo; si no la tiene, se anota y se decide**); `G2` Grafx2 (solo interfaz gráfica: sesión manual del usuario que guarda ficheros); `DP` DPaint V en emulador (sesión manual y conversión a PNG fuera del repositorio). Un corpus es siempre un fichero guardado por el programa, nunca una captura de pantalla.
- *Clase de unidad*: **núcleo** (sin ella no sale la primera versión pública, la del hito 8), **deseable** (entra si el plazo lo permite y no bloquea nada de lo núcleo) y **opcional** (la primera versión sale sin ella). Si una unidad núcleo dependiera de una deseable u opcional, la spec de la núcleo lo declara y se parte.
- *Decisiones*: `D01`…`D30`, en la sección 8.

## 1. Resumen y orden

| Hito | Unidades | Núcleo · deseable · opcional | Toca `px-core` o `px-raster` (solo con los cambios exactos que liste la spec, D02) | Dependencias nuevas posibles | Corpus |
| --- | --- | --- | --- | --- | --- |
| 3 · Capas, alfa y referencias | 10 (3.0 a 3.9) | 7 · 3 · 0 | 3.1 y 3.2 (no aditivas), 3.5 a 3.7 (comandos nuevos) | `miniz_oxide` directa para `.aseprite` (D10; ya está en el árbol) | `AS` por lotes (`specs/corpus-hito-3.md`) |
| 4 · Herramientas de pixel art | 14 (4.1 a 4.14) | 8 · 4 · 2 | 4.1, 4.2, 4.5, 4.6, 4.7 si D17 lo pide | fuente (4.13, solo si falta rasterizador) | `AS` en formas, relleno y cuantizador; `G2` y `DP` manuales (lista única) |
| 5 · Animación | 14 (5.0 a 5.13) | 9 · 2 · 3 | 5.0 | `gif` (D25) | `AS` por lotes (GIF, hojas) |
| 6 · Más formatos | 5 (6.1 a 6.5) | 0 · 3 · 2 | ninguna | posible `zip` | ficheros ajenos |
| 7 · Estilo DPaint (opcional) | 8 (7.1 a 7.8) | 0 · 0 · 8 | 7.1 y 7.2 probables | ninguna | `DP` |
| 8 · Automatización y lanzamiento | 10 (8.1 a 8.10) | 3 · 4 · 3 | 8.7 | scripting, portapapeles, i18n | — |
| **Total** | **61** | **27 · 16 · 18** | | | |

**Orden propuesto.** 3 → 4 → 5 en serie, con una excepción posible: tras 3.1 a 3.3 el resto del 3 y el 4 pueden correr en worktrees distintos, porque las herramientas del 4 ya escriben en «la capa activa» (D01, se confirma en la primera spec del 4). El 6 puede ir en paralelo con cualquiera desde 3.8. El 7 espera al 4 y al 5. El 8 se reparte: 8.1 (preferencias) y 8.2 (CLI) pueden adelantarse al 5.

**Reglas que se mantienen** (de `CLAUDE.md`): `test-writer` solo en `px-core`, `px-formats`, `px-raster` y lógica delicada; `license-auditor` solo con dependencias nuevas o cambios en esos tres crates; una prueba anterior que cambie más que firmas, literales o cifras detiene la unidad; informe final de 20 líneas.

**Aprobación de `px-core` y `px-raster` (D02).** El visto bueno a la spec de una unidad marcada vale como aprobación expresa **solo para los cambios exactos que la spec liste**; si durante la unidad hace falta algo más, se para y se pregunta. Las specs de 3.1 y 3.2 (modelo de capas) lo dicen de entrada: **no son aditivas**, y proponen el plan de migración (qué firmas y pruebas cambian, en qué commits y con qué orden).

**Lista única de casos manuales por hito.** Al empezar cada hito (3, 4, 5 y, cuando toque, 7), antes de la primera unidad y sin contar como unidad, se escribe `specs/corpus-hito-N.md` con **todos** los casos que van a necesitar corpus ajeno, agrupados por programa y por secuencia de trabajo para que el usuario los genere de una sentada: identificador, nombre exacto del fichero (`golden/<programa>/h<N>-<id>.png`), procedimiento paso a paso y unidad que lo consume. Las specs de unidad citan los casos por identificador y no piden sesiones sueltas. Con `AS` el procedimiento es el guion por lotes que ejecuta el usuario; con `G2` y `DP`, pasos de interfaz y un fichero guardado por caso.

## 2. Mapa de funciones

### 2.1 Herramientas de dibujo

| ID | Función (original) | Qué hace | Hoy | Hito | Depende de | Riesgo | Corpus |
| --- | --- | --- | --- | --- | --- | --- | --- |
| T01 | Pinceles integrados | Diez formas fijas (redondas y cuadradas) con tamaño de 1 a 100 px; el tamaño se cambia arrastrando y con teclas | no (solo 1 px) | 4 (4.3) | 3.3 | medio: la forma exacta de los redondos pequeños es una decisión nuestra | `AS` `G2` |
| T02 | Mano alzada punteada (`s`) | Estampa el pincel en cada muestra del ratón sin unirlas; si se mueve rápido quedan huecos | no | 4 (4.3) | T01 | medio: depende del muestreo; se prueba con trazas de eventos grabadas | — |
| T03 | Mano alzada continua (`d`) | Une las muestras con segmentos hechos con el pincel | parcial (lápiz de 1 px) | 4 (4.3) | T01 | bajo | `AS` |
| T04 | Forma libre rellena (`D`) | Al soltar cierra el trazo con una recta y rellena el interior con el tipo de relleno activo | no | 4 (4.4) | T26, T10 | medio: regla de interior (par-impar) y borde | `AS` `G2` |
| T05 | Línea (`v`) | Segmento con el grosor del pincel | parcial (1 px) | 4 (4.3) | T01 | bajo | `AS` `G2` |
| T06 | Espaciado (requisito de la línea y de las formas sin relleno) | Elige cómo se reparten los sellos: continuo, uno cada N píxeles o N sellos en total | no | 4 (4.3) | T05 | medio: «N en total» sobre curvas y formas cerradas | — |
| T07 | Trazas (`Ctrl` al arrastrar) | Deja pintada cada posición intermedia de la figura mientras se arrastra | no | 4 (4.3) | T05 | bajo: un solo paso de deshacer por gesto | — |
| T08 | Restringir a un eje (`Mayús`) | Fuerza horizontal o vertical según el primer movimiento tras pulsar | no | 4 (4.3) | T05 | bajo | — |
| T09 | Curva (`q`) | Arco entre dos puntos; un tercer clic fija la curvatura | no | 4 (4.3) | T05 | alto: curva (cuadrática o cúbica) y rasterizado entero deterministas; sin oráculo cómodo | `DP` |
| T10 | Relleno (`f`) | Inunda una zona contigua con el color activo | sí | — | — | — | — |
| T11 | Relleno hasta el color de fondo (`i`+clic) | Extiende el relleno hacia fuera hasta tocar el color de fondo; sirve para zonas con degradado | no | 4 (4.5) | T10 | medio | `DP` |
| T12 | Relleno con patrón | Rellena con un mosaico hecho desde el pincel actual, alineado a la página | no | 4 (4.5) | T10, 4.8 | bajo: origen del patrón | `G2` |
| T13 | Relleno con pincel (estirado) | Una copia del pincel propio escalada para cubrir la zona | no | 7 (7.6) | 4.8 | medio | `DP` |
| T14 | Relleno envolvente | El pincel adaptado a la forma horizontal y vertical de la zona, como si se envolviera un sólido | no | 7 (7.6) | T13 | alto: deformación sin definición clara | `DP` |
| T15 | Relleno en perspectiva | Mosaico del pincel proyectado en un plano 3D | no | 7 (7.2) | E03 | alto | `DP` |
| T16 | Relleno de modo Halfbrite | Oscurece o aclara con la mitad de intensidad (solo modo de pantalla del Amiga) | — | descartada | — | — | — |
| T17 | Degradados clásicos: horizontal, vertical y por línea | Reparten un rango de colores en esa dirección; el de «por línea» ajusta cada fila al contorno | no | 4 (4.7) | C16 | medio | `G2` `AS` |
| T18 | Degradado direccional recto y ajustado a la forma | La dirección se indica con una línea desde el centro; el ajustado sigue los contornos de la figura | no | 4 (4.7) | T17 | alto: definición propia de «seguir la forma» | `DP` |
| T19 | Degradado radial: circular, contornos y luz | Irradia desde el punto elegido; contornos y luz tienen en cuenta la forma y se parecen entre sí | no | 4 (4.7) | T18 | alto: distancia al borde, sin definición en el manual | `DP` |
| T20 | Tramado del degradado: aleatorio, de patrón y suave | Mezcla colores vecinos para reducir saltos; el aleatorio se controla con un deslizador y el suave es difusión de error | no | 4 (4.7) y 4.2 | C16 | medio: semilla fija para que sea reproducible | `AS` `G2` |
| T21 | Relleno animado (`Alt`+clic) | Rellena en cada fotograma desde el mismo punto | no | 5 (5.9) | T10, 5.0 | medio: sin `Alt`; botón y tecla de alternancia en el panel de animación (D03, D24) | — |
| T22 | Aerógrafo | Rocía el pincel: estilo aleatorio y estilo estándar que acumula opacidad; radio y caudal de 1 a 100; el estándar solo con pinceles redondos | no | 4 (4.11) | T01, M12 | medio: aleatoriedad con semilla por trazo; en indexado depende del contraste de la paleta | — |
| T23 | Rectángulo (`r`, `R`) | Contorno con el pincel o relleno según el tipo activo; `Mayús` iguala los lados | no | 4 (4.4) | T05 | bajo | `AS` `G2` |
| T24 | Círculo (`c`, `C`) | Desde el centro arrastrando el radio | no | 4 (4.4) | T05 | medio: algoritmo de píxeles exactos; las referencias pueden discrepar | `AS` `G2` |
| T25 | Elipse (`e`, `E`) | Centro y dos radios; arrastrando en círculo se gira (el ángulo se muestra) | no | 4 (4.4; sin girar) y 4.12 (girada) | T24 | alto la girada: rasterizado entero | `DP` |
| T26 | Polígono (`w`, `W`) | Un clic por esquina; `Espacio` lo cierra (solo relleno) | no | 4 (4.4) | T05 | medio | `AS` `G2` |
| T27 | Contorno y relleno a la vez (`i` al elegir la herramienta de relleno) | Rellena la forma y la contornea con el pincel, que puede ser de otro color | no | 4 (4.4) | T23 a T26 | bajo: opción en el panel, no una letra | — |
| T28 | Selector de pincel rectangular (`b`) | Recoge un rectángulo de la imagen como pincel propio; `Mayús` lo hace cuadrado | no | 4 (4.8) | 3.3 | medio | — |
| T29 | Selector poligonal y a mano alzada | Recoge una zona irregular dibujando su contorno; `Espacio` lo cierra | no | 4 (4.8) | T28, T26 | medio | — |
| T30 | Varita mágica de pincel | Un clic recoge la zona contigua del mismo color (con `Ctrl`, solo ese color) | no | 4 (4.8) | T28, T10 | medio: contigüidad y pincel transparente de un píxel | — |
| T31 | Copiar o cortar al recoger | El botón izquierdo copia; el derecho corta y deja el color de fondo | no | 4 (4.8) | T28 | bajo | — |
| T32 | Restaurar el último pincel propio (`B`, clic derecho en el selector) | Vuelve al pincel propio anterior tras usar uno integrado o transformarlo | no | 4 (4.8) | T28 | bajo | — |
| T33 | Transparencia del pincel recogido (color de fondo y, opcional, esquinas) | El color de fondo del momento se vuelve transparente; con la opción de esquinas, el color común de las cuatro | no | 4 (4.8) | T28 | bajo | — |
| T34 | Texto (`t`) | Escribe con el color de primer plano; la tecla lleva cada carácter, `Esc` sale | no | 4 (4.13; opcional) | D19 | medio: fuente, métricas y deshacer por carácter | — |
| T35 | Rejilla (`g`, `G`) | Ajusta al punto de rejilla las herramientas de trazo; separación X e Y configurable y ajuste arrastrando | no | 4 (4.9) | 4.3 a 4.4 | bajo: ajuste de coordenadas antes de la herramienta | — |
| T36 | Simetría de punto (`/`) | Repite lo dibujado alrededor de un centro, en espejo o cíclica, con un orden N | no | 4 (4.9) | 4.3 a 4.4 | medio: replicar la salida de cada herramienta en un solo paso | — |
| T37 | Simetría de mosaico | Copia lo dibujado a un mosaico de ancho y alto dados; ideal para patrones | no | 4 (4.9) | T36 | medio | — |
| T38 | Lupa (`m`) y zoom | Divide la pantalla y muestra ampliada una zona | parcial (zoom ×64 y alejamiento) | — | — | — | — |
| T39 | Deshacer (`u`), Borrar página (`K`), cuentagotas (`,`), borrador (`k`), indicador de color, paleta | Ya hechos en el hito 2 y en las unidades I.1 a I.4 | sí | — | — | — | — |
| T40 | Borrar fotogramas con CLR y rango | Con varios fotogramas, elige borrar el actual, un rango o todos | no | 5 (5.1) | 5.0 | bajo | — |
| T41 | Presión de tableta | Varía color, tamaño o ciclo con la presión | no | descartada por ahora (decidido el 20 de octubre) | — | — | — |

### 2.2 Color, paleta y rangos

| ID | Función (original) | Qué hace | Hoy | Hito | Depende de | Riesgo | Corpus |
| --- | --- | --- | --- | --- | --- | --- | --- |
| C01 | Editor de color RGB y HSV | Controles para editar el color elegido | sí (RGB, HSL, hex y alfa, 2.11c) | — | — | — | — |
| C02 | Zona de mezcla (pintar para mezclar colores) | Un área donde se pintan colores para obtener otros nuevos | no | descartada | — | — | — |
| C03 | Copiar color entre entradas | Copia el color de una entrada o de la imagen a otra entrada | no | 4 (4.6) | — | bajo | — |
| C04 | Intercambiar dos entradas | Cambia de sitio dos entradas; en el original la imagen se desajusta salvo que se remapee | parcial (`MovePaletteEntry` ya remapea) | 4 (4.6) | — | bajo | — |
| C05 | Degradado entre dos entradas (spread) | Rellena las entradas intermedias con una transición uniforme; el original interpola en HSV | no | 4 (4.6) | 2.10 | bajo: camino del matiz (corto o largo) | `G2` `DP` |
| C06 | Selección de varias entradas (rango contiguo y suelto) | `Mayús` rango, `Ctrl` suelto (decisión del 20 de octubre) | no | 4 (4.6) | 2.11d | bajo | — |
| C07 | Borrar entradas | Quita una o varias entradas de la paleta | parcial (una) | 4 (4.6) | C06 | bajo | — |
| C08 | Conjunto de colores ampliado (256 casillas fuera de la paleta) | Zona de trabajo de colores que no están todavía en la paleta | no | descartada | — | — | — |
| C09 | Cargar y guardar paleta | Paleta en fichero sin tocar la imagen; con la variante de añadir o sustituir parte | no | 4 (4.6) | 4.2 | bajo: formatos de texto sencillos (D13) | `G2` |
| C10 | Paleta por defecto | Restablece la paleta inicial | no | 4 (4.6) | — | bajo | — |
| C11 | Paleta del pincel y restaurar paleta | Adoptar la paleta de un pincel cargado y volver | no | 4 (4.8) | 4.8 | bajo: «volver» lo da el deshacer | — |
| C12 | Fondo → primer plano y Fondo ↔ primer plano en la imagen | Cambia (o intercambia) en toda la imagen los píxeles de un color por otro | no | 4 (4.6) | — | bajo: un `ReplaceColor` con deshacer, que el original no tiene | — |
| C13 | Remapear | Reasigna los índices de la imagen a una paleta nueva por color más cercano | parcial (`nearest`, 2.8) | 4 (4.2) | 4.2 | medio | `AS` |
| C14 | Recomputar y escala RGB4 → RGB8 | Ajustes de los modos HAM y de 4 bits del Amiga | — | descartada | — | — | — |
| C15 | Rangos (hasta 8 por imagen) | Lista ordenada de colores con posición, velocidad, transparencia por cuenta y tramado; la usan degradados, sombreado y ciclo | no | 4 (4.7) | 3.8 | alto: dónde vive y cómo se guarda (D17) | `DP` |
| C16 | Elegir y recorrer rangos | Paso al rango anterior o siguiente sin abrir el editor | no | 4 (4.7) | C15 | bajo | — |
| C17 | Ciclo de color (`Tab`) | Los colores de un rango giran entre sí; velocidad propia en fps (D23); la curva de velocidad exacta del original es opcional del 7 | no | 5 (5.8) (D23) | C15, `RotateColors` | medio: textura; puede reabrir la 14b | — |
| C18 | Ciclo de un solo registro e híbrido | Un registro recorre colores fuera de la paleta; mezcla de ambos | no | 7 (7.6) | C17 | medio | `DP` |
| C19 | Cuantizador (RGBA o gris con alfa a indexado con más de 256 colores) | Elige una paleta de N colores y reasigna los píxeles; opciones de tramado, de paleta ajena y de umbral de alfa | no | 4 (4.2) | 4.1 | alto: algoritmo, determinismo y coste a 8192² (D13) | `AS` |

### 2.3 Pincel propio

| ID | Función (original) | Qué hace | Hoy | Hito | Depende de | Riesgo | Corpus |
| --- | --- | --- | --- | --- | --- | --- | --- |
| B01 | Cargar y guardar pincel | El pincel como fichero pequeño con transparencia (PNG propio) | no | 4 (4.8) | 4.8 | bajo | — |
| B02 | Copiar y pegar pincel | Con el portapapeles | no | 4 (4.8; interno) y 8 (8.8; del sistema) | 4.8 | medio: dependencia del sistema | — |
| B03 | Pincel de reserva: guardar y cambiar | Un segundo hueco de pincel y su intercambio | no | 4 (4.8) | 4.8 | bajo | — |
| B04 | Metamorfosis de pincel | Genera un pincel animado que transforma el actual en el de reserva | no | 7 (7.3) | 5.10 | alto | `DP` |
| B05 | Estirar o encoger libre (`Z`), mitad (`h`), doble (`H`) | Cambia el tamaño; con `Mayús` conserva proporciones | no | 4 (4.8) | 4.8 | medio: algoritmo de escalado para pixel art | — |
| B06 | Doble en horizontal o en vertical (`X`, `Y`) | Corrige el aspecto de los píxeles no cuadrados del Amiga | — | descartada | — | — | — |
| B07 | Voltear (`x`, `y`) | Espejo horizontal o vertical | no | 4 (4.8) | 4.8 | bajo | — |
| B08 | Contorno: añadir o recortar un píxel (`o`, `O`) | Rodea el pincel con el color de primer plano o le quita el borde | no | 4 (4.8) | 4.8 | bajo | — |
| B09 | Girar 90° (`z`) | Giro de cuarto de vuelta | no | 4 (4.8) | 4.8 | bajo | — |
| B10 | Girar en cualquier ángulo | Giro libre arrastrando | no | 4 (4.12; opcional, D20) | 4.8 | alto: sin remuestreo que destroce el pixel art | — |
| B11 | Cizalla y curvado (horizontal y vertical) | Deformaciones del pincel | no | 7 (7.7) | 4.12 | medio | `DP` |
| B12 | Punto de agarre: centro, esquinas o colocado | Qué punto del pincel sigue al cursor | no | 4 (4.8) | 4.8 | bajo: las teclas con `Alt` se reasignan (D03) | — |
| B13 | Cuerpo sólido | Hace opacos los huecos translúcidos rodeados por píxeles opacos | no | 7 (7.7) | 4.8 | bajo | `DP` |
| B14 | Solo marco | Dibuja solo el recuadro del pincel para ir más rápido | — | descartada | — | — | — |
| B15 | Liberar pincel | Descarta los pinceles propios | no | 4 (4.8) | 4.8 | bajo | — |
| B16 | Cambiar el color transparente del pincel | Pasa a ser transparente el color de fondo actual | no | 4 (4.8) | 4.8 | bajo | — |
| B17 | Colores del pincel: fondo → primer plano, intercambio y remapeo | Las mismas operaciones de color, solo sobre el pincel | no | 4 (4.8) | C12, C13 | bajo | — |
| B18 | Excluir borde derecho e inferior al recoger con rejilla | Evita bordes dobles al usar el pincel como patrón | no | 4 (4.8) | T35 | bajo | — |

### 2.4 Modos de pintura y efectos de color

> Nota de I.5 (2026-10-21): `F1` es la ayuda y no se reasigna; las teclas `F1` a `F8` de las filas M01 a M08 (y `F1` a `F3` de la unidad 4.10) pasan a `Mayús`+`F1` a `Mayús`+`F8`.

| ID | Función (original) | Qué hace | Hoy | Hito | Depende de | Riesgo | Corpus |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M01 | Matte (`F1`) | Pincel propio tal cual, con su color transparente | no | 4 (4.10) | 4.8 | bajo | — |
| M02 | Color (`F2`) | Usa solo la silueta del pincel y la pinta con el color activo | no | 4 (4.10) | M01 | bajo | — |
| M03 | Reemplazar (`F3`) | Como Matte pero sin transparencia | no | 4 (4.10) | M01 | bajo | — |
| M04 | Difuminar a dedo (Smear, `F4`) | Arrastra los colores que hay bajo el pincel sin crear colores nuevos | no | 7 (7.5) (D16) | 4.10 | medio | `DP` |
| M05 | Sombrear (Shade, `F5`) | Avanza o retrocede un paso dentro del rango en los colores que pertenecen a él | no | 4 (4.10) (D16) | C15, 4.7 | medio | `DP` |
| M06 | Mezclar (Blend, `F6`) | Promedia colores bajo el pincel y toma el más cercano de la paleta | no | 7 (7.5) (D16) | `nearest` (2.8) | medio | `DP` |
| M07 | Ciclar al pintar (Cycle, `F7`) y ciclo múltiple | Pinta recorriendo los colores del rango; con ciclo múltiple, cada color del pincel por su rango | no | 4 (4.10; su spec decide, propuesta: con Sombrear) | C15 | medio | `DP` |
| M08 | Suavizar (Smooth, `F8`) | Suaviza bordes duros con colores intermedios de la paleta | no | 7 (7.5) (D16) | `nearest` | medio | `DP` |
| M09 | Mix | El color del pincel se mezcla en vivo con la imagen | no | 7 (7.5) (D16) | M12 | medio | `DP` |
| M10 | Rubthru | Calca a través de la página de reserva con la translucidez activa | no | 7 (7.5) | 3.6, M12 | medio | `DP` |
| M11 | Halfbrite | Modo de pantalla del Amiga | — | descartada | — | — | — |
| M12 | Translucidez (activar y porcentaje) | Pinta como un filtro: mezcla el color con lo que hay en la proporción elegida | no | 4 (4.10) | 3.2 | medio: redondeo y modos sin alfa | `DP` |
| M13 | Proceso: tinte, matiz, valor | Cambia solo una componente del color de destino | no | 7 (7.5) | M12 | medio | `DP` |
| M14 | Antialias bajo y alto | Suaviza los bordes de lo pintado y de los pinceles reducidos o girados | no | 7 (7.5) (opcional) | — | medio: va en contra del pixel art | — |
| M15 | Texturas y medios (tizas, acuarela…) | Una imagen de textura modula cómo se deposita la pintura; los medios dependen de ella | no | 7 (7.4) | M12 | alto: formato propio de medios | `DP` |

### 2.5 Imagen y páginas

| ID | Función (original) | Qué hace | Hoy | Hito | Depende de | Riesgo | Corpus |
| --- | --- | --- | --- | --- | --- | --- | --- |
| I01 | Abrir y guardar imagen | PNG por menú, soltar y `Ctrl+S` | sí | — | — | — | — |
| I02 | Nombre, autor y copyright al guardar | Metadatos del fichero | no | 8 (8.7) | 3.8 | bajo | — |
| I03 | Cargar varios ficheros como fotogramas | Una secuencia numerada se convierte en animación | no | 5 (5.7) | 5.0 | bajo | — |
| I04 | Guardar un rango de fotogramas como imágenes numeradas | Exporta una secuencia | no | 5 (5.7) | 5.0 | bajo | `AS` |
| I05 | Borrar fichero e imprimir | Los hace el sistema operativo | — | descartada | — | — | — |
| I06 | Voltear la imagen (horizontal y vertical) | Espejo de toda la página (todos los fotogramas, un rango o uno) | no | 4 (4.1) | 4.1 | bajo: todas las celdas en un paso | `AS` |
| I07 | Página de reserva: intercambiar (`j`), copiar a la reserva (`Ctrl+j`), fusionar delante o detrás, borrar | Segunda página de trabajo con su paleta propia; la fusión hace transparente el color de fondo | no | 3 (3.6) (D09) | 3.1 | medio | — |
| I08 | Tamaño de página | Cambia el tamaño del lienzo; aquí, con ancla, recorte o ampliación y relleno | no | 4 (4.1) | 4.1 | medio: memoria doble en el paso | `AS` |
| I09 | Mostrar página (`S`) | Oculta la interfaz y enseña la imagen completa | no | 8 (8.8) | — | bajo | — |
| I10 | Formato de pantalla, modos del Amiga, resolución y overscan | Ajustes de hardware | — | descartada | — | — | — |
| I11 | Convertir modo | Entre RGBA, indexado, gris y gris con alfa, con informe | sí (2.20) | — | — | — | — |
| I12 | Nueva imagen, Acerca de y Salir | Ya hechos (Archivo y Ayuda) | sí | — | — | — | — |
| I13 | Formato con capas `.aseprite` (sin equivalente en el manual) | Guardar y abrir documentos con capas desde la especificación pública, por etapas (D10) | no | 3 (3.8; capas, celdas y paleta, un fotograma), 5 (5.5; fotogramas y etiquetas) y 6 (6.1; el resto) | 3.1 | alto: compromiso a largo plazo; dependencia directa de `miniz_oxide` | `AS` |

### 2.6 Animación

| ID | Función (original) | Qué hace | Hoy | Hito | Depende de | Riesgo | Corpus |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A01 | Fotogramas: fijar el número, añadir tras el actual con copia, copiar rango o insertar, borrar rango | Alta, baja y copia de fotogramas | no | 5 (5.0, 5.1) | 3.1 | alto: memoria de las celdas | — |
| A02 | Navegación: anterior, siguiente, ir a, repetir el último «ir a», primero y último | Cambia el fotograma actual (cifras 1 a 6 en el original) | no | 5 (5.1) | A01 | bajo: las cifras ya están reservadas | — |
| A03 | Reproducir: adelante, atrás, una vez, ping-pong y continuo; `Espacio` para; `r` invierte | Reproductor dentro de la app | no | 5 (5.3) | A01 | medio: reloj de la interfaz y coste de la textura por fotograma | — |
| A04 | Rango de reproducción | Reproduce solo un tramo | no | 5 (5.3) | A03 | bajo | — |
| A05 | Cadencia global o por fotograma | Imágenes por segundo o pausa propia en cada fotograma (el original usa jiffies, que se descartan) | no | 5 (5.0, 5.3) | A01 | bajo: milisegundos por fotograma | `AS` |
| A06 | Panel de control y deslizador de fotogramas | Botones y barra para navegar y reproducir | no | 5 (5.2) | A02 | medio: sitio en la pantalla (D29) | — |
| A07 | Mesa de luz (`l`) | Muestra, atenuados, hasta cuatro fotogramas relativos al actual detrás del lienzo; atenuación de −255 a 255; fusión; bloqueo al rango | no | 5 (5.4) | A01, 3.2 | medio: atenuar en indexado | `AS` |
| A08 | Paleta por fotograma | Varias paletas en una animación indexada | no | 5 (5.11) (opcional, D26) | A01 | alto: afecta a ordenar, convertir y GIF | `AS` |
| A09 | Pintar animando | Cada sello cae en un fotograma distinto; el original lo activa con una tecla al pulsar, aquí es un botón y una tecla de alternancia (D24) | no | 5 (5.9) | A01, 4.3 | medio: sin `Alt` (D24) | — |
| A10 | Pincel animado: recoger N fotogramas, dirección (adelante, atrás, ping-pong), duración, celda actual, usar, liberar, cargar y guardar | Pincel con varias celdas que avanza al pintar | no | 5 (5.10) | 4.8, A01 | alto | `DP` |
| A11 | Mover (trayectoria de un pincel con distancia y giro por eje, cíclico, lento al entrar y al salir, orden de pintado, vista previa, estelas, cargar y guardar) | Anima un pincel sobre N fotogramas | no | 5 (5.12; solo plano, opcional, D26) y 7 (7.3; 3D) | A01, 4.8 | alto | `DP` |
| A12 | Mover cámara | Desplaza y hace zoom de la página en N fotogramas, con envoltura | no | 7 (7.3) | A11 | alto | `DP` |
| A13 | Método de memoria (comprimido o expandido) | Elige cómo se guarda la animación | — | descartada | — | — | — |
| A14 | Cargar y guardar animación | Fichero de animación del original | no | 5 (GIF y APNG, 5.6, 5.7) y 6 (IFF ANIM) | A01 | medio | `AS` |
| A15 | Etiquetas de animación y hojas de sprites (sin equivalente en el manual) | Tramos con nombre y dirección; exportar fotogramas en una hoja con metadatos | no | 5 (5.5, 5.7) | A01 | medio | `AS` |

### 2.7 Efectos y vista

| ID | Función (original) | Qué hace | Hoy | Hito | Depende de | Riesgo | Corpus |
| --- | --- | --- | --- | --- | --- | --- | --- |
| E01 | Plantilla por color (stencil): hacer, rehacer, invertir, mostrar, activar, liberar, tolerancia | Protege del pintado los píxeles de los colores elegidos | no | 7 (7.1) | 3.3 | medio | `DP` |
| E02 | Plantilla pintada, cargar y guardar plantilla | Protege zonas dibujadas a mano | no | 7 (7.1) | E01 | medio | `DP` |
| E03 | Perspectiva: entrar, centro, ajustes, giros en tres ejes, rellenar pantalla, reiniciar | Manipula el pincel como un plano en 3D y lo estampa | no | 7 (7.2) | 4.8 | alto | `DP` |
| E04 | Fijar fondo, liberar y bloquear primer plano | Congela la imagen para pintar encima sin perderla | no | descartada: lo hacen las capas (3.1); «bloquear primer plano» queda como plantilla pintada en 7.1 | 3.1 | — | — |
| E05 | Mesa de luz | Ver A07 | no | 5 (5.4) | — | — | — |

### 2.8 Preferencias, menús y sistema

| ID | Función (original) | Qué hace | Hoy | Hito | Depende de | Riesgo | Corpus |
| --- | --- | --- | --- | --- | --- | --- | --- |
| X01 | Coordenadas y origen arriba a la izquierda | Posición del cursor; el original añade el desplazamiento relativo mientras se pulsa | parcial (coordenadas en la barra de estado) | 8 (8.8) | — | bajo | — |
| X02 | Respuesta rápida, seguimiento del ratón, Workbench, ajuste rápido, píxel cuadrado | Ajustes de rendimiento y de hardware | — | descartada (el seguimiento pasa a ser un estabilizador de trazo opcional en 8.8) | — | — | — |
| X03 | Opciones de recogida y de rejilla de perspectiva | Excluir borde, transparencia automática, rejilla automática | no | 4 (4.8) y 7 (7.2) | 4.8, E03 | bajo | — |
| X04 | Submenú de herramientas, iconos de ficheros, directorio de fuentes | Atajos a ventanas, iconos de Amiga | — | descartada (las opciones viven en el panel fijo, I.3) | — | — | — |
| X05 | Guardar, restaurar y reponer preferencias | Preferencias en disco | no | 8 (8.1) | — | bajo | — |
| X06 | Macros: grabar con o sin detalle, ejecutar, cargar, guardar y diez ranuras (menú de usuario) | Registrar y repetir acciones | no | 8 (8.5) | 8.3 | alto: la tabla de acciones debe serializarse | — |
| X07 | Repetir la última orden (`a`) | Repite la acción anterior | no | 8 (8.5) | tabla de acciones | medio | — |
| X08 | ARexx y utilidad Player | Control externo y reproductor | — | descartada: lo sustituyen el scripting y la CLI (8.2, 8.3), y el reproductor propio (5.3) | — | — | — |
| X09 | Ocultar caja y barra (`F10`, `F9`) | Pantalla despejada | no | 8 (8.8) | — | bajo | — |
| X10 | Información en la barra de título | Muestra relleno, rango, modo, translucidez y fotograma | parcial (barra de estado) | 4 (en cada unidad) y 5 (fotograma) | — | bajo | — |
| X11 | Menús: Pincel, Modo, Animación, Color y Efecto | Se añaden con su función (decidido el 20 de octubre) | no | 4, 5, 7 | — | bajo | — |
| X12 | Menú `Capa` (sin equivalente en el manual) | Capas y referencias | no | 3 (3.4) (D08) | — | bajo: ancho de la barra (ya hay desbordamiento) | — |
| X13 | Capas, opacidad, mezcla y referencias (sin equivalente) | Núcleo del hito 3 | no | 3 | — | ver sección 3 | `AS` |

## 3. Hito 3 · Capas, alfa y referencias

El original no tiene capas: tiene página de reserva y mesa de luz, así que el diseño sale de lo ya construido. Hoy un `Document` guarda imágenes con `ImageId`, todo comando nombra la imagen que edita y `Command` es `#[non_exhaustive]` (spec `hito-1-document.md`, comportamiento 1): ese ya es el sitio de las celdas. Un documento de una capa debe comportarse **igual** que hoy y todas las pruebas actuales (3379) deben seguir en verde; lo que no pueda cumplirse sin cambiar firmas lo lista la spec de 3.1 con su plan de migración.

Reglas decididas (D04 a D08, D10): celdas del tamaño del lienzo y las vacías no reservan memoria; aviso de presupuesto al crear capas en lienzos grandes; un modo por documento y la paleta en cada imagen, con una prueba de que **todas las paletas del documento son idénticas tras cualquier operación**; el indexado no tiene opacidad ni mezcla (los controles se atenúan con el motivo «convierte a RGBA») y pintar con alfa reemplaza el píxel; ocho modos de mezcla con fórmulas propias de enteros, documentadas.

**Puerta del hito:** dorados de mezcla en verde (fórmulas propias exactas; el oráculo es Aseprite con tolerancia 0 y, si los datos muestran un ±1 de redondeo, se decide caso a caso; D07) y un documento con capas que se guarda como `.aseprite` y se vuelve a abrir sin pérdida.

| Unidad | Clase | Contenido · crates | Puerta de salida | Decisiones | Riesgo técnico |
| --- | --- | --- | --- | --- | --- |
| 3.0 | deseable | `px-app` en lib y binario; la prueba de la puerta (módulo `puerta`) puede pasar a `px-golden` (**decidido en 3.9: no pasa**; 3726 líneas, 30 pruebas, 9 campos públicos de `Session`, `px-golden` de 41 a unos 180 paquetes). Solo estructura · `px-app`, `px-golden` | Mismas pruebas, mismas cifras; sin avisos de clippy | — | bajo a medio: visibilidad y rutas de módulos en muchas pruebas; si una necesita más que rutas, parar |
| 3.1 | núcleo | Modelo de capas (**no aditivo**, con plan de migración): `Layer` (nombre, visible, opacidad), orden y tabla de celdas capa × fotograma (un fotograma por ahora), celdas vacías sin imagen; comandos `AddLayer`, `RemoveLayer`, `MoveLayer`, `RenameLayer`, `SetLayerOpacity` y `SetLayerVisible` (uno por propiedad; **desvío aprobado el 21 de octubre**: `DuplicateLayer` pasa a 3.4, el modo de mezcla y su comando a 3.2 y el bloqueo y su comando a 3.3; `SetLayerProps` desaparece; spec: `hito-3-capas-modelo.md`); los comandos de paleta se aplican a todas las celdas; estimación de bytes para el aviso de presupuesto; coste de historia de la capa borrada · `px-core` (solo los cambios que liste la spec) | Propiedades de hacer y deshacer con capas; **propiedad: las paletas de todas las celdas son idénticas tras cualquier operación**; un documento de una capa pasa las pruebas anteriores | D02, D04, D05 | alto: invariante de modo y paleta; límites de capas y memoria |
| 3.2 | núcleo | Composición determinista por regiones (**no aditiva** si cambia lo que consume el lienzo; la spec lo declara): opacidad, **campo `blend` de `Layer` y su comando `SetLayerBlend` (aditivos, vienen de 3.1)**, ocho modos de mezcla (Normal, Multiplicar, Trama, Superponer, Oscurecer, Aclarar, Sumar, Diferencia) con fórmulas propias de enteros y redondeo documentados; alfa de 8 bits; indexado solo con visibilidad e índice transparente · `px-raster` (solo lo que liste la spec) | Vectores propios exactos (alfa 0 y 255, opacidad 0 y 100, cada modo) y propiedades; los dorados contra los PNG de Aseprite (`corpus-hito-3.md`) se añaden en 3.9, cuando 3.8 ya lee los ficheros de entrada | D02, D06, D07 | alto: precisión de la mezcla; coste de componer a 8192² |
| 3.3 | núcleo | Pintar sobre capas: capa activa en la sesión; las herramientas escriben en su celda; el lienzo y la textura salen de la composición por baldosas sucias (14a); cuentagotas de la capa o de lo visible; capa oculta no se pinta (**el bloqueo —`locked` y `SetLayerLocked`— pasa a 3.4, visto bueno de 3.3 del 21 de octubre**); pintar con alfa reemplaza; migrar los llamadores de `main_image()` (se queda como atajo de pruebas, obsoleta con `TODO(3.9)`, sin usos de producción); el guardado PNG pasa a la composición visible, con «Guardando…» si compone mucho · `px-core` (solo `Document::canvas_image`), `px-ui`, `px-app` | Todas las herramientas de hoy sobre varias capas con deshacer exacto; coste por gesto medido con 1, 3 y 8 capas | D06 | alto: rendimiento de la textura; pruebas anteriores que dicen «la imagen» (lista por lectura en la spec) |
| 3.4 | núcleo | **No puede empezar hasta cerrar la 3.3** (hasta entonces la interfaz no expone los comandos de capa y `px-app` no lleva guardia: el guardado PNG ignoraría las capas). Panel de capas: **ventana flotante** (cerrable, no modal, no captura entrada fuera de su rectángulo) con ojo, capa activa, añadir, duplicar (comando `DuplicateLayer`, nuevo aquí), borrar, subir, bajar, nombre, opacidad y mezcla (atenuadas en indexado con el motivo «convierte a RGBA»); menú `Capa` nuevo; aviso de presupuesto al crear capas en lienzos grandes; **bloqueo de capa (`locked` en `Layer` y `SetLayerLocked`, aditivos en `px-core`, con su candado en el panel y la guardia al pintar; viene de 3.3)**; opción de fuente del cuentagotas (`DropperSource`, ya en `ToolState`) · `px-core` (solo lo que liste la spec), `px-ui`, `px-app` | Recorrido con clics reales de cada control; clic fuera del rectángulo llega al lienzo; contraste de texto como en 2.19c | D08 | medio: la capa activa debe seguir a la entrada al deshacer (mismo problema que el pulido de `Move`) |
| 3.5 | núcleo | **Implementada el 22 de octubre (`hito-3-combinar.md`).** Combinar con la de abajo, combinar visibles y aplanar; exportar PNG aplanado; `ConvertAll` en «Convertir modo» · `px-core` (solo lo que liste la spec), `px-app` | Aplanar da la misma imagen que la composición; deshacer devuelve las capas | D02 | medio |
| 3.6 | deseable | **Aplazada fuera del hito 3 (22 de octubre, `hito-3-puerta.md`): deseable, no estaba en la puerta.** Página de reserva: intercambiar (`j`), copiar a la reserva, fusionar delante o detrás con el color de fondo transparente, borrar · `px-core`, `px-ui` | Las cuatro operaciones con deshacer de un paso | D09 (en su spec) | medio: tamaño de la reserva |
| 3.7 | deseable | **Aplazada fuera del hito 3 (22 de octubre, `hito-3-puerta.md`): deseable, no estaba en la puerta.** Imágenes de referencia: capa no exportable ni componible, importada de un PNG, con posición, escala entera, opacidad y delante o detrás; ninguna herramienta la edita · `px-core`, `px-ui` | La referencia nunca aparece en la exportación aplanada ni en el guardado como imagen | D11 (en su spec) | medio: no entra en la historia de píxeles |
| 3.8 | núcleo | **Implementada el 22 de octubre (`hito-3-aseprite.md`).** `.aseprite`, etapa 1 (D10): lectura y escritura de capas, celdas y paleta con **un fotograma**, desde la especificación pública; qué hace `Ctrl+S` con un PNG que ya tiene más de una capa; la descompresión y compresión zlib salen de `miniz_oxide` (ya en el árbol por `png` y `flate2`; se pasa a dependencia directa, se elige una de sus dos versiones, se verifica con `cargo deny` y se anota en `docs/DEPENDENCIES.md`; si no sirve, parar) · `px-formats` | Lee el corpus de Aseprite (`corpus-hito-3.md`, que ya existe: el lector se prueba primero) e ida y vuelta sin pérdida; fuzz del lector; límites antes de reservar; lo que escribe Pixoko se abre en Aseprite por lotes (prueba manual) | D10; **riesgo de 3.2:** el techo de error 2 de la mezcla con fondo translúcido y modo ≠ Normal es un máximo observado sobre los pares de alfa del corpus, no una garantía; si un `.aseprite` con otros pares de alfa lo supera, se re-evalúa con datos, no se ensancha la tolerancia (`hito-3-mezcla.md`, «Datos conocidos») | alto: compromiso a largo plazo (versionado); `license-auditor` por la dependencia directa |
| 3.9 | núcleo | **Implementada el 22 de octubre (`hito-3-puerta.md`; hito 3 cerrado tras la prueba manual y los fuzzers, `hito-3-resumen.md`).** Puerta del hito: prueba automática (sesión con capas, mezclas, guardado y reapertura), dorados contra los PNG que Aseprite exportó (lista única: `specs/corpus-hito-3.md`), lista manual de 15 pasos, `hito-3-resumen.md` y cierre · `px-app`, `px-golden` | Cierre completo en verde y prueba manual del usuario | —; **riesgo de 3.2:** los dorados con fondo translúcido y modo ≠ Normal usan el techo 2 (observado, no garantizado); si algún caso de la puerta lo supera, se re-evalúa con datos y nunca se ensancha la tolerancia (`hito-3-mezcla.md`, «Datos conocidos») | medio |

## 4. Hito 4 · Herramientas de pixel art

Todas las herramientas escriben en la capa activa (3.3) con un solo paso de deshacer por gesto. Se respeta el diseño de interfaz: letras del teclado, dos colores, botón derecho con el fondo, panel fijo de opciones (13 columnas por fila), un icono único de pincel con ventana en el clic derecho, `k` para el borrador y ninguna tecla `Alt`. Las formas con y sin relleno son **una** herramienta con la opción de relleno en el panel (D15), no dos iconos. Si la spec de una unidad no cabe en 12 filas y 4 propiedades, lo deseable de esa unidad (la curva, las trazas) pasa a otra, sin subir de 14 unidades.

De los modos de pintura, el hito 4 trae Matte, Color, Reemplazar, translucidez y Sombrear (D16); difuminar, mezclar, suavizar y mezcla viva pasan al 7.

**Nota tras C1:** tras C1 la caja de herramientas aloja 3 filas (12 herramientas), no 4 (16), con el panel mínimo de 277 u. El hito 4 tiene unas veinte herramientas: decidir en su primera unidad de interfaz entre desplazamiento de la caja, dos columnas o iconos menores, y qué hace el recuadro de opciones de 138 u.

**Nota tras C3 (2026-10-23, `ui-palette-handle.md`, para la primera unidad de interfaz del hito 4):** con la cola de la paleta (barra de color, campo hexadecimal e iconos: 64 u) el panel mínimo es de 286 u y la ventana mínima de 322 u de alto (429,33 pt a `ppp` 1,5, el 59,6 % de 720; techo 432). La regla de capacidad de la caja con 20 herramientas a 10 columnas (`head` 159 u) queda con **0 u de sobra** (con 21 ya no cabe) y en la ventana mínima solo se ve **una fila de rejilla** (antes 2). Revisar en esa unidad: desplazamiento de la caja, dos columnas o iconos menores, antes de añadir una herramienta más.

**Estado 4.1 (2026-10-24):** cerrada (`hito-4-imagen.md`, verificada por el usuario); medida a 8192² hecha (peor caso 1,9 s con 4 capas, +1 copia de las celdas) y sin «Aplicando…».

**Puerta del hito:** dorados contra referencias en verde y la prueba manual del usuario; el cierre fija la decisión D29 (revisión de la pantalla).

| Unidad | Clase | Contenido · crates | Puerta de salida | Decisiones | Riesgo técnico |
| --- | --- | --- | --- | --- | --- |
| 4.1 | núcleo | Imagen y operaciones de imagen: comando `ReplaceImages` (cambia varias celdas a la vez como un paso de deshacer, con el coste de historia contado), funciones puras de volteo, recorte y ampliación y giro de 90°, y el menú `Imagen` (voltear, tamaño del lienzo con ancla, girar 90°) · `px-core` y `px-raster` (solo lo que liste la spec), `px-ui`, `px-app` | Propiedades: hacer y deshacer exactos; todas las celdas en un solo paso; pico de memoria medido | D02 | medio: memoria doble por paso a 8192² |
| 4.2 | núcleo | Cuantizador: RGBA o gris con alfa a indexado con N ≤ 256 colores; tramado de difusión y ordenado; paleta ajena; umbral de alfa; se conecta a «Convertir modo» y quita el error `TooManyColors` de la interfaz · `px-raster`, `px-core`, `px-app` (solo lo que liste la spec) | Determinista; error medio no peor que la referencia más un margen fijado en la spec; 8192² con tiempo y memoria medidos | D02, D13 | alto: algoritmo, coste y empates deterministas. **Cerrada el 27 de octubre** (`hito-4-cuantizador.md`; falta la referencia de Aseprite, `corpus-hito-4-cuantizador.md`) |
| 4.3 | núcleo | Pincel y trazos: modelo `Brush`, diez formas integradas, estampado, mano alzada punteada y continua, línea, espaciado (continuo, cada N, N en total), `Mayús` por eje, `Ctrl` para trazas, curva; icono único con ventana de formas en el clic derecho; teclas `+` y `-` para el tamaño; primer contenido real del panel de opciones · `px-raster`, `px-ui`, `px-app` | Estampado y líneas exactos contra referencias; la curva con propiedades y decisión anotada; el zoom queda en `<`, `>`, rueda e `Inicio` | D14 | alto: es la unidad más grande; la tabla de teclas cambia (pruebas anteriores medidas en la spec) |
| 4.4 | núcleo | Formas: rectángulo, círculo, elipse (sin girar), polígono y forma libre rellena; relleno y contorno como opciones del panel; la caja con más de 16 herramientas · `px-raster`, `px-ui`, `px-app` | Dorados de contornos y rellenos; ninguna forma toca fuera del lienzo | D15 | medio: círculos pequeños y redondeo de la elipse |
| 4.5 | núcleo | Relleno ampliado: sólido (ya hecho), patrón y «hasta el color de fondo»; **reescritura del relleno por filas** (aplazada del hito 2) · `px-raster`, `px-core` (solo lo que liste la spec), `px-ui` | Mismos resultados que el relleno actual en el corpus de la puerta 2.22, más rápido (cifras medidas) | D02, D22 | medio: no cambiar los topes de 14c |
| 4.6 | núcleo | Paleta: copiar, intercambiar, degradado entre dos entradas, rangos de entradas con `Mayús` y `Ctrl`, borrar varias, paleta por defecto, cargar y guardar paleta, cambiar o intercambiar un color en toda la imagen · `px-core` (comandos nuevos, solo lo que liste la spec), `px-ui` | Cada operación con deshacer de un paso; la imagen no cambia al reordenar | D13 | bajo a medio |
| 4.7 | deseable | Rangos y degradados: modelo de rango (hasta 8, cuentas con posición, tramado, velocidad en fps y transparencia por cuenta), editor y selector rápido; degradados horizontal, vertical, por línea, direccional recto y ajustado, radial circular, contornos y luz; tramado aleatorio, de patrón y suave · `px-core` o sesión según D17, `px-raster`, `px-ui` | Dorados lineales contra `AS` (si tiene la función; se confirma en la spec) o `G2`; los radiales y ajustados con casos `DP` de la lista única o decisión propia anotada | D17, D18 | alto: dónde viven los rangos; contornos, luz y «ajustado» |
| 4.8 | núcleo | Selector y menú `Pincel`: selector rectangular, poligonal, a mano alzada y varita; copiar con el botón izquierdo y cortar con el derecho; transparencia por color de fondo o por esquinas; punto de agarre; restaurar (`B`); excluir borde; voltear, girar 90°, mitad, doble, estirar, contorno y recorte, pincel de reserva, colores del pincel, cargar y guardar (PNG), copiar y pegar interno · `px-raster`, `px-ui`, `px-app` | Recoger y estampar da la imagen original; recoger, cortar y estampar equivale a mover; cada operación contra un modelo independiente | D12 | medio: escalado y volumen de la unidad |
| 4.9 | deseable | Rejilla (separación X e Y, ajuste de las herramientas de trazo, superposición, ajuste visual) y simetría (punto con espejo y cíclica, orden N, centro colocable; mosaico) · `px-raster`, `px-ui` | Propiedad: la imagen resultante es simétrica; un solo paso de deshacer; la superposición no entra en la imagen | — | medio: replicar la salida de cada herramienta |
| 4.10 | deseable | Modos de pintura: Matte, Color y Reemplazar (`F1` a `F3`), translucidez con porcentaje y Sombrear (usa 4.7); «ciclar al pintar» se decide en su spec (propuesta: con Sombrear); el motor de pintura recibe el modo como parámetro · `px-raster`, `px-ui`, `px-app` | Dorados de translucidez con tolerancia; en indexado, color más cercano; sin 4.7 la unidad sale sin Sombrear | D16 | medio: redondeo de la mezcla |
| 4.11 | deseable | Aerógrafo: estilo aleatorio y estándar, radio y caudal; generador pseudoaleatorio propio con semilla por trazo · `px-raster`, `px-ui` | Misma semilla, mismo resultado; caudal monótono | — | medio |
| 4.12 | opcional | Giro libre del pincel y elipse girada · `px-raster` | Decisión de algoritmo anotada; propiedades de aproximación al giro exacto | D20 | alto: sin remuestreo que destroce el pixel art |
| 4.13 | opcional | Texto con fuente de píxeles propia · `px-ui` y, si falta, un rasterizador | Cada carácter, sin desenfoque, coincide con un modelo | D19 | medio |
| 4.14 | núcleo | Puerta del hito: dorados contra referencias, prueba automática de sesión, lista manual de 15 pasos, `hito-4-resumen.md` y cierre · `px-golden`, `px-app` | Cierre completo en verde y prueba manual del usuario | D29 | medio |

## 5. Hito 5 · Animación

Parte de las celdas de 3.1: un fotograma más es una columna más, con las celdas vacías sin memoria y el aviso de presupuesto al crear fotogramas en lienzos grandes (D04). La mesa de luz y el ciclo de color se apoyan en la composición (3.2) y en los rangos (4.7). El hito 5 se hace después de la revisión de pantalla (D29), porque la línea de tiempo necesita sitio. `.aseprite` sigue por etapas (D10): fotogramas y etiquetas en 5.5.

**Puerta del hito:** GIF y hojas de sprites verificados, con salida comparada con Aseprite por lotes (cada unidad confirma en su spec que Aseprite tiene la función).

| Unidad | Clase | Contenido · crates | Puerta de salida | Decisiones | Riesgo técnico |
| --- | --- | --- | --- | --- | --- |
| 5.0 | núcleo | Modelo de fotogramas: duración por fotograma, comandos de alta, duplicado, baja, mover y duración; celdas vacías sin imagen; tope de memoria de todo el documento y aviso de presupuesto · `px-core` (solo lo que liste la spec) | Propiedades de hacer y deshacer; un documento de un fotograma pasa las pruebas anteriores; paletas idénticas en todas las celdas | D02, D04, D27 | alto: memoria de capas × fotogramas |
| 5.1 | núcleo | Navegación: fotograma actual, estado `3/12` en la barra, cifras 1 a 6 (anterior, siguiente, ir a…), borrar con rango (CLR) y copiar rango · `px-ui`, `px-app` | Recorrido con teclas y clics; el borrado con rango en un paso | D03 | bajo |
| 5.2 | núcleo | Línea de tiempo y panel de animación: rejilla capas × fotogramas, cabezal, selección de rango, duración, y los controles de reproducción y de pintar animando · `px-ui` | Recorrido con clics reales; cabe en la ventana mínima | D29 | medio: espacio en pantalla |
| 5.3 | núcleo | Reproducción: adelante, atrás, una vez, ping-pong y continuo, rango, parar con `Espacio`, `r` invierte, cadencia global o por fotograma · `px-ui`, `px-app` | La cadencia medida no se desvía más de lo fijado en la spec; un fotograma a 8192² no bloquea la interfaz | — | medio: reloj y textura |
| 5.4 | núcleo | Mesa de luz: hasta cuatro fotogramas relativos, atenuación, bloqueo al rango, fusión (que aquí sí se puede deshacer) · `px-raster`, `px-ui` | Dorados de la mezcla atenuada contra `AS` si Aseprite tiene la función (se confirma en la spec; si no, casos propios); indexado atenuado con color más cercano | — | medio |
| 5.5 | núcleo | Etiquetas (nombre, rango, dirección; reproducir y exportar una) y `.aseprite`, etapa 2: fotogramas, duraciones y etiquetas · `px-core`, `px-ui`, `px-formats` | Ida y vuelta por `.aseprite`; sin etiquetas, el comportamiento anterior; fuzz del lector ampliado | D28 | medio |
| 5.6 | núcleo | GIF: escribir y leer, bucle, retardos, transparencia, paleta global y local; usa el cuantizador (4.2) · `px-formats` | GIF abierto y decodificado por herramientas externas; ida y vuelta de fotogramas indexados sin pérdida | D25 | medio: LZW, descartes y límites antes de reservar; fuzz del lector |
| 5.7 | núcleo | Hojas de sprites (fila, columna, rejilla, márgenes y metadatos) y secuencias de PNG numerados (exportar e importar); APNG si la crate lo permite · `px-formats`, `px-app` | Hoja idéntica a la de `AS` en los casos simples; la importación respeta el orden | — | bajo a medio |
| 5.8 | deseable | Ciclo de color (`Tab`) con **velocidad propia en fps** (D23): los rangos de 4.7 giran sobre `RotateColors`; la curva de velocidad exacta del original queda como opcional del 7; se mide la subida de textura por si hay que retomar la 14b · `px-ui`, `px-app` | Visto bueno de la medida; el ciclo no ensucia el historial | D23 | medio |
| 5.9 | deseable | Pintar animando (D24): cada sello cae en un fotograma distinto, **sin `Alt`**, con un botón y una tecla de alternancia en el panel de animación; relleno animado · `px-ui`, `px-app` | Un solo paso de deshacer por gesto; el estado se ve en el panel | D24 | medio |
| 5.10 | opcional | Pincel animado: recoger fotogramas como celdas, dirección, duración, celda actual, usar, liberar, cargar y guardar · `px-raster`, `px-ui` | Recoger y estampar reproduce la animación original | — | alto |
| 5.11 | opcional | Paleta por fotograma · `px-core` | Cambiar de paleta no altera lo ya pintado | D26 | alto: choca con la invariante de paletas idénticas |
| 5.12 | opcional | Mover un pincel por una trayectoria 2D (distancia, giro, cíclico, lento al entrar y salir, orden de pintado, vista previa) · `px-raster`, `px-ui` | Resultado exacto contra un modelo; cíclico cierra sin repetir fotograma | D26 | alto |
| 5.13 | núcleo | Puerta del hito: GIF y hojas verificados, lista manual de 15 pasos, `hito-5-resumen.md` y cierre · `px-golden`, `px-app` | Cierre completo en verde y prueba manual del usuario | — | medio |

## 6. Hitos 6, 7 y 8 (a nivel grueso)

**Hito 6 · Más formatos** (5 unidades; la primera versión pública puede salir sin él). 6.1 `.aseprite`, resto (modos de color distintos, celdas enlazadas, perfil, propiedades; deseable), 6.2 IFF ILBM (lectura y escritura, `CMAP`, rangos de color como rangos de 4.7; HAM y Halfbrite solo se leen y se convierten a RGBA; deseable), 6.3 IFF ANIM (opcional), 6.4 BMP y PCX (opcional), 6.5 puerta: corpus de ficheros ajenos, fuzz de los lectores nuevos y la decisión sobre los datos tras `IEND` (deseable). Riesgo principal: límites antes de reservar en cada lector.

**Hito 7 · Estilo DPaint** (8 unidades, todas opcionales; todo por caja negra con el emulador y su lista única de casos). 7.1 plantillas (por color, pintada, cargar y guardar), 7.2 perspectiva (entrar, centro, ajustes, relleno), 7.3 mover en 3D, mover cámara y metamorfosis, 7.4 texturas y medios, 7.5 modos de color y efectos (difuminar, mezclar, suavizar, mezcla viva, calcar, proceso y antialias; D16), 7.6 rellenos restantes (pincel y envolvente), ciclo de un solo registro e híbrido y la curva de velocidad exacta del original (D23), 7.7 cizalla, curvado y cuerpo sólido, 7.8 puerta con los casos del emulador. Riesgo principal: no hay especificación; cada caso es una observación.

**Hito 8 · Automatización y lanzamiento** (10 unidades). Núcleo: 8.1 preferencias en disco (también teclas y gestos editables, ficheros recientes y directorio recordado), 8.9 instaladores en los tres sistemas, 8.10 puerta (instaladores y licencias revisadas). Deseables: 8.2 CLI (`px-cli`: convertir, cuantizar, redimensionar, exportar hojas), 8.6 traducciones, 8.7 persistencia PNG (marca de transparencia al guardar, metadatos y perfil de color, escritor por flujo si hay un caso real de falta de memoria) y 8.8 pulidos de interfaz (sección 9). Opcionales: 8.3 scripting (lenguaje por decidir, con su licencia), 8.4 servidor MCP y 8.5 macros con «repetir la última orden». Los pulidos que toquen una unidad núcleo de antes se hacen con esa unidad.

- **Al actualizar dependencias (hito 8):** al actualizar eframe o egui, comprobar si el egui-winit nuevo usa winit 0.31 o superior, que trae una API de arrastrar y soltar con soporte para Wayland; si es así, probar soltar ficheros en Wayland y retirar la nota de limitación (`specs/hito-2-abrir-guardar.md` y `docs/DISENO-INTERFAZ.md`, «Para más adelante»).

## 7. Corpus y oráculos

- **Instalados** (`REFERENCIAS.md`): **Aseprite** (hito 3: generador y oráculo de los `.aseprite` y de sus PNG aplanados, por lotes; hitos 4 y 5: formas, relleno, cuantización, GIF y hojas de sprites, donde tenga la función) y Grafx2 2.9 (solo interfaz gráfica; referencia manual donde el mapa ya la usa por algo propio). LibreSprite **ya no es oráculo de ninguna unidad** (sus dos PNG del hito 1 siguen en `golden/png/` como corpus ya hecho). **DPaint V** en emulador, solo para el original (hito 7).
- **Corpus del hito 3** (decidido el 21 de octubre, cambia D10): la única referencia es Aseprite. Un script de Lua propio (`tools/corpus-h3/`) crea cada caso desde cero y guarda el `.aseprite`; el PNG aplanado sale de exportarlo con el modo por lotes; el usuario lo ejecuta, los resultados se commitean en `golden/aseprite/` y el CI no ejecuta Aseprite. Las pruebas de Pixoko comparan contra esos PNG con **tolerancia 0** (D07). Lista de casos: `specs/corpus-hito-3.md`.
- **Corpus de Aseprite en los hitos 4 y 5** (decidido el 21 de octubre): mismo mecanismo que el del hito 3 (scripts propios, ejecuta el usuario, resultados en `golden/aseprite/`, el CI no ejecuta Aseprite) para formas, relleno, cuantización, GIF y hojas de sprites; la spec de cada unidad confirma antes que Aseprite tiene la función. LibreSprite desaparece como oráculo en todo el plan.
- **Lotes, no casos sueltos** (cambio 3 al dar el visto bueno): al empezar cada hito se escribe `specs/corpus-hito-N.md` (ver sección 1) con todos los casos de `AS`, `G2` y `DP` y su procedimiento exacto, para generarlos de una sentada. Esto resuelve D21. Previsión orientativa: hito 3, un lote `AS` (hecho: `corpus-hito-3.md`, 25 casos de capas, mezcla, celdas, indexado y ocultas); hito 4, `G2` (círculo, elipse, polígono, línea, rectángulo, patrón, degradados lineales) y `DP` (curva, elipse girada, degradados de contornos y luz, sombrear); hito 5, `AS` (GIF, hojas, etiquetas, mesa de luz, donde Aseprite tenga la función). El detalle sale de las specs de cada hito, no de aquí.
- **Cuantizador y mezcla**: no se compara bit a bit con otro programa, sino con una medida de error y una tolerancia escrita en la spec.
- **Sin corpus ajeno** donde no haya oráculo: decisión propia anotada en la spec y propiedades.

## 8. Decisiones

**Decididas el 21 de octubre de 2026** al dar el visto bueno al mapa.

| ID | Decisión | Resultado |
| --- | --- | --- |
| D02 | Aprobación de `px-core` y `px-raster` | El visto bueno a la spec de cada unidad marcada cuenta como aprobación expresa **solo para los cambios exactos que la spec liste**; si hace falta más, se para. Las specs de 3.1 y 3.2 no son aditivas: lo dicen y proponen el plan de migración |
| D04 | Tamaño de las celdas | Del tamaño del lienzo; las vacías no reservan memoria; aviso de presupuesto al crear capas o fotogramas en lienzos grandes |
| D05 | Modo y paleta | Un modo por documento y la paleta en cada imagen; prueba de que todas las paletas del documento son idénticas tras cualquier operación |
| D06 | Indexado y alfa | Indexado sin opacidad ni mezcla (atenuados con el motivo «convierte a RGBA»); pintar con alfa reemplaza el píxel |
| D07 | Mezcla | Ocho modos con fórmulas propias de enteros documentadas. **La referencia es Aseprite** (oráculo de los PNG aplanados de `corpus-hito-3.md`; actualizada el 21 de octubre, tras decidir que LibreSprite no se usa). El objetivo es **tolerancia 0**; si los datos muestran diferencias de redondeo de ±1, se decide con ellos caso a caso y se anota en la spec de 3.2 o de 3.9 |
| D08 | Panel de capas | Ventana flotante (cerrable, no modal, no captura entrada fuera de su rectángulo) y menú `Capa` nuevo |
| D10 | Formato con capas | `.aseprite` por etapas: capas, celdas y paleta con un fotograma en el hito 3; etiquetas y fotogramas en el 5; el resto en el 6. Los `.aseprite` del corpus y sus PNG de referencia los genera Aseprite con scripts propios (`corpus-hito-3.md`, cambio del 21 de octubre: antes los escribía Pixoko y los convertía otro programa); el usuario los ejecuta, se commitean y el CI no los ejecuta. Zlib sale de `miniz_oxide` si ya está en el árbol (lo está); se verifica con `cargo deny` y se anota en `docs/DEPENDENCIES.md` |
| D16 | Modos de color | Sombrear en el hito 4 (usa los rangos y el degradado de 4.7); difuminar, mezclar, suavizar y mezcla viva en el 7 |
| D21 | Corpus de DPaint | Resuelta por el cambio 3: lista única de casos al empezar cada hito |
| D23 | Ciclo de color | Hito 5 con velocidad propia en fps; la curva de velocidad exacta del original queda como opcional del 7 |
| D24 | Pintar animando | Sin `Alt`: un botón y una tecla de alternancia en el panel de animación |
| D03 | Teclas con `Alt` | Vigente (regla del diseño de interfaz, 20 de octubre): ninguna; toda combinación con `Alt` del original se reasigna en su spec |

**Para la spec de cada unidad** (D12 a D28 y D01, por indicación del usuario) **y las que no se mencionaron**: D01 (orden y solapes, propuesta en la sección 1), D09 (página de reserva, spec de 3.6), D11 (referencias incrustadas o enlazadas, spec de 3.7), D12 (selección como pincel propio, 4.8), D13 (cuantizador y formatos de paleta, 4.2 y 4.6), D14 (`+` y `-` como tamaño, 4.3), D15 (relleno y contorno como opción del panel, 4.4), D17 (rangos en el documento o en la sesión, 4.7), D18 (degradado en color continuo, 4.7), D19 (texto, 4.13), D20 (giro libre, 4.12), D22 (relleno por filas dentro de 4.5), D25 (crate `gif`, 5.6), D26 (paleta por fotograma y mover 2D, tras 5.10), D27 (memoria de la animación, 5.0), D28 (etiquetas, 5.5), D29 (revisión de la pantalla, **después de 4.14**), D30 (lenguaje de scripting, 8.3).

## 9. Aplazadas ya anotadas: dónde caen

| Qué | Origen | Unidad |
| --- | --- | --- |
| Cuantizador de color | `hito-2-reparto-cierre.md` D8 | 4.2 |
| `ConvertAll` y `ConvertMode` en un `Batch` | `hito-2-conversion-modos.md` | 3.5 |
| Partir `px-app` en lib y binario (**hecho en 3.0**; **decidido en 3.9: la puerta NO pasa a `px-golden`**, con datos: `hito-3-puerta.md`) | `hito-2-puerta.md` D1 | 3.0 |
| Relleno por filas | `hito-2-notas-unidad-14.md` | 4.5 (D22) |
| Marca de transparencia al guardar | `hito-2-fijar-transparente.md` | 8.7 |
| Preferencias en disco (fondo del lienzo y su color, ranuras de color) | `hito-2-reparto-cierre.md` D12 | 8.1 |
| Escritor de PNG por flujo | `hito-2-notas-unidad-14.md` | 8.7 (si hay un caso real) |
| Perfil de color y metadatos del PNG; chunks auxiliares al guardar | `hito-1-png.md` | 8.7 |
| APNG | `hito-1-png.md` | 5.7 |
| Datos tras `IEND` | `hito-1-png.md` | 6.5 |
| Ficheros recientes, directorio recordado, guardado automático, pestañas, `fsync` | `hito-2-dialogos-archivo.md`, `hito-2-abrir-guardar.md` | 8.1 y 8.8 |
| **Pulidos de interfaz**: realce de la fila marcada del menú, alfa base opaco por campo, campos numéricos y pasos con teclado en el editor de color, seguir a la entrada al deshacer un `Move` (y la capa activa), orden de `Cancelar` y `Aceptar` en macOS, otro cuentagotas temporal, «Suavizar al alejar», ocultar interfaz (`F9`, `F10`), mostrar página (`S`), coordenadas relativas | varias specs del hito 2 | 8.8 (los que toquen una unidad de antes se hacen con ella) |
| `convert_report` y paleta en el sombreador | `hito-2-notas-unidad-14.md` | 8.8 y 5.8 |

## 10. Pendiente para después del hito 4

Tras 4.14 y antes de 5.2, revisar con la herramienta completa en la mano la **composición de la pantalla** (D29): editor de color que se abre a demanda con el clic derecho en el indicador, indicador de color más grande, paleta compacta, y si hace falta un **tema gris con biseles** además del oscuro de hoy. Se decide entonces, con capturas hechas por el usuario y sin cambiar nada antes: 3.4 usa una ventana flotante precisamente para no depender de esta revisión.

## 11. Limpiezas pendientes (una unidad corta entre hitos)

Reunidas el 22 de octubre de 2026, al cerrar el hito 3 (`hito-3-puerta.md`, condición 3). Todas son cambios sin comportamiento nuevo y casi todas tocan pruebas anteriores: una sola unidad corta, con su spec y su visto bueno, entre el hito 3 y el 4 (o cuando convenga). Cada una con su spec de origen:

| Qué | Dónde está hoy | Spec de origen |
| --- | --- | --- |
| `window_title`, alias de una línea de `title_text` que se conserva porque lo citan varias specs y pruebas; se quita cuando dejen de citarlo | `px-app/src/title.rs` | `hito-2-limpieza-api.md` (decisión 3 y nota «debe quitarse cuando las specs dejen de citarlo») |
| `Canvas::interact` e `interact_ordered`, con `TODO(hito 8)` («retirar cuando los llamadores migren a `interact_roles`») | `px-ui/src/canvas.rs` | `interfaz-raton.md` (punto 8) e `interfaz-dos-colores.md` (decisión 2) |
| `info_flags`, `info_flags_range`, `info_flags_alpha0` e `info_flags_roles` (cuatro variantes encadenadas, las tres primeras sin usos nuevos) | `px-ui/src/color.rs` | `hito-2-fijar-transparente.md` (`info_flags_alpha0`) e `interfaz-dos-colores.md` (decisión 7: «no cambian» `info_flags*`) |
| `show_toolbar` y `show_toolbar_with`: **ya retirados** en I.4; solo quedan menciones en comentarios de `px-ui/tests/keymap.rs` y `window_stack.rs` | `px-ui/tests` | `interfaz-caja-derecha.md` (punto 6) e `interfaz-iconos.md` (decisión 13) |
| Variantes `Analysis` (`MultiLayerGray`…) y `DialogKind` (diálogo de exportar) que 3.5 evitó para no añadir brazos a tres `match` exhaustivos de pruebas anteriores; hoy el diálogo de exportar se titula «Guardar PNG» y el motivo viaja en `LayerBlock` | `px-app/src/convert_mode.rs`, `px-app/src/dialogs.rs` | `hito-3-combinar.md` (decisión 3 de «Decisiones», «Sin variantes nuevas en `Analysis` ni en `DialogKind`») |
| `DocumentError::CelMismatch { layer: 0 }` devuelto también para un lienzo imposible (ancho o alto 0, más de `Image::MAX_BYTES`) y para una paleta que no corresponde al modo: el nombre no dice lo que pasa | `px-core/src/document.rs` | `hito-3-aseprite.md` (corrección posterior 3) |
| `Document::main_image()`, obsoleta, con `TODO(limpieza)`: 475 usos en 58 ficheros (2 de producción, en `demo.rs`; el resto, pruebas) | `px-core/src/document.rs` | `hito-3-capas-modelo.md` (decisión 14), `hito-3-capas-vista.md` (C-02) |
| Hallazgo del `grep` de «TODO»: `action_for_key`, `file_action_for_key` y `tool::tool_action_for_key`, vistas de la tabla de atajos con `TODO(hito 8)` («eliminarlas cuando los llamadores migren a `Keymap`») | `px-ui/src/shortcuts.rs` | `interfaz-teclado.md` (decisión 8) |
| Deuda: abreviar el texto de la barra de estado para poder estrechar la ventana por debajo de 468 u (hoy la fijan sus 66 columnas) | `px-ui/src/status.rs` y `px-ui/src/edit_layout.rs` (`min_width_terms_u`) | `ui-single-column.md` (corrección posterior, punto 1) |

**Aplazada el 22 de octubre de 2026 (decisión 2 de `limpieza-1.md`):** `action_for_key`, `file_action_for_key` y `tool::tool_action_for_key` (última fila) no entran en la unidad L1; siguen para el hito 8, con unos 220 cambios de tipo en pruebas. Las demás filas de esta lista las cierra `limpieza-1.md` (con `info_flags_alpha0` y el `Option` del rol de fondo también aplazados).
