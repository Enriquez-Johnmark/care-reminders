// Every server-data key in one place, so a change in one feature can
// refresh what another shows.

export const reminderKeys = {
  all: () => ["reminders"] as const,
  queue: () => [...reminderKeys.all(), "queue"] as const,
  detail: (id: string) => [...reminderKeys.all(), "detail", id] as const,
  timeline: (followUp: string) => [...reminderKeys.all(), "timeline", followUp] as const,
}

export const activityKeys = {
  list: (actor: string, action: string) => ["activity", actor, action] as const,
}
