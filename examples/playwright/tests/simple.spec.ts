import { test, expect } from "@playwright/test";

test("has heading", async ({ page }) => {
  await page.goto("/");

  // Expect heading to ensure we are looking at the intended page.
  await expect(
    page.getByRole("heading", { name: "Playwright!" }),
  ).toBeVisible();
});

test("reload", async ({ page, request }) => {
  await page.goto("/");

  // Wait for load event on this page.
  const reload = page.waitForRequest("/");

  // Trigger a reload.
  await request.post("/reload");

  // Ensure reload actually happened.
  await reload;
});

test.fail("no reload", async ({ page }) => {
  await page.goto("/");

  // Ensure no reload happens when not triggered.
  await page.waitForRequest("/", { timeout: 100 });
});
