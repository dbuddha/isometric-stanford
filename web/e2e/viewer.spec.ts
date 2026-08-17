import { expect, test, type Page, type TestInfo } from "@playwright/test";

const DZI = `<?xml version="1.0" encoding="UTF-8"?>
<Image TileSize="512" Overlap="0" Format="webp" xmlns="http://schemas.microsoft.com/deepzoom/2008">
  <Size Width="1" Height="1"/>
</Image>
`;
const WEBP = Buffer.from("UklGRhwAAABXRUJQVlA4TBAAAAAvAAAAAM1VICICDa9DuyMB", "base64");
const FIXTURE_DZI = "**/fixture/hero.dzi";
const FIXTURE_TILE = "**/fixture/hero_files/0/0_0.webp";

async function installFixture(page: Page) {
  if (process.env.E2E_DZI_URL) {
    return;
  }
  await page.route(FIXTURE_DZI, (route) =>
    route.fulfill({ status: 200, contentType: "application/xml", body: DZI }),
  );
  await page.route(FIXTURE_TILE, (route) =>
    route.fulfill({ status: 200, contentType: "image/webp", body: WEBP }),
  );
}

test.beforeEach(async ({ page }) => installFixture(page));

test("released viewer is accessible and paints artwork", async ({ page }, testInfo: TestInfo) => {
  await page.goto("./");
  await expect(page.getByRole("heading", { name: "Isometric Stanford" })).toBeVisible();
  await expect(page.getByRole("status")).toContainText("Artwork ready");
  await expect(page.getByRole("button", { name: "Zoom in" })).toBeVisible();
  await expect(page.getByText("No captured people or vehicles.")).toBeVisible();
  const canvas = page.locator(".viewer canvas");
  await expect(canvas).toBeVisible();
  expect((await canvas.screenshot()).byteLength).toBeGreaterThan(100);
  await page.screenshot({ path: testInfo.outputPath("viewer.png"), fullPage: true });
});

test("failed tile retries are visible and recoverable", async ({ page }) => {
  const tilePattern = process.env.E2E_DZI_URL ? "**/hero_files/**/*.webp" : FIXTURE_TILE;
  await page.unroute(tilePattern);
  let failing = true;
  await page.route(tilePattern, (route) =>
    failing
      ? route.fulfill({ status: 503, body: "temporary failure" })
      : route.continue(),
  );
  if (!process.env.E2E_DZI_URL) {
    await page.unroute(FIXTURE_TILE);
    await page.route(FIXTURE_TILE, (route) =>
      failing
        ? route.fulfill({ status: 503, body: "temporary failure" })
        : route.fulfill({ status: 200, contentType: "image/webp", body: WEBP }),
    );
  }
  await page.goto("./");
  await expect(page.getByRole("status")).toContainText("Some artwork tiles failed");
  failing = false;
  await page.getByRole("button", { name: "Retry artwork" }).click();
  await expect(page.getByRole("status")).toContainText("Artwork ready");
});

test("descriptor failure can be retried without reloading the page", async ({ page }) => {
  const descriptorPattern = process.env.E2E_DZI_URL ? "**/art/hero.dzi" : FIXTURE_DZI;
  await page.unroute(descriptorPattern);
  let failing = true;
  await page.route(descriptorPattern, (route) =>
    failing
      ? route.fulfill({ status: 503, body: "temporary failure" })
      : process.env.E2E_DZI_URL
        ? route.continue()
        : route.fulfill({ status: 200, contentType: "application/xml", body: DZI }),
  );
  await page.goto("./");
  await expect(page.getByRole("status")).toContainText("Artwork unavailable");
  failing = false;
  await page.getByRole("button", { name: "Retry artwork" }).click();
  await expect(page.getByRole("status")).toContainText("Artwork ready");
});

test("display context interruption redraws after restoration", async ({ page }) => {
  await page.goto("./");
  await expect(page.getByRole("status")).toContainText("Artwork ready");
  const canvas = page.locator(".viewer canvas");
  await canvas.evaluate((element) =>
    element.dispatchEvent(new Event("contextlost", { cancelable: true })),
  );
  await expect(page.getByRole("status")).toContainText("Artwork display interrupted");
  await canvas.evaluate((element) => element.dispatchEvent(new Event("contextrestored")));
  await expect(page.getByRole("status")).toContainText("Artwork ready");
});
