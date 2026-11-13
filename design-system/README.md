# Black Steel

The theming guide for `kauppalista.app`, the shopping list and recipe app in this repository, built from its app icon: a checklist basket in Rust orange on a steel tile. One rule runs through everything: **the orange stays; steel and shell swap.** Light theme ("Light steel") puts dark ink on cream and white. Dark theme ("Black steel") puts light ink on steel black. The brand orange is the same hue in both, deeper in light (`#c83c00`) so orange text and white-on-orange buttons pass 4.5:1. The light app icon keeps its brighter `#e04400`, since a graphic only needs 3:1.

## In this project

This directory is the reference; nothing in it is served. The app is server-rendered HTML (handlebars templates, htmx) with one stylesheet, `assets/app.css`, which implements these tokens. Everything the browser loads comes from the app's own origin under `/assets/`, and the Content-Security-Policy allows nothing else: no inline styles or scripts, no `data:` URLs, no other hosts.

- `tokens.json` is the source of the values. `tokens.css` is the same values as CSS custom properties.
- `components/` describes how buttons, text fields and list rows look and behave.
- `assets/` holds the app icons and their sources. Its README gives the icons' own colours, which are not the site's: site colours, including the browser's `theme-color`, come from this file.

Some of what follows describes where the app is going rather than what it does today: the floating add button, toasts with Undo, swipe to delete, the "To get" / "In cart" groups and a theme choice aren't built yet. Adoption is tracked in `.agentic/missions/MB-0054-design-system-and-branding.md`.

## Principles

- **Orange means "done" and "do".** Brand orange marks the primary action on a screen and every checked item. Nothing decorative is orange, so a glance at a list shows progress.
- **Checked items fade, they don't disappear.** A ticked item keeps its place, switches to `ink-subtle` with a line-through, and moves to the "In cart" group. People mis-tap in shops; undo must be one tap away.
- **One hand, one thumb.** Rows are at least 48px tall, the add button sits bottom-right, and nothing important lives in the top corners on phones.
- **Rust-flavoured, not Rust-branded.** We borrow the crab orange and the black of the gear. We never use Ferris, the Rust logo or the word "Rust" in the product's own branding.

## Themes

| | Light steel | Black steel |
|---|---|---|
| Page `bg` | `#fbf8f5` warm off-white | `#121212` |
| Cards `surface` | `#ffffff` | `#1b1b1b` (the icon tile) |
| Text `ink` | `#1b1b1b` | `#f2ede9` |
| `brand` | `#c83c00` | `#f74c00` |
| Text on brand `on-brand` | `#ffffff` | `#1b1b1b` |
| Tint `brand-subtle` | `#fff1e6` (the light icon tile) | `#2a1a12` |

The swap of `on-brand` mirrors the icon: the light icon has a black tick, the dark icon a white tick on its tile, and filled orange controls carry white text in light and steel-black text in dark. White on `#f74c00` is only 3.5:1, so dark theme must never put white text on orange.

`steel` and `shell` are fixed brand colours that do **not** change with the theme. Use them for brand moments (the icon, a marketing band). For UI, use the theme tokens.

### Contrast (checked)

| Pair | Light | Dark |
|---|---|---|
| `ink` on `surface` | 17.2:1 | 14.8:1 |
| `ink-muted` on `surface` | 8.3:1 | 8.0:1 |
| `ink-subtle` on `surface` | 5.2:1 | 5.1:1 |
| `brand` on `bg` (links) | 4.8:1 | 5.3:1 |
| `on-brand` on `brand` (buttons) | 5.1:1 | 4.9:1 |
| `danger` on `surface` | 5.6:1 | 6.2:1 |
| `border-strong` on `surface` (control edges) | 4.1:1 | 3.2:1 |

## Colour usage

- **Primary button, checked checkbox, links, active tab:** `brand`, hover `brand-hover`, text `on-brand`.
- **Selected row, active nav item, info banner:** `brand-subtle` background with `ink` text. Don't put `brand` text on `brand-subtle` for long runs.
- **Errors and delete:** `danger` text or fill, `danger-subtle` behind banners. Delete always asks or offers undo.
- **Success** (`success`) is for system confirmations like "List shared". Checked items are brand orange, not green.
- **Dividers** use `border`. Any control's edge uses `border-strong`; a hairline alone never marks something clickable.
- **Focus:** a 2px `focus` ring with 2px offset on every interactive element, both themes.

## Typography

Space Grotesk for headings, Inter for everything people read and tap, JetBrains Mono for quantities and code.

The app loads nothing from other hosts, so there is no Google Fonts link. Until the font files are served from `/assets/` (a decision for MB-0054), the stacks in `--font-display`, `--font-sans` and `--font-mono` fall through to the system fonts they end with, and the layout must look right with those.

- List names are `heading-1`, aisle or category headings `heading-2`.
- Item names are `body` (16px, so iOS doesn't zoom when editing), metadata `body-sm` in `ink-muted`.
- Quantities use `quantity` with `font-variant-numeric: tabular-nums`, right-aligned so they line up.
- Group labels ("To get · 5", "In cart · 3") use `overline` in `ink-muted`.

## Spacing and shape

A 4px base: `space-1` 4 … `space-8` 64. Rows use `space-3` vertical and `space-4` horizontal padding. Cards pad `space-4` on phones and `space-5` on desktop, with `space-5` between them.

Corners echo the icon: `radius-sm` for checkboxes, `radius-md` for buttons and inputs, `radius-lg` for cards, and `radius-icon` (23%, the icon's squircle ratio) for avatar tiles and thumbnails. Pills use `radius-full`.

In dark theme elevation is an edge, not a shadow: `shadow-sm` and `shadow-md` turn into a 1px `#3a3a3a` outline (the icon's edge colour), plus a deep shadow for floating layers.

## Iconography

Match the logo's line: rounded caps and joins, about 2px stroke at 24px, no fills except small dots. Icons use `ink`, or `brand` only when they mean the same as a checked item or the primary action. Always pair icon-only buttons with an `aria-label`.

The app icon itself lives in the Assets section: `icon-light.svg`, `icon-dark.svg` and the adaptive `favicon.svg`, which switches with the OS colour scheme.

## Implementing in CSS

Tokens become CSS custom properties in `assets/app.css`. Light is the default and dark applies from the OS setting. The `data-theme` selectors are there for a later theme choice (an attribute the server would render on `<html>`); the app has none today, so only the OS setting decides:

```css
:root {
  --bg: #fbf8f5; --surface: #ffffff; --ink: #1b1b1b;
  --brand: #c83c00; --on-brand: #ffffff; /* …every token, light values */
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --bg: #121212; --surface: #1b1b1b; --ink: #f2ede9;
    --brand: #f74c00; --on-brand: #1b1b1b; /* …dark values */
    color-scheme: dark;
  }
}
:root[data-theme="dark"] {
  --bg: #121212; --surface: #1b1b1b; --ink: #f2ede9;
  --brand: #f74c00; --on-brand: #1b1b1b;
  color-scheme: dark;
}
body { background: var(--bg); color: var(--ink); font: 16px/24px var(--font-sans); }
```

Components only ever read tokens (`background: var(--surface)`), never raw hex, so a theme switch is a single attribute change.

## Implementing in the templates

`templates/layout.hbs` is the one place for `<head>`. Both `theme-color` metas go there, with the page backgrounds, so the mobile browser bar matches the page:

```html
<meta name="theme-color" content="#fbf8f5" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#121212" media="(prefers-color-scheme: dark)">
```

Styles live in `assets/app.css` only: no `style` attributes and no `<style>` blocks in templates. Interactivity is htmx attributes and native HTML (`popover`, `<details>`); a design that needs new JavaScript needs a decision first.

## Writing

Short, plain and kind. Name things the way shoppers do: "list", "item", "in cart", not "collection" or "entity". Buttons say what happens ("Add item", "Clear checked"), and toasts confirm it in the past tense ("3 items cleared · Undo").

## Don't

- Don't put white text on orange in dark theme.
- Don't use orange for decoration, headings or icons that aren't actions or checked states.
- Don't hide checked items immediately or remove them without undo.
- Don't use raw hex in components; add a token instead.
- Don't load fonts, styles or images from another host.
- Don't use the Rust logo, Ferris or Rust trademarks in product branding.
