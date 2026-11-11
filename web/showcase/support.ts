/// <reference lib="dom" />
// Runs partly in the browser (the overlay), so it needs the DOM types.

import { execFileSync } from "node:child_process"
import { mkdirSync, writeFileSync } from "node:fs"
import { join, resolve } from "node:path"

import type { Locator, Page } from "@playwright/test"

export const ADMIN = "admin@family.example.test"
export const COORDINATOR = "carla@family.example.test"
/** Where the screenshots and the video go. */
export const OUT = resolve(import.meta.dirname, "../../docs/showcase")
export const MAILPIT = process.env.MAILPIT_URL ?? "http://127.0.0.1:8025"

/** The newest reminder email to a patient, from Mailpit. */
export async function newestReminder(): Promise<{ id: string; to: string; text: string }> {
  for (let attempt = 0; attempt < 240; attempt++) {
    const list = (await (await fetch(`${MAILPIT}/api/v1/messages`)).json()) as {
      messages: { ID: string; Subject: string; To: { Address: string }[] }[]
    }
    const message = list.messages.find((m) => m.Subject.startsWith("A message from"))
    if (message) {
      const full = (await (await fetch(`${MAILPIT}/api/v1/message/${message.ID}`)).json()) as {
        Text: string
      }
      return { id: message.ID, to: message.To[0]?.Address ?? "", text: full.Text }
    }
    await new Promise((done) => setTimeout(done, 250))
  }
  throw new Error("no reminder email arrived")
}

/** Runs the care-reminders command, as the operator would. */
export function command(...args: string[]): string {
  return execFileSync(resolve(import.meta.dirname, "../../target/debug/care-reminders"), args, {
    encoding: "utf8",
  })
}

declare global {
  interface Window {
    showCaption?: (text: string) => void
  }
}

/** Adds a caption bar and a visible mouse pointer to every page, for video. */
export async function addOverlay(page: Page) {
  await page.addInitScript(() => {
    const install = () => {
      if (document.getElementById("showcase-caption")) return
      const caption = document.createElement("div")
      caption.id = "showcase-caption"
      Object.assign(caption.style, {
        position: "fixed",
        left: "50%",
        bottom: "28px",
        transform: "translateX(-50%)",
        maxWidth: "80%",
        padding: "12px 22px",
        borderRadius: "12px",
        background: "rgba(15, 23, 42, 0.9)",
        color: "#fff",
        font: "500 22px/1.4 system-ui, sans-serif",
        textAlign: "center",
        zIndex: "2147483647",
        pointerEvents: "none",
        opacity: "0",
        transition: "opacity 0.3s",
      })
      const pointer = document.createElement("div")
      Object.assign(pointer.style, {
        position: "fixed",
        left: "-50px",
        top: "-50px",
        width: "22px",
        height: "22px",
        borderRadius: "50%",
        border: "2px solid rgba(15, 118, 110, 0.9)",
        background: "rgba(45, 212, 191, 0.35)",
        transform: "translate(-50%, -50%)",
        zIndex: "2147483647",
        pointerEvents: "none",
      })
      document.addEventListener(
        "mousemove",
        (event) => {
          pointer.style.left = `${event.clientX}px`
          pointer.style.top = `${event.clientY}px`
        },
        true
      )
      document.body.append(caption, pointer)
    }
    window.showCaption = (text) => {
      install()
      const caption = document.getElementById("showcase-caption")
      if (!caption) return
      caption.textContent = text
      caption.style.opacity = text ? "1" : "0"
    }
    if (document.body) install()
    else document.addEventListener("DOMContentLoaded", install)
  })
}

/** Shows `text` as the caption and gives viewers time to read it. */
export async function say(page: Page, text: string, readFor = 5500) {
  await page.evaluate((caption) => window.showCaption?.(caption), text)
  await page.waitForTimeout(readFor)
}

/** Glides the pointer onto `target`, so viewers see what is clicked. */
export async function pointAt(page: Page, target: Locator) {
  await target.scrollIntoViewIfNeeded()
  const box = await target.boundingBox()
  if (!box) return
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 25 })
  await page.waitForTimeout(400)
}

export async function press(page: Page, target: Locator) {
  await pointAt(page, target)
  await target.click()
}

/**
 * Records `page` with Chromium's screencast into `dir`; the returned
 * function stops and writes an MP4 with ffmpeg. (Playwright's own video
 * needs an ffmpeg build this machine may not have.) Chromium sends a frame
 * only when the screen changes, so each frame lasts until the next.
 */
export async function record(page: Page, dir: string) {
  mkdirSync(dir, { recursive: true })
  const session = await page.context().newCDPSession(page)
  const frames: { file: string; at: number }[] = []
  session.on("Page.screencastFrame", ({ data, metadata, sessionId }) => {
    const file = join(dir, `frame-${String(frames.length).padStart(6, "0")}.jpg`)
    writeFileSync(file, Buffer.from(data, "base64"))
    frames.push({ file, at: metadata.timestamp ?? Date.now() / 1000 })
    session.send("Page.screencastFrameAck", { sessionId }).catch(() => {})
  })
  await session.send("Page.startScreencast", { format: "jpeg", quality: 90 })

  return async function stop(output: string) {
    await session.send("Page.stopScreencast")
    const list = frames.flatMap((frame, index) => {
      const next = frames[index + 1]?.at ?? frame.at + 2
      return [`file '${frame.file}'`, `duration ${(next - frame.at).toFixed(3)}`]
    })
    // The concat format needs the last file repeated to honour its duration.
    list.push(`file '${frames[frames.length - 1].file}'`)
    const listFile = join(dir, "frames.txt")
    writeFileSync(listFile, list.join("\n"))
    execFileSync("ffmpeg", [
      "-y",
      "-loglevel",
      "error",
      "-f",
      "concat",
      "-safe",
      "0",
      "-i",
      listFile,
      "-vf",
      "fps=30,scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2,format=yuv420p",
      "-c:v",
      "libx264",
      "-crf",
      "26",
      "-movflags",
      "+faststart",
      output,
    ])
  }
}
