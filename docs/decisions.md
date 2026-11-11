# Decisions

Approved by the owner, newest last.

## 2026-11-10 (recorded in Lab Report Assistant's `docs/decisions.md`)

- **A healthcare series:** four portfolio projects, each in its own repository, built one at a time, each finished before the next: lab reports, care follow-up reminders (this one), a care-team message assistant and a record-access audit.
- **Language and engine:** Rust on Nudgewright, with React and TypeScript pages.

## 2026-11-11

- **Start:** the owner created this repository; research begins now that Lab Report Assistant is finished.
- **Research:** done (`docs/research/README.md`).
- **Brief, Round 1** (all ten answered "yes"; `docs/product/brief.md`):
  1. For a small US primary-care clinic (made up); a care coordinator approves each reminder.
  2. Three kinds of follow-up, each from a rule the clinic writes: a visit after an abnormal result, a screening recall, and a visit within 7 days of a hospital discharge.
  3. Who is due comes only from the clinic's rules, shown with their reason; never from the model.
  4. Claude only words the reminder; code fills in name, date, time and clinic and checks nothing else crept in; by default no reason for the visit, test or specialty; the message says it was drafted with AI and approved by a named person.
  5. The demo compares Claude's wording with a fixed template and measures approval time; it never claims better attendance.
  6. Reminders stop on reply, booking or opt-out (opt-out per kind of reminder); at most 1 a day and 3 a week per patient, enforced in code.
  7. Email only in the first version, through Nudgewright as it is; SMS later as an engine addition.
  8. Made-up FHIR R4 data fetched from a local HAPI FHIR server; the "due" items written by hand.
  9. Left out of the first version: SMS, voice, booking inside the tool, payer care-gap reports, writing back to the record system, and any patient chatbot.
  10. Real companies may be named only in research documents, as cited sources.
