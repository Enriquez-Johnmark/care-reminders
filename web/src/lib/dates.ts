// Dates come from the API; the app only formats them, in the viewer's own
// time zone (docs/frontend.md: dates are shown, not computed).

const dateTimeFormat = new Intl.DateTimeFormat("en-US", {
  weekday: "short",
  day: "numeric",
  month: "short",
  year: "numeric",
  hour: "numeric",
  minute: "2-digit",
})

/** An RFC 3339 moment → "Tue, Nov 10, 2026, 9:00 AM". */
export function formatDateTime(moment: string) {
  return dateTimeFormat.format(new Date(moment))
}

const dateFormat = new Intl.DateTimeFormat("en-US", {
  day: "numeric",
  month: "short",
  year: "numeric",
  timeZone: "UTC",
})

/** A calendar date, "2026-12-09" → "Dec 9, 2026", the same everywhere. */
export function formatDate(day: string) {
  return dateFormat.format(new Date(`${day}T00:00:00Z`))
}
