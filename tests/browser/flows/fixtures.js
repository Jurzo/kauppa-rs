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

export const test = base.extend({
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

  page: async ({ page, baseURL, failedLoads }, use) => {
    const problems = [];
    const origin = new URL(baseURL).origin;

    await page.route('**/*', (route) => {
      const url = new URL(route.request().url());
      if (url.origin !== origin) {
        problems.push(`request to another origin: ${url.href}`);
        return route.abort('blockedbyclient');
      }
      return route.continue();
    });

    await page.exposeFunction('__kauppaReport', (kind, detail) => {
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
      problems.push(`console error: ${msg.text()}${url ? ` (${url})` : ''}`);
    });
    page.on('pageerror', (err) => problems.push(`page error: ${err.message}`));

    await use(page);

    expect(problems, 'CSP violations, console errors, htmx errors, other origins').toEqual([]);
  },
});

export { expect };
