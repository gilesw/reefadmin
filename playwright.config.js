const { defineConfig } = require('@playwright/test');
module.exports = defineConfig({
  testDir: './tests/browser',
  workers: 1,
  use: { baseURL: 'http://127.0.0.1:39127', browserName: 'chromium', trace: 'retain-on-failure' },
  webServer: {
    command: 'cargo test --locked --test browser_server -- --ignored --nocapture',
    url: 'http://127.0.0.1:39127/health',
    timeout: 120000,
    reuseExistingServer: false,
  },
});
