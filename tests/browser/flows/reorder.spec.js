// MB-0066 (RR-0053): reordering by dragging, in the browser.
//
// - P1: on the categories page, on a favorites group and on a recipe's
//   ingredients, dragging a row's handle saves the whole new order in one
//   request, and the order holds after a reload.
// - P4: by keyboard, and without JavaScript, the ↑/↓ buttons still reorder;
//   without JavaScript the drag handles don't show.
//
// Every page runs under the fixtures' watch (CSP, console errors, htmx
// errors, other origins), so Sortable and `assets/app/sortable.js` run
// under the unchanged CSP (RR-0053 constraint 4).
import { test, expect, t } from './fixtures.js';

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

/** Create categories named `names`; returns their ids, by name. */
async function categories(page, baseURL, names) {
  for (const name of names) {
    await postForm(page, baseURL, '/categories', { name });
  }
  await page.goto('/categories');
  const ids = {};
  for (const name of names) {
    const input = page.locator(`#category-list input[type="text"][value="${name}"]`);
    ids[name] = (await input.getAttribute('id')).replace(/^cat-/, '');
  }
  return ids;
}

/**
 * Drag `from` (a handle) onto the upper edge of `to` (a row), by mouse, at a
 * pointer's pace: Sortable ignores drag-overs while a row it just moved is
 * still animating.
 */
async function drag(page, from, to) {
  const a = await from.boundingBox();
  const b = await to.boundingBox();
  const [x0, y0] = [a.x + a.width / 2, a.y + a.height / 2];
  const [x1, y1] = [b.x + b.width / 2, b.y + 3];
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  const steps = 30;
  for (let i = 1; i <= steps; i += 1) {
    await page.mouse.move(x0 + ((x1 - x0) * i) / steps, y0 + ((y1 - y0) * i) / steps);
    await page.waitForTimeout(20);
  }
  await page.waitForTimeout(200);
  await page.mouse.move(x1, y1 + 1);
  await page.mouse.up();
}

/** Which of `names` the `rows` show, in row order. */
async function orderOf(rows, names) {
  const texts = await rows.evaluateAll((els) => els.map((el) => el.textContent));
  return texts.flatMap((text) => names.filter((n) => text.includes(n)));
}

test('dragging reorders categories, a favorites group and ingredients', async ({ page, baseURL }) => {
  const id = unique();
  const [a, b, c] = ['A', 'B', 'C'].map((x) => `Drag ${x} ${id}`);
  const ids = await categories(page, baseURL, [a, b, c]);

  // Categories: C onto A.
  const categoryRow = (name) =>
    page.locator('#category-list > .category-row').filter({ has: page.locator(`input[value="${name}"]`) });
  const categoryNames = () =>
    page
      .locator('#category-list > .category-row input[type="text"]')
      .evaluateAll((els) => els.map((el) => el.value))
      .then((all) => all.filter((n) => [a, b, c].includes(n)));
  await expect(categoryRow(a).getByRole('img', { name: t('common.drag_to_reorder', { name: a }) })).toBeVisible();
  const saved = page.waitForResponse(
    (r) => new URL(r.url()).pathname === '/categories/order' && r.request().method() === 'POST',
  );
  await drag(page, categoryRow(c).locator('.drag-handle'), categoryRow(a));
  expect((await saved).status()).toBe(200);
  await expect.poll(categoryNames).toEqual([c, a, b]);
  await page.reload();
  expect(await categoryNames()).toEqual([c, a, b]);

  // A favorites group: Kurkku onto Omena.
  for (const name of ['Omena', 'Banaani', 'Kurkku']) {
    await postForm(page, baseURL, '/favorites', { name: `${name} ${id}`, category_id: ids[a] });
  }
  const names = ['Omena', 'Banaani', 'Kurkku'].map((n) => `${n} ${id}`);
  await page.goto(`/favorites?open=group-${ids[a]}`);
  const group = page.locator(`#group-${ids[a]}`);
  const favorite = (name) => group.locator('.product-row').filter({ hasText: name });
  const favoriteNames = () =>
    orderOf(group.locator('.product-row .product-name'), names);
  expect(await favoriteNames()).toEqual(names);
  const savedFavorites = page.waitForResponse((r) => new URL(r.url()).pathname === '/favorites/order');
  await drag(page, favorite(names[2]).locator('.drag-handle'), favorite(names[0]));
  expect((await savedFavorites).status()).toBe(200);
  await expect.poll(favoriteNames).toEqual([names[2], names[0], names[1]]);
  await page.goto(`/favorites?open=group-${ids[a]}`);
  expect(await favoriteNames()).toEqual([names[2], names[0], names[1]]);

  // A recipe's ingredients: Sipuli onto Peruna.
  const recipe = await postForm(page, baseURL, '/recipes', { title: `Keitto ${id}` });
  const recipePath = new URL(recipe.url()).pathname;
  const ingredients = ['Peruna', 'Porkkana', 'Sipuli'].map((n) => `${n} ${id}`);
  for (const name of ingredients) {
    await postForm(page, baseURL, `${recipePath}/ingredients`, { name, quantity: '', category_id: ids[b] });
  }
  await page.goto(recipePath);
  const ingredient = (name) => page.locator('#ingredient-list > .product-row').filter({ hasText: name });
  const ingredientNames = () =>
    orderOf(page.locator('#ingredient-list > .product-row .product-name'), ingredients);
  expect(await ingredientNames()).toEqual(ingredients);
  const savedIngredients = page.waitForResponse(
    (r) => new URL(r.url()).pathname === `${recipePath}/ingredients/order`,
  );
  await drag(page, ingredient(ingredients[2]).locator('.drag-handle'), ingredient(ingredients[0]));
  expect((await savedIngredients).status()).toBe(200);
  await expect.poll(ingredientNames).toEqual([ingredients[2], ingredients[0], ingredients[1]]);
  await page.reload();
  expect(await ingredientNames()).toEqual([ingredients[2], ingredients[0], ingredients[1]]);
});

test('by keyboard the arrow buttons still reorder', async ({ page, baseURL }) => {
  const id = unique();
  const [a, b] = ['A', 'B'].map((x) => `Keys ${x} ${id}`);
  await categories(page, baseURL, [a, b]);
  const names = () =>
    page
      .locator('#category-list > .category-row input[type="text"]')
      .evaluateAll((els) => els.map((el) => el.value))
      .then((all) => all.filter((n) => [a, b].includes(n)));
  expect(await names()).toEqual([a, b]);

  const up = page.getByRole('button', { name: t('common.move_up', { name: b }) });
  await up.focus();
  await page.keyboard.press('Enter');
  await expect.poll(names).toEqual([b, a]);
});

test('without JavaScript the arrow buttons reorder and no handle shows', async ({ browser, baseURL }) => {
  const context = await browser.newContext({
    baseURL,
    locale: 'en-US',
    storageState: '.auth/user.json',
    javaScriptEnabled: false,
  });
  const page = await context.newPage();
  const id = unique();
  const [a, b] = ['A', 'B'].map((x) => `Plain ${x} ${id}`);
  for (const name of [a, b]) {
    await page.goto('/categories');
    await page.getByLabel(t('categories.new')).fill(name);
    await page.getByRole('button', { name: t('common.add'), exact: true }).click();
  }
  await page.goto('/categories');
  const names = () =>
    page
      .locator('#category-list > .category-row input[type="text"]')
      .evaluateAll((els) => els.map((el) => el.value))
      .then((all) => all.filter((n) => [a, b].includes(n)));
  expect(await names()).toEqual([a, b]);
  await expect(page.locator('.drag-handle').first()).toBeHidden();

  await page.getByRole('button', { name: t('common.move_up', { name: b }) }).click();
  await expect(page).toHaveURL(/\/categories$/);
  expect(await names()).toEqual([b, a]);
  await context.close();
});
