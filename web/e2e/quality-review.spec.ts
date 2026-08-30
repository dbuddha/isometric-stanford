import { expect, test, type TestInfo } from "@playwright/test";
import { installQualityFixture } from "./quality-fixture";

test("quality lab compares source LOD and raster sampling on one footprint", async ({ page }, testInfo: TestInfo) => {
  await installQualityFixture(page);
  await page.goto("./review/quality");
  await expect(page.getByRole("status")).toContainText("LOD ceiling measured");
  await expect(page.getByText("SSE 8 reaches Google’s available Stanford LOD ceiling.")).toBeVisible();
  await expect(page.getByText("784 / 1000")).toBeVisible();
  await expect(page.getByText("1,370,554").first()).toBeVisible();
  await expect(page.getByTestId("review-viewport")).toHaveCount(1);

  await page.getByRole("button", { name: "Trees" }).click();
  await expect(page.getByTestId("review-viewport")).toHaveAttribute("data-zoom", "4.0000");

  await page.getByLabel("Left quality evidence").selectOption("sample-sse8-125mm");
  await expect(page.getByLabel("Left quality evidence")).toHaveValue("sample-sse8-125mm");
  await page.getByRole("button", { name: "1:1 pixels" }).click();
  await expect(page.getByRole("status")).toContainText("LOD ceiling measured");
  await page.screenshot({ path: testInfo.outputPath("quality-lab.png"), fullPage: true });
});

test("quality lab rejects a candidate whose image hash is not registered", async ({ page }) => {
  await installQualityFixture(page, true);
  await page.goto("./review/quality");
  await expect(page.getByRole("alert")).toContainText("SHA-256");
  await expect(page.getByRole("status")).toContainText("Evidence rejected");
});
