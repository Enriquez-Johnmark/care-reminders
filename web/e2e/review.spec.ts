import { expect, test } from "@playwright/test"

import { logIn } from "./support.ts"

const admin = process.env.E2E_ADMIN ?? "admin@family.example.test"
const coordinator = process.env.E2E_COORDINATOR ?? "carla@family.example.test"

test.describe.configure({ mode: "serial" })

test("the administrator fetches from the FHIR server and the rules choose who is due", async ({
  page,
}) => {
  await logIn(page, admin)
  await page.getByRole("button", { name: "Fetch from the FHIR server" }).click()
  await expect(page.getByRole("status")).toContainText("40 follow-ups chosen")
  // The worker drafts each first reminder in the background.
  await expect(page.getByRole("row")).toHaveCount(41, { timeout: 60_000 })
})

test("the coordinator approves a reminder, sees an edit blocked, uses the template and skips one", async ({
  page,
}) => {
  await logIn(page, coordinator)
  const ready = page.getByRole("row").filter({ hasText: "Ready to review" })

  await ready.first().getByRole("link").click()
  await expect(page.getByText("As the patient will read it")).toBeVisible()
  await expect(page.getByText("(555) 010-0100").first()).toBeVisible()
  await expect(page.getByText(/The reminder never says why/)).toBeVisible()
  await page.getByRole("button", { name: "Approve this text" }).click()
  await expect(page.getByText("This exact text was approved on")).toBeVisible()
  // Its email waits for the sending hours or has gone, as the clock says;
  // a patient who asked not to be emailed is never emailed.
  await expect(page.getByText(/^(Waiting to go|Sent|Not emailed)$/)).toBeVisible({
    timeout: 15_000,
  })

  await page.getByRole("link", { name: "Reminders" }).first().click()
  await ready.first().getByRole("link").click()
  await page.getByRole("button", { name: "Edit" }).click()
  const text = page.getByLabel("Edit the reminder")
  await text.fill(`${await text.inputValue()} It's about your cholesterol.`)
  await page.getByRole("button", { name: "Save the text" }).click()
  await expect(page.getByText("This draft failed a check, so it can't be approved")).toBeVisible()
  await expect(page.getByText(/Clinical words:/)).toBeVisible()
  await expect(page.getByRole("button", { name: "Approve this text" })).toHaveCount(0)
  await page.getByRole("button", { name: "Use the template" }).click()
  await expect(page.getByRole("button", { name: "Approve this text" })).toBeVisible()

  await page.getByRole("button", { name: "Skip this reminder" }).click()
  await page.getByLabel(/Why\?/).fill("Called the patient instead")
  await page.getByRole("button", { name: "Skip it" }).click()
  await expect(page.getByText("This reminder is finished.")).toBeVisible()

  await page.getByRole("link", { name: "Replies" }).click()
  await expect(page.getByText("No replies yet.")).toBeVisible()
})

test("the administrator sees every step in the activity log", async ({ page }) => {
  await logIn(page, admin)
  await page.getByRole("link", { name: "Activity" }).click()
  await page.getByLabel("Step").selectOption({ label: "Skipped" })
  await expect(page.getByRole("row")).toHaveCount(2)
  await expect(page.getByText("Called the patient instead")).toBeVisible()
})
