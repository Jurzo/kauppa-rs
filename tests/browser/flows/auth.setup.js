// Registers the flows' accounts with the invitations scripts/browser/run.sh
// created (`kauppa-web admin invite`), through the boosted form, and saves
// each one's signed-in cookies for the other flows. The second account is
// for the flows where one user sees another's changes (MB-0057).
import { mkdirSync } from 'node:fs';
import { test, expect, t, user, secondUser, htmxRequest } from './fixtures.js';

async function register(page, invite, account, statePath) {
  await page.goto(invite);
  await page.getByLabel(t('auth.display_name')).fill(account.name);
  await page.getByLabel(t('auth.email')).fill(account.email);
  await page.getByLabel(t('auth.password')).fill(account.password);
  const sent = page.waitForRequest(htmxRequest('POST', '/auth/register'));
  await page.getByRole('button', { name: t('auth.create_account_button') }).click();
  await sent;

  // Signed in: the navigation offers Sign out.
  await expect(page.getByRole('button', { name: t('nav.sign_out') })).toBeVisible();

  mkdirSync('.auth', { recursive: true });
  await page.context().storageState({ path: statePath });
}

test('register the test account from an invitation', async ({ page }) => {
  const invite = process.env.KAUPPA_INVITE_PATH;
  expect(invite, 'KAUPPA_INVITE_PATH from scripts/browser/run.sh').toBeTruthy();
  await register(page, invite, user, '.auth/user.json');
});

test('register the second test account from an invitation', async ({ page }) => {
  const invite = process.env.KAUPPA_INVITE_PATH_2;
  expect(invite, 'KAUPPA_INVITE_PATH_2 from scripts/browser/run.sh').toBeTruthy();
  await register(page, invite, secondUser, '.auth/second.json');
});
