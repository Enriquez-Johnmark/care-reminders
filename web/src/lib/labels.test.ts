import { describe, expect, it } from "vitest"

import { actionLabel, checkLabel, kindLabel, statusLabel } from "@/lib/labels"

describe("labels", () => {
  it("names each code in words", () => {
    expect(statusLabel("blocked").label).toBe("Blocked by a check")
    expect(kindLabel("screening-recall")).toBe("Screening recall")
    expect(checkLabel("clinical-words")).toBe("Clinical words")
    expect(actionLabel("reminder.approved")).toBe("Approved")
  })

  it("shows an unknown code as it is", () => {
    expect(actionLabel("reminder.sent_twice")).toBe("reminder.sent_twice")
    expect(statusLabel("odd").label).toBe("odd")
  })
})
