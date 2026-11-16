// MB-0057 P5 (RR-0049 sub-answer 7, constraint 6): a live update doesn't
// discard what the viewer is typing.
//
// The harness has one account, so the change comes from a second tab of
// the same user: another page has another tab id, so the first page is
// told about it over its event stream (RR-0049 sub-answer 8). The first
// page runs under the fixtures' watch (CSP, console errors, htmx errors).
import { test, expect, t, markDocument, expectSameDocument, htmxRequest } from './fixtures.js';

/** The list's row for the item `text`. */
function row(page, text) {
  return page.locator('.list-items .product-row', { hasText: text });
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
  const addField = page.getByLabel(t('lists.item'), { exact: true });
  await addField.click();
  await addField.pressSequentially('Kah');
  let refetch = page.waitForRequest(htmxRequest('GET', listPath));
  await addItem(other, 'Juusto');
  // The update is a re-fetch of the list that swaps only its items part.
  expect((await refetch).headers()['hx-target']).toBe(`list-items-${listPath.split('/').pop()}`);
  await expect(row(page, 'Juusto')).toBeVisible();
  await expect(addField).toHaveValue('Kah');
  await expect(addField).toBeFocused();
  // Typing goes on where it was, and submitting works.
  await page.keyboard.type('vi');
  await expect(addField).toHaveValue('Kahvi');
  const sent = page.waitForRequest(htmxRequest('POST', `${listPath}/items`));
  await page.keyboard.press('Enter');
  await sent;
  await expect(row(page, 'Kahvi')).toBeVisible();
  await expect(addField).toHaveValue('');
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
