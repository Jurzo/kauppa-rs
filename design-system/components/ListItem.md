# ListItem

A shopping list row: checkbox, item name and quantity, the core of the app and the motif of its icon.

- Row height at least 48px; padding `space-4` horizontal; `space-3` gap between checkbox and name. The whole row is the tap target (a `<label>` wrapping a real `<input type="checkbox">`).
- Checkbox: 22px, `radius-sm`, 2px `border-strong` edge when open; `brand` fill with an `on-brand` tick when checked. That is the icon's tick, so keep the stroke rounded.
- Disabled checkbox (the row can't be changed, e.g. an archived list's store view, MB-0062): the same box and size, but quieter. Open: a 2px `border` edge (not `border-strong`) on a `surface-sunken` fill. Checked: `ink-subtle` fill and edge with a `surface` tick, never `brand`, since orange means "you can act here". The name keeps its usual styling (checked rows still `ink-subtle` with a line-through), so found and not-found stay easy to tell apart. No hover change and a default cursor. In `app.css`: `.shop-toggle.is-disabled`.
- Name: `body` in `ink`. Checked: `ink-subtle` with a line-through. It stays readable (5:1+), because people re-check what's in the cart.
- Quantity: `quantity` style (JetBrains Mono, tabular figures) in `ink-muted`, right-aligned.
- Groups: "To get · n" above, "In cart · n" below, as `overline` labels. Checking moves a row to "In cart" after a short delay, and the toast offers Undo.
- Dividers between rows use `border`; the list sits in a `surface` card with `radius-lg` and `shadow-sm`.
- Reorderable row (categories, a favorites group, a recipe's ingredients; RR-0055): one line on a phone, left to right: the drag handle (`ink-muted`, shown only with JavaScript), the name (it wraps), ↑ above ↓ (the one at the end disabled), a pencil and a trash icon button (`Button.md`, Icon). The pencil opens the row's edit panel on a line of its own below the row; the trash deletes after a confirmation (asked with JavaScript). The keyboard order is the visual order: ↑, ↓, pencil, trash.
- Swipe to delete reveals `danger-subtle` with a `danger` label; always undoable.
