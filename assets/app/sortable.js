// MB-0066 (RR-0053): connects the vendored Sortable to htmx, so the rows
// of a `[data-sortable]` list can be dragged by their `.drag-handle`.
//
// Sortable fires an `end` event on the list after a drop; the list's order
// form listens for it (`hx-trigger="end from:#…"`) and posts the rows'
// hidden ids, which join that form through their `form` attribute, in their
// new document order. The server saves the order only if it is exactly the
// current set, and answers with the section, which htmx swaps in; `onLoad`
// then connects the new list.
//
// RR-0053 constraint 2: no inline code, no evaluated strings and no HTML
// strings; this file only creates Sortable instances and marks <html> so
// the handles show.
(function () {
  "use strict";

  if (typeof htmx === "undefined" || typeof Sortable === "undefined") {
    return;
  }
  document.documentElement.classList.add("sortable-ready");

  function connect(list) {
    if (Sortable.get(list)) {
      return;
    }
    Sortable.create(list, {
      handle: ".drag-handle",
      draggable: "li",
      animation: 150,
    });
  }

  htmx.onLoad(function (root) {
    if (root.matches && root.matches("[data-sortable]")) {
      connect(root);
    }
    if (root.querySelectorAll) {
      root.querySelectorAll("[data-sortable]").forEach(connect);
    }
  });
})();
