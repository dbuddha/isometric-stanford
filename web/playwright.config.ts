import { defineConfig, devices } from "@playwright/test";

const externalBaseUrl = process.env.E2E_BASE_URL;

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: externalBaseUrl ?? "http://127.0.0.1:4173/isometric-stanford/",
    trace: "retain-on-failure",
  },
  webServer: externalBaseUrl
    ? undefined
    : {
        command: "npm run build && npm run preview -- --host 127.0.0.1",
        url: "http://127.0.0.1:4173/isometric-stanford/",
        reuseExistingServer: !process.env.CI,
        env: {
          VITE_DZI_URL:
            process.env.E2E_DZI_URL ?? "/isometric-stanford/fixture/hero.dzi",
          VITE_RELEASE_URL:
            process.env.E2E_RELEASE_URL ?? "/isometric-stanford/fixture/release.json",
        },
      },
  projects: [
    { name: "desktop-chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "mobile-chromium", use: { ...devices["Pixel 7"] } },
  ],
});
