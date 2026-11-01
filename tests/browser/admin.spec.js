const { test, expect } = require('@playwright/test');

test.beforeEach(async ({ context }) => {
  await context.addCookies([{ name: 'admin_test_session', value: 'a', url: 'http://127.0.0.1:39127' }]);
});

test('search, filters, sorting and selection survive fragment swaps', async ({ page }) => {
  await page.goto('/staff/accounts');
  await page.evaluate(() => { window.pageMarker = 'original'; });
  await page.getByRole('searchbox').fill('alice');
  await expect(page).toHaveURL(/q=alice/);
  await expect(page.locator('tbody tr')).toHaveCount(1);
  await expect(page.locator('tbody')).toContainText('alice');
  await page.getByRole('checkbox', { name: 'Select all rows' }).check();
  await expect(page.getByRole('checkbox', { name: 'Select row', exact: true })).toBeChecked();
  await page.getByRole('searchbox').fill('');
  await expect(page.locator('tbody tr')).toHaveCount(2);
  await expect(page.getByRole('checkbox', { name: 'Select all rows' })).not.toBeChecked();
  await page.locator('.admin-filter a').filter({ hasText: /^Premium$/ }).click();
  await expect(page.locator('tbody tr')).toHaveCount(1);
  await expect(page.locator('tbody')).toContainText('bob');
  await page.locator('thead a').filter({ hasText: /^Username/ }).click();
  await expect(page).toHaveURL(/o=/);
  expect(await page.evaluate(() => window.pageMarker)).toBe('original');
});

test('pagination, history and reload restore full pages', async ({ page }) => {
  await page.goto('/staff/exchanges');
  await expect(page.locator('tbody')).toContainText('XAMS');
  await page.getByRole('link', { name: 'Next', exact: true }).click();
  await expect(page).toHaveURL(/p=2/);
  await expect(page.locator('tbody')).toContainText('XOFF');
  await page.goBack();
  await expect(page.locator('tbody')).toContainText('XAMS');
  await page.goForward();
  await expect(page.locator('tbody')).toContainText('XOFF');
  await page.reload();
  await expect(page.locator('nav.navbar')).toBeVisible();
  await expect(page.locator('tbody')).toContainText('XOFF');
});

test('autocomplete returns options and submits the selected ID', async ({ page }) => {
  await page.goto('/staff/rules/1/change');
  const input = page.locator('[data-admin-autocomplete]');
  await input.fill('Lon');
  await expect(page.locator('datalist option[value="XLON"]')).toHaveCount(1);
  await input.fill('XLON');
  await page.getByRole('button', { name: 'Save and continue editing', exact: true }).click();
  await expect(input).toHaveValue('XLON');
  await expect(page.locator('datalist option[value="XLON"]')).toHaveCount(1);
});

test('inline writes use CSRF and leave the row unchanged on rejection', async ({ page, context }) => {
  await page.goto('/staff/accounts');
  await page.route('**/toggle/is_staff', async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 200));
    await route.continue();
  });
  let row = page.locator('tbody tr').filter({ hasText: 'alice' });
  const button = row.getByRole('button', { name: /Turn/ });
  const previous = await button.textContent();
  await button.focus();
  await page.keyboard.press("Enter");
  await expect(row.getByRole('button', { name: /Turn/ })).not.toHaveText(previous);
  await expect(row.getByRole("button", { name: /Turn/ })).toBeFocused();
  await expect(page.locator("#admin-status")).toContainText("Row updated");
  const updated = await row.textContent();
  await page.locator('#admin-toggle input[name="_csrf"]').evaluate((input) => { input.value = 'invalid'; });
  await row.getByRole('button', { name: /Turn/ }).click();
  await expect(page.locator('#admin-request-error')).toBeVisible();
  await expect(row).toHaveText(updated);
  await context.clearCookies();
  await row.getByRole('button', { name: /Turn/ }).click();
  await expect(page.locator('#admin-request-error')).toBeVisible();
  await expect(row).toHaveText(updated);
});

test('redirected login content is never inserted into results', async ({ page }) => {
  await page.goto('/staff/accounts');
  await page.route('**/staff/accounts?*', (route) => route.fulfill({ contentType: 'text/html', body: '<h1>Unexpected login page</h1>' }));
  await page.getByRole('searchbox').fill('test');
  await expect(page.locator('#admin-request-error')).toBeVisible();
  await expect(page.locator('body')).not.toContainText('Unexpected login page');
  await expect(page.locator('tbody tr')).toHaveCount(2);
});

test('custom templates, filters, assets and narrow screens work', async ({ page }) => {
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/staff');
  await expect(page.getByRole('heading', { name: 'TEST ADMIN' })).toBeVisible();
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--admin-cell-space').trim())).toBe('.4rem');
  await page.goto('/staff/accounts');
  await page.getByRole('searchbox').focus();
  await page.keyboard.type('alice');
  await expect(page).toHaveURL(/q=alice/);
  await expect(page.getByRole('searchbox')).toBeFocused();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  expect(errors).toEqual([]);
});

test.describe('without JavaScript', () => {
  test.use({ javaScriptEnabled: false });
  test('ordinary search, pagination and forms still work', async ({ page }) => {
    await page.goto('/staff/accounts');
    await page.getByRole('searchbox').fill('bob');
    await page.getByRole('button', { name: 'Search', exact: true }).click();
    await expect(page.locator('tbody tr')).toHaveCount(1);
    await expect(page.locator('tbody')).toContainText('bob');
    await page.goto('/staff/exchanges');
    await page.getByRole('link', { name: 'Next', exact: true }).click();
    await expect(page.locator('tbody')).toContainText('XOFF');
    await page.goto('/staff/rules/1/change');
    await page.locator('[data-admin-autocomplete]').fill('XAMS');
    await page.getByRole('button', { name: 'Save and continue editing', exact: true }).click();
    await expect(page.locator('[data-admin-autocomplete]')).toHaveValue('XAMS');
  });
});

test('a slower old lookup cannot replace newer options', async ({ page }) => {
  await page.goto('/staff/rules/1/change');
  let firstStarted;
  const started = new Promise((resolve) => { firstStarted = resolve; });
  await page.route('**/staff/exchanges/autocomplete?*', async (route) => {
    const query = new URL(route.request().url()).searchParams.get('q');
    if (query === 'old') {
      firstStarted();
      await new Promise((resolve) => setTimeout(resolve, 900));
    }
    await route.fulfill({ contentType: 'text/html', headers: { 'X-Loco-Admin-Fragment': 'true' }, body: `<option value="${query}">${query}</option>` }).catch(() => {});
  });
  const input = page.locator('[data-admin-autocomplete]');
  await input.fill('old');
  await started;
  await input.fill('new');
  await expect(page.locator('datalist option')).toHaveAttribute('value', 'new');
  await page.waitForTimeout(1000);
  await expect(page.locator('datalist option')).toHaveAttribute('value', 'new');
});

test('pending toggles cannot be submitted twice', async ({ page }) => {
  await page.goto('/staff/accounts');
  let requests = 0;
  await page.route('**/toggle/is_staff', async (route) => {
    requests++;
    await new Promise((resolve) => setTimeout(resolve, 300));
    await route.continue();
  });
  const button = page.locator('tbody tr').filter({ hasText: 'alice' }).getByRole('button', { name: /Turn/ });
  await button.evaluate((element) => { element.click(); element.click(); });
  await expect(button).toBeDisabled();
  await expect(button).toBeEnabled();
  expect(requests).toBe(1);
});
