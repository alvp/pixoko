-- Corpus del relleno (specs/hito-4-relleno.md, «Referencia»): dibuja con Aseprite los casos de
-- golden/aseprite-fill/cases.txt en un lienzo RGBA de 16 × 16, rellena con el cubo (contiguo, tolerancia
-- 0) en rojo desde la semilla de cada caso y escribe un PNG por caso en <salida>/<nombre>.png.
--
-- Lo lanza generate-fill.sh (no a mano):
--   aseprite -b --script-param salida=<carpeta> --script-param casos=<cases.txt> --script tools/corpus-h4/fill.lua
--
-- Todo es propio y se crea desde cero. Licencia MIT, como el resto del repositorio. SIN EJECUTAR:
-- las dudas sobre la API están marcadas con "comprobar".

local salida = app.params["salida"] -- comprobar: app.params y --script-param
local casos = app.params["casos"]
if not salida or not casos then
  error("faltan --script-param salida=<carpeta> y casos=<cases.txt>")
end

local COLORES = {
  B = app.pixelColor.rgba(0, 0, 255, 255), -- comprobar: app.pixelColor.rgba
  G = app.pixelColor.rgba(0, 255, 0, 255),
}
local ROJO = Color { r = 255, g = 0, b = 0, a = 255 }

local function procesa(nombre, x, y, filas)
  local sprite = Sprite(16, 16, ColorMode.RGB)
  local imagen = Image(16, 16, ColorMode.RGB) -- comprobar: Image(w, h, modo)
  for fy = 1, 16 do
    for fx = 1, 16 do
      local c = COLORES[filas[fy]:sub(fx, fx)]
      if c then
        imagen:putPixel(fx - 1, fy - 1, c)
      end
    end
  end
  sprite:newCel(sprite.layers[1], 1, imagen, Point(0, 0)) -- comprobar: newCel pisa la celda inicial
  app.useTool { -- comprobar: app.useTool, "paint_bucket" y los campos contiguous y tolerance
    tool = "paint_bucket",
    color = ROJO,
    points = { Point(x, y) },
    contiguous = true,
    tolerance = 0,
  }
  sprite:saveCopyAs(salida .. "/" .. nombre .. ".png") -- comprobar: saveCopyAs
  sprite:close()
end

local nombre, x, y, filas
for linea in io.lines(casos) do
  local n, cx, cy = linea:match("^# case (%S+) (%d+) (%d+)$")
  if n then
    nombre, x, y, filas = n, tonumber(cx), tonumber(cy), {}
  elseif nombre and #linea == 16 then
    filas[#filas + 1] = linea
    if #filas == 16 then
      procesa(nombre, x, y, filas)
      nombre = nil
    end
  end
end
