import { join } from "node:path"

import { expect, test, type Page } from "@playwright/test"

import { logIn } from "../e2e/support.ts"
import {
  addOverlay,
  ADMIN,
  command,
  COORDINATOR,
  MAILPIT,
  newestReminder,
  OUT,
  pointAt,
  press,
  record,
  say,
} from "./support.ts"

const size = { width: 1280, height: 720 }

/** A screenshot for people to look at, into docs/showcase/. */
async function shot(page: Page, name: string) {
  await page.evaluate(() => window.showCaption?.(""))
  await page.waitForTimeout(400)
  await page.screenshot({ path: join(OUT, `${name}.png`) })
}

/** Opens the queue row of `patient` whose kind label matches `kind`. */
async function openReminder(page: Page, patient: string, kind?: RegExp) {
  await page.goto("/")
  let row = page.getByRole("row").filter({ hasText: patient })
  if (kind) row = row.filter({ hasText: kind })
  await press(page, row.first().getByRole("link"))
  await expect(page.getByText("As the patient will read it")).toBeVisible()
}

/** Waits until the reminder page says its email went. */
async function waitUntilSent(page: Page) {
  for (let attempt = 0; attempt < 60; attempt++) {
    if (await page.getByText("Sent", { exact: true }).isVisible()) return
    await page.waitForTimeout(1000)
    await page.reload()
  }
  throw new Error("the reminder was never sent")
}

/** About three minutes from an empty clinic to a patient's reply, with
 *  captions; saved as docs/showcase/walkthrough.mp4, with screenshots on
 *  the way. Runs on a fresh clinic whose FHIR server holds the made-up
 *  patients, on Tuesday 10 November 2026 at 10 am in New York. */
test("walkthrough", async ({ browser }, testInfo) => {
  const context = await browser.newContext({ viewport: size, deviceScaleFactor: 2 })
  const page = await context.newPage()
  await logIn(page, ADMIN)
  await addOverlay(page)
  const stop = await record(page, testInfo.outputPath("frames"))

  // Who is due, by the clinic's rules.
  await page.goto("/")
  await say(page, "Care Reminders, at a made-up family practice with 25 made-up patients.")
  await say(
    page,
    "Their records live on the clinic's FHIR server, the healthcare data standard: results, hospital stays, screenings and appointments."
  )
  await press(page, page.getByRole("button", { name: "Fetch from the FHIR server" }))
  await expect(page.getByRole("status")).toContainText("40 follow-ups chosen")
  await say(
    page,
    "The clinic's own rules choose who is due for follow-up care: plain code, never the model. Here, 40 follow-ups."
  )
  await say(page, "Claude drafts each first reminder in the background.", 3500)
  await expect(page.getByRole("row")).toHaveCount(41, { timeout: 90_000 })
  await shot(page, "desktop-1-queue")

  // The coordinator reviews one.
  await say(page, "The care coordinator logs in with an emailed link.", 3000)
  await context.clearCookies()
  await logIn(page, COORDINATOR)
  await openReminder(page, "Ana Reyes", /result/i)
  await pointAt(page, page.getByText(/The reminder never says why/))
  await say(
    page,
    "Staff see why the patient is due and which rule version chose them. The reminder itself never says why."
  )
  await pointAt(page, page.getByText("As the patient will read it"))
  await say(
    page,
    "Claude writes the words with fill-ins. Code fills in the name, phone and date, and checks for clinical words, dates of its own and reading level."
  )
  await say(
    page,
    "Claude never sees who the patient is: only the kind of reminder, which one, and the tone."
  )
  await shot(page, "desktop-2-review")
  await press(page, page.getByRole("button", { name: "Approve this text" }))
  await expect(page.getByText("This exact text was approved on")).toBeVisible()
  await say(page, "Approving binds this exact text. Any later change needs approving again.", 4000)
  await waitUntilSent(page)
  await say(
    page,
    "It goes once, in the clinic's sending hours: weekdays 8 am to 6 pm, at most one a day and three a week per patient."
  )
  await shot(page, "desktop-3-sent")

  // What the patient gets.
  const email = await newestReminder()
  await page.goto(`${MAILPIT}/view/${email.id}`)
  await page.waitForTimeout(1500)
  await say(
    page,
    "The patient's email, in the local test inbox: the clinic, its phone and a date. Nothing about the test or the reason."
  )
  await shot(page, "desktop-4-email")
  const optOut = email.text.split(/\s+/).find((word) => word.includes("/o/")) ?? ""
  await page.goto(new URL(optOut).pathname)
  await say(
    page,
    "Its link opens a page with one button. Opening it changes nothing, so link scanners can't opt anyone out."
  )
  await shot(page, "desktop-5-opt-out")
  await press(page, page.getByRole("button", { name: "Stop these reminders" }))
  await expect(page.getByRole("heading", { name: "These reminders are stopped" })).toBeVisible()
  await say(page, "Pressing it stops that kind of reminder only, and it's in the log.", 4000)

  // The caps, a patient who asked not to be emailed, and a blocked edit.
  await openReminder(page, "Ana Reyes", /screening/i)
  await press(page, page.getByRole("button", { name: "Approve this text" }))
  await expect(page.getByText("Waiting to go")).toBeVisible({ timeout: 15_000 })
  await pointAt(page, page.getByText("Waiting to go"))
  await say(page, "Her screening reminder waits: she already had one today.")
  await openReminder(page, "Jun Torres")
  await pointAt(page, page.getByText(/asked not to be emailed/))
  await say(
    page,
    "This patient asked not to be emailed, so the coordinator contacts him another way."
  )
  await openReminder(page, "Ben Cruz")
  await press(page, page.getByRole("button", { name: "Edit" }))
  const text = page.getByLabel("Edit the reminder")
  await text.fill(`${await text.inputValue()} It's about your cholesterol.`)
  await press(page, page.getByRole("button", { name: "Save the text" }))
  await expect(page.getByText("This draft failed a check, so it can't be approved")).toBeVisible()
  await say(
    page,
    "The coordinator's own edits go through the same checks. This one names a condition, so it's blocked."
  )
  await shot(page, "desktop-6-blocked-edit")
  await press(page, page.getByRole("button", { name: "Use the template" }))
  await press(page, page.getByRole("button", { name: "Approve this text" }))
  await waitUntilSent(page)

  // A reply stops the follow-up.
  command(
    "simulate-reply",
    "--clinic",
    process.env.CLINIC ?? "",
    "--patient",
    "p02",
    "--text",
    "Thanks, I'll call on Monday to book."
  )
  await page.goto("/replies")
  await expect(page.getByText("Thanks, I'll call on Monday to book.")).toBeVisible()
  await say(
    page,
    "A reply stops that follow-up's reminders and lands here for the coordinator to answer."
  )
  await shot(page, "desktop-7-replies")

  // Every step is logged.
  await context.clearCookies()
  await logIn(page, ADMIN)
  await page.goto("/activity")
  await expect(page.getByRole("row").nth(5)).toBeVisible()
  await say(
    page,
    "Every fetch, choice, draft, check, approval, send, reply and opt-out is in an append-only log."
  )
  await shot(page, "desktop-8-activity")
  await say(
    page,
    "Made-up clinic, made-up patients. The code enforces every rule, and the tests prove them.",
    4500
  )

  await stop(join(OUT, "walkthrough.mp4"))
  await context.close()
})

/** The review page on a phone. */
test("phone", async ({ browser }) => {
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 3,
  })
  const page = await context.newPage()
  await logIn(page, COORDINATOR)
  await page.goto("/")
  await page.screenshot({ path: join(OUT, "phone-1-queue.png") })
  await openReminder(page, "Carla Santos")
  await page.screenshot({ path: join(OUT, "phone-2-review.png"), fullPage: true })
  await context.close()
})
