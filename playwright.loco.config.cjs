const { defineConfig } = require('@playwright/test');
const proxy = process.env.REEFADMIN_ZAP_PROXY;
module.exports = defineConfig({
  testDir: './examples/loco/tests/browser',
  workers: 1,
  use: {
    baseURL: 'http://127.0.0.1:39128',
    browserName: 'chromium',
    trace: 'retain-on-failure',
    ...(proxy ? { proxy: { server: proxy }, launchOptions: { args: ['--proxy-bypass-list=<-loopback>'] } } : {}),
  },
  webServer: {
    command: 'cargo test --locked --manifest-path examples/loco/Cargo.toml --test browser_server -- --ignored --nocapture',
    url: 'http://127.0.0.1:39128/health',
    timeout: 120000,
    reuseExistingServer: false,
  },
});
