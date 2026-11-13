# App icons: Black steel / Light steel

## Files

| File | Use |
|---|---|
| `favicon.svg` | Main favicon. Light steel by default; switches to Black steel when the OS is in dark mode. |
| `favicon.ico` | Fallback for older browsers (16, 32 and 48 px, Black steel). |
| `favicon-16x16.png`, `favicon-32x32.png` | PNG fallbacks (Black steel). |
| `apple-touch-icon.png` | iOS home screen (180 px, full-bleed square; iOS rounds the corners itself). |
| `icon-192.png`, `icon-512.png` | PWA / Android icons (Black steel, rounded, transparent corners). |
| `icon-maskable-512.png` | Android adaptive icon (full-bleed, mark inside the safe zone). |
| `icon-light-192.png`, `icon-light-512.png` | Light steel versions, if you want them anywhere. |
| `source/` | Editable SVG sources and `build_icons.py`, which rewrites the SVGs and renders the PNGs and the ICO into this directory (needs `rsvg-convert` and Pillow). Run it only to change the icons: the app pins the files by checksum. |

The 16 to 48 px files use a simplified mark (one ticked row, thicker strokes). The 180 px and larger files use the full two-row mark.

## `<head>` tags

The app serves static files only under `/assets/`, so the icons MB-0054 ships are copied there and linked from `templates/layout.hbs`:

```html
<link rel="icon" href="/assets/favicon.ico" sizes="48x48">
<link rel="icon" href="/assets/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/assets/apple-touch-icon.png">
<link rel="manifest" href="/assets/manifest.webmanifest">
```

The `theme-color` metas are site colours, not icon colours: see `../README.md` (`#fbf8f5` light, `#121212` dark).

`favicon.svg` switches with an inline `<style>` block. The app's Content-Security-Policy forbids inline styles, so whether browsers apply it has to be checked under the app's headers (MB-0054).

## `manifest.webmanifest`

The app's manifest is `assets/manifest.webmanifest`. Its `name` is `kauppalista.app`. It gains these icons, and its colours are the site's light page background (a manifest can't follow the colour scheme, and light is the default theme):

```json
{
  "icons": [
    { "src": "/assets/icon-192.png", "sizes": "192x192", "type": "image/png" },
    { "src": "/assets/icon-512.png", "sizes": "512x512", "type": "image/png" },
    { "src": "/assets/icon-maskable-512.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" }
  ],
  "background_color": "#fbf8f5",
  "theme_color": "#fbf8f5"
}
```

## Icon colours

These are the colours drawn into the icon files. They are not site colours; the site uses the tokens in `../README.md`.

| | Tile | Cart | Tick | Edge (small sizes only) |
|---|---|---|---|---|
| Black steel (dark) | `#1B1B1B` | `#F74C00` | `#FFFFFF` | `#3A3A3A` |
| Light steel (light) | `#FFF1E6` | `#E04400` | `#1B1B1B` | `#EAD3C2` |
