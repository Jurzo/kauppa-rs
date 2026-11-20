# Button

Buttons start an action; one `brand` primary per screen, everything else secondary, quiet or danger.

| Variant | Use | Tokens |
|---|---|---|
| Primary | The main action: "Add item", "Create list" | `brand` fill, `on-brand` text, hover `brand-hover` |
| Secondary | Supporting actions: "Share list", "Sort" | `surface` fill, `border-strong` edge, `ink` text, hover `brand-subtle` |
| Quiet | Low-weight actions inside lists: "Clear checked", "Undo" | `brand` text, hover `brand-subtle` |
| Danger | Destructive: "Delete list" | `danger` text and edge, hover `danger-subtle` |
| Floating add | The always-there add button on phones, bottom-right | Primary, 56px, `radius-full`, `shadow-md`, needs `aria-label="Add item"` |
| Icon | A row's actions where text doesn't fit: edit (pencil), delete (trash) | 44px square, no fill or edge, `radius-md`; icon in `brand` for edit, `danger` for delete; hover `brand-subtle` (`danger-subtle` for delete) |

- Height at least 44px; label style `label` (Inter 14/20, 600); radius `radius-md`. One exception: a reorderable row's ↑/↓ pair, stacked one above the other, may be 44px wide and 30px tall each (RR-0055), above WCAG 2.2's 24px minimum (2.5.8); a mis-tap moves a row by one place, which the other arrow undoes.
- Icon buttons have no visible label: an `aria-label` names the action and its object ("Delete Voi", "Edit Voi"), and the icon is `aria-hidden="true" focusable="false"`, drawn as the README's Iconography says (24px, 2px round stroke).
- Focus: 2px `focus` outline, 2px offset.
- Disabled at 45% opacity; prefer explaining why over disabling.
- Labels are verbs that say what happens. No "OK" or "Submit".
- Never white text on orange in dark theme: `on-brand` handles it.
