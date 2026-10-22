-- Vuelca capas y celdas de un .aseprite que ya se ha generado (solo lectura, para mirar):
--   aseprite -b <fichero.aseprite> --script tools/corpus-h3/verify.lua
-- Sirve para comprobar que el fichero conserva lo que declara specs/corpus-hito-3.md: sobre todo
-- que las celdas que salen del lienzo no se recortan al guardar. generate.sh --verificar lo lanza
-- con todos los ficheros. Las dudas sobre la API están marcadas con "comprobar".

local s = app.sprite -- comprobar: con un fichero en la línea de órdenes, app.sprite es ese sprite
if not s then
  error("no hay sprite activo")
end
local modos = { [ColorMode.RGB] = "rgba", [ColorMode.INDEXED] = "indexado", [ColorMode.GRAY] = "grises" }
print(string.format("%s: %dx%d %s capas=%d fotogramas=%d",
  s.filename, s.width, s.height, modos[s.colorMode] or "?", #s.layers, #s.frames))
for i, l in ipairs(s.layers) do
  local c = l:cel(1) -- comprobar: Layer:cel(fotograma)
  local celda = "sin celda"
  if c then
    celda = string.format("celda %dx%d en (%d,%d)",
      c.image.width, c.image.height, c.position.x, c.position.y)
  end
  print(string.format("  %d %-22s visible=%-5s opacidad=%-3d mezcla=%s  %s",
    i, l.name, tostring(l.isVisible), l.opacity, tostring(l.blendMode), celda))
end
