// MB-0068 (RR-0055): the reorderable rows at a phone's width.
//
// - P1, P2: at 360 × 800, every row of the categories page, a favorites
//   group and a recipe's ingredients keeps its controls on one line, left
//   to right: the drag handle, the name, ↑ above ↓, the pencil, the trash;
//   nothing scrolls sideways. Long names wrap inside the row.
// - The pencil opens the row's panel on a line of its own below it, and the
//   trash stays on the row's line; the panel's form saves (a rename, a
//   favorite's name, an ingredient's quantity).
// - The trash deletes after a confirmation: today's for categories and
//   favorites, a new one for ingredients; dismissing it keeps the row.
// - Without JavaScript the same forms rename and delete (no handle shows,
//   and no confirmation is asked: `hx-confirm` is htmx's).
//
// Every page runs under the fixtures' watch (CSP, console and htmx errors).
import { test, expect, t } from './fixtures.js';

const PHONE = { width: 360, height: 800 };

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

/** The id of the category called `name`, from the categories page. */
async function categoryId(page, name) {
  await page.goto('/categories');
  const input = page.locator(`#category-list input[type="text"][value="${name}"]`);
  return (await input.getAttribute('id')).replace(/^cat-/, '');
}

/**
 * The boxes of a row and its controls, in CSS pixels from the page's top
 * left corner (so a click that scrolls the page doesn't move them).
 */
function measure(row) {
  return row.evaluate((li) => {
    const box = (el) => {
      if (!el) return null;
      const r = el.getBoundingClientRect();
      const [x, y] = [r.x + window.scrollX, r.y + window.scrollY];
      return { x, y, w: r.width, h: r.height, right: x + r.width, bottom: y + r.height, cy: y + r.height / 2 };
    };
    const q = (selector) => li.querySelector(selector);
    return {
      row: box(li),
      handle: box(q('.drag-handle')),
      name: box(q('.row-name')),
      up: box(q('.row-move button[value="up"]')),
      down: box(q('.row-move button[value="down"]')),
      pencil: box(q('.row-edit > summary')),
      trash: box(q('.row-delete button')),
      panel: box(q('.row-edit > form')),
    };
  });
}

/**
 * The row's controls share one line, left to right, inside the row; ↑ is
 * above ↓; each target is at least 44 × 44 px (each arrow 44 × 30).
 */
function expectOneLine(m, what, { handle = true } = {}) {
  const near = (a, b) => Math.abs(a - b) <= 3;
  const arrows = (m.up.y + m.down.bottom) / 2;
  const parts = { name: m.name, pencil: m.pencil, trash: m.trash };
  if (handle) parts.handle = m.handle;
  for (const [part, b] of Object.entries(parts)) {
    expect(near(b.cy, arrows), `${what}: ${part} centred on the arrows' line (${b.cy} vs ${arrows})`).toBe(true);
    expect(b.y >= m.row.y - 0.5 && b.bottom <= m.row.bottom + 0.5, `${what}: ${part} inside the row`).toBe(true);
  }
  expect(Math.abs(m.up.x - m.down.x) <= 0.5 && m.up.bottom <= m.down.y + 0.5, `${what}: ↑ above ↓`).toBe(true);
  const order = [m.name, m.up, m.pencil, m.trash];
  if (handle) order.unshift(m.handle);
  for (let i = 1; i < order.length; i += 1) {
    expect(order[i - 1].right <= order[i].x + 0.5, `${what}: left to right (${i})`).toBe(true);
  }
  expect(m.trash.right <= m.row.right + 0.5, `${what}: the trash inside the row`).toBe(true);
  for (const [part, b] of Object.entries({ pencil: m.pencil, trash: m.trash, ...(handle ? { handle: m.handle } : {}) })) {
    expect(b.w >= 44 && b.h >= 44, `${what}: ${part} is ${b.w} × ${b.h}`).toBe(true);
  }
  for (const [part, b] of Object.entries({ up: m.up, down: m.down })) {
    expect(b.w >= 44 && b.h >= 30, `${what}: ${part} is ${b.w} × ${b.h}`).toBe(true);
  }
}

/** Nothing on the page scrolls sideways. */
async function expectNoSideScroll(page, what) {
  const [scroll, client, wide] = await page.evaluate(() => {
    const width = document.documentElement.clientWidth;
    // The innermost elements that reach past the page's right edge.
    const past = [...document.body.querySelectorAll('*')].filter(
      (el) => el.getBoundingClientRect().right > width + 0.5 && el.getClientRects().length > 0,
    );
    const innermost = past.filter((el) => !past.some((other) => other !== el && el.contains(other)));
    const describe = (el) =>
      `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ''}${el.className && typeof el.className === 'string' ? `.${el.className.trim().split(/\s+/).join('.')}` : ''} (right ${Math.round(el.getBoundingClientRect().right)})`;
    return [document.documentElement.scrollWidth, width, innermost.slice(0, 5).map(describe)];
  });
  expect(scroll, `${what}: page width; too wide: ${wide.join(', ')}`).toBeLessThanOrEqual(client);
}

/**
 * Every row in `rows` keeps one line; a row whose name fits one line of text
 * is compact (about 68 px: the arrows' 60 and the padding), and unless
 * `shortName` is false, one such row is there to check.
 */
async function expectRowsOneLine(page, rows, what, options = {}) {
  const count = await rows.count();
  expect(count, `${what}: rows`).toBeGreaterThan(1);
  let compact = 0;
  for (let i = 0; i < count; i += 1) {
    const m = await measure(rows.nth(i));
    expectOneLine(m, `${what}, row ${i}`, options);
    if (m.name.h <= 32) {
      expect(m.row.h, `${what}, row ${i}: compact`).toBeLessThanOrEqual(72);
      compact += 1;
    }
  }
  if (options.shortName !== false) {
    expect(compact, `${what}: a row with a short name`).toBeGreaterThan(0);
  }
  await expectNoSideScroll(page, what);
}

/**
 * Open the row's panel with its pencil: the panel shows below the row's
 * line, and the pencil and the trash stay where they were.
 */
async function openPanel(row, pencilName, what) {
  const before = await measure(row);
  await row.getByLabel(pencilName, { exact: true }).click();
  await expect(row.locator('.row-edit > form')).toBeVisible();
  const after = await measure(row);
  expect(after.panel.y, `${what}: the panel below the line`).toBeGreaterThanOrEqual(after.pencil.bottom - 0.5);
  for (const part of ['pencil', 'trash']) {
    const [was, is] = [before[part], after[part]];
    expect(Math.abs(is.cy - was.cy) <= 0.5 && Math.abs(is.x - was.x) <= 0.5,
      `${what}: the ${part} stays on the row's line`).toBe(true);
  }
}

/**
 * Click `button` and answer the confirmation with `accept`; returns its
 * message.
 */
async function confirmClick(page, button, accept) {
  // Answered inside the listener: a click doesn't return while a dialog is
  // open.
  let message = null;
  page.once('dialog', (dialog) => {
    message = dialog.message();
    return accept ? dialog.accept() : dialog.dismiss();
  });
  await button.click();
  await expect.poll(() => message, 'a confirmation was asked').not.toBeNull();
  return message;
}

test('rows keep one line at 360 px; pencil and trash work', async ({ page, baseURL }) => {
  await page.setViewportSize(PHONE);
  const id = unique();
  // Short names fit the name's column on one line; the long ones wrap.
  // (A category's name also sizes the category pickers, which can't wrap:
  // the add form's picker is wider than a phone past about 30 characters,
  // a known limit outside MB-0068.)
  const short = id.slice(-4);
  const long = `Hedelmät ja vihannekset ${short}`;
  const [cold, dry] = [`Kylmä ${short}`, `Kuiva ${short}`];
  for (const name of [long, cold, dry]) {
    await postForm(page, baseURL, '/categories', { name });
  }

  // Categories.
  await page.goto('/categories');
  const categoryRows = page.locator('#category-list > .category-row');
  await expect(categoryRows.first().locator('.drag-handle')).toBeVisible();
  await expectRowsOneLine(page, categoryRows, 'categories');
  const categoryRow = (name) => categoryRows.filter({ has: page.locator('.row-name', { hasText: name }) });

  const renamed = `Kylmät ${short}`;
  await openPanel(categoryRow(cold), t('common.rename_named', { name: cold }), 'category');
  await categoryRow(cold).getByLabel(t('categories.name_of', { name: cold }), { exact: true }).fill(renamed);
  await categoryRow(cold).getByRole('button', { name: t('common.rename'), exact: true }).click();
  await expect(categoryRow(renamed)).toHaveCount(1);
  await expect(categoryRow(renamed).locator('.row-edit')).not.toHaveAttribute('open', '');

  const deleteDry = categoryRow(dry).getByRole('button', { name: t('common.delete_named', { name: dry }), exact: true });
  expect(await confirmClick(page, deleteDry, false)).toBe(t('common.delete_confirm', { name: dry }));
  await expect(categoryRow(dry)).toHaveCount(1);
  expect(await confirmClick(page, deleteDry, true)).toBe(t('common.delete_confirm', { name: dry }));
  await expect(categoryRow(dry)).toHaveCount(0);

  // A favorites group.
  const groupId = await categoryId(page, renamed);
  const favorites = [`Voi ${short}`, `Maito ${short}`, `Laktoositon kevytmaitojuoma ${id}`];
  for (const name of favorites) {
    await postForm(page, baseURL, '/favorites', { name, category_id: groupId });
  }
  await page.goto(`/favorites?open=group-${groupId}`);
  const group = page.locator(`#group-${groupId}`);
  const favoriteRows = group.locator('.product-row');
  await expectRowsOneLine(page, favoriteRows, 'favorites');
  const favoriteRow = (name) => favoriteRows.filter({ has: page.locator('.row-name', { hasText: name }) });

  const butter = `Voi ${short}`;
  const salted = `Suolainen voi ${short}`;
  await openPanel(favoriteRow(butter), t('common.edit_named', { name: butter }), 'favorite');
  await favoriteRow(butter).getByLabel(t('common.name'), { exact: true }).fill(salted);
  await favoriteRow(butter).getByRole('button', { name: t('common.save'), exact: true }).click();
  await expect(favoriteRow(salted)).toHaveCount(1);

  const milk = `Maito ${short}`;
  const deleteMilk = favoriteRow(milk).getByRole('button', { name: t('common.delete_named', { name: milk }), exact: true });
  expect(await confirmClick(page, deleteMilk, true)).toBe(t('common.delete_confirm', { name: milk }));
  await expect(favoriteRow(milk)).toHaveCount(0);

  // A recipe's ingredients.
  const recipe = await postForm(page, baseURL, '/recipes', { title: `Keitto ${id}` });
  const recipePath = new URL(recipe.url()).pathname;
  const ingredients = [`Peruna ${short}`, `Porkkana ${short}`, `Purjosipuli ja persilja ${id}`];
  for (const [i, name] of ingredients.entries()) {
    await postForm(page, baseURL, `${recipePath}/ingredients`, { name, quantity: i === 0 ? '500 g' : '', category_id: groupId });
  }
  await page.goto(recipePath);
  const ingredientRows = page.locator('#ingredient-list > .product-row');
  // An ingredient's name carries its category, so none is a single line.
  await expectRowsOneLine(page, ingredientRows, 'ingredients', { shortName: false });
  const ingredientRow = (name) => ingredientRows.filter({ has: page.locator('.row-name', { hasText: name }) });

  const carrot = ingredients[1];
  await openPanel(ingredientRow(carrot), t('common.edit_named', { name: carrot }), 'ingredient');
  await ingredientRow(carrot).getByLabel(t('common.quantity'), { exact: true }).fill('3');
  await ingredientRow(carrot).getByRole('button', { name: t('common.save'), exact: true }).click();
  await expect(ingredientRow(carrot).locator('.ingredient-quantity')).toHaveText('3');

  const potato = ingredients[0];
  const removePotato = ingredientRow(potato).getByRole('button', { name: t('common.remove_named', { name: potato }), exact: true });
  expect(await confirmClick(page, removePotato, false)).toBe(t('common.remove_confirm', { name: potato }));
  await expect(ingredientRow(potato)).toHaveCount(1);
  expect(await confirmClick(page, removePotato, true)).toBe(t('common.remove_confirm', { name: potato }));
  await expect(ingredientRow(potato)).toHaveCount(0);
});

test('without JavaScript the rows keep one line and their forms work', async ({ browser, baseURL }) => {
  const context = await browser.newContext({
    baseURL,
    locale: 'en-US',
    storageState: '.auth/user.json',
    javaScriptEnabled: false,
    viewport: PHONE,
  });
  const page = await context.newPage();
  const id = unique();
  const short = id.slice(-4);
  const [a, b] = [`Plain A ${short}`, `Plain B ${short}`];
  for (const name of [a, b]) {
    await page.goto('/categories');
    await page.getByLabel(t('categories.new')).fill(name);
    await page.getByRole('button', { name: t('common.add'), exact: true }).click();
  }
  await page.goto('/categories');
  const rows = page.locator('#category-list > .category-row');
  await expect(rows.first().locator('.drag-handle')).toBeHidden();
  await expectRowsOneLine(page, rows, 'categories without JavaScript', { handle: false });
  const row = (name) => rows.filter({ has: page.locator('.row-name', { hasText: name }) });

  const renamed = `Plain C ${short}`;
  await row(a).getByLabel(t('common.rename_named', { name: a }), { exact: true }).click();
  await row(a).getByLabel(t('categories.name_of', { name: a }), { exact: true }).fill(renamed);
  await row(a).getByRole('button', { name: t('common.rename'), exact: true }).click();
  await expect(page).toHaveURL(/\/categories$/);
  await expect(row(renamed)).toHaveCount(1);

  await row(b).getByRole('button', { name: t('common.delete_named', { name: b }), exact: true }).click();
  await expect(page).toHaveURL(/\/categories$/);
  await expect(row(b)).toHaveCount(0);
  await context.close();
});
