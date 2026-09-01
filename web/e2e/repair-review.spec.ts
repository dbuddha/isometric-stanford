import { expect, test, type TestInfo } from "@playwright/test";
import { installRepairFixture } from "./repair-fixture";

test("repair lab compares deterministic candidates and exposes blockers", async ({ page }, testInfo: TestInfo) => {
  await installRepairFixture(page);
  await page.goto("./review/repair");
  await expect(page.getByRole("status")).toContainText("Pilot not qualified");
  await expect(page.getByText("Deterministic filtering is viable, but this pilot does not qualify expansion.")).toBeVisible();
  await expect(page.getByText("construction-region-lacks-an-accepted-instance-mask")).toBeVisible();
  await expect(page.getByText("97.30%")).toBeVisible();
  await expect(page.getByTestId("wipe-overlay")).toHaveCSS("clip-path", "inset(0px 0px 0px 50%)");
  await page.getByLabel("Repair comparison wipe").fill("25");
  await expect(page.getByTestId("wipe-overlay")).toHaveCSS("clip-path", "inset(0px 0px 0px 25%)");
  const evidenceLabels = page.locator(".review-viewport__label");
  const leftLabel = await evidenceLabels.nth(0).boundingBox();
  const rightLabel = await evidenceLabels.nth(1).boundingBox();
  expect(leftLabel).not.toBeNull();
  expect(rightLabel).not.toBeNull();
  expect(leftLabel!.x + leftLabel!.width).toBeLessThanOrEqual(rightLabel!.x);
  await page.getByRole("button", { name: "Cars" }).click();
  await expect(page.getByTestId("review-viewport")).toHaveAttribute("data-zoom", "5.0000");
  await page.getByLabel("Left repair evidence").selectOption("canopy-mask");
  await expect(page.getByLabel("Left repair evidence")).toHaveValue("canopy-mask");
  await page.screenshot({ path: testInfo.outputPath("repair-lab.png"), fullPage: true });
});

test("repair lab rejects an image whose hash is not registered", async ({ page }) => {
  await installRepairFixture(page, true);
  await page.goto("./review/repair");
  await expect(page.getByRole("alert")).toContainText("SHA-256");
  await expect(page.getByRole("status")).toContainText("Evidence rejected");
});
