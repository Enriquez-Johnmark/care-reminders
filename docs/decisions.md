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
- **Spec, Round 1** (all ten answered "yes"; `docs/product/spec.md`): Example Family Practice with an administrator and a care coordinator; about 25 made-up patients and 40 follow-ups with planted tricky cases; fixed rule types with settings and versions; up to three reminders per follow-up, each approved separately; the model sees only the kind, which reminder and the tone, and writes with fixed fill-ins; drafts above 8th-grade reading level blocked, above 6th warned; stops on booking (next fetch), reply (engine) or opt-out (button on a signed-link page); sending only 8 am–6 pm on weekdays, clinic time; patients who asked not to be emailed never are; Claude Sonnet by default with recorded answers, Haiku measured beside it.
- **Spec, Round 2** (all ten answered "yes"): emailed login links, only the coordinator approves; the queue and review page as in the spec; a coordinator's edit passes the same checks; fetching by button and hourly; drafting by a background job on days 0, 7 and 21; an append-only activity log with timelines and filters; the three measurement sets; the evidence pack; the Lab Report Assistant's tooling.
- **Spec, Round 3:** the 42 acceptance criteria (A1–J1) approved; the spec is complete.
- **Architecture, Round 1** (all ten answered "yes"; `docs/architecture.md`): one Rust crate on Nudgewright v0.4.5; FHIR fetching copied and adapted from Lab Report Assistant, not moved into the engine; rules as plain functions with stored versions; one approval covers the text and the email (ADR 0001); sending rules in a pre-send check that holds or refuses; signed reply codes and a signed inbound webhook; the opt-out page from a signed link with a button; Flesch–Kincaid reading level; build order M1–M7.
- **Build:** approved; M1 starts.
