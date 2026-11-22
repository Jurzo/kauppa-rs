// MB-0071: dragging a favorite into another category.
//
// - P1, P4: on /favorites, dragging a favorite's handle into another open
//   group's list moves it there, at the place it was dropped. One request
//   (`POST /favorites/order` with `moved`) saves it, the catalog comes back
//   with both groups open, and the move holds after a reload.
// - A closed group opens when the held row rests on its heading, and takes
//   the drop (the owner's decision, 2026-11-22). Uncategorized takes a drop
//   too, which clears the category.
// - P5: recipe products have no handle; without JavaScript the edit panel's
//   category picker still moves a favorite.
//
// Every page runs under the fixtures' watch (CSP, console errors, htmx
// errors, other origins), so the changed `assets/app/sortable.js` runs under
// the unchanged CSP (RR-0053 constraint 4).
import { test, expect, t, drag, inView, glide, release } from './fixtures.js';

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

/** Create a category; returns its id. */
async function category(page, baseURL, name) {
  await postForm(page, baseURL, '/categories', { name });
  await page.goto('/categories');
  const input = page.locator(`#category-list input[type="text"][value="${name}"]`);
  return (await input.getAttribute('id')).replace(/^cat-/, '');
}

/** The favorites a group shows, in order, among `names`. */
function namesIn(group, names) {
  return group
    .locator('.product-row .product-name')
    .evaluateAll((els) => els.map((el) => el.textContent.trim()))
    .then((all) => all.filter((n) => names.includes(n)));
}

/** Every `POST /favorites/order` the page sends, with its form fields. */
function orderRequests(page) {
  const sent = [];
  page.on('request', (r) => {
    if (new URL(r.url()).pathname === '/favorites/order' && r.method() === 'POST') {
      sent.push(new URLSearchParams(r.postData() ?? ''));
    }
  });
  return sent;
}

test('dragging a favorite into another category moves it there', async ({ page, baseURL }) => {
  const id = unique();
  const [a, b] = [`Regroup A ${id}`, `Regroup B ${id}`];
  const ids = { [a]: await category(page, baseURL, a), [b]: await category(page, baseURL, b) };
  const names = ['Omena', 'Päärynä', 'Kurkku', 'Tomaatti'].map((n) => `${n} ${id}`);
  const [omena, paaryna, kurkku, tomaatti] = names;
  for (const [name, cat] of [[omena, a], [paaryna, a], [kurkku, b], [tomaatti, b]]) {
    await postForm(page, baseURL, '/favorites', { name, category_id: ids[cat] });
  }
  const group = (cat) => page.locator(`#group-${ids[cat]}`);
  const row = (name) => page.locator('.product-group .product-row').filter({ hasText: name });
  const sent = orderRequests(page);

  // Both groups open (A above B). Tomaatti up onto Päärynä's upper edge:
  // into A, between Omena and Päärynä. (Dragged upward, a row lands before
  // the row it is dropped on, as in MB-0066's flows.)
  await page.goto(`/favorites?open=group-${ids[a]},group-${ids[b]}`);
  await expect(row(tomaatti).getByRole('img', { name: t('common.drag_to_reorder', { name: tomaatti }) })).toBeVisible();
  const tomaattiId = await row(tomaatti).locator('input[name="id"]').getAttribute('value');
  let saved = page.waitForResponse((r) => new URL(r.url()).pathname === '/favorites/order');
  await drag(page, row(tomaatti).locator('.drag-handle'), row(paaryna));
  expect((await saved).status()).toBe(200);
  await expect.poll(() => namesIn(group(a), names)).toEqual([omena, tomaatti, paaryna]);
  expect(await namesIn(group(b), names)).toEqual([kurkku]);
  // One request, from the group it was dropped in, naming the moved favorite.
  expect(sent.length, 'one order request').toBe(1);
  expect(sent[0].get('category')).toBe(ids[a]);
  expect(sent[0].get('moved')).toBe(tomaattiId);
  expect(sent[0].getAll('id').length).toBe(3);
  // The catalog came back with both groups open (RR-0044 item 1).
  await expect(group(a)).toHaveAttribute('open', '');
  await expect(group(b)).toHaveAttribute('open', '');
  await page.goto(`/favorites?open=group-${ids[a]},group-${ids[b]}`);
  expect(await namesIn(group(a), names)).toEqual([omena, tomaatti, paaryna]);
  expect(await namesIn(group(b), names)).toEqual([kurkku]);

  // B closed: hold Päärynä on B's heading until it opens, then drop it on
  // Kurkku. Dragged downward over a row, a row swaps past it (Sortable's
  // rule within a list too), so it lands after Kurkku: the order the page
  // shows at the drop is the order saved.
  await page.goto(`/favorites?open=group-${ids[a]}`);
  await expect(group(b)).not.toHaveAttribute('open', '');
  const heading = group(b).locator('> summary');
  const [handle, head] = await inView(page, row(paaryna).locator('.drag-handle'), heading);
  const start = [handle.x + handle.width / 2, handle.y + handle.height / 2];
  const over = [head.x + Math.min(head.width / 2, 120), head.y + head.height / 2];
  saved = page.waitForResponse((r) => new URL(r.url()).pathname === '/favorites/order');
  await page.mouse.move(...start);
  await page.mouse.down();
  await glide(page, start, over);
  await expect(group(b)).toHaveAttribute('open', '', { timeout: 2_000 });
  const [first] = await inView(page, row(kurkku));
  const target = [first.x + first.width / 2, first.y + 3];
  await glide(page, over, target);
  await release(page, target);
  expect((await saved).status()).toBe(200);
  await expect.poll(() => namesIn(group(b), names)).toEqual([kurkku, paaryna]);
  expect(await namesIn(group(a), names)).toEqual([omena, tomaatti]);
  expect(sent.length, 'one more order request').toBe(2);
  expect(sent[1].get('category')).toBe(ids[b]);
  await page.reload();
  expect(await namesIn(group(b), names)).toEqual([kurkku, paaryna]);

  // Into Uncategorized: the category is cleared. (Other flows' favorites
  // may be there too, so only membership is checked.)
  const loose = `Irtonainen ${id}`;
  await postForm(page, baseURL, '/favorites', { name: loose, category_id: ids[a] });
  await page.goto(`/favorites?open=group-${ids[a]}`);
  const looseId = await row(loose).locator('input[name="id"]').getAttribute('value');
  await postForm(page, baseURL, `/favorites/${looseId}`, { name: loose, category_id: 'none' });
  await page.goto(`/favorites?open=group-${ids[a]},group-uncategorized`);
  const uncategorized = page.locator('#group-uncategorized');
  saved = page.waitForResponse((r) => new URL(r.url()).pathname === '/favorites/order');
  await drag(page, row(tomaatti).locator('.drag-handle'), row(loose));
  expect((await saved).status()).toBe(200);
  await expect.poll(async () => (await namesIn(uncategorized, [tomaatti, loose])).sort()).toEqual([loose, tomaatti].sort());
  expect(sent.at(-1).get('category')).toBe('none');
  expect(sent.at(-1).get('moved')).toBe(tomaattiId);
  await page.reload();
  expect((await namesIn(uncategorized, [tomaatti, loose])).sort()).toEqual([loose, tomaatti].sort());
  expect(await namesIn(group(a), names)).toEqual([omena]);

  // Recipe products ("From recipes") have no handle to drag.
  await expect(page.locator('#recipe-products .drag-handle')).toHaveCount(0);
});

test('without JavaScript the edit panel still moves a favorite', async ({ browser, baseURL }) => {
  const context = await browser.newContext({
    baseURL,
    locale: 'en-US',
    storageState: '.auth/user.json',
    javaScriptEnabled: false,
  });
  const page = await context.newPage();
  const id = unique();
  const [a, b] = [`Plain regroup A ${id}`, `Plain regroup B ${id}`];
  const ids = {};
  for (const name of [a, b]) {
    await page.goto('/categories');
    await page.getByLabel(t('categories.new')).fill(name);
    await page.getByRole('button', { name: t('common.add'), exact: true }).click();
  }
  await page.goto('/categories');
  for (const name of [a, b]) {
    ids[name] = (await page.locator(`#category-list input[type="text"][value="${name}"]`).getAttribute('id')).replace(/^cat-/, '');
  }
  const favorite = `Siirrettävä ${id}`;
  await page.goto('/favorites');
  await page.getByLabel(t('products.product')).fill(favorite);
  await page.locator('#favorites .add-product select').selectOption(ids[a]);
  await page.getByRole('button', { name: t('common.add'), exact: true }).click();

  await page.goto(`/favorites?open=group-${ids[a]}`);
  await expect(page.locator('.drag-handle').first()).toBeHidden();
  const row = page.locator(`#group-${ids[a]} .product-row`).filter({ hasText: favorite });
  await row.getByLabel(t('common.edit_named', { name: favorite }), { exact: true }).click();
  await row.locator('.row-edit select').selectOption(ids[b]);
  await row.getByRole('button', { name: t('common.save'), exact: true }).click();
  await expect(page.locator(`#group-${ids[b]} .product-row`).filter({ hasText: favorite })).toHaveCount(1);
  await expect(page.locator(`#group-${ids[a]}`)).toHaveCount(0);
  await context.close();
});
