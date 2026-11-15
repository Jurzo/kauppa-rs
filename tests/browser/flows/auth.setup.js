// Registers the flows' account with the invitation scripts/browser/run.sh
// created (`kauppa-web admin invite`), through the boosted form, and saves
// the signed-in cookies for the other flows.
import { mkdirSync } from 'node:fs';
import { test, expect, t, user, htmxRequest } from './fixtures.js';

test('register the test account from an invitation', async ({ page }) => {
  const invite = process.env.KAUPPA_INVITE_PATH;
  expect(invite, 'KAUPPA_INVITE_PATH from scripts/browser/run.sh').toBeTruthy();

  await page.goto(invite);
  await page.getByLabel(t('auth.display_name')).fill(user.name);
  await page.getByLabel(t('auth.email')).fill(user.email);
  await page.getByLabel(t('auth.password')).fill(user.password);
  const sent = page.waitForRequest(htmxRequest('POST', '/auth/register'));
  await page.getByRole('button', { name: t('auth.create_account_button') }).click();
  await sent;

  // Signed in: the navigation offers Sign out.
  await expect(page.getByRole('button', { name: t('nav.sign_out') })).toBeVisible();

  mkdirSync('.auth', { recursive: true });
  await page.context().storageState({ path: '.auth/user.json' });
});
