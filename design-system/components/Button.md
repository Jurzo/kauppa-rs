# Button

Buttons start an action; one `brand` primary per screen, everything else secondary, quiet or danger.

| Variant | Use | Tokens |
|---|---|---|
| Primary | The main action: "Add item", "Create list" | `brand` fill, `on-brand` text, hover `brand-hover` |
| Secondary | Supporting actions: "Share list", "Sort" | `surface` fill, `border-strong` edge, `ink` text, hover `brand-subtle` |
| Quiet | Low-weight actions inside lists: "Clear checked", "Undo" | `brand` text, hover `brand-subtle` |
| Danger | Destructive: "Delete list" | `danger` text and edge, hover `danger-subtle` |
| Floating add | The always-there add button on phones, bottom-right | Primary, 56px, `radius-full`, `shadow-md`, needs `aria-label="Add item"` |

- Height at least 44px; label style `label` (Inter 14/20, 600); radius `radius-md`.
- Focus: 2px `focus` outline, 2px offset.
- Disabled at 45% opacity; prefer explaining why over disabling.
- Labels are verbs that say what happens. No "OK" or "Submit".
- Never white text on orange in dark theme: `on-brand` handles it.
