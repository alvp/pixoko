# Hito 2 · Unidad 3: línea determinista

Estado: **aprobada** (2026-10-17; decisiones 1 a 5 aceptadas, con los matices que están incorporados al texto: aritmética solo entera, cuatro filas calculadas a mano, recorte con `width` o `height` cero y coordenadas negativas, y la nota de memoria de la decisión 5; la nota «Para el hito 4» está al final). Las filas de «Casos límite» salen de un modelo en Python de la fórmula del comportamiento 2 (aparte del repo), que además cumple simetría, extremos y 8-conectividad en 20 000 pares aleatorios; las cuatro marcadas con «a mano» se calcularon además a mano (ver bajo la tabla).

## Objetivo

Dar en `px-raster` los píxeles de un segmento entre dos puntos, igual en cualquier sistema y en cualquier sentido, y una función que lo ejecute sobre un `Document` como un `Command::SetPixels`. Es la base de la herramienta de línea (unidad 7). Grosor 1 solamente.

## Entradas y salidas

- `px_raster::Line`: iterador **perezoso** de `(i32, i32)`. Deriva `Clone` y `Debug`. No guarda píxeles: solo extremos, índice y longitud, así que ocupa memoria constante aunque el segmento tenga 2³² píxeles.
- `Line::new(a: (i32, i32), b: (i32, i32)) -> Line`: no falla ni hace pánico con ningún `i32`.
- `Line::len(&self) -> u64` e `is_empty(&self) -> bool`: píxeles que **quedan** por devolver. Es `u64` porque `i32::MIN` a `i32::MAX` son 2³² píxeles y no caben en `u32`; no se implementa `ExactSizeIterator` (exige `usize`, y en 32 bits no cabría). `size_hint` está fijado en el comportamiento 11.
- `Iterator::nth` se sobrescribe y cuesta O(1); `next` también. Es lo que permite recortar sin recorrer el segmento.
- `Line::clip_to(self, width: u32, height: u32) -> Line`: deja solo los píxeles con `0 <= x < width` y `0 <= y < height`, en el mismo orden. Puede quedar vacía.
- `line_command(doc: &Document, image: ImageId, a, b, pixel: Pixel) -> Result<Option<Command>, DocumentError>`: lee el tamaño de la imagen en `doc` (solo lectura), recorta y devuelve `Some(SetPixels)` o `None` si no queda ningún píxel. Errores: `UnknownImage` si `image` no existe y `OutOfMemory` si no se puede reservar el `Vec` (`try_reserve_exact`).
- `draw_line(doc: &mut Document, image, a, b, pixel) -> Result<bool, DocumentError>`: `line_command` y, solo si hay `Some`, `doc.apply`. Con `None` devuelve `Ok(false)` sin llamar a `apply`. Si no, devuelve lo que devuelva `apply`.
- `px-raster` pasa a depender de `px-core` (dependencia del workspace, sin crates externas nuevas; `proptest` 1.12.0 como dev-dependency, ya anotada en `docs/DEPENDENCIES.md` para otras crates: se añade `px-raster` a esa fila).

## Comportamiento

1. **Sin coma flotante ni redondeos del sistema.** Todo es aritmética entera (`f32` y `f64` no aparecen). Los deltas se calculan en `i64` (`i32::MAX - i32::MIN` no cabe en `i32`); `2·dy·i + n` llega a 2⁶⁵ y no cabe en `i64`, así que esa cuenta se hace en `i128`, o con un término de error incremental que no pase de `i64`, a elección de la implementación. Ningún `as` que pueda truncar: las conversiones a `i32` usan `try_from` sobre valores que están entre los dos extremos, así que no pueden fallar.
2. **Definición.** Sea `dx = bx - ax`, `dy = by - ay`, `n = max(|dx|, |dy|)`. El segmento tiene `n + 1` píxeles, indexados `i = 0..=n` desde `a` hacia `b`. Si `n == 0` es solo `a`.
   - Eje mayor x (`|dx| >= |dy|`): `x = ax + signo(dx)·i` y `y = ay + floor((2·dy·i + n) / (2·n))`, es decir, `y = floor(y_real + 1/2)`.
   - Eje mayor y (`|dy| > |dx|`): `y = ay + signo(dy)·i` y `x = ax + floor((2·dx·i + n) / (2·n))`.
   - `floor` es la división entera hacia −∞ (con denominador positivo), no la truncada hacia cero. Es la definición, no un algoritmo copiado: la implementación puede ser Bresenham incremental si da exactamente esto.
3. **Desempate y simetría.** Un empate ocurre cuando `y_real` (o `x_real`) cae justo en `k + 1/2`: **la mitad se redondea hacia +y en líneas de eje mayor x y hacia +x en las de eje mayor y** (hacia abajo y hacia la derecha en pantalla). Con `|dx| == |dy|` la línea es una diagonal exacta y no hay empates. Como la regla depende solo de la recta (no de qué extremo es el origen) y la elección de eje es simétrica, **`Line::new(a, b)` es exactamente `Line::new(b, a)` al revés**: los mismos píxeles y en orden inverso. No hay orientación canónica ni caso especial. Coste conocido: no es simétrica por espejo (el espejo vertical de `(0,0)→(2,1)` no da el espejo de sus píxeles, porque el empate siempre va hacia +y).
4. **Todos los píxeles son distintos**, el primero es `a` y el último es `b`, y cada uno está pegado al anterior (8-conectividad: `max(|Δx|, |Δy|) == 1`). Las coordenadas del eje mayor son estrictamente monótonas y las del otro, monótonas no estrictas.
5. **Recorte por intervalo de índices, no por filtro.** La función base no conoce el lienzo y produce el segmento completo; `clip_to` es un método aparte. Como `x(i)` e `y(i)` son monótonas, el conjunto de `i` cuyo píxel cae dentro del rectángulo es un intervalo contiguo. `clip_to` lo calcula (el eje mayor con una cuenta directa; el otro con una búsqueda binaria sobre la misma función `píxel(i)`, ≤ 33 pasos) y devuelve la línea restringida a ese intervalo. Los píxeles de dentro salen de la misma función `píxel(i)` que sin recortar, así que **son los mismos por construcción**, y cuesta lo mismo con 10 píxeles que con 2³². Filtrar con `filter` sería correcto pero tardaría minutos con `i32::MIN` a `i32::MAX`.
6. **Memoria.** Solo `line_command` reserva, y lo hace una vez, con `try_reserve_exact`, por el número de píxeles ya recortados (a lo sumo `max(width, height)`); si falla, `Err(OutOfMemory)`, nunca aborta. No hay tope propio, igual que `Document::apply`, que reserva por su cuenta y ya devuelve `OutOfMemory`. Una línea de la longitud de un lienzo enorme sigue costando memoria hasta que la unidad 4 (historia por regiones) lo abarate; esta API no cambia.
7. **Comando.** Cada píxel recortado pasa a `PixelEdit { x: u32, y: u32, pixel }` (`try_from` sobre valores ya dentro de `0..width` y `0..height`) y se agrupan en `Command::SetPixels` en el orden de la línea (de `a` a `b`). Nunca se toca la `Image` directamente: `px-raster` solo recibe `&Document` para leer el tamaño y `&mut Document` en `draw_line` para llamar a `apply`.
8. **Trazo vacío.** Si no queda ningún píxel, `line_command` devuelve `Ok(None)` y `draw_line` devuelve `Ok(false)`; el documento y su historia quedan intactos (`SetPixels` vacío es `EmptyCommand` en `apply`, y esto lo evita sin error).
9. **El modo del `Pixel` lo valida `Document`.** Un `Pixel` que no coincide con el modo de la imagen (o un índice fuera de la paleta) hace que `apply` falle con `DocumentError::Pixel { pixel: 0, .. }` y sin cambios, como cualquier otro `SetPixels`. `line_command` no lo comprueba.
10. **Sin pánico** con ningún `i32`, ni con `width` o `height` iguales a 0 (lienzo vacío: `clip_to` da una línea vacía).
11. **Dos decisiones fijadas tras la implementación** (la spec no las decía; las probaron los tests):
    - **`size_hint`.** Con `len` los píxeles que quedan: si `len` cabe en `usize`, devuelve `(len, Some(len))`; si no, `(usize::MAX, None)`. En destinos de 64 bits `len` (como mucho 2³²) siempre cabe, así que la segunda rama solo se da en 32 bits, donde `Line::new((i32::MIN, 0), (i32::MAX, 0))` da `(usize::MAX, None)`.
    - **Orden de errores en `line_command` (y `draw_line`).** Se busca la imagen antes de recortar: con un `ImageId` que no existe, el resultado es `Err(UnknownImage(id))` aunque la línea quedara totalmente fuera del lienzo (y sin saber su tamaño no podría quedar fuera). `Ok(None)` solo se da con una imagen que existe.

## Fuera de alcance

- Grosor mayor que 1, pinceles y modo *pixel perfect* (quitar esquinas en L): hito 4.
- **Comparación con Grafx2 y Aseprite** (antes LibreSprite; cambio del 21 de octubre de 2026): hito 4. Se hará ejecutando los programas y guardando la salida en `golden/`, sin leer su código. En pendientes con empate (como `(0,0)→(2,1)`) es esperable que difieran, porque su regla de desempate puede ser otra; en horizontales, verticales y diagonales deberían coincidir. No hay dorados en esta unidad.
- Recorte contra un rectángulo cualquiera (selecciones): solo el lienzo `0..width × 0..height`.
- Curvas, rectángulos y elipses; línea con degradado o con patrón.
- `DoubleEndedIterator`: no se necesita, la simetría se obtiene al construir con los extremos al revés.

## Casos límite

`Line::new(a, b)` salvo que se diga otra cosa; «recortada a W×H» es `.clip_to(W, H)`. Las listas son el resultado completo, en orden.

| Entrada | Resultado |
| --- | --- |
| `(0,0)→(0,0)` | `[(0,0)]` |
| **a mano** `(0,0)→(4,0)` (horizontal) | `(0,0),(1,0),(2,0),(3,0),(4,0)` |
| **a mano** `(0,0)→(0,3)` (vertical) | `(0,0),(0,1),(0,2),(0,3)` |
| **a mano** `(0,0)→(3,3)` (diagonal pura); además `(3,3)→(0,0)` y `(0,0)→(3,-3)` | `(0,0),(1,1),(2,2),(3,3)`; `(3,3),(2,2),(1,1),(0,0)`; `(0,0),(1,-1),(2,-2),(3,-3)` |
| **a mano** `(0,0)→(2,1)` (empate, eje x); además `(2,1)→(0,0)` | `(0,0),(1,1),(2,1)`; `(2,1),(1,1),(0,0)` (la anterior al revés) |
| `(0,1)→(2,0)` (empate con pendiente negativa) | `(0,1),(1,1),(2,0)` |
| `(0,0)→(4,2)` | `(0,0),(1,1),(2,1),(3,2),(4,2)` |
| `(0,0)→(2,4)` (empate, eje y: va hacia +x) | `(0,0),(1,1),(1,2),(2,3),(2,4)` |
| `(0,0)→(3,1)` (sin empate) | `(0,0),(1,0),(2,1),(3,1)` |
| `(0,0)→(-4,-2)` y `(-4,-2)→(0,0)` (floor con negativos) | `(0,0),(-1,0),(-2,-1),(-3,-1),(-4,-2)`; `(-4,-2),(-3,-1),(-2,-1),(-1,0),(0,0)` |
| `(i32::MIN,0)→(i32::MAX,0)` | `len() == 4294967296`; `nth(0)` es `(MIN,0)` y `nth(2³²−1)` es `(MAX,0)`; sin pánico ni reserva |
| `(i32::MIN,i32::MIN)→(i32::MAX,i32::MAX)` | `len() == 4294967296`; `nth(0)`, `nth(1)`, `nth(2³²−1)` son `(MIN,MIN)`, `(MIN+1,MIN+1)`, `(MAX,MAX)` |
| `(i32::MIN,0)→(i32::MAX,1)` (`dx` impar de 2³²−1) | `nth(2³¹−1)` es `(-1,0)` y `nth(2³¹)` es `(0,1)`: el salto de fila ocurre ahí |
| `(i32::MIN,i32::MIN)→(i32::MAX,i32::MAX)` recortada a 4×4 | `(0,0),(1,1),(2,2),(3,3)`, al instante (no recorre 2³² píxeles) |
| `(0,0)→(4,2)` recortada a 3×2 | `(0,0),(1,1),(2,1)` (el prefijo de la línea sin recortar) |
| `(-3,2)→(2,-3)` recortada a 4×4 (la caja envolvente corta el lienzo, la línea no); `(10,10)→(20,20)` a 4×4 | vacía las dos; `line_command` da `Ok(None)` |
| Coordenadas negativas: `(-5,-5)→(-1,-1)`, `(-5,0)→(-1,0)` y `(0,-9)→(0,-1)`, las tres a 4×4 | vacías (todos los píxeles tienen `x < 0` o `y < 0`) |
| `(-2,0)→(5,0)` recortada a 4×1; `(0,0)→(3,3)` recortada a 0×4, a 4×0 y a 0×0 | `(0,0),(1,0),(2,0),(3,0)`; las tres últimas vacías (`width` o `height` cero no admiten ningún píxel) |
| `line_command` de `(0,0)→(3,3)` con `Pixel::Rgba` sobre una imagen RGBA 4×4 | `Some(SetPixels)` con 4 `PixelEdit` `(0,0),(1,1),(2,2),(3,3)`, en ese orden |
| `line_command` y `draw_line` con `ImageId` desconocido y una línea totalmente fuera de una imagen de 4×4 (`(10,10)→(20,20)`, `(-5,-5)→(-1,-1)`, `(MIN,MIN)→(MIN,MIN)`, `(MIN,0)→(MIN,5)`, `(-3,2)→(2,-3)`, `(MAX,MAX)→(MIN,MAX)`); y con una imagen que existe y las mismas seis líneas | `Err(UnknownImage(id))` en las seis, con las dos funciones, y el documento intacto; `Ok(None)` (`line_command`) u `Ok(false)` (`draw_line`) con la imagen existente |
| `draw_line` totalmente fuera; con el mismo color que ya hay; con `ImageId` desconocido; con `Pixel::Gray` sobre imagen RGBA | `Ok(false)` sin tocar `can_undo`; `Ok(false)` (lo decide `apply`); `Err(UnknownImage)`; `Err(Pixel { pixel: 0, .. })`, con la imagen intacta |

**Calculadas a mano** (aplicando a mano la fórmula del comportamiento 2; en los tests son listas escritas literalmente, no derivadas del modelo ni de la implementación):

- Horizontal `(0,0)→(4,0)`: `dx = 4`, `dy = 0`, `n = 4`, eje x. `y = floor((0 + 4) / 8) = 0` para todo `i`; `x = i`.
- Vertical `(0,0)→(0,3)`: `dx = 0`, `dy = 3`, `n = 3`, eje y (`|dy| > |dx|`). `x = floor((0 + 3) / 6) = 0`; `y = i`.
- Diagonal `(0,0)→(3,3)`: `n = 3`, eje x. `y = floor((6i + 3) / 6)`: `i = 0, 1, 2, 3` da `0, 1, 2, 3` (`3/6`, `9/6`, `15/6`, `21/6` bajan a `0, 1, 2, 3`).
- Empate `(0,0)→(2,1)`: `dx = 2`, `dy = 1`, `n = 2`, eje x, `y = floor((2i + 2) / 4)`: `i = 0` da `floor(2/4) = 0`; `i = 1` da `floor(4/4) = 1` (`y_real = 0,5`, el empate sube a 1); `i = 2` da `floor(6/4) = 1`. Resultado `(0,0),(1,1),(2,1)`. Al revés, `(2,1)→(0,0)`: `dx = -2`, `dy = -1`, `y = 1 + floor((-2i + 2) / 4)`: `i = 1` da `1 + floor(0/4) = 1`, y `i = 2` da `1 + floor(-2/4) = 0`; resultado `(2,1),(1,1),(0,0)`.

## Tests

En `crates/px-raster/tests/line.rs`, sobre la API pública. **Unitarios**: una prueba por fila de la tabla. Los casos de 2³² píxeles solo usan `len`, `nth` y `clip_to`: ninguna prueba recorre ni reserva un segmento grande.

**Propiedades** (`proptest`). `a` y `b` salen de dos regímenes: coordenadas pequeñas (−20..=20, para colisiones y empates) y `i32` completo (para desbordes). Con `i32` completo solo se consultan `len`, `nth` y los extremos.

1. **Simetría A-B.** Con coordenadas pequeñas, `Line::new(a, b).collect()` es igual a `Line::new(b, a).collect()` invertido; en consecuencia los dos conjuntos de píxeles coinciden. Con `i32` completo, `nth(i)` de `(a, b)` es `nth(n − i)` de `(b, a)` para `i` en 0, 1, `n/2`, `n − 1` y `n`.
2. **Extremos incluidos.** El primer píxel es `a` y el último es `b` (con `i32` completo, `nth(0)` y `nth(n)`).
3. **8-conectividad.** Con coordenadas pequeñas, cada píxel difiere del anterior en `max(|Δx|, |Δy|) == 1` (así no hay repetidos), y el eje mayor es monótono estricto. Con `i32` completo, se comprueba en pares `nth(i)`, `nth(i + 1)` para unos `i` elegidos al azar.
4. **Longitud y exactitud.** `len() == max(|dx|, |dy|) + 1` calculado en `i128`, con cualquier `i32`; y cada `nth(i)` coincide con el modelo exacto de la definición (comportamiento 2, escrito en los tests con `i128` y sin copiar la implementación). `len` baja de uno en uno al iterar y llega a 0.
5. **Recorte.** Con coordenadas pequeñas y cualquier `width`, `height` en `0..=12`, `clip_to` da lo mismo que `Line::new(a, b).filter(dentro)`, en el mismo orden. Los rectángulos de `0` de ancho o de alto y las coordenadas negativas se generan siempre (no solo por azar). Con `i32` completo y un lienzo de hasta 16×16, todo píxel recortado está dentro, coincide con el modelo exacto, y el resultado tiene una cantidad de elementos igual a la de los píxeles del modelo que caen dentro, contados solo en las columnas (eje mayor x) o filas (eje mayor y) del lienzo.
6. **Comando a través de `Document`.** Sobre una imagen RGBA de 1 a 16 de lado y cualquier `a`, `b` (pequeños y extremos): `line_command` es `None` si y solo si el recorte está vacío; si no, tras `doc.apply` los píxeles del recorte valen el color y **todos los demás son iguales que antes**, `apply` devuelve `true` si y solo si algún píxel cambió de valor, y `undo` deja la imagen igual que la copia previa. Con `None`, `draw_line` devuelve `Ok(false)` y `can_undo`, `can_redo` e `history_bytes` no cambian.

Fijan el comportamiento 11: `size_hint` por una prueba unitaria (con las dos ramas escritas según `usize::BITS`; la de 32 bits solo se ejecuta en un destino de 32 bits, que el CI no tiene) y por la propiedad 4 (que exige `None` como cota superior cuando `len` no cabe en `usize`), y el orden de errores por la fila de `UnknownImage` con línea fuera de la tabla.

No hay dorado: ver «Fuera de alcance».

## Decisiones

Las marcadas con **?** necesitan visto bueno; el resto son consecuencia directa de ellas.

1. **? Desempate por fórmula: la mitad va hacia +y (eje x) o +x (eje y).** Frente a un Bresenham con orientación canónica (calcular siempre desde el extremo menor y dar la vuelta a la lista), la fórmula no necesita caso especial y es la misma para `nth`, el recorte y el modelo de los tests. Coste: no hay simetría por espejo, y puede diferir de otros editores en las pendientes con `.5`.
2. **? `Line` es un iterador perezoso con `len() -> u64`**, sin `Vec` en la función base ni `ExactSizeIterator`. Evita reservar 2³² posiciones con `i32::MIN` a `i32::MAX`, y funciona igual en 32 y 64 bits. La alternativa (un `Vec<(i32, i32)>`) obliga a poner un tope y a decidir qué pasa por encima.
3. **? El recorte es un método (`clip_to`) por intervalo de índices, no un filtro ni un recorte dentro de `new`.** Así `new` no sabe nada del lienzo, la propiedad «los de dentro son los mismos» se cumple por construcción y no depende de recorrer el segmento. Coste: algo más de código que un `filter` (la búsqueda binaria).
4. **? `px-raster` depende de `px-core`; `line_command` y `draw_line` están en `px-raster`**, con `Pixel` (no `Rgba`) como color, para que valga en los cuatro modos, y `DocumentError` como error, sin tipo nuevo. Alternativa: dejar `px-raster` como geometría pura y poner el `Command` en `px-ui`; el PLAN lista la unidad en `px-raster` («se ejecuta como `SetPixels`»), por eso la propuesta la deja ahí.
5. **Sin tope de longitud en `line_command`**; `try_reserve_exact` y `OutOfMemory`, igual que `Document::apply`. Límites conocidos:
   - Con formas patológicas (por ejemplo un lienzo de 1 × 2³⁰ con una línea vertical) el `Vec` de `PixelEdit` y los cambios que reserva `apply` pueden llegar a gigabytes.
   - En Linux, con el *overcommit* por defecto, `try_reserve_exact` puede tener éxito aunque luego falte memoria real, y entonces el sistema puede matar el proceso en lugar de devolver `OutOfMemory`. Esta unidad no lo puede evitar.
   - La ventana «Nueva imagen» (unidad 15) limitará cada dimensión a un valor práctico, lo que acota la línea más larga posible.
   - La unidad 4 (historia por regiones) lo abarata; la API de esta unidad no cambia.
6. **Sin dependencias externas nuevas.** Solo `px-core` y, en dev, `proptest` 1.12.0.

## Para el hito 4

La herramienta de simetría debe **reflejar el conjunto de píxeles ya generado** (reflejar cada `(x, y)` del trazo y dibujarlo), no recalcular la línea con los extremos reflejados. El desempate de esta unidad no es simétrico por espejo (comportamiento 3), así que `Line::new(espejo(a), espejo(b))` puede dar píxeles distintos de los espejos de `Line::new(a, b)`, y el dibujo simétrico saldría torcido en las pendientes con `.5`.
