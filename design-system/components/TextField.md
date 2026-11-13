# TextField

A labelled single-line input for adding items, naming lists and searching.

- Always a visible `<label>` (`label` style); placeholders show an example, never the label.
- Input text is `body` at 16px so mobile browsers don't zoom; min height 44px; `radius-md`.
- Edge `border-strong`, hover `ink-muted`, focus a 2px `focus` ring with the edge turning `brand`.
- Fill `surface`; inside a card that is already `surface`, use `surface-sunken` for a search field at rest.
- Helper text `body-sm` in `ink-muted`. Errors replace it in `danger`, say what to do, and are linked with `aria-describedby`; set `aria-invalid="true"`.
- The add-item field submits on Enter and stays focused, so people can type a whole list in one go.
