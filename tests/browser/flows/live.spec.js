// MB-0057 (RR-0049): live list updates, in the browser.
//
// - P1, P2: another user's changes reach the list page and the shopping
//   view without a reload (two accounts, two browser contexts).
// - P3: a viewer whose share is revoked sees the error page, and the
//   stream stops without retrying.
// - P5 (RR-0049 sub-answer 7, constraint 6): a live update doesn't discard
//   what the viewer is typing. Here the change comes from a second tab of
//   the same user: another page has another tab id (sub-answer 8).
// - P6: a dropped stream reconnects and catches up; without JavaScript the
//   page works as before.
//
// Every page runs under the fixtures' watch (CSP, console errors, htmx
// errors, other origins).
import {
  test,
  expect,
  t,
  secondUser,
  markDocument,
  expectSameDocument,
  htmxRequest,
} from './fixtures.js';

/** The list's row for the active item `text` (not the removed ones). */
function row(page, text) {
  return page.locator('.list-items > .product-list > .product-row', { hasText: text });
}

/** The page's add field. */
function addField(page) {
  return page.getByLabel(t('lists.item'), { exact: true });
}

/** As the list's owner: create a list with one item, "Maito". */
async function createList(page) {
  const name = `Live list ${Date.now()}${Math.floor(Math.random() * 1000)}`;
  await page.goto('/lists');
  await page.getByLabel(t('lists.new')).fill(name);
  await page.getByRole('button', { name: t('common.create'), exact: true }).click();
  await page.locator('#lists').getByRole('link', { name }).click();
  await expect(page.getByRole('heading', { level: 1, name })).toBeVisible();
  const path = new URL(page.url()).pathname;
  await addItem(page, 'Maito');
  return { name, path, id: path.split('/').pop() };
}

/** As the owner: share the list with the second account as `role`. */
async function share(page, list, role) {
  await page.goto(`${list.path}/sharing`);
  await page.getByLabel(t('auth.email')).fill(secondUser.email);
  await page.getByLabel(t('sharing.access')).selectOption(role);
  await page.getByRole('button', { name: t('sharing.share'), exact: true }).click();
  // The form answers the same whether or not an account matched; the
  // people with access show on the next page load.
  await expect(page.locator(`#share-form-${list.id}`).getByRole('status')).toHaveText(t('sharing.notice'));
  await page.goto(`${list.path}/sharing`);
  await expect(page.locator(`#grantees-${list.id}`)).toContainText(secondUser.email);
  const access = role === 'edit' ? t('sharing.can_edit_items') : t('sharing.can_view');
  await expect(page.locator(`#grantees-${list.id}`)).toContainText(access);
}

/** The list's Edit section on the owner's index. */
function indexRow(page, list) {
  return page.locator('#lists .product-row').filter({ has: page.getByRole('link', { name: list.name }) });
}

/** Add `text` through the page's own add form and wait for its row. */
async function addItem(page, text) {
  await page.getByLabel(t('lists.item'), { exact: true }).fill(text);
  await page.getByRole('button', { name: t('common.add'), exact: true }).click();
  await expect(row(page, text)).toBeVisible();
}

/** Open `text`'s edit section on `page`, set its quantity and save. */
async function saveQuantity(page, text, quantity) {
  const item = row(page, text);
  await item.locator('summary').click();
  await item.locator('input[name="quantity"]').fill(quantity);
  await item.getByRole('button', { name: t('common.save'), exact: true }).click();
  await expect(item.locator('.ingredient-quantity')).toHaveText(quantity);
}

/**
 * The P5 flow. `moveBefore: false` first removes `Element.moveBefore` from
 * the page, so htmx keeps a preserved element the way it does in a browser
 * without it (taking it out of the document and putting it back).
 */
async function typingSurvivesLiveUpdates(page, testInfo, { moveBefore }) {
  if (!moveBefore) {
    await page.addInitScript(() => {
      delete Element.prototype.moveBefore;
    });
  }
  const listName = `Live list ${Date.now()}`;

  await page.goto('/lists');
  await page.getByLabel(t('lists.new')).fill(listName);
  await page.getByRole('button', { name: t('common.create'), exact: true }).click();
  await page.locator('#lists').getByRole('link', { name: listName }).click();
  await expect(page.getByRole('heading', { level: 1, name: listName })).toBeVisible();
  const listPath = new URL(page.url()).pathname;

  await addItem(page, 'Maito');
  await addItem(page, 'Leipä');

  // The same user's second tab on the same list.
  const other = await page.context().newPage();
  await other.goto(listPath);
  await expect(row(other, 'Maito')).toBeVisible();

  const mark = await markDocument(page);
  const hasMoveBefore = await page.evaluate(() => typeof Element.prototype.moveBefore === 'function');
  expect(hasMoveBefore, 'the pinned Chromium has Element.moveBefore, unless the flow removed it').toBe(moveBefore);
  testInfo.annotations.push({ type: 'Element.moveBefore', description: String(hasMoveBefore) });

  // 1. Typing in the add field while the other tab adds an item.
  const field = addField(page);
  await field.click();
  await field.pressSequentially('Kah');
  let refetch = page.waitForRequest(htmxRequest('GET', listPath));
  await addItem(other, 'Juusto');
  // The update is a re-fetch of the list that swaps only its items part.
  expect((await refetch).headers()['hx-target']).toBe(`list-items-${listPath.split('/').pop()}`);
  await expect(row(page, 'Juusto')).toBeVisible();
  await expect(field).toHaveValue('Kah');
  await expect(field).toBeFocused();
  // Typing goes on where it was, and submitting works.
  await page.keyboard.type('vi');
  await expect(field).toHaveValue('Kahvi');
  const sent = page.waitForRequest(htmxRequest('POST', `${listPath}/items`));
  await page.keyboard.press('Enter');
  await sent;
  await expect(row(page, 'Kahvi')).toBeVisible();
  await expect(field).toHaveValue('');
  // The other tab is told about it in turn.
  await expect(row(other, 'Kahvi')).toBeVisible();

  // 2. An open edit section with a typed quantity, while a different item
  // changes: the section is the same element, still open, with its text.
  const milk = row(page, 'Maito');
  await milk.locator('summary').click();
  const quantity = milk.locator('input[name="quantity"]');
  await quantity.click();
  await quantity.pressSequentially('3 l');
  await milk.locator('details').evaluate((el) => {
    el.dataset.kept = 'yes';
  });
  refetch = page.waitForRequest(htmxRequest('GET', listPath));
  await saveQuantity(other, 'Leipä', '2');
  await refetch;
  await expect(row(page, 'Leipä').locator('.ingredient-quantity')).toHaveText('2');
  await expect(milk.locator('details')).toHaveAttribute('open', '');
  await expect(milk.locator('details')).toHaveAttribute('data-kept', 'yes');
  await expect(quantity).toHaveValue('3 l');
  const focusKept = await quantity.evaluate((el) => document.activeElement === el);
  testInfo.annotations.push({ type: 'focus kept in the open edit section', description: String(focusKept) });
  // Where the browser can move an element without detaching it
  // (`Element.moveBefore`), focus stays too. Without it the section stays
  // open with its text, and focus leaves the field.
  expect(focusKept, 'focus in the preserved edit section').toBe(moveBefore);

  // 3. The same item changes: its section is replaced by the saved state
  // (two people editing one field: the last write wins).
  refetch = page.waitForRequest(htmxRequest('GET', listPath));
  await saveQuantity(other, 'Maito', '5');
  await refetch;
  await expect(milk.locator('.ingredient-quantity')).toHaveText('5');
  await expect(milk.locator('details')).not.toHaveAttribute('open');
  await expect(milk.locator('details')).not.toHaveAttribute('data-kept');
  await expect(milk.locator('input[name="quantity"]')).toHaveValue('5');

  await expectSameDocument(page, mark);
  await other.close();
}

test('a live update keeps typed text, focus and an open edit section', async ({ page }, testInfo) => {
  await typingSurvivesLiveUpdates(page, testInfo, { moveBefore: true });
});

test('without Element.moveBefore a live update still keeps typed text and the open section', async ({ page }, testInfo) => {
  await typingSurvivesLiveUpdates(page, testInfo, { moveBefore: false });
});

test("another user's changes reach the list page", async ({ page, second }) => {
  const list = await createList(page);
  await share(page, list, 'edit');

  await second.goto(list.path);
  await expect(row(second, 'Maito')).toBeVisible();
  const mark = await markDocument(second);
  // What the second user is typing stays through all of the below.
  await addField(second).pressSequentially('Kah');

  // An item added, its quantity saved, removed, restored.
  await page.goto(list.path);
  await addItem(page, 'Leipä');
  await expect(row(second, 'Leipä')).toBeVisible();
  await saveQuantity(page, 'Leipä', '2');
  await expect(row(second, 'Leipä').locator('.ingredient-quantity')).toHaveText('2');
  await row(page, 'Leipä').locator('summary').click();
  await row(page, 'Leipä').getByRole('button', { name: t('common.remove'), exact: true }).click();
  await expect(row(second, 'Leipä')).toHaveCount(0);
  await expect(second.locator('.removed-items summary')).toHaveText(t('lists.removed', { count: 1 }));
  await page.locator('.removed-items summary').click();
  await page.locator('.removed-items').getByRole('button', { name: t('lists.restore') }).click();
  await expect(row(second, 'Leipä')).toBeVisible();
  await expect(addField(second)).toHaveValue('Kah');

  // Renamed: the heading and the title, which sit outside the swapped part.
  const renamed = `${list.name} renamed`;
  await page.goto('/lists');
  await indexRow(page, list).locator('summary').click();
  await indexRow(page, list).getByLabel(t('common.name')).fill(renamed);
  await indexRow(page, list).getByRole('button', { name: t('common.rename') }).click();
  await expect(second.getByRole('heading', { level: 1 })).toHaveText(renamed);
  await expect(second).toHaveTitle(`${renamed} — kauppalista.app`);
  await expect(addField(second)).toHaveValue('Kah');
  list.name = renamed;

  // Archived: the notice takes the add form's place, and items lose their
  // edit sections.
  await indexRow(page, list).locator('summary').click();
  await indexRow(page, list).getByRole('button', { name: t('lists.archive') }).click();
  await expect(second.locator('.archived-notice')).toBeVisible();
  await expect(addField(second)).toHaveCount(0);
  await expect(row(second, 'Maito')).toBeVisible();
  await expect(row(second, 'Maito').locator('details')).toHaveCount(0);

  // Restored: the add form and the edit sections are back.
  await page.locator('#archived-lists summary').first().click();
  await page.locator('#archived-lists').getByRole('button', { name: t('lists.restore') }).click();
  await expect(addField(second)).toBeVisible();
  await expect(second.locator('.archived-notice')).toHaveCount(0);
  await expect(row(second, 'Maito').locator('details')).toHaveCount(1);
  // And the restored form works.
  await addItem(second, 'Kahvi');

  // The share's role becomes "view": the add form goes.
  await share(page, list, 'view');
  await expect(second.getByText(t('lists.view_shared'))).toBeVisible();
  await expect(addField(second)).toHaveCount(0);
  await expect(row(second, 'Kahvi')).toBeVisible();
  await expect(row(second, 'Kahvi').locator('details')).toHaveCount(0);

  await expectSameDocument(second, mark);
});

test("another user's changes reach the shopping view", async ({ page, second }) => {
  const list = await createList(page);
  await share(page, list, 'edit');

  await second.goto(`${list.path}/shop`);
  await expect(second.getByRole('status')).toHaveText(t('shop.progress', { found: 0, total: 1 }));
  const mark = await markDocument(second);

  // An item added on the list page.
  await page.goto(list.path);
  await addItem(page, 'Leipä');
  await expect(second.getByRole('button', { name: t('shop.mark_found', { item: 'Leipä' }) })).toBeVisible();
  await expect(second.getByRole('status')).toHaveText(t('shop.progress', { found: 0, total: 2 }));

  // Checked in the owner's shopping view; then the other way round.
  await page.goto(`${list.path}/shop`);
  const ownerMark = await markDocument(page);
  await page.getByRole('button', { name: t('shop.mark_found', { item: 'Maito' }) }).click();
  await expect(second.getByRole('status')).toHaveText(t('shop.progress', { found: 1, total: 2 }));
  await expect(
    second.getByRole('button', { name: t('shop.mark_not_found', { item: 'Maito' }) }),
  ).toHaveAttribute('aria-pressed', 'true');
  await second.getByRole('button', { name: t('shop.mark_found', { item: 'Leipä' }) }).click();
  await expect(page.getByRole('status')).toHaveText(t('shop.progress', { found: 2, total: 2 }));
  // Removed in the second user's shopping view.
  await second.getByRole('button', { name: t('shop.remove', { item: 'Leipä' }) }).click();
  await expect(page.getByRole('status')).toHaveText(t('shop.progress', { found: 1, total: 1 }));
  await expectSameDocument(page, ownerMark);

  // Renamed: the shopping view's heading and title.
  const renamed = `${list.name} renamed`;
  await page.goto('/lists');
  await indexRow(page, list).locator('summary').click();
  await indexRow(page, list).getByLabel(t('common.name')).fill(renamed);
  await indexRow(page, list).getByRole('button', { name: t('common.rename') }).click();
  await expect(second.getByRole('heading', { level: 1 })).toHaveText(renamed);
  await expect(second).toHaveTitle(`${renamed} — kauppalista.app`);

  await expectSameDocument(second, mark);
});

test('a revoked viewer sees the error page and the stream stops', async ({ page, second, failedLoads }) => {
  const list = await createList(page);
  await share(page, list, 'view');

  // The second user's page: what its event stream does.
  const streams = [];
  second.on('request', (r) => {
    if (new URL(r.url()).pathname === `${list.path}/events`) streams.push(r.url());
  });
  await second.addInitScript(() => {
    window.__sseClosed = [];
    document.addEventListener('htmx:sseClose', (e) => window.__sseClosed.push(e.detail.type));
  });
  await second.goto(list.path);
  await expect(row(second, 'Maito')).toBeVisible();
  const mark = await markDocument(second);

  // The re-fetch after the revoke is a 404, which the page shows.
  failedLoads.allow(404, list.path);
  const refetch = second.waitForResponse(
    (r) => new URL(r.url()).pathname === list.path && r.request().headers()['hx-request'] === 'true',
  );
  page.once('dialog', (dialog) => dialog.accept());
  await page.getByRole('button', { name: t('sharing.revoke') }).click();
  await expect(page.locator(`#grantees-${list.id}`)).not.toContainText(secondUser.email);

  expect((await refetch).status()).toBe(404);
  // The error page, in place of the list: not a blank page, and no stale
  // list content.
  await expect(second.getByRole('heading', { level: 1 })).toHaveText(t('error_page.not_found.title'));
  await expect(second.getByText(t('error_page.not_found.text'))).toBeVisible();
  await expect(second.getByText('Maito')).toHaveCount(0);
  await expect(second.locator('[sse-connect]')).toHaveCount(0);
  await expectSameDocument(second, mark);

  // The stream was closed by the server's `stop` (the extension reports
  // the close as "message"), not by an error, so nothing retries: the
  // fixtures fail on any `htmx:sseError`, and there was one connection.
  expect(await second.evaluate(() => window.__sseClosed)).toContain('message');
  expect(streams).toHaveLength(1);
});

test('a dropped stream reconnects and catches up', async ({ page, second, sseErrors }) => {
  const list = await createList(page);
  await share(page, list, 'edit');
  await second.goto(list.path);
  await expect(row(second, 'Maito')).toBeVisible();
  await page.goto(list.path);
  await expect(row(page, 'Maito')).toBeVisible();
  const mark = await markDocument(page);

  // Hold the reconnect until the other user's change is saved, so the
  // change happens while the stream is down.
  let changeSaved;
  const saved = new Promise((resolve) => {
    changeSaved = resolve;
  });
  let reconnects = 0;
  await page.route(`**${list.path}/events*`, async (route) => {
    reconnects += 1;
    await saved;
    await route.fallback();
  });

  // Drop the stream as a lost connection does: the EventSource is closed
  // and reports an error. From here this flow, and only this one, may see
  // `htmx:sseError` (RR-0049 sub-answer 9).
  sseErrors.allowAfterDrop();
  const reconnected = page.waitForResponse((r) => new URL(r.url()).pathname === `${list.path}/events`);
  await page.locator('section[sse-connect]').evaluate((section) => {
    const source = section['htmx-internal-data'].sseEventSource;
    source.close();
    source.dispatchEvent(new Event('error'));
  });

  await addItem(second, 'Juusto');
  changeSaved();

  // The extension reconnects by itself; the new stream starts with the
  // change it missed, and the page shows it without a reload.
  expect((await reconnected).status()).toBe(200);
  await expect(row(page, 'Juusto')).toBeVisible();
  expect(reconnects).toBe(1);
  // Exactly one stream error was let through: htmx's event in both
  // spellings, its general error event, and the line it logs.
  expect([...sseErrors.seen].sort()).toEqual(
    ['console: [object Event]', 'htmx:error', 'htmx:sse-error', 'htmx:sseError'].sort(),
  );

  // The new stream is live.
  await addItem(second, 'Kahvi');
  await expect(row(page, 'Kahvi')).toBeVisible();
  await expectSameDocument(page, mark);
});

test('without JavaScript the list page works as before', async ({ browser, baseURL }) => {
  const context = await browser.newContext({
    baseURL,
    locale: 'en-US',
    storageState: '.auth/user.json',
    javaScriptEnabled: false,
  });
  const page = await context.newPage();
  const streams = [];
  page.on('request', (r) => {
    if (new URL(r.url()).pathname.endsWith('/events')) streams.push(r.url());
  });

  const name = `Plain list ${Date.now()}`;
  await page.goto('/lists');
  await page.getByLabel(t('lists.new')).fill(name);
  await page.getByRole('button', { name: t('common.create'), exact: true }).click();
  // A plain form post: the new list's page, after a redirect.
  await expect(page.getByRole('heading', { level: 1, name })).toBeVisible();
  await addField(page).fill('Maito');
  await page.getByRole('button', { name: t('common.add'), exact: true }).click();
  await expect(row(page, 'Maito')).toBeVisible();
  await expect(addField(page)).toHaveValue('');
  await page.getByRole('link', { name: t('lists.go_shopping') }).click();
  await page.getByRole('button', { name: t('shop.mark_found', { item: 'Maito' }) }).click();
  await expect(page.getByRole('status')).toHaveText(t('shop.progress', { found: 1, total: 1 }));
  // No script, no stream.
  expect(streams).toEqual([]);
  await context.close();
});
