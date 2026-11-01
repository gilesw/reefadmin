const { test, expect } = require('@playwright/test');
async function login(page, email = 'alice@example.test') {
  await page.goto('/login');
  await page.getByLabel('Email').fill(email);
  await page.getByLabel('Password').fill('test-only-inventory-password');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page).toHaveURL(/\/account$/);
}

test('host login, scoped edit, lookup, toggle and logout', async ({ page }) => {
  await page.goto('/staff');
  await expect(page).toHaveURL(/\/login$/);
  await login(page);
  await page.goto('/staff/products');
  await expect(page.getByText('Alice compass', { exact: true })).toBeVisible();
  await expect(page.getByText('Bob radio', { exact: true })).toHaveCount(0);
  await page.getByRole('link', { name: 'Alice compass', exact: true }).click();
  await page.locator('input[name="name"]').fill('Updated compass');
  await page.locator('[data-admin-autocomplete]').fill('Alice');
  await expect(page.locator('datalist option')).toContainText('Alice equipment');
  await page.locator('[data-admin-autocomplete]').fill('1');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page).toHaveURL(/\/staff\/products/);
  await page.goto('/staff/products');
  await expect(page.getByText('Updated compass', { exact: true })).toBeVisible();
  const row = page.locator('tr').filter({ hasText: 'Updated compass' });
  await row.getByRole('button', { name: /Turn/ }).click();
  await expect(page.locator('#admin-status')).toContainText('Row updated');
  await page.goto('/account');
  await page.getByRole('button', { name: 'Sign out' }).click();
  await page.goto('/staff/products');
  await expect(page).toHaveURL(/\/login$/);
});

test('member denied and an expired session leaves the admin', async ({ page }) => {
  await login(page, 'member@example.test');
  const denied = await page.goto('/staff');
  expect(denied.status()).toBe(403);
  await page.context().clearCookies();
  await login(page, 'bob@example.test');
  await page.goto('/staff/products');
  await expect(page.getByText('Bob radio', { exact: true })).toBeVisible();
  await page.context().clearCookies();
  await page.getByRole('searchbox').fill('radio');
  await expect(page).toHaveURL(/\/login$/);
});

test.describe('ordinary forms', () => {
  test.use({ javaScriptEnabled: false });
  test('host login and protected writes work without JavaScript', async ({ page }) => {
    await login(page, 'bob@example.test');
    await page.goto('/staff/products/3/change');
    await page.locator('input[name="name"]').fill('Ordinary radio');
    await page.getByRole('button', { name: 'Save', exact: true }).click();
    await page.goto('/staff/products');
    await expect(page.getByText('Ordinary radio', { exact: true })).toBeVisible();
  });
});
