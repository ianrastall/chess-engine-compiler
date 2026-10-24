"""Regenerate the code-designed rook/compiler mark. Development-only dependency: Pillow."""
from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[1]
assets = root / "src" / "ChessEngineCompiler.App" / "Assets"
assets.mkdir(parents=True, exist_ok=True)
# Shared vector geometry in a 256-unit viewbox. A neutral Jupiter tile, a rook,
# and compiler brackets remain legible without assigning the app its own accent.
rook = [(78, 42), (102, 42), (102, 64), (116, 64), (116, 42),
        (140, 42), (140, 64), (154, 64), (154, 42), (178, 42),
        (178, 90), (162, 106), (166, 156), (180, 172), (180, 190),
        (76, 190), (76, 172), (90, 156), (94, 106), (78, 90)]
left = [(55, 112), (35, 132), (55, 152)]
right = [(201, 112), (221, 132), (201, 152)]
svg_points = lambda points: " ".join(f"{x},{y}" for x, y in points)
svg = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256">
  <rect x="8" y="8" width="240" height="240" rx="48" fill="#24221f"/>
  <polygon points="{svg_points(rook)}" fill="#e8e4dc"/>
  <g fill="none" stroke="#e8e4dc" stroke-width="12" stroke-linecap="round" stroke-linejoin="round">
    <polyline points="{svg_points(left)}"/><polyline points="{svg_points(right)}"/>
    <path d="M80 211H176"/>
  </g>
</svg>
'''
(assets / "app.svg").write_text(svg, encoding="utf-8")
scale = 4
canvas = Image.new("RGBA", (256 * scale, 256 * scale))
draw = ImageDraw.Draw(canvas)
def coords(points):
    return [(x * scale, y * scale) for x, y in points]
draw.rounded_rectangle((8*scale, 8*scale, 248*scale, 248*scale), radius=48*scale, fill="#24221f")
draw.polygon(coords(rook), fill="#e8e4dc")
for points in [left, right, [(80, 211), (176, 211)]]:
    draw.line(coords(points), fill="#e8e4dc", width=12*scale, joint="curve")
    for x, y in points:
        draw.ellipse(((x-6)*scale, (y-6)*scale, (x+6)*scale, (y+6)*scale), fill="#e8e4dc")
preview = canvas.resize((256, 256), Image.Resampling.LANCZOS)
preview.save(assets / "app.png")
preview.save(assets / "app.ico", sizes=[(s, s) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)])
print(assets)
