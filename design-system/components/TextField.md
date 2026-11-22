# TextField

A labelled single-line input for adding items, naming lists and searching.

- Always a visible `<label>` (`label` style); placeholders show an example, never the label.
- Input text is `body` at 16px so mobile browsers don't zoom; min height 44px; `radius-md`.
- Edge `border-strong`, hover `ink-muted`, focus a 2px `focus` ring with the edge turning `brand`.
- Fill `surface`; inside a card that is already `surface`, use `surface-sunken` for a search field at rest.
- Helper text `body-sm` in `ink-muted`. Errors replace it in `danger`, say what to do, and are linked with `aria-describedby`; set `aria-invalid="true"`.
- The add-item field submits on Enter and stays focused, so people can type a whole list in one go.
- A select (`<select>`) has the field's box, with the app's chevron in place of the browser's arrow. It is as wide as its longest option, up to the width of its form; it never widens the page. On a phone a long name is cut off in the closed select, and opening it shows every option in full.
