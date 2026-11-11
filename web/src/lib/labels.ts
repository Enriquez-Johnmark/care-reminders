// The API sends codes; these are the words people see. An unknown code is
// shown as it is rather than hidden.

const statuses: Record<
  string,
  { label: string; variant: "default" | "success" | "destructive" | "secondary" }
> = {
  waiting: { label: "Ready to review", variant: "default" },
  blocked: { label: "Blocked by a check", variant: "destructive" },
  approved: { label: "Approved", variant: "success" },
}

export function statusLabel(status: string) {
  return statuses[status] ?? { label: status, variant: "secondary" as const }
}

const kinds: Record<string, string> = {
  "after-result": "After a result",
  "screening-recall": "Screening recall",
  "after-discharge": "After a discharge",
}

export function kindLabel(kind: string) {
  return kinds[kind] ?? kind
}

const sources: Record<string, string> = {
  model: "Drafted by Claude",
  template: "The clinic's template",
  coordinator: "Written by the coordinator",
}

export function sourceLabel(source: string) {
  return sources[source] ?? source
}

const checks: Record<string, string> = {
  answer: "The model's answer",
  "fill-ins": "Fill-ins",
  "numbers-and-dates": "Numbers and dates",
  "clinical-words": "Clinical words",
  "reading-level": "Reading level",
}

export function checkLabel(check: string) {
  return checks[check] ?? check
}

const actions: Record<string, string> = {
  "fhir.fetched": "Fetched from the FHIR server",
  "fhir.fetch_failed": "Fetch from the FHIR server failed",
  "fhir.server_set": "FHIR server set",
  "rule.added": "Rule added",
  "rule.changed": "Rule changed",
  "follow_up.selected": "Chosen by a rule",
  "follow_up.booked": "Stopped: the patient booked",
  "follow_up.opted_out": "Stopped: the patient opted out",
  "follow_up.replied": "Stopped: the patient replied",
  "reminder.drafted": "Drafted",
  "reminder.checked": "Checked",
  "reminder.edited": "Edited",
  "reminder.templated": "Template used",
  "reminder.redrafted": "New draft asked for",
  "reminder.approved": "Approved",
  "reminder.skipped": "Skipped",
  "reminder.not_emailed": "Not emailed: contact another way",
  "reminder.sent": "Sent",
  "reminder.withdrawn": "Withdrawn before sending",
  "outbox.approved": "Email queued",
  "outbox.send_started": "Email sending",
  "outbox.sent": "Email accepted by the mail service",
  "outbox.cancelled": "Email cancelled",
}

export function actionLabel(action: string) {
  return actions[action] ?? action
}

/** The actions the activity page can filter by. */
export const actionChoices = Object.entries(actions)
