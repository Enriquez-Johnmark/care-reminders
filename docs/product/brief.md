# Brief: Care Reminders

Status: **approved by the owner** (2026-11-11, Round 1). The spec (`spec.md`) is decided round by round from it. "Care Reminders" is a working name.

## In one sentence

An AI assistant that finds a clinic's patients who are due for follow-up care by the clinic's own rules. Claude words each reminder, code checks it, and a care coordinator approves the exact text. The reminder is sent once, and stops when the patient replies, books or opts out. Every step is in an append-only audit log.

## Why it exists

A portfolio project for AI roles in health software, and the second of four healthcare projects, after [Lab Report Assistant](https://github.com/Enriquez-Johnmark/lab-report-assistant). It shows what matters when AI writes to patients: code, not the model, decides who is contacted and what a message may reveal; a person approves the exact text; nothing is sent twice or too often; and the clinic can prove every step. It is built on Nudgewright, the owner's engine for human-approved loops.

## The problem

Recommended follow-up care is often missed (`docs/research/README.md` §1):

- about 1 in 14 abnormal test results have no record that the patient was told;
- after a positive stool test, often fewer than half of patients at community health centres have the follow-up colonoscopy within six months;
- half of Medicare patients readmitted within 30 days saw no physician in between.

Plain reminders help: text reminders raise attendance from about 68% to 79%, and reminders raise cancer screening by 10–14 points.

Writing those reminders takes staff time, and a model can draft them. But a model can also invent a date, name the wrong person, add clinical advice, or reveal the reason for a visit to whoever sees the patient's screen. No trial shows that AI-written reminders work better than a good template. So the useful product is not "AI writes reminders". It is: the clinic's rules decide who is due, AI words the message, code checks it, a person approves it, and the clinic can prove it.

## Who uses it

- **A care coordinator** at the clinic sees who is due and why, reviews each drafted reminder, edits it if needed, and approves or skips it.
- **A patient** receives the reminder by email and can reply, book, or opt out of that kind of reminder.
- **A clinic administrator** writes the clinic's follow-up rules and sees the audit log.

(In the portfolio, all three are shown in a demo of a made-up US primary-care clinic.)

## What it does (first version)

1. **Reads the clinic's data** as FHIR R4, fetched from a FHIR server (a local HAPI FHIR server in the demo): made-up patients with their results, visits, discharges, orders and booked appointments.
2. **Finds who is due** by rules the clinic writes, never by the model. There are three kinds:
   - a follow-up visit after an abnormal result;
   - a screening recall, such as colorectal screening or an annual exam;
   - a visit within 7 days of a hospital discharge.

   Each patient's reason is shown to staff.
3. **Drafts a reminder** with Claude, which only words it. Code fills in the patient's name, the date, the time and the clinic's details. By default the message names no reason for the visit, test or specialty. It says that it was drafted with AI and approved by a named person, and how to reach someone.
4. **Checks the draft in code** before anyone sees it:
   - no date, time, name or number other than the ones code filled in;
   - no clinical advice, diagnosis, test or sensitive specialty;
   - plain enough to read.

   A draft that fails is blocked with the reason; the coordinator can use the clinic's fixed template instead.
5. **Lets the coordinator approve** the exact text. An edit is checked again, and the approval binds the text: if it changes, it needs approving again.
6. **Sends it once** by email, through Nudgewright's outbox, so a crash or a double click never sends it twice. It sends at most 1 reminder a day and 3 a week per patient, enforced in code.
7. **Stops** when the patient replies, books (an appointment appears in the clinic's data), or opts out. An opt-out covers one kind of reminder (follow-up or screening), as the new FCC rule allows. The reason is recorded.
8. **Logs every step** in an append-only audit log.
9. **Compares with a template.** The demo shows Claude's wording beside the clinic's fixed template and measures how long approval takes. It never claims better attendance than the evidence shows.
10. **Produces an evidence pack** linking each acceptance criterion to the test that proves it.

## What it must never do

- Decide who needs care by the model, or contact a patient no rule selected.
- Send a reminder nobody approved, or a different text from the one approved.
- State a date, time, name or detail that isn't in the clinic's data.
- Give clinical advice, or reveal a diagnosis, test result, reason for the visit or sensitive specialty in a message.
- Send more often than 1 a day or 3 a week, or after the patient replied, booked or opted out.
- Use real patient data, or a real company's name or branding.
- Claim to be "HIPAA compliant" or certified, or claim better attendance without evidence.

## How success is shown

- **The checks, measured** on a test set of made-up drafts with planted faults: a wrong date, a wrong name, advice, a leaked reason for the visit. The README reports the faults caught (target: all of them) and the good drafts wrongly blocked.
- **Safety tests that always pass:** nothing unapproved, changed, repeated, too frequent or after a stop is ever sent.
- **The evidence pack**, published with each commit.
- A README with screenshots and a walkthrough video, and a page on the owner's portfolio.

## Technology (proposed)

**Rust** (axum) on **Nudgewright**, pinned to a release; **Postgres 17**; **React** with TypeScript and shadcn for the staff pages; **Claude** through the owner's API key; FHIR R4 from a local **HAPI FHIR** server, with the "due" items written by hand (Synthea doesn't make them); Playwright browser tests; Mailpit for email in development; GitHub Actions. The details are decided in the spec and the architecture.

## Not in the first version

Text messages (SMS comes later as an engine addition, with the 160-character limit, STOP handling and carrier registration), voice calls, booking inside the tool, payer care-gap reports (DEQM `$care-gaps`), writing back to the clinic's record system, and any patient chatbot.
