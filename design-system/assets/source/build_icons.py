"""Rebuilds the kauppalista.app icons.

Writes the source SVGs next to this script and favicon.svg, the PNGs and
favicon.ico into the parent directory (design-system/assets/), replacing the
files there. Needs `rsvg-convert` (librsvg) on PATH and Pillow.

    python3 design-system/assets/source/build_icons.py
"""
import subprocess, pathlib
from PIL import Image

SRC = pathlib.Path(__file__).resolve().parent
OUT = SRC.parent

DARK = dict(tile="#1B1B1B", edge="#3A3A3A", line="#F74C00", check="#FFFFFF")
LIGHT = dict(tile="#FFF1E6", edge="#EAD3C2", line="#E04400", check="#1B1B1B")

def full_mark(c):
    # detailed geometry, for 64 px and up
    return f'''<path d="M5 9h4l2 5" fill="none" stroke="{c['line']}" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M10.5 14H42l-3.6 16H14Z" fill="none" stroke="{c['line']}" stroke-width="2.6" stroke-linejoin="round"/>
  <path d="M15.5 19.5l2 2 3.5-4" fill="none" stroke="{c['check']}" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M24.5 19.5h13" stroke="{c['line']}" stroke-opacity="0.5" stroke-width="2.6" stroke-linecap="round"/>
  <rect x="15.8" y="23.6" width="4.4" height="4.4" rx="1.2" fill="none" stroke="{c['line']}" stroke-width="1.6"/>
  <path d="M24.5 25.8h11" stroke="{c['line']}" stroke-width="2.6" stroke-linecap="round"/>
  <circle cx="17" cy="37" r="2.6" fill="{c['line']}"/>
  <circle cx="35" cy="37" r="2.6" fill="{c['line']}"/>'''

def small_mark(c):
    # simplified geometry, tuned for 16-48 px
    return f'''<path d="M4 8h4.5l2 6" fill="none" stroke="{c['line']}" stroke-width="4.5" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M9.5 14H43l-3.8 17H13.5Z" fill="none" stroke="{c['line']}" stroke-width="4.5" stroke-linejoin="round"/>
  <path d="M15 22.5l3 3 5-5.5" fill="none" stroke="{c['check']}" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M27 22.5h9" stroke="{c['line']}" stroke-opacity="0.55" stroke-width="4" stroke-linecap="round"/>
  <circle cx="17" cy="38.5" r="3.6" fill="{c['line']}"/>
  <circle cx="35" cy="38.5" r="3.6" fill="{c['line']}"/>'''

def svg(body):
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48">\n  {body}\n</svg>\n'

def rounded(c, mark):
    return svg(f'<rect x="0.5" y="0.5" width="47" height="47" rx="10.5" fill="{c["tile"]}" stroke="{c["edge"]}"/>\n  ' + mark)

def rounded_noedge(c, mark):
    return svg(f'<rect width="48" height="48" rx="10.5" fill="{c["tile"]}"/>\n  ' + mark)

def full_bleed(c, mark, scale):
    off = 24 * (1 - scale)
    return svg(f'<rect width="48" height="48" fill="{c["tile"]}"/>\n  <g transform="translate({off:.2f} {off:.2f}) scale({scale})">\n  {mark}\n  </g>')

def write(name, text, folder=SRC):
    p = folder / name
    p.write_text(text)
    return p

def render(svg_path, png_name, size):
    out = OUT / png_name
    subprocess.run(["rsvg-convert", "-w", str(size), "-h", str(size), "-o", str(out), str(svg_path)], check=True)
    return out

# 1. Adaptive favicon.svg: light by default, dark when the OS is in dark mode
favicon = '''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48">
  <style>
    .tile { fill: #FFF1E6; stroke: #EAD3C2 }
    .ln   { stroke: #E04400 }
    .wh   { fill: #E04400 }
    .tick { stroke: #1B1B1B }
    @media (prefers-color-scheme: dark) {
      .tile { fill: #1B1B1B; stroke: #3A3A3A }
      .ln   { stroke: #F74C00 }
      .wh   { fill: #F74C00 }
      .tick { stroke: #FFFFFF }
    }
  </style>
  <rect class="tile" x="0.5" y="0.5" width="47" height="47" rx="10.5"/>
  <path class="ln" d="M4 8h4.5l2 6" fill="none" stroke-width="4.5" stroke-linecap="round" stroke-linejoin="round"/>
  <path class="ln" d="M9.5 14H43l-3.8 17H13.5Z" fill="none" stroke-width="4.5" stroke-linejoin="round"/>
  <path class="tick" d="M15 22.5l3 3 5-5.5" fill="none" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
  <path class="ln" d="M27 22.5h9" stroke-opacity="0.55" stroke-width="4" stroke-linecap="round"/>
  <circle class="wh" cx="17" cy="38.5" r="3.6"/>
  <circle class="wh" cx="35" cy="38.5" r="3.6"/>
</svg>
'''
write("favicon.svg", favicon, OUT)

# 2. Source SVGs
s_small_dark = write("icon-small-dark.svg", rounded(DARK, small_mark(DARK)))
s_small_light = write("icon-small-light.svg", rounded(LIGHT, small_mark(LIGHT)))
s_full_dark = write("icon-dark.svg", rounded_noedge(DARK, full_mark(DARK)))
s_full_light = write("icon-light.svg", rounded_noedge(LIGHT, full_mark(LIGHT)))
s_apple = write("apple-touch-icon.svg", full_bleed(DARK, full_mark(DARK), 0.82))
s_mask = write("icon-maskable.svg", full_bleed(DARK, full_mark(DARK), 0.68))

# 3. PNGs
render(s_small_dark, "favicon-16x16.png", 16)
render(s_small_dark, "favicon-32x32.png", 32)
render(s_small_dark, "favicon-48x48.png", 48)
render(s_apple, "apple-touch-icon.png", 180)
render(s_full_dark, "icon-192.png", 192)
render(s_full_dark, "icon-512.png", 512)
render(s_mask, "icon-maskable-512.png", 512)
render(s_full_light, "icon-light-192.png", 192)
render(s_full_light, "icon-light-512.png", 512)

# 4. favicon.ico (16/32/48, Black steel)
imgs = [Image.open(OUT / f"favicon-{s}x{s}.png").convert("RGBA") for s in (16, 32, 48)]
imgs[2].save(OUT / "favicon.ico", sizes=[(16, 16), (32, 32), (48, 48)], append_images=imgs[:2])
(OUT / "favicon-48x48.png").unlink()

print(sorted(p.relative_to(OUT).as_posix() for p in OUT.rglob("*") if p.is_file()))
