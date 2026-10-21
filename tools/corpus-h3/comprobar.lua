-- Paso 0 de specs/corpus-hito-3.md: prueba mínima del modo por lotes.
--   aseprite -b --script tools/corpus-h3/comprobar.lua --script-param salida=<carpeta>
-- Escribe <carpeta>/humo.aseprite: 4 x 4, RGBA, dos capas, sin paleta ni recurso del programa.
-- Resultado esperado al exportarlo (ver la spec): esquinas (196, 46, 52, 255) y centro opaco.
-- Las dudas sobre la API están marcadas con "comprobar".

print("version: " .. tostring(app.version)) -- comprobar: app.version
local salida = app.params["salida"] -- comprobar: app.params y --script-param
if not salida then
  error("falta --script-param salida=<carpeta>")
end

local s = Sprite(4, 4, ColorMode.RGB) -- comprobar: Sprite(ancho, alto, modo)
local pal = Palette(2) -- comprobar: constructor Palette(n)
pal:setColor(0, Color{ r = 0, g = 0, b = 0, a = 255 })
pal:setColor(1, Color{ r = 255, g = 255, b = 255, a = 255 })
s:setPalette(pal) -- comprobar: Sprite:setPalette

local abajo = s.layers[1]
abajo.name = "abajo"
local fondo = Image(4, 4, ColorMode.RGB) -- comprobar: Image(ancho, alto, modo)
for y = 0, 3 do
  for x = 0, 3 do
    fondo:putPixel(x, y, app.pixelColor.rgba(196, 46, 52, 255)) -- comprobar: putPixel
  end
end
s:newCel(abajo, 1, fondo, Point(0, 0)) -- comprobar: newCel(capa, fotograma, imagen, posición)

local arriba = s:newLayer()
arriba.name = "arriba"
local centro = Image(2, 2, ColorMode.RGB)
for y = 0, 1 do
  for x = 0, 1 do
    centro:putPixel(x, y, app.pixelColor.rgba(44, 92, 200, 128))
  end
end
s:newCel(arriba, 1, centro, Point(1, 1))

s:saveAs(salida .. "/humo.aseprite") -- comprobar: guarda en formato .aseprite
print("ok humo")
