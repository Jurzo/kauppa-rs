// MB-0047 P1 and P2 (RR-0041): the three flows markup tests can't prove.
// Each runs under the fixtures' watch for CSP violations, console errors,
// htmx error events and other origins (P2).
import {
  test,
  expect,
  t,
  user,
  markDocument,
  expectSameDocument,
  htmxRequest,
} from './fixtures.js';

test.describe('signed out', () => {
  test.use({ storageState: { cookies: [], origins: [] } });

  // The #58 regression: `hx-ext="response-targets"` must sit where htmx
  // reads it (on <body>, RR-0025), or the 401 page is never swapped in.
  test('a boosted wrong-password sign-in shows its error', async ({ page, failedLoads }) => {
    failedLoads.allow(401, '/auth/login');

    await page.goto('/auth/login');
    const mark = await markDocument(page);

    await page.getByLabel(t('auth.email')).fill(user.email);
    await page.getByLabel(t('auth.password')).fill('not the right password');
    const sent = page.waitForRequest(htmxRequest('POST', '/auth/login'));
    const answered = page.waitForResponse(
      (r) => r.request().method() === 'POST' && new URL(r.url()).pathname === '/auth/login',
    );
    await page.getByRole('button', { name: t('auth.sign_in') }).click();
    await sent;
    expect((await answered).status()).toBe(401);

    await expect(page.getByRole('alert')).toHaveText(t('auth.invalid_login'));
    await expectSameDocument(page, mark);
  });
});

test('a list item add and check swap their fragment', async ({ page }) => {
  const listName = `Browser list ${Date.now()}`;
  const item = 'Maito';

  await page.goto('/lists');
  let mark = await markDocument(page);

  // Create a list: #lists is replaced by htmx.
  await page.getByLabel(t('lists.new')).fill(listName);
  let sent = page.waitForRequest(htmxRequest('POST', '/lists'));
  await page.getByRole('button', { name: t('common.create'), exact: true }).click();
  await sent;
  const listLink = page.locator('#lists').getByRole('link', { name: listName });
  await expect(listLink).toBeVisible();
  await expectSameDocument(page, mark);

  // Open it (boosted), then add an item: #list-<id> is replaced.
  await listLink.click();
  await expect(page.getByRole('heading', { level: 1, name: listName })).toBeVisible();
  const listPath = new URL(page.url()).pathname;
  mark = await markDocument(page);

  await page.getByLabel(t('lists.item'), { exact: true }).fill(item);
  sent = page.waitForRequest(htmxRequest('POST', `${listPath}/items`));
  await page.getByRole('button', { name: t('common.add'), exact: true }).click();
  expect((await sent).headers()['hx-target']).toBe(`list-${listPath.split('/').pop()}`);
  await expect(page.locator('.shopping-list .product-row', { hasText: item })).toBeVisible();
  await expect(page.getByText(t('lists.no_items'))).toHaveCount(0);
  await expect(page.getByLabel(t('lists.item'), { exact: true })).toHaveValue('');
  await expectSameDocument(page, mark);

  // Shopping view (boosted), then check the item: #shop-<id> is replaced.
  await page.getByRole('link', { name: t('lists.go_shopping') }).click();
  await expect(page.getByRole('status')).toHaveText(t('shop.progress', { found: 0, total: 1 }));
  mark = await markDocument(page);

  const toggle = page.getByRole('button', { name: t('shop.mark_found', { item }) });
  const checked = page.waitForRequest(
    (r) =>
      r.method() === 'POST' &&
      /\/shop\/items\/[^/]+\/check$/.test(new URL(r.url()).pathname) &&
      r.headers()['hx-request'] === 'true',
  );
  await toggle.click();
  await checked;
  await expect(page.getByRole('status')).toHaveText(t('shop.progress', { found: 1, total: 1 }));
  await expect(
    page.getByRole('button', { name: t('shop.mark_not_found', { item }) }),
  ).toHaveAttribute('aria-pressed', 'true');
  await expectSameDocument(page, mark);
});

test('boosted navigation updates the page', async ({ page }) => {
  await page.goto('/lists');
  const mark = await markDocument(page);
  const nav = page.getByRole('navigation', { name: t('nav.label') });
  await expect(nav.getByRole('link', { name: t('nav.lists') })).toHaveAttribute('aria-current', 'page');

  const sent = page.waitForRequest(
    (r) =>
      r.method() === 'GET' &&
      new URL(r.url()).pathname === '/recipes' &&
      r.headers()['hx-boosted'] === 'true',
  );
  await nav.getByRole('link', { name: t('nav.recipes') }).click();
  await sent;

  await expect(page).toHaveURL(/\/recipes$/);
  await expect(page).toHaveTitle(`${t('recipes.title')} — kauppalista.app`);
  await expect(nav.getByRole('link', { name: t('nav.recipes') })).toHaveAttribute('aria-current', 'page');
  await expect(nav.getByRole('link', { name: t('nav.lists') })).not.toHaveAttribute('aria-current', 'page');
  await expectSameDocument(page, mark);
});
