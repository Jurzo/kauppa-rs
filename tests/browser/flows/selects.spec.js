// MB-0070: selects fit a phone's width, however long the names in them.
//
// - P1: at 360 × 800, with a category, a shopping list and a recipe each
//   named 60 characters long, no page scrolls sideways and every select
//   shown lies inside its form: the favorites add form, a favorite's edit
//   panel, a recipe's add-to-list form and its category prompt for a new
//   ingredient, a list's add-recipe form, a list item's edit panel and the
//   export review's category picker.
// - P3: at 1280 × 720 the same selects keep today's width: at least as wide
//   as their longest option, up to their form's width, and the add forms'
//   at least 10 rem.
//
// Every page runs under the fixtures' watch (CSP, console and htmx errors).
import { test, expect, t } from './fixtures.js';

const PHONE = { width: 360, height: 800 };
const DESKTOP = { width: 1280, height: 720 };

/** A run-unique suffix, since the flows share one account. */
function unique() {
  return `${Date.now()}${Math.floor(Math.random() * 1000)}`;
}

/** POST a form as the signed-in user, same-origin, as a page would. */
async function postForm(page, baseURL, path, form) {
  const resp = await page.request.post(path, {
    form,
    headers: { Origin: new URL(baseURL).origin },
  });
  expect(resp.ok(), `${path}: ${resp.status()}`).toBe(true);
  return resp;
}

/** A name exactly 60 characters long, starting with `id`. */
function longName(id, words) {
  return `${id} ${words.repeat(5)}`.slice(0, 60).replace(/\s$/, 'x');
}

/** The selects the page shows, with their boxes, their form's and what their longest option needs. */
function shownSelects(page) {
  return page.evaluate(async () => {
    await document.fonts.ready;
    const ctx = document.createElement('canvas').getContext('2d');
    return [...document.querySelectorAll('select')]
      .filter((s) => s.getClientRects().length > 0)
      .map((s) => {
        const box = s.getBoundingClientRect();
        const form = (s.closest('form') ?? s.parentElement).getBoundingClientRect();
        const cs = getComputedStyle(s);
        ctx.font = `${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
        const text = Math.max(...[...s.options].map((o) => ctx.measureText(o.text).width));
        const sides = ['paddingLeft', 'paddingRight', 'borderLeftWidth', 'borderRightWidth']
          .map((p) => parseFloat(cs[p]))
          .reduce((a, b) => a + b, 0);
        return {
          name: s.name,
          addForm: s.closest('form.add-product') !== null,
          left: box.left,
          right: box.right,
          width: box.width,
          formLeft: form.left,
          formRight: form.right,
          need: text + sides,
          rem: parseFloat(getComputedStyle(document.documentElement).fontSize),
        };
      });
  });
}

/**
 * Nothing reaches past the page's right edge; lists what does. The checks
 * are soft, so one run reports every form that doesn't fit.
 */
async function expectNoSideScroll(page, what) {
  const [scroll, client, wide] = await page.evaluate(() => {
    const width = document.documentElement.clientWidth;
    const past = [...document.body.querySelectorAll('*')].filter(
      (el) => el.getBoundingClientRect().right > width + 0.5 && el.getClientRects().length > 0,
    );
    const innermost = past.filter((el) => !past.some((other) => other !== el && el.contains(other)));
    const describe = (el) =>
      `${el.tagName.toLowerCase()}${el.name ? `[name=${el.name}]` : ''} (right ${Math.round(el.getBoundingClientRect().right)})`;
    return [document.documentElement.scrollWidth, width, innermost.slice(0, 5).map(describe)];
  });
  expect.soft(scroll, `${what}: page width; too wide: ${wide.join(', ')}`).toBeLessThanOrEqual(client);
}

/** P1: the page doesn't scroll sideways, and each select (`names` among them) lies inside its form. */
async function expectFits(page, what, names) {
  const selects = await shownSelects(page);
  expect(selects.map((s) => s.name), `${what}: the selects shown`).toEqual(expect.arrayContaining(names));
  for (const s of selects) {
    const at = `${Math.round(s.left)}–${Math.round(s.right)} in ${Math.round(s.formLeft)}–${Math.round(s.formRight)}`;
    expect.soft(s.left >= s.formLeft - 0.5 && s.right <= s.formRight + 0.5, `${what}: ${s.name} inside its form (${at})`).toBe(true);
  }
  await expectNoSideScroll(page, what);
}

/** P3: each select is as wide as its longest option (up to its form), and an add form's at least 10 rem. */
async function expectUncut(page, what, names) {
  const selects = await shownSelects(page);
  expect(selects.map((s) => s.name), `${what}: the selects shown`).toEqual(expect.arrayContaining(names));
  for (const s of selects) {
    const room = s.formRight - s.formLeft;
    expect.soft(s.width, `${what}: ${s.name} shows its longest option (needs ${Math.round(s.need)} px)`).toBeGreaterThanOrEqual(
      Math.min(s.need, room) - 2,
    );
    if (s.addForm) {
      expect.soft(s.width, `${what}: ${s.name} at least 10 rem`).toBeGreaterThanOrEqual(10 * s.rem - 0.5);
    }
  }
}

test('selects fit 360 px with long names, and keep their width on a desktop', async ({ page, baseURL }) => {
  const id = unique();
  const category = longName(id, 'Hedelmät vihannekset ja tuoretuotteet ');
  const listName = longName(id, 'Viikonlopun juhlat ja mökkireissu ');
  const recipeTitle = longName(id, 'Isoäidin karjalanpiirakat ja munavoi ');
  const favorite = `Selects ${id}`;
  const item = `Item ${id}`;

  await postForm(page, baseURL, '/categories', { name: category });
  await page.goto('/categories');
  const categoryId = (await page.locator(`#category-list input[type="text"][value="${category}"]`).getAttribute('id')).replace(/^cat-/, '');
  await postForm(page, baseURL, '/favorites', { name: favorite, category_id: categoryId });
  const list = await postForm(page, baseURL, '/lists', { name: listName });
  const listPath = new URL(list.url()).pathname;
  const recipe = await postForm(page, baseURL, '/recipes', { title: recipeTitle });
  const recipePath = new URL(recipe.url()).pathname;
  await postForm(page, baseURL, `${listPath}/items`, { text: item, quantity: '' });

  // Each form with a select, brought up as a user would; and the selects it must show.
  const forms = [
    ['the favorites add form', ['category_id'], async () => {
      await page.goto('/favorites');
    }],
    ["a favorite's edit panel", ['category_id'], async () => {
      await page.goto(`/favorites?open=group-${categoryId}`);
      await page.getByLabel(t('common.edit_named', { name: favorite }), { exact: true }).click();
      await expect(page.locator('.row-edit[open] select')).toBeVisible();
    }],
    ["a recipe's add-to-list form", ['list_id'], async () => {
      await page.goto(recipePath);
    }],
    ["a recipe's category prompt", ['category_id'], async () => {
      await page.goto(recipePath);
      await page.locator('#new-ingredient-name').fill(`Uusi ${id}`);
      await page.locator('#new-ingredient-name').press('Enter');
      await expect(page.locator('#ingredients select[name="category_id"], .add-product select[name="category_id"]').first()).toBeVisible();
    }],
    ["a list's add-recipe form", ['recipe_id'], async () => {
      await page.goto(listPath);
    }],
    ["a list item's edit panel", ['category_id'], async () => {
      await page.goto(listPath);
      const edit = page.locator('details.product-edit').filter({ hasText: item });
      if (!(await edit.evaluate((d) => d.open))) {
        await edit.locator('summary').click();
      }
      await expect(edit.locator('select[name="category_id"]')).toBeVisible();
    }],
    ['the export review', [], async () => {
      await page.goto(listPath);
      await page.locator('.export-form button[type="submit"]').click();
      await expect(page.locator('.export-row select').first()).toBeVisible();
    }],
  ];

  await page.setViewportSize(PHONE);
  for (const [what, names, open] of forms) {
    await open();
    await expectFits(page, `360 px, ${what}`, names);
  }

  await page.setViewportSize(DESKTOP);
  for (const [what, names, open] of forms) {
    await open();
    await expectUncut(page, `1280 px, ${what}`, names);
  }
});
