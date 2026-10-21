//! Geometría del panel de paletas, sin tipos de egui (specs/hito-2-panel-paletas.md).
//!
//! Las medidas son constantes en «píxeles de interfaz» `u`: `u` es `n = text_scale(ppp)`
//! píxeles físicos, siempre un entero, así que todo cae en píxeles físicos enteros. Las
//! funciones trabajan en píxeles físicos (`i64`) con `n` y con posiciones relativas al origen de
//! la rejilla (la esquina superior izquierda de la primera muestra).

use px_core::Rgba;

use crate::color::Role;
use crate::icons::IconSet;

use crate::view::ScreenRect;

/// Muestras por fila.
pub const COLUMNS: usize = 16;
/// Lado de una muestra.
pub const CELL_U: i64 = 10;
/// Hueco entre muestras: cabe el anillo de la elegida.
pub const GAP_U: i64 = 4;
/// Paso entre muestras: el alto de una línea de texto.
pub const PITCH_U: i64 = CELL_U + GAP_U;
/// Lado de un cuadrado del damero (cuatro por muestra).
pub const CHECK_U: i64 = 5;
/// Ancho del panel: 4 de margen, 16 pasos, 2 de hueco, 2 de pista y 1 de trazo.
pub const PANEL_WIDTH_U: i64 = 233;
/// Origen de la rejilla en el panel.
pub const GRID_X_U: i64 = 4;
pub const GRID_Y_U: i64 = INDICATOR_H_U + 41;
/// Banda del indicador de color y del selector «editar primer plano / fondo», encima del texto
/// (specs/interfaz-dos-colores.md, decisión 5). De ella cuelga todo lo demás del panel.
pub const INDICATOR_H_U: i64 = 36;
/// Rectángulo pequeño del indicador (primer plano) y grande (fondo, debajo del pequeño): origen
/// y lado, desde la esquina del panel.
pub const IND_FG_X_U: i64 = 4;
pub const IND_FG_Y_U: i64 = 4;
pub const IND_FG_SIZE_U: i64 = 14;
pub const IND_BG_X_U: i64 = 12;
pub const IND_BG_Y_U: i64 = 12;
pub const IND_BG_SIZE_U: i64 = 20;
/// Borde de 1 u (`SURFACE_3`) por fuera de cada rectángulo del indicador.
pub const IND_BORDER_U: i64 = 1;
/// Selector de qué color edita el editor: dos filas excluyentes a la derecha del indicador.
pub const SEL_X_U: i64 = 40;
pub const SEL_W_U: i64 = 182;
pub const SEL_FG_Y_U: i64 = 4;
pub const SEL_BG_Y_U: i64 = 18;
pub const SEL_H_U: i64 = 14;
/// Marca redonda (aro y punto) dentro de la fila y origen del texto.
pub const SEL_MARK_U: i64 = 10;
pub const SEL_TEXT_X_U: i64 = SEL_X_U + 14;
/// Texto: origen, ancho útil y alto de línea.
pub const TEXT_X_U: i64 = 4;
pub const TEXT_WIDTH_U: i64 = 225;
pub const LINE_H_U: i64 = 14;
pub const LINE1_Y_U: i64 = INDICATOR_H_U + 4;
pub const LINE2_Y_U: i64 = INDICATOR_H_U + 18;
/// Franja de 1 u entre el texto y la rejilla.
pub const SEPARATOR_Y_U: i64 = INDICATOR_H_U + 36;
/// Pista del indicador de desplazamiento.
pub const TRACK_X_U: i64 = 230;
pub const TRACK_W_U: i64 = 2;
/// Trazo derecho del panel (1 u).
pub const RIGHT_LINE_X_U: i64 = 232;
/// Grosor de los anillos de la muestra elegida y de la del cursor.
pub const RING_ACTIVE_U: i64 = 2;
pub const RING_HOVER_U: i64 = 1;
/// Distancia que debe recorrer el puntero (en `u`) para que pulsar una muestra pase de clic a
/// arrastre (specs/hito-2-paleta-operaciones.md).
pub const DRAG_START_U: i64 = 4;
/// Media separación entre muestras: la captura del destino de un arrastre llega hasta ahí.
pub const DROP_HALF_GAP_U: i64 = GAP_U / 2;
/// Grosor y largo de los brazos de las esquinas del anillo del fondo.
pub const RING_BACKGROUND_U: i64 = 2;
pub const RING_BACKGROUND_ARM_U: i64 = 5;
/// Anillo de las muestras del rango de rotación y del destino de una quita pendiente.
pub const RING_RANGE_U: i64 = 1;
pub const RING_DEST_U: i64 = 2;
/// Ancho de la barra que marca el destino de un arrastre.
pub const DROP_MARK_W_U: i64 = 2;
/// Damero de las muestras con alfa.
pub const CHECK_A: Rgba = Rgba::opaque(0x4a, 0x4a, 0x4a);
pub const CHECK_B: Rgba = Rgba::opaque(0x6e, 0x6e, 0x6e);

/// Filas (y columnas) que recorre `edges_u`: las de la paleta más grande (256 entradas).
const MAX_ROWS: i64 = 16;

fn scale(n: u32) -> i64 {
    i64::from(n.max(1))
}

/// Todas las coordenadas, `x` e `y`, en `u` y desde la esquina del panel, que usa la maqueta:
/// bordes del panel, textos, separador, pista y trazo, y los bordes de cada muestra (con su
/// damero y sus anillos) de una paleta de 256 entradas.
pub fn edges_u() -> Vec<i64> {
    let mut edges = vec![
        0,
        PANEL_WIDTH_U,
        TEXT_X_U,
        TEXT_X_U + TEXT_WIDTH_U,
        LINE1_Y_U,
        LINE1_Y_U + LINE_H_U,
        LINE2_Y_U,
        LINE2_Y_U + LINE_H_U,
        SEPARATOR_Y_U,
        SEPARATOR_Y_U + 1,
        TRACK_X_U,
        TRACK_X_U + TRACK_W_U,
        RIGHT_LINE_X_U,
        RIGHT_LINE_X_U + 1,
        INDICATOR_H_U,
        IND_FG_X_U - IND_BORDER_U,
        IND_FG_X_U,
        IND_FG_X_U + IND_FG_SIZE_U,
        IND_FG_X_U + IND_FG_SIZE_U + IND_BORDER_U,
        IND_FG_Y_U - IND_BORDER_U,
        IND_FG_Y_U,
        IND_FG_Y_U + IND_FG_SIZE_U,
        IND_FG_Y_U + IND_FG_SIZE_U + IND_BORDER_U,
        IND_BG_X_U - IND_BORDER_U,
        IND_BG_X_U,
        IND_BG_X_U + IND_BG_SIZE_U,
        IND_BG_X_U + IND_BG_SIZE_U + IND_BORDER_U,
        IND_BG_Y_U - IND_BORDER_U,
        IND_BG_Y_U,
        IND_BG_Y_U + IND_BG_SIZE_U,
        IND_BG_Y_U + IND_BG_SIZE_U + IND_BORDER_U,
        SEL_X_U,
        SEL_X_U + SEL_W_U,
        SEL_TEXT_X_U,
        SEL_FG_Y_U,
        SEL_FG_Y_U + SEL_H_U,
        SEL_BG_Y_U,
        SEL_BG_Y_U + SEL_H_U,
    ];
    for step in 0..MAX_ROWS {
        for origin in [GRID_X_U, GRID_Y_U] {
            let start = origin + step * PITCH_U;
            edges.extend([
                start - RING_ACTIVE_U - 1,
                start - RING_ACTIVE_U,
                start - RING_HOVER_U,
                start,
                start + CHECK_U,
                start + CELL_U,
                start + CELL_U + RING_HOVER_U,
                start + CELL_U + RING_ACTIVE_U,
                start + CELL_U + RING_ACTIVE_U + 1,
            ]);
        }
    }
    edges.sort_unstable();
    edges.dedup();
    edges
}

/// Filas que ocupan `count` muestras.
pub fn total_rows(count: usize) -> usize {
    count.div_ceil(COLUMNS)
}

/// Filas enteras que caben en un panel de `panel_h_px` píxeles físicos de alto:
/// `floor((alto − 41·n) / (14·n))`, 0 si es negativo. Cada fila cuenta su hueco final.
pub fn visible_rows(panel_h_px: i64, n: u32) -> usize {
    let n = scale(n);
    let available = panel_h_px.saturating_sub(GRID_Y_U * n);
    if available < 0 {
        return 0;
    }
    usize::try_from(available / (PITCH_U * n)).unwrap_or(usize::MAX)
}

/// Como [`visible_rows`], pero con `reserved_px` píxeles físicos al fondo del panel que la
/// rejilla no usa (el editor de color). `visible_rows(h, n)` es `visible_rows_reserved(h, 0, n)`.
pub fn visible_rows_reserved(panel_h_px: i64, reserved_px: i64, n: u32) -> usize {
    visible_rows(panel_h_px.saturating_sub(reserved_px), n)
}

/// Mayor primera fila posible: `total − max(visible, 1)`, saturado.
pub fn max_first_row(total: usize, visible: usize) -> usize {
    total.saturating_sub(visible.max(1))
}

/// Rejilla visible: qué muestra ocupa cada rectángulo. Posiciones en píxeles físicos y
/// relativas al origen de la rejilla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    n: i64,
    count: usize,
    visible_rows: usize,
    first_row: usize,
}

impl Grid {
    pub fn new(n: u32, count: usize, visible_rows: usize, first_row: usize) -> Grid {
        Grid {
            n: scale(n),
            count,
            visible_rows,
            first_row,
        }
    }

    /// Rectángulo de la muestra `index`, o `None` si no existe o su fila no se ve.
    pub fn cell_rect(&self, index: usize) -> Option<ScreenRect> {
        if index >= self.count {
            return None;
        }
        let (row, col) = (index / COLUMNS, index % COLUMNS);
        let shown = row.checked_sub(self.first_row)?;
        if shown >= self.visible_rows {
            return None;
        }
        let pitch = PITCH_U * self.n;
        Some(ScreenRect {
            x: i64::try_from(col).ok()? * pitch,
            y: i64::try_from(shown).ok()?.checked_mul(pitch)?,
            width: CELL_U * self.n,
            height: CELL_U * self.n,
        })
    }

    /// Destino de un arrastre bajo el punto `(x, y)`: el índice que tendrá la entrada movida. Cada
    /// muestra captura su paso entero (con media separación por cada lado: no hay huecos
    /// muertos); más allá del final de la última fila vale el último índice; fuera de la zona,
    /// `None`.
    pub fn drop_index(&self, x: i64, y: i64) -> Option<usize> {
        let half = DROP_HALF_GAP_U * self.n;
        let pitch = PITCH_U * self.n;
        let (x, y) = (x.checked_add(half)?, y.checked_add(half)?);
        if x < 0 || y < 0 {
            return None;
        }
        let col = usize::try_from(x / pitch).ok()?;
        let shown = usize::try_from(y / pitch).ok()?;
        let rows_left = total_rows(self.count).saturating_sub(self.first_row);
        if col >= COLUMNS || shown >= self.visible_rows.min(rows_left) {
            return None;
        }
        let index = self
            .first_row
            .checked_add(shown)?
            .checked_mul(COLUMNS)?
            .checked_add(col)?;
        Some(index.min(self.count.checked_sub(1)?))
    }

    /// Muestra bajo el punto `(x, y)`, o `None` en un hueco, fuera de la rejilla, en una fila
    /// que no se ve o sin muestra.
    pub fn index_at(&self, x: i64, y: i64) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }
        let (pitch, cell) = (PITCH_U * self.n, CELL_U * self.n);
        if x % pitch >= cell || y % pitch >= cell {
            return None;
        }
        let col = usize::try_from(x / pitch).ok()?;
        let shown = usize::try_from(y / pitch).ok()?;
        if col >= COLUMNS || shown >= self.visible_rows {
            return None;
        }
        let index = self
            .first_row
            .checked_add(shown)?
            .checked_mul(COLUMNS)?
            .checked_add(col)?;
        (index < self.count).then_some(index)
    }
}

/// Desplazamiento vertical: primera fila visible y píxeles físicos acumulados de la fila
/// siguiente (`0 ≤ rest < 14·n`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll {
    pub first_row: usize,
    pub rest: i64,
}

impl Scroll {
    /// Desplaza `delta_px` píxeles físicos (positivo: filas posteriores). Suma saturando a
    /// `rest`, avanza `div_euclid(14·n)` filas y limita a `0..=max_first_row`; `rest` queda en
    /// 0 si se limitó o si `first_row` es el máximo (no hay más que acumular).
    pub fn by(&mut self, delta_px: i64, n: u32, total: usize, visible: usize) {
        let pitch = PITCH_U * scale(n);
        let pending = self.rest.saturating_add(delta_px);
        let target = i128::try_from(self.first_row)
            .unwrap_or(i128::MAX)
            .saturating_add(i128::from(pending.div_euclid(pitch)));
        let max = max_first_row(total, visible);
        match usize::try_from(target) {
            Ok(row) if row < max => {
                self.first_row = row;
                self.rest = pending.rem_euclid(pitch);
            }
            Ok(_) => {
                self.first_row = max;
                self.rest = 0;
            }
            Err(_) => {
                self.first_row = 0;
                self.rest = 0;
            }
        }
        if self.first_row == max {
            self.rest = 0;
        }
    }

    /// Mantiene `first_row` en `0..=max_first_row` (la ventana puede crecer).
    pub fn clamp(&mut self, total: usize, visible: usize) {
        let max = max_first_row(total, visible);
        if self.first_row >= max {
            self.first_row = max;
            self.rest = 0;
        }
    }
}

impl Scroll {
    /// Mueve `first_row` lo mínimo para que `row` quede visible y lo limita a
    /// `0..=max_first_row`; `rest` queda en 0 si se movió. Nada con `row ≥ total`.
    pub fn reveal(&mut self, row: usize, total: usize, visible: usize) {
        if row >= total {
            return;
        }
        let visible = visible.max(1);
        let before = self.first_row;
        if row < self.first_row {
            self.first_row = row;
        } else if row >= self.first_row.saturating_add(visible) {
            self.first_row = row + 1 - visible;
        }
        self.first_row = self.first_row.min(max_first_row(total, visible));
        if self.first_row != before {
            self.rest = 0;
        }
    }
}

/// Estado del panel entre fotogramas, además del de la rejilla ([`Scroll`]): el desplazamiento
/// de todo el panel cuando no cabe (specs/hito-2-panel-ventana-baja.md) y lo que hace falta para
/// mostrar la fila elegida.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PanelView {
    /// Desplazamiento del panel entero, en píxeles físicos (múltiplo de `n`).
    pub offset_px: i64,
    /// Píxeles que aún no llegan a `n`.
    pub rest_px: i64,
    /// Filas visibles de la rejilla en el fotograma anterior.
    pub last_visible: Option<usize>,
    /// Hay que mostrar la fila elegida (cambió por teclado, deshacer o rehacer).
    pub reveal: bool,
    /// Las texturas de los iconos de los botones del editor (vacío: se pintan sin icono).
    pub icons: IconSet,
}

impl PanelView {
    /// Desplaza `delta_px` (positivo: más abajo) de `n` en `n`, guardando el resto, y limita a
    /// `0..=max_offset_px` (con el resto en 0 al recortar y en el máximo; en 0 se acumula, para
    /// que los saltos de menos de `n` salgan de arriba).
    pub fn scroll_by(&mut self, delta_px: i64, n: u32, max_offset_px: i64) {
        let step = scale(n);
        let pending = self.rest_px.saturating_add(delta_px);
        let target = self
            .offset_px
            .saturating_add(pending.div_euclid(step).saturating_mul(step));
        let max = max_offset_px.max(0);
        self.rest_px = pending.rem_euclid(step);
        self.offset_px = target.clamp(0, max);
        if self.offset_px != target || self.offset_px == max {
            self.rest_px = 0;
        }
    }
}

/// Indicador del desplazamiento del panel entero: `(y, alto)` relativos a la pista, que mide
/// `panel_h_px`. Alto mínimo `2·n`. `None` si el panel cabe (`block_h_px ≤ panel_h_px`).
pub fn panel_thumb(offset_px: i64, panel_h_px: i64, block_h_px: i64, n: u32) -> Option<(i64, i64)> {
    if panel_h_px <= 0 || block_h_px <= panel_h_px {
        return None;
    }
    let (track, block) = (i128::from(panel_h_px), i128::from(block_h_px));
    let height = (track * track / block).max(i128::from(2 * scale(n)));
    let max_offset = block - track;
    let y = (track - height) * i128::from(offset_px.clamp(0, i64::try_from(max_offset).ok()?))
        / max_offset;
    Some((
        i64::try_from(y.max(0)).ok()?,
        i64::try_from(height.min(track)).ok()?,
    ))
}

/// Indicador del desplazamiento: `(y, alto)` relativos a la pista, que mide `visible·14·n`.
/// Alto mínimo `2·n`. `None` si no hace falta (`total ≤ visible`) o no se ve ninguna fila.
pub fn thumb(first_row: usize, visible: usize, total: usize, n: u32) -> Option<(i64, i64)> {
    if visible == 0 || total <= visible {
        return None;
    }
    let n = scale(n);
    let (visible, total) = (i128::try_from(visible).ok()?, i128::try_from(total).ok()?);
    let track = visible * i128::from(PITCH_U * n);
    let height = (track * visible / total).max(i128::from(2 * n));
    let first = i128::try_from(first_row).ok()?;
    let y = (track * first / total).min(track - height);
    Some((i64::try_from(y).ok()?, i64::try_from(height).ok()?))
}

fn rect(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

/// Rectángulos de una muestra, relativos a su esquina y en orden de pintado: alfa 255, el
/// color entero; alfa 0, solo el damero (cuatro cuadrados de 5·n); en medio, el damero y encima
/// el color con su alfa.
pub fn swatch_parts(n: u32, color: Rgba) -> Vec<(ScreenRect, Rgba)> {
    let n = scale(n);
    let whole = rect(0, 0, CELL_U * n, CELL_U * n);
    if color.a == 255 {
        return vec![(whole, color)];
    }
    let check = CHECK_U * n;
    let mut parts: Vec<(ScreenRect, Rgba)> = [
        (0, 0, CHECK_A),
        (check, 0, CHECK_B),
        (0, check, CHECK_B),
        (check, check, CHECK_A),
    ]
    .into_iter()
    .map(|(x, y, tone)| (rect(x, y, check, check), tone))
    .collect();
    if color.a > 0 {
        parts.push((whole, color));
    }
    parts
}

/// Barra que marca el destino de un arrastre junto a `cell`: en la separación de su izquierda
/// (`after` falso) o de su derecha, centrada en ella, con la altura de un paso.
pub fn drop_mark_rect(cell: ScreenRect, after: bool, n: u32) -> ScreenRect {
    let n = scale(n);
    let x = if after {
        cell.x + cell.width + n
    } else {
        cell.x - (GAP_U - DROP_MARK_W_U / 2) * n
    };
    rect(
        x,
        cell.y - DROP_HALF_GAP_U * n,
        DROP_MARK_W_U * n,
        PITCH_U * n,
    )
}

fn indicator_rect(x: i64, y: i64, size: i64, n: u32) -> ScreenRect {
    let k = scale(n);
    rect(x * k, y * k, size * k, size * k)
}

/// Los dos rectángulos del indicador en píxeles físicos desde la esquina del panel: `(primer
/// plano, fondo)`. El pequeño se pinta encima del grande.
pub fn indicator_rects(n: u32) -> (ScreenRect, ScreenRect) {
    (
        indicator_rect(IND_FG_X_U, IND_FG_Y_U, IND_FG_SIZE_U, n),
        indicator_rect(IND_BG_X_U, IND_BG_Y_U, IND_BG_SIZE_U, n),
    )
}

fn contains(r: &ScreenRect, x: i64, y: i64) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

/// Rol del rectángulo del indicador bajo `(x, y)` (píxeles desde la esquina del panel): donde se
/// superponen manda el pequeño; las esquinas vacías de la caja que los contiene no son de nadie.
pub fn indicator_hit(x: i64, y: i64, n: u32) -> Option<Role> {
    let (fg, bg) = indicator_rects(n);
    if contains(&fg, x, y) {
        Some(Role::Foreground)
    } else if contains(&bg, x, y) {
        Some(Role::Background)
    } else {
        None
    }
}

/// Fila del selector de `role`, en píxeles desde la esquina del panel.
pub fn selector_rect(role: Role, n: u32) -> ScreenRect {
    let k = scale(n);
    let y = match role {
        Role::Foreground => SEL_FG_Y_U,
        Role::Background => SEL_BG_Y_U,
    };
    rect(SEL_X_U * k, y * k, SEL_W_U * k, SEL_H_U * k)
}

/// Selector bajo `(x, y)` (píxeles desde la esquina del panel).
pub fn selector_hit(x: i64, y: i64, n: u32) -> Option<Role> {
    [Role::Foreground, Role::Background]
        .into_iter()
        .find(|role| contains(&selector_rect(*role, n), x, y))
}

/// Partes de un rectángulo de `width × height` píxeles con el color `color`, en el orden en que se
/// pintan y relativas a su esquina: alfa 255, un rectángulo; alfa 0, el damero de cuadrados de
/// `square` píxeles (recortado al borde) sin color; alfa de 1 a 254, el damero y encima el
/// rectángulo con su alfa. Es `swatch_parts` para un tamaño cualquiera.
pub fn block_parts(width: i64, height: i64, square: i64, color: Rgba) -> Vec<(ScreenRect, Rgba)> {
    let solid = (rect(0, 0, width, height), color);
    if color.a == 255 {
        return vec![solid];
    }
    let mut parts = Vec::new();
    let square = square.max(1);
    let (mut j, mut y) = (0, 0);
    while y < height {
        let (mut i, mut x) = (0, 0);
        while x < width {
            let tone = if (i + j) % 2 == 0 { CHECK_A } else { CHECK_B };
            parts.push((
                rect(x, y, square.min(width - x), square.min(height - y)),
                tone,
            ));
            i += 1;
            x += square;
        }
        j += 1;
        y += square;
    }
    if color.a > 0 {
        parts.push(solid);
    }
    parts
}

/// Anillo del fondo: cuatro esquinas en L (ocho rectángulos disjuntos) de `thickness` píxeles y
/// brazos de `arm` píxeles, a `offset` píxeles de la muestra (0: pegado a ella). Orden: por
/// esquina (arriba izquierda, arriba derecha, abajo izquierda, abajo derecha), el brazo
/// horizontal y luego el vertical.
pub fn corner_rects(cell: ScreenRect, offset: i64, thickness: i64, arm: i64) -> [ScreenRect; 8] {
    let t = thickness;
    let (x0, y0) = (cell.x - offset - t, cell.y - offset - t);
    let (x1, y1) = (
        cell.x + cell.width + offset + t,
        cell.y + cell.height + offset + t,
    );
    let tall = (arm - t).max(0);
    [
        rect(x0, y0, arm, t),
        rect(x0, y0 + t, t, tall),
        rect(x1 - arm, y0, arm, t),
        rect(x1 - t, y0 + t, t, tall),
        rect(x0, y1 - t, arm, t),
        rect(x0, y1 - arm, t, tall),
        rect(x1 - arm, y1 - t, arm, t),
        rect(x1 - t, y1 - arm, t, tall),
    ]
}

/// Anillo de `thickness` píxeles alrededor de `cell`: arriba, abajo, izquierda y derecha. Las
/// franjas de arriba y abajo llevan las esquinas.
pub fn ring_rects(cell: ScreenRect, thickness: i64) -> [ScreenRect; 4] {
    let t = thickness;
    [
        rect(cell.x - t, cell.y - t, cell.width + 2 * t, t),
        rect(cell.x - t, cell.y + cell.height, cell.width + 2 * t, t),
        rect(cell.x - t, cell.y, t, cell.height),
        rect(cell.x + cell.width, cell.y, t, cell.height),
    ]
}
