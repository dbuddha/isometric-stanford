import { expect, test, type Page, type TestInfo } from "@playwright/test";
import { writeFile } from "node:fs/promises";

const DZI = `<?xml version="1.0" encoding="UTF-8"?>
<Image TileSize="512" Overlap="0" Format="webp" xmlns="http://schemas.microsoft.com/deepzoom/2008">
  <Size Width="1" Height="1"/>
</Image>
`;
const WEBP = Buffer.from("UklGRhwAAABXRUJQVlA4TBAAAAAvAAAAAM1VICICDa9DuyMB", "base64");
const FIXTURE_DZI = "**/fixture/hero.dzi";
const FIXTURE_TILE = "**/fixture/hero_files/0/0_0.webp";
const FIXTURE_RELEASE = "**/fixture/release.json";
const RELEASE = JSON.stringify({
  schema: "isometric-release/v1",
  status: "artifact-candidate",
  qualified: false,
  style_id: "stanford_v1.candidate_c.1",
  style_sha256: "761cbedd340b6cd9dc4b5be899c9cadf9eb7056def1844ac96e6ef7fd964ddc2",
  world_sha256: "0f20877ff045b4180612c2b4f656aefe72ebe92390e1252ac604d0eaa06ccbcd",
  dzi: {
    descriptor: "hero.dzi",
    width: 7_623,
    height: 3_325,
    tile_size: 512,
    overlap: 0,
    format: "webp",
    tile_count: 1,
    encoded_bytes: WEBP.byteLength,
    tile_set_sha256: "c".repeat(64),
  },
});

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
  await page.route(FIXTURE_RELEASE, (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: RELEASE }),
  );
}

test.beforeEach(async ({ page }) => installFixture(page));

test("released viewer is accessible and paints artwork", async ({ page }, testInfo: TestInfo) => {
  await page.goto("./");
  await expect(page.getByRole("heading", { name: "Isometric Stanford" })).toBeVisible();
  await expect(page.getByRole("status")).toContainText("Artwork ready");
  await expect(page.getByRole("note")).toContainText("Unqualified engineering preview");
  await expect(page.getByRole("note")).toContainText("not received final visual");
  await expect(page.getByRole("button", { name: "Zoom in" })).toBeVisible();
  await expect(page.getByText(/Passenger cars may remain/)).toBeVisible();
  const canvas = page.locator(".viewer canvas");
  await expect(canvas).toBeVisible();
  expect((await canvas.screenshot()).byteLength).toBeGreaterThan(100);
  await page.screenshot({ path: testInfo.outputPath("viewer.png"), fullPage: true });
});

test("release evidence does not shift the viewer when metadata arrives", async ({ page }) => {
  test.skip(Boolean(process.env.E2E_RELEASE_URL), "fixture controls metadata timing");
  await page.unroute(FIXTURE_RELEASE);
  await page.route(FIXTURE_RELEASE, async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 500));
    await route.fulfill({ status: 200, contentType: "application/json", body: RELEASE });
  });
  await page.goto("./");
  const frame = page.locator(".viewer-frame");
  const before = await frame.boundingBox();
  await expect(page.getByRole("note")).toContainText("Unqualified engineering preview");
  const after = await frame.boundingBox();
  expect(Math.abs((after?.y ?? 0) - (before?.y ?? 0))).toBeLessThan(1);
});

test("landmark controls restore stable review URLs and browser history", async ({ page }) => {
  await page.goto("./#view=hoover-tower");
  const viewer = page.getByTestId("viewer");
  await expect(page.getByRole("status")).toContainText("Artwork ready");
  await expect(viewer).toHaveAttribute("data-review-view", "hoover-tower");
  await expect(page.getByRole("button", { name: "Hoover Tower" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  await page.getByRole("button", { name: "Memorial Church" }).click();
  await expect(page).toHaveURL(/#view=memorial-church$/);
  await expect(viewer).toHaveAttribute("data-review-view", "memorial-church");
  await page.getByRole("button", { name: "Main Quad" }).click();
  await expect(page).toHaveURL(/#view=main-quad$/);
  await expect(viewer).toHaveAttribute("data-review-view", "main-quad");

  await page.goBack();
  await expect(viewer).toHaveAttribute("data-review-view", "memorial-church");
  await page.getByRole("button", { name: "Reset map view" }).click();
  await expect(page).toHaveURL(/#view=campus$/);
  await expect(viewer).toHaveAttribute("data-review-view", "campus");
});

test("invalid landmark fragments fail closed to the whole campus", async ({ page }) => {
  await page.goto("./#view=not-a-landmark");
  await expect(page.getByRole("status")).toContainText("Artwork ready");
  await expect(page).toHaveURL(/#view=campus$/);
  await expect(page.getByTestId("viewer")).toHaveAttribute("data-review-view", "campus");
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

test("release evidence failure is visible and recovers after reload", async ({ page }) => {
  const releasePattern = process.env.E2E_RELEASE_URL ? "**/art/release.json" : FIXTURE_RELEASE;
  await page.unroute(releasePattern);
  let failing = true;
  await page.route(releasePattern, (route) =>
    failing
      ? route.fulfill({ status: 503, body: "temporary failure" })
      : process.env.E2E_RELEASE_URL
        ? route.continue()
        : route.fulfill({ status: 200, contentType: "application/json", body: RELEASE }),
  );
  await page.goto("./");
  await expect(page.getByRole("status")).toContainText("Artwork or evidence failed");
  failing = false;
  await page.getByRole("button", { name: "Retry artwork" }).click();
  await expect(page.getByRole("status")).toContainText("Artwork ready");
  await expect(page.getByRole("note")).toContainText("Unqualified engineering preview");
});

test("display context interruption redraws after restoration", async ({ page }) => {
  await page.goto("./");
  await expect(page.getByRole("status")).toContainText("Artwork ready");
  const releaseEvidence = page.getByTestId("release-evidence");
  await expect(releaseEvidence).toHaveAttribute("data-style-id", "stanford_v1.candidate_c.1");
  expect(await releaseEvidence.getAttribute("data-world-sha256")).toMatch(/^[0-9a-f]{64}$/);
  const canvas = page.locator(".viewer canvas");
  await canvas.evaluate((element) =>
    element.dispatchEvent(new Event("contextlost", { cancelable: true })),
  );
  await expect(page.getByRole("status")).toContainText("Artwork display interrupted");
  await canvas.evaluate((element) => element.dispatchEvent(new Event("contextrestored")));
  await expect(page.getByRole("status")).toContainText("Artwork ready");
});

test("real Candidate C pyramid records bounded browser regression evidence", async ({ page }, testInfo) => {
  test.skip(!process.env.E2E_DZI_URL, "requires the complete generated pyramid");
  let initialWebpBytes = 0;
  let initialWebpRequests = 0;
  page.on("response", async (response) => {
    if (!response.url().endsWith(".webp")) {
      return;
    }
    const length = Number(response.headers()["content-length"] ?? 0);
    initialWebpBytes += length || (await response.body()).byteLength;
    initialWebpRequests += 1;
  });

  await page.goto("./");
  await expect(page.getByRole("status")).toContainText("Artwork ready");
  const viewer = page.getByTestId("viewer");
  const cacheTileLimit = Number(await viewer.getAttribute("data-cache-tile-limit"));
  const decodedBudgetBytes = Number(await viewer.getAttribute("data-decoded-budget-bytes"));
  const viewerBox = await viewer.boundingBox();
  const frameProbe = await viewer.evaluate((element) =>
    new Promise<{ frames: number; durationMs: number; longestGapMs: number }>((resolve) => {
      let frames = 0;
      let started = 0;
      let previous = 0;
      let longestGapMs = 0;
      const sample = (timestamp: number) => {
        started ||= timestamp;
        if (previous > 0) {
          longestGapMs = Math.max(longestGapMs, timestamp - previous);
        }
        previous = timestamp;
        frames += 1;
        element.dispatchEvent(
          new WheelEvent("wheel", { deltaY: frames % 2 === 0 ? -2 : 2, bubbles: true }),
        );
        if (timestamp - started < 1_000) {
          requestAnimationFrame(sample);
        } else {
          resolve({ frames, durationMs: timestamp - started, longestGapMs });
        }
      };
      requestAnimationFrame(sample);
    }),
  );
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Performance.enable");
  const performanceMetrics = await cdp.send("Performance.getMetrics");
  const javascriptHeapBytes =
    performanceMetrics.metrics.find((metric) => metric.name === "JSHeapUsedSize")?.value ?? 0;
  const documentHeight = await page.evaluate(() => document.documentElement.scrollHeight);
  const framesPerSecond = (frameProbe.frames - 1) / (frameProbe.durationMs / 1_000);
  const metrics = {
    project: testInfo.project.name,
    viewport: page.viewportSize(),
    initialWebpBytes,
    initialWebpRequests,
    cacheTileLimit,
    decodedBudgetBytes,
    viewerHeight: viewerBox?.height ?? 0,
    javascriptHeapBytes,
    documentHeight,
    framesPerSecond,
    longestFrameGapMs: frameProbe.longestGapMs,
  };
  const metricsPath = testInfo.outputPath("browser-metrics.json");
  await writeFile(metricsPath, JSON.stringify(metrics, null, 2));
  await testInfo.attach("browser-metrics", {
    path: metricsPath,
    contentType: "application/json",
  });

  expect(initialWebpBytes).toBeLessThanOrEqual(2.5 * 1_024 * 1_024);
  expect(cacheTileLimit * 512 * 512 * 4).toBeLessThanOrEqual(decodedBudgetBytes / 2);
  if (testInfo.project.name === "mobile-chromium") {
    expect(viewerBox?.height ?? Number.POSITIVE_INFINITY).toBeLessThanOrEqual(
      (page.viewportSize()?.height ?? 0) * 0.65,
    );
    expect(documentHeight).toBeLessThanOrEqual(page.viewportSize()?.height ?? 0);
  }
  expect(framesPerSecond).toBeGreaterThanOrEqual(30);
  expect(frameProbe.longestGapMs).toBeLessThan(250);

  for (const review of ["Hoover Tower", "Memorial Church", "Main Quad"]) {
    await page.getByRole("button", { name: review, exact: true }).click();
    await expect(page.getByRole("status")).toContainText("Artwork ready");
    await page.screenshot({
      path: testInfo.outputPath(`${review.toLowerCase().replaceAll(" ", "-")}.png`),
      fullPage: true,
    });
  }
});
