import { expect, test } from "@playwright/test";

test("unqualified viewer is usable without a release", async ({ page }) => {
  await page.goto("./");
  await expect(page.getByRole("heading", { name: "Isometric Stanford" })).toBeVisible();
  await expect(page.getByRole("status")).toContainText("Qualification in progress");
  await expect(page.getByRole("button", { name: "Zoom in" })).toBeVisible();
  await expect(page.getByText("No captured people or vehicles.")).toBeVisible();
});
