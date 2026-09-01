import { expect, test, type TestInfo } from "@playwright/test";

import { installOverlapFixture } from "./overlap-fixture";

test("overlap workbench verifies and compares independent joins at source pixels", async ({ page }, testInfo: TestInfo) => {
  await installOverlapFixture(page);
  await page.goto("./review/overlap");
  await expect(page.getByRole("status")).toContainText("Overlap qualified");
  await expect(page.getByTestId("review-viewport")).toHaveCount(1);
  await expect(page.getByText("Registered join passed")).toBeVisible();
  await expect(page.getByText("282 / 450")).toBeVisible();
  await expect(page.getByText("0, 0 px · 0 ppm")).toBeVisible();
  await expect(page.getByLabel("Left evidence")).toHaveValue("joined-core");
  await expect(page.getByLabel("Right evidence")).toHaveValue("monolithic-core");

  await page.getByRole("button", { name: "1:1 pixels" }).click();
  const viewport = page.getByTestId("review-viewport");
  const nativeZoom = Number(await viewport.getAttribute("data-zoom"));
  const fitScale = Number(await viewport.getAttribute("data-fit-scale"));
  expect(Math.abs(nativeZoom * fitScale - 1)).toBeLessThan(0.01);

  await page.getByRole("button", { name: "Guard overlap" }).click();
  await expect(page.getByLabel("Left evidence")).toHaveValue("overlap-left");
  await expect(page.getByLabel("Right evidence")).toHaveValue("overlap-right");
  await page.getByLabel("Right evidence").selectOption("overlap-heatmap");
  await page.getByLabel("Comparison wipe").fill("68");
  await expect(page.getByTestId("wipe-overlay")).toHaveCSS("clip-path", "inset(0px 0px 0px 68%)");
  await expect(page.getByRole("table")).toContainText("linear-depth");
  await page.screenshot({ path: testInfo.outputPath("overlap-workbench.png"), fullPage: true });
});

test("a hash mismatch rejects the complete overlap experiment", async ({ page }) => {
  await installOverlapFixture(page, { corruptHash: true });
  await page.goto("./review/overlap");
  await expect(page.getByRole("alert")).toContainText("cannot be displayed");
  await expect(page.getByRole("alert")).toContainText("SHA-256");
  await expect(page.getByRole("status")).toContainText("Evidence rejected");
  await expect(page.getByTestId("review-viewport")).toHaveCount(0);
});

test("a clean independent source seam is not hidden by unqualified lighting", async ({ page }) => {
  await installOverlapFixture(page, { partialSourcePass: true });
  await page.goto("./review/overlap");
  await expect(page.getByRole("status")).toContainText("Source seam reproduced");
  await expect(page.getByRole("heading", { level: 2 })).toContainText(
    "Source join passed; lighting unqualified",
  );
  await expect(page.getByText("256 GLB · 26 JSON")).toBeVisible();
  await expect(page.getByText("monolithic-oracle-level-of-detail, shadow-phase")).toBeVisible();
});
