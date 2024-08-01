import { test, expect } from "@playwright/test";

test("has heading", async ({ page }) => {
  await page.goto("/");

  // Expect heading to ensure we are looking at the intended page.
  await expect(
    page.getByRole("heading", { name: "Playwright!" }),
  ).toBeVisible();
});
