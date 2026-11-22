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
// MB-0071: lists that share a `data-sortable-group` (the favorites groups)
// let a row move between them. On such a drop, `end` fires on the list the
// row left; it is stopped there, so that list's form doesn't post. Instead
// the row's id joins the order form of the list it was dropped in
// (`data-order-form`), whose `moved` field names it, and that list gets the
// `end`: one request saves the category and the order. While a row is held,
// a closed group (`<details>`) opens when the pointer rests on its heading.
//
// RR-0053 constraint 2: no inline code, no evaluated strings and no HTML
// strings; this file only creates Sortable instances, marks <html> so the
// handles show, and moves a dropped row's id between order forms.
(function () {
  "use strict";

  if (typeof htmx === "undefined" || typeof Sortable === "undefined") {
    return;
  }
  document.documentElement.classList.add("sortable-ready");

  // How long the pointer rests on a closed group's heading before it opens.
  var OPEN_DELAY_MS = 500;

  var hover = { summary: null, timer: 0 };

  // The heading of a closed group that holds a list of `group`, under the
  // point, if any.
  function closedHeadingAt(x, y, group) {
    var el = document.elementFromPoint(x, y);
    var summary = el && el.closest("summary");
    var details = summary && summary.parentElement;
    if (!details || details.tagName !== "DETAILS" || details.open) {
      return null;
    }
    var list = details.querySelector("[data-sortable-group]");
    if (!list || list.getAttribute("data-sortable-group") !== group) {
      return null;
    }
    return summary;
  }

  function track(x, y, group) {
    var summary = closedHeadingAt(x, y, group);
    if (summary === hover.summary) {
      return;
    }
    window.clearTimeout(hover.timer);
    hover.summary = summary;
    if (summary) {
      hover.timer = window.setTimeout(function () {
        summary.parentElement.open = true;
        hover.summary = null;
      }, OPEN_DELAY_MS);
    }
  }

  // Follow the pointer while a row is held: `dragover` while the browser
  // drags (a mouse), pointer and touch moves under Sortable's fallback (a
  // finger).
  function holding(group) {
    function onDrag(event) {
      track(event.clientX, event.clientY, group);
    }
    function onTouch(event) {
      if (event.touches && event.touches.length) {
        track(event.touches[0].clientX, event.touches[0].clientY, group);
      }
    }
    return {
      start: function () {
        document.addEventListener("dragover", onDrag);
        document.addEventListener("pointermove", onDrag);
        document.addEventListener("touchmove", onTouch, { passive: true });
      },
      stop: function () {
        document.removeEventListener("dragover", onDrag);
        document.removeEventListener("pointermove", onDrag);
        document.removeEventListener("touchmove", onTouch);
        window.clearTimeout(hover.timer);
        hover.summary = null;
      },
    };
  }

  // A row dropped into another list of its group: its id joins that list's
  // order form as `moved`, and that list's form posts.
  function moveInto(list, row) {
    var formId = list.getAttribute("data-order-form") || "";
    var form = document.getElementById(formId);
    var id = row.querySelector('input[type="hidden"][name="id"]');
    var moved = form && form.querySelector('input[name="moved"]');
    if (!form || !id || !moved) {
      return;
    }
    // Not `form.id`: the rows' inputs are named `id`, which shadows it.
    id.setAttribute("form", formId);
    moved.value = id.value;
    moved.disabled = false;
    list.dispatchEvent(new CustomEvent("end", { bubbles: true }));
  }

  // Sortable's own `end` carries `from` and `to`; the one `moveInto` sends
  // doesn't, so it passes.
  document.addEventListener(
    "end",
    function (event) {
      if (!event.from || !event.to || event.from === event.to) {
        return;
      }
      event.stopImmediatePropagation();
      moveInto(event.to, event.item);
    },
    true
  );

  function connect(list) {
    if (Sortable.get(list)) {
      return;
    }
    var options = {
      handle: ".drag-handle",
      draggable: "li",
      animation: 150,
    };
    var group = list.getAttribute("data-sortable-group");
    if (group) {
      var hold = holding(group);
      options.group = group;
      options.onStart = hold.start;
      options.onEnd = hold.stop;
    }
    Sortable.create(list, options);
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
