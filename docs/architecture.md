# Architecture

Status: **Round 1 decided 2026-11-11.** The spec is [product/spec.md](product/spec.md); the engine is [Nudgewright](https://github.com/Enriquez-Johnmark/nudgewright) v0.4.5. The patterns reused come from [Lab Report Assistant](https://github.com/Enriquez-Johnmark/lab-report-assistant) and [Kompleto](https://github.com/Enriquez-Johnmark/kompleto).

## What the engine already gives

Checked in Nudgewright v0.4.5 before this was written:

| Engine module | Used for |
|---|---|
| `outbox` | Each reminder's email: approval bound to a content hash, sending exactly once, the crash test, pre-send checks that refuse a message or hold it until later, and cancelling an approved message before it goes (D3, E1–E5, F1) |
| `inbound` | Parsing replies and recognising automatic replies (F2) |
| `signed` | The opt-out link and the reply code (F3, F4) |
| `accounts` | Login by emailed link, sessions, the administrator and the care coordinator (H1) |
| `jobs` | Drafting (E6) |
| `llm`, `cost_log` | Calling Claude and logging each call's cost (I3) |
| `audit` | The append-only log: a database trigger refuses updates and deletes (G1, G2) |
| `clock`, `ids`, `tenants` | Time only through the injected `Clock`; typed IDs; every query scoped to a `TenantId` (A4) |

## Code home (Round 1, 2026-11-11)

- **One Rust crate** with modules, the engine re-exported as `crate::engine`. Its own modules:

| Module | What it does | Criteria |
|---|---|---|
| `fhir` | Reads FHIR R4 into strict types (patients, results, visits, discharges, orders, appointments); refuses what it can't read | H2 |
| `fhir_server` | Fetches the clinic's data from its FHIR server, adapted from Lab Report Assistant: https only (or this computer), time and size limits, same server only | H2, H3 |
| `demo_data` | The generator of the made-up patients and their care, with the planted tricky cases | A1 |
| `rules` | The three rule types as plain functions over a patient's data; rule versions | B1–B5 |
| `follow_ups` | Follow-ups, their reminders and stops | B2–B4, E6, F1 |
| `drafting` | Builds the model request (kind, which reminder, tone); parses the answer; fills the fill-ins | A2, C1, C6 |
| `checks` | The draft checks and the reading level | C2–C5, C7, D4 |
| `review` | Approve, edit, use the template, new draft, skip | D1–D6 |
| `sending` | The pre-send checks, replies and the opt-out page | E1–E5, E7, F2–F4 |
| `evidence` | The evidence pack | J1 |
| `web` | The JSON API, the opt-out page and serving the React app | H1 |

- **The clinic is the engine's tenant.** Every repository function takes a `ClinicId` (the engine's `TenantId`). The exceptions:
  - system-wide workers (the outbox sender and job workers), which still name the row's tenant in every update;
  - login, before any clinic is known;
  - the opt-out page and the reply webhook, which find the follow-up from a signed code.
- **The hourly fetch** is a sweep in the worker (`work`): each minute it fetches for every clinic not fetched from in the last hour, so a missed hour catches up. It assumes one worker; a second would need a lock on the clinic's row.
- **FHIR fetching stays in this product.** Nudgewright's rule is that the engine holds nothing domain-specific, so the Lab Report Assistant's fetch code is copied and adapted here rather than moved into the engine.

## Who is due is decided by rules, never the model (Round 1, 2026-11-11)

- Each rule type is a plain function from a patient's data and the rule's settings to "due, by this date, for this reason" or "not due". It makes no model call and no database call, so B1–B5 are unit tests.
- Each change to a rule stores a new **rule version**; a follow-up keeps the version that selected it.
- **One follow-up visit answers every follow-up of its type:** a booked or fulfilled `FOLLOWUP` appointment after a result or discharge stops each open after-result and after-discharge follow-up it comes after, since one visit can cover both; a `ROUTINE` one answers screening recalls.
- **The day the rules use** is the UTC date until M5 gives each clinic its time zone; from then, choosing and sending both use the clinic's day.
- A patient whose stored record **isn't current** (gone from the server at the last fetch, or their newer record refused) is never selected: the rules don't remind on stale data. Staff see the reason.

## One approval covers the text and the email (Round 1, 2026-11-11)

- The coordinator's approval, in **one database transaction**, stores the **content hash of the reminder's exact text** and puts its **email** into the outbox, already approved. Either both happen or neither ([ADR 0001](adr/0001-one-approval-covers-the-text-and-the-email.md)).

## Sending rules, checked again just before sending (Round 1, 2026-11-11)

A `PreSendCheck` runs at approval and again before each send:

- **held until later** (the engine's "not until"): more than 1 reminder that day or 3 that week for the patient, or outside 8 am–6 pm on weekdays, clinic time;
- **refused**: the follow-up has stopped, the patient asked not to be emailed, or the patient's record isn't current (they may have been removed from the clinic's server; their open follow-ups wait rather than stop, in case they come back).

A booking found at a fetch also **cancels** an approved reminder that hasn't gone yet.

## Replies and opting out (Round 1, 2026-11-11)

- Each reminder's reply-to address carries a **signed code**, so a reply finds its follow-up. Replies arrive on a signed inbound webhook; automatic replies are ignored. In the demo, a "simulate a reply" command stands in for real inbound mail.
- The **opt-out page** is server-rendered from a signed link, in the clinic letter style. Opening it shows a button; only pressing it records the opt-out, for that kind of reminder.

## Reading level (Round 1, 2026-11-11)

- The **Flesch–Kincaid grade** is computed in code on the draft with its fill-ins replaced by fixed sample words, so the same draft always scores the same.

## Recorded model answers (Round 1, 2026-11-11)

- As in Lab Report Assistant, Claude's answers are recorded once with the owner's key and replayed in tests, CI and the demo (`serve --recorded`), so they need no key and cost nothing.

## Checks (Round 1, 2026-11-11)

Before every commit: `cargo fmt --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings` and `cargo nextest run --all-features --locked`; for `web/`, its lint, typecheck, tests and build. CI also runs the browser tests against a real HAPI FHIR server and builds the evidence pack.

## Build order (Round 1, 2026-11-11)

| Milestone | What | Criteria | Status |
|---|---|---|---|
| M1 | Data and fetching: the generator, FHIR reading, HAPI FHIR, fetching, clinic scoping, the first evidence pack | A1, A3, A4, H2, H3, J1 | done (2026-11-11) |
| M2 | Rules and follow-ups | B1–B5 | done (2026-11-11) |
| M3 | Drafts and checks: the model request, fill-ins, every check, recorded answers, the 40 bad drafts, the drafting job | A2, C1–C7, E6, I1 | in progress |
| M4 | Logins, review and the activity log, with the staff pages | D1–D6, G1–G3, H1 | planned |
| M5 | Sending and stops: the outbox, caps, sending hours, no-email patients, the opt-out page, replies, bookings | E1–E5, E7, F1–F4 | planned |
| M6 | Measurement: Claude's recorded drafts, Sonnet against Haiku against the template | I2, I3 | planned |
| M7 | Showcase: README, screenshots, walkthrough video, portfolio page | none | planned |

Each milestone ends with a code review and an evidence page.
