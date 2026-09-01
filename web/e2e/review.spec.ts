import { expect, test, type TestInfo } from "@playwright/test";

import { installReferenceFixture } from "./reference-fixture";

test("registered workbench verifies, compares, and explains all six layers", async ({ page }, testInfo: TestInfo) => {
  await installReferenceFixture(page);
  await page.goto("./review");
  await expect(page.getByRole("status")).toContainText("Bundle verified");
  const workbench = page.getByTestId("reference-review");
  await expect(workbench).toHaveAttribute("data-bundle-id", "hoover-review-fixture");
  await expect(workbench).toHaveAttribute("data-manifest-sha256", /^[0-9a-f]{64}$/);
  await expect(page.getByLabel("Left layer")).toHaveValue("color");
  await expect(page.getByLabel("Right layer")).toHaveValue("whitebox");
  await expect(page.locator("img[data-layer-kind=color]")).toBeVisible();
  await expect(page.locator("img[data-layer-kind=whitebox]")).toBeVisible();
  await expect(page.getByText("100.00%")).toBeVisible();
  await expect(page.getByText("fixture:synthetic")).toBeVisible();

  await page.getByLabel("Right layer").selectOption("linear-depth");
  await expect(page.locator("img[data-layer-kind=linear-depth]")).toBeVisible();
  await page.getByRole("button", { name: "Wipe" }).click();
  await page.getByLabel("Comparison wipe").fill("63");
  await expect(page.getByTestId("wipe-overlay")).toHaveCSS("clip-path", "inset(0px 0px 0px 63%)");
  await page.screenshot({ path: testInfo.outputPath("reference-review-workbench.png"), fullPage: true });
});

test("all visible panels share source-pixel pan, zoom, keyboard, and native scale", async ({ page }) => {
  await installReferenceFixture(page);
  await page.goto("./review");
  await expect(page.getByRole("status")).toContainText("Bundle verified");
  const viewports = page.getByTestId("review-viewport");
  await expect(viewports).toHaveCount(2);
  await viewports.first().hover();
  await page.mouse.wheel(0, -120);
  await expect(viewports.first()).toHaveAttribute("data-zoom", "1.2500");
  await expect(viewports.nth(1)).toHaveAttribute("data-zoom", "1.2500");
  await viewports.first().focus();
  await page.keyboard.press("ArrowRight");
  const leftPan = await viewports.first().getAttribute("data-pan-x");
  expect(Number(leftPan)).not.toBe(0);
  await expect(viewports.nth(1)).toHaveAttribute("data-pan-x", leftPan ?? "");

  await page.getByRole("button", { name: "1:1 pixels" }).click();
  const nativeZoom = Number(await viewports.first().getAttribute("data-zoom"));
  const fitScale = Number(await viewports.first().getAttribute("data-fit-scale"));
  expect(Math.abs(nativeZoom * fitScale - 1)).toBeLessThan(0.01);
  await page.getByRole("button", { name: "Fit" }).click();
  await expect(viewports.first()).toHaveAttribute("data-zoom", "1.0000");
  await expect(viewports.first()).toHaveAttribute("data-pan-x", "0.000");
});

test("corrupt registration fails closed before any layer is displayed", async ({ page }) => {
  await installReferenceFixture(page, { corruptManifest: true });
  await page.goto("./review");
  await expect(page.getByRole("alert")).toContainText("cannot be displayed");
  await expect(page.getByRole("alert")).toContainText("shared pixel grid");
  await expect(page.getByTestId("review-viewport")).toHaveCount(0);
});

test("a missing registered artifact fails the complete review bundle", async ({ page }) => {
  await installReferenceFixture(page, { missingLayer: "whitebox.png" });
  await page.goto("./review");
  await expect(page.getByRole("alert")).toContainText("status 404");
  await expect(page.getByRole("status")).toContainText("Bundle rejected");
  await expect(page.getByTestId("review-viewport")).toHaveCount(0);
});
