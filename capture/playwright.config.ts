import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "tests/e2e",
  timeout: 30_000,
  use: {
    browserName: "chromium",
    headless: true,
  },
  webServer: {
    command: "npm run dev -- --host 127.0.0.1 --port 4317",
    port: 4317,
    reuseExistingServer: false,
    timeout: 30_000,
  },
  reporter: [["list"], ["html", { outputFolder: "test-results/report", open: "never" }]],
  outputDir: "test-results/artifacts",
});
