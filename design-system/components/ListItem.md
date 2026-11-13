# ListItem

A shopping list row: checkbox, item name and quantity, the core of the app and the motif of its icon.

- Row height at least 48px; padding `space-4` horizontal; `space-3` gap between checkbox and name. The whole row is the tap target (a `<label>` wrapping a real `<input type="checkbox">`).
- Checkbox: 22px, `radius-sm`, 2px `border-strong` edge when open; `brand` fill with an `on-brand` tick when checked. That is the icon's tick, so keep the stroke rounded.
- Name: `body` in `ink`. Checked: `ink-subtle` with a line-through. It stays readable (5:1+), because people re-check what's in the cart.
- Quantity: `quantity` style (JetBrains Mono, tabular figures) in `ink-muted`, right-aligned.
- Groups: "To get · n" above, "In cart · n" below, as `overline` labels. Checking moves a row to "In cart" after a short delay, and the toast offers Undo.
- Dividers between rows use `border`; the list sits in a `surface` card with `radius-lg` and `shadow-sm`.
- Swipe to delete reveals `danger-subtle` with a `danger` label; always undoable.
