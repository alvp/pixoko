# Costes en tokens por unidad

Estimación (la que se da al proponer la unidad) frente a lo medido. La cifra de la sesión
principal sale del contador de tokens restantes que muestra el arnés (cada turno arranca en
15 000 000, así que lo gastado en un turno es la diferencia); la de cada subagente, del informe
de uso que da al terminar. No incluyen lo que consumen los comandos de Cargo fuera del modelo.

## Hito 2 · unidad 11a: texto en la app

Estimación al proponer la unidad: **20 000 a 25 000 tokens** (tests ≈ 8 000, código ≈ 7 000,
`THIRD-PARTY.md` y revisión el resto). Esa estimación cubría solo la implementación, no la
investigación previa.

| Fase | Tokens | Cómo se midió |
| --- | --- | --- |
| Investigación y spec (dos turnos: banco en el scratchpad, comparación de las tres opciones, spec) | ≈ 255 000 | 177 848 + 77 438 del contador; aproximado, porque el primer turno se interrumpió y se volvió a lanzar |
| Implementación, sesión principal (matices de la spec, `text.rs`, `theme.rs`, `status.rs`, `main.rs`, correcciones, commits) | 87 786 | contador, hasta el último commit de código |
| Subagente `test-writer` (39 + 12 + 11 tests en `px-ui`, 12 en `px-app`: 74 en total) | 200 554 | informe del subagente |
| Subagente `license-auditor` (dos pasadas: la fuente, y el diff final) | 76 675 | 23 344 + 53 331 del informe de cada pasada |
| **Implementación total** (sin investigación) | **≈ 365 000** | suma de las tres filas anteriores |

Resultado: la implementación costó unas **15 veces** lo estimado. Por qué, según lo que se vio:

- La estimación no contó el volumen real de tests que pide la spec (17 filas y 5 propiedades,
  con un banco de egui, un `sha256` propio sin dependencias y barridos de `ppp`): el subagente
  de tests fue más de la mitad del coste.
- La spec aprobada llevaba erratas que solo salieron al ejecutar (puntero de la fila 13, `→`,
  tolerancias de las filas 6 y 11) y un fallo real de redondeo en `f32` (`ppp` 1,3 y 1,35):
  hubo que corregir la spec y el código y volver a pasar todo.
- Los matices del visto bueno (texto externo, truncado propio, tema neutro, lienzo desplazado)
  añadieron cuatro filas y una propiedad.

Para la próxima estimación: contar los tests como el mayor coste de una unidad con egui, y dejar
margen para una pasada de erratas de la spec al ejecutar los tests por primera vez.
