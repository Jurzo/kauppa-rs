// Shared fixtures for the flows (MB-0047, RR-0041).
//
// Every page is watched for the problems P2 names, and the test fails at
// teardown if any occurred:
// - a `securitypolicyviolation` event (the app's CSP, never bypassed);
// - a console error or an uncaught page error;
// - any `htmx:*Error` / `htmx:*-error` event;
// - a request to another origin (aborted; the page loads only the app's
//   origin, RR-0041 constraint 3).
// A flow that expects an error status declares it with
// `failedLoads.allow(status, path)`, which accepts only Chromium's
// "Failed to load resource" console line for that exact response.
//
// `second` is a page of the second account, in its own browser context,
// under the same watch (MB-0057: one user sees another's changes).
//
// The one exception to the htmx rule (RR-0049 sub-answer 9, amending
// RR-0041 constraint 2): the reconnect flow drops its own event stream and
// calls `sseErrors.allowAfterDrop()` first. From then on that flow accepts
// `htmx:sseError`, the `htmx:error` htmx sends with it, and the console
// line htmx logs for it. No other flow may.
import { readFileSync } from 'node:fs';
import { test as base, expect } from 'playwright/test';

const catalog = JSON.parse(
  readFileSync(new URL('../../../locales/en.json', import.meta.url), 'utf8'),
);

/** The English catalog's text for `key`, with `{name}` placeholders filled. */
export function t(key, vars = {}) {
  const text = catalog[key];
  if (text === undefined) throw new Error(`locales/en.json has no key ${key}`);
  return text.replace(/\{(\w+)\}/g, (whole, name) =>
    name in vars ? String(vars[name]) : whole,
  );
}

/** The account the setup project registers and the flows sign in with. */
export const user = {
  name: 'Browser Test',
  email: 'browser-test@example.invalid',
  password: 'correct horse battery staple',
};

/** The second account (the setup project registers both). */
export const secondUser = {
  name: 'Second Tester',
  email: 'browser-test-2@example.invalid',
  password: 'correct horse battery staple',
};

/**
 * Tag the current document, so a later `expectSameDocument` proves htmx
 * swapped content in place instead of the browser loading a new page.
 */
export async function markDocument(page) {
  const mark = `m${Date.now()}${Math.random()}`;
  await page.evaluate((m) => {
    window.__kauppaMark = m;
  }, mark);
  return mark;
}

export async function expectSameDocument(page, mark) {
  expect(
    await page.evaluate(() => window.__kauppaMark),
    'htmx swapped the page in place (no full page load)',
  ).toBe(mark);
}

/** A predicate for `page.waitForRequest`: an htmx request to `path`. */
export function htmxRequest(method, path) {
  return (req) =>
    req.method() === method &&
    new URL(req.url()).pathname === path &&
    req.headers()['hx-request'] === 'true';
}

// What htmx dispatches when an event stream fails: the error event in both
// spellings, and the general `htmx:error` that accompanies an error event.
const SSE_ERROR_EVENTS = ['htmx:sseError', 'htmx:sse-error', 'htmx:error'];

/**
 * Watch `page` for the problems P2 names; each is pushed to `problems`.
 */
async function watch(page, origin, problems, failedLoads, sseErrors) {
  await page.route('**/*', (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== origin) {
      problems.push(`request to another origin: ${url.href}`);
      return route.abort('blockedbyclient');
    }
    return route.continue();
  });

  await page.exposeFunction('__kauppaReport', (kind, detail) => {
    if (kind === 'htmx error event' && sseErrors.allows(detail)) return;
    problems.push(`${kind}: ${detail}`);
  });
  await page.addInitScript(() => {
    document.addEventListener(
      'securitypolicyviolation',
      (e) => {
        window.__kauppaReport(
          'CSP violation',
          `${e.violatedDirective} blocked ${e.blockedURI || '(inline)'} in ${e.sourceFile || e.documentURI}:${e.lineNumber}`,
        );
      },
      true,
    );
    // htmx dispatches its events with dispatchEvent; catch every error
    // event, both spellings (htmx:responseError, htmx:response-error).
    const dispatch = EventTarget.prototype.dispatchEvent;
    EventTarget.prototype.dispatchEvent = function (event) {
      if (/^htmx:.*error/i.test(event.type)) {
        window.__kauppaReport('htmx error event', event.type);
      }
      return dispatch.call(this, event);
    };
  });

  page.on('console', (msg) => {
    if (msg.type() !== 'error') return;
    const url = msg.location().url;
    if (failedLoads.allows(msg.text(), url)) return;
    if (sseErrors.allowsConsole(msg.text(), url)) return;
    problems.push(`console error: ${msg.text()}${url ? ` (${url})` : ''}`);
  });
  page.on('pageerror', (err) => problems.push(`page error: ${err.message}`));
}

export const test = base.extend({
  // Everything the watched pages reported; the test fails if any is left.
  problems: async ({}, use) => {
    const problems = [];
    await use(problems);
    expect(problems, 'CSP violations, console errors, htmx errors, other origins').toEqual([]);
  },

  failedLoads: async ({}, use) => {
    const allowed = [];
    await use({
      allow(status, path) {
        allowed.push({ status, path });
      },
      allows(text, url) {
        const m = /^Failed to load resource: the server responded with a status of (\d{3})/.exec(text);
        if (!m || !url) return false;
        const status = Number(m[1]);
        const path = new URL(url).pathname;
        return allowed.some((a) => a.status === status && a.path === path);
      },
    });
  },

  // RR-0049 sub-answer 9: only the reconnect flow calls `allowAfterDrop`.
  sseErrors: async ({}, use) => {
    let allowed = false;
    const seen = [];
    await use({
      allowAfterDrop() {
        allowed = true;
      },
      /** The allowed events and console lines seen, for the flow to check. */
      seen,
      allows(eventType) {
        if (!allowed || !SSE_ERROR_EVENTS.includes(eventType)) return false;
        seen.push(eventType);
        return true;
      },
      // htmx logs the error event it was given: the console shows an Event
      // object, from the vendored htmx file.
      allowsConsole(text, url) {
        const fromHtmx = url && /\/assets\/vendor\/htmx-[\d.]+\.min\.js$/.test(new URL(url).pathname);
        if (!allowed || !fromHtmx || text !== '[object Event]') return false;
        seen.push(`console: ${text}`);
        return true;
      },
    });
  },

  page: async ({ page, baseURL, problems, failedLoads, sseErrors }, use) => {
    await watch(page, new URL(baseURL).origin, problems, failedLoads, sseErrors);
    await use(page);
  },

  second: async ({ browser, baseURL, problems, failedLoads, sseErrors }, use) => {
    const context = await browser.newContext({ baseURL, locale: 'en-US', storageState: '.auth/second.json' });
    const page = await context.newPage();
    await watch(page, new URL(baseURL).origin, problems, failedLoads, sseErrors);
    await use(page);
    await context.close();
  },
});

/** The name a row shows, for messages. */
function rowName(locator) {
  return locator.evaluate((el) => {
    const row = el.closest('li') ?? el;
    return (row.querySelector('.row-name') ?? row).textContent.trim().replace(/\s+/g, ' ');
  });
}

/**
 * Scroll `locators` into view and return their boxes; if they can't all be
 * shown, fail at once, naming their rows (MB-0069: the mouse works in
 * viewport coordinates, and on a repeated run the rows sit below earlier
 * runs' rows).
 */
export async function inView(page, ...locators) {
  for (const locator of locators) {
    await locator.scrollIntoViewIfNeeded();
  }
  const boxes = [];
  for (const locator of locators) {
    boxes.push(await locator.boundingBox());
  }
  const { width, height } = page.viewportSize();
  const shown = (box) =>
    box !== null && box.x >= 0 && box.y >= 0 && box.x + box.width <= width && box.y + box.height <= height;
  if (!boxes.every(shown)) {
    const where = (box) => (box === null ? 'not shown' : `at y ${Math.round(box.y)}–${Math.round(box.y + box.height)}`);
    const parts = [];
    for (const [i, locator] of locators.entries()) {
      parts.push(`"${await rowName(locator)}" (${where(boxes[i])})`);
    }
    throw new Error(`drag: ${parts.join(' and ')} can't both be shown in the ${width} × ${height} viewport`);
  }
  return boxes;
}

/** Move the held mouse from `[x0, y0]` to `[x1, y1]` at a pointer's pace. */
export async function glide(page, [x0, y0], [x1, y1], steps = 30) {
  for (let i = 1; i <= steps; i += 1) {
    await page.mouse.move(x0 + ((x1 - x0) * i) / steps, y0 + ((y1 - y0) * i) / steps);
    await page.waitForTimeout(20);
  }
}

/** Drop at `[x, y]` after a pause: Sortable ignores drag-overs while a row it just moved is still animating. */
export async function release(page, [x, y]) {
  await page.waitForTimeout(200);
  await page.mouse.move(x, y + 1);
  await page.mouse.up();
}

/**
 * Drag `from` (a handle) onto the upper edge of `to` (a row), by mouse, at a
 * pointer's pace (MB-0066). Both are brought into view first (MB-0069).
 */
export async function drag(page, from, to) {
  const [a, b] = await inView(page, from, to);
  const start = [a.x + a.width / 2, a.y + a.height / 2];
  const end = [b.x + b.width / 2, b.y + 3];
  await page.mouse.move(...start);
  await page.mouse.down();
  await glide(page, start, end);
  await release(page, end);
}

export { expect };
