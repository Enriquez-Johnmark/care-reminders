# Spec: Care Reminders

Decided round by round with the owner; the single source for spec decisions. The brief is `brief.md`. Status: **in progress** (Rounds 1–2 decided 2026-11-11; the acceptance criteria are Round 3).

## Words

- A **rule** is one of the clinic's follow-up rules: a fixed rule type with the clinic's settings. Each change makes a new **rule version**.
- A **follow-up** is one patient due for one kind of care because a rule selected them, with the **reason** shown to staff and the date it is **due by**.
- There are three **kinds** of follow-up: **after a result**, **screening recall** and **after discharge**.
- A **reminder** is one message about a follow-up; a follow-up has at most three. A **draft** is a reminder's text before approval; an **approved reminder** is one a care coordinator approved.
- A **fill-in** is a named gap in a draft, such as `{first_name}`, that code fills from the clinic's data.
- A **check** is a rule in code that a draft must pass before a person sees it.
- A **stop** ends a follow-up's reminders: the patient **booked**, **replied** or **opted out**.

## The made-up clinic (Round 1, 2026-11-11)

- **Example Family Practice**, a made-up US primary-care clinic.
- Two people: an **administrator**, who writes the rules, and a **care coordinator**, who approves reminders.
- Every page, file and email says the clinic and patients are made up.

## Made-up data (Round 1, 2026-11-11)

- About **25 made-up patients** and about **40 follow-ups** across the three kinds, as FHIR R4.
- Patients, results, visits and discharges come from our own generator, from a fixed seed. The "due" items (orders, appointments) are written by hand, since Synthea doesn't make them.
- Tricky cases are planted on purpose:
  - a patient who has already booked;
  - one who opted out;
  - one who asked not to be emailed;
  - one whose reason for the visit is sensitive;
  - one discharged too long ago to be due.

## The clinic's rules (Round 1, 2026-11-11)

- A fixed set of **rule types** with settings, edited by the administrator on a Settings page; never free-form code and never the model:
  - **after a result:** an abnormal result of a listed test → a follow-up visit within N days;
  - **screening recall:** a screening not done in N months, for ages A to B;
  - **after discharge:** a hospital discharge → a visit within 7 days.
- Each follow-up records the rule and the rule version that selected it.

## Reminders per follow-up (Round 1, 2026-11-11)

- Up to **three reminders**, planned for day 0, day 7 and day 21 of the follow-up.
- **Each reminder is approved separately**: an approval binds one exact text.

## What the model sees and writes (Round 1, 2026-11-11)

- The model sees only the **kind** of follow-up, **which reminder** it is (first, second or third) and the clinic's **tone** setting. Never a name, a date or a reason.
- It writes with fixed **fill-ins**: `{first_name}`, `{clinic_name}`, `{clinic_phone}`, `{due_by}`, `{opt_out_link}`. Code fills them in from the clinic's data.
- **Claude Sonnet** drafts by default; **Claude Haiku** is measured beside it. Claude's answers are recorded, so tests and the demo need no API key.

## Checks on every draft (Round 1, 2026-11-11)

A draft is **blocked**, with the reason shown, if it has:

- a fill-in that isn't on the list, or is missing `{clinic_phone}` or `{opt_out_link}`;
- a **digit**, a **date** or a **time** of its own;
- **clinical words**: a condition, test, specialty or medication, or advice;
- a reading level above **8th grade**.

Above **6th grade**, the coordinator sees a warning.

## How reminders stop (Round 1, 2026-11-11)

- **Booked:** a booked appointment for the patient appears in the clinic's data at the next fetch.
- **Replied:** the patient's email reply comes back through the engine's reply handling.
- **Opted out:** the patient presses a button on a page opened from a signed link in the email; opening the link alone changes nothing, so email scanners can't opt a patient out. An opt-out covers one kind of reminder.

## Sending (Round 1, 2026-11-11)

- **Email only**, through Nudgewright's outbox, sent once even across a crash.
- At most **1 reminder a day and 3 a week** per patient, enforced in code.
- Only between **8 am and 6 pm on weekdays**, clinic time; a reminder that misses the window waits for the next one.
- A patient who asked **not to be emailed** is never emailed. They show in the queue as "contact another way", with the reason.

## People and logins (Round 2, 2026-11-11)

- Logging in is by an **emailed link**; there are no passwords.
- Only the **care coordinator** can approve; the administrator's approval is refused.

## The coordinator's queue (Round 2, 2026-11-11)

- One page lists the follow-ups that need a reminder: drafts that passed every check first, then blocked ones.
- Each row shows the patient, the kind, the reason, the due-by date and which reminder it is (1 to 3).

## The review page (Round 2, 2026-11-11)

- The reason, and the rule (and its version) that selected the patient.
- Claude's draft with the fill-ins filled in, beside the clinic's fixed template, with the reading level.
- The coordinator can **approve**, **edit**, **use the template**, **ask for a new draft** (at most twice) or **skip** the reminder with a reason.

## Edits (Round 2, 2026-11-11)

- A coordinator's edit goes through the **same checks** as Claude's drafts. A clinical word or a digit in an edit **blocks** it: a coordinator mustn't add clinical content.

## Fetching (Round 2, 2026-11-11)

- The administrator presses **"Fetch from the FHIR server"**, and a background job also fetches **every hour**.
- Each fetch finds new follow-ups by the rules, stops follow-ups whose patient has since booked, and is logged.

## Drafting (Round 2, 2026-11-11)

- A background job drafts each new or next reminder.
- A reminder becomes due on its planned day (0, 7 or 21) only if nothing has stopped the follow-up.

## The activity log (Round 2, 2026-11-11)

- Every step is in an **append-only** log that the database refuses to change.
- Each follow-up has its own **timeline**; the administrator sees the whole clinic's log, filtered by person and step.
- Steps logged: fetch, selected, drafted, checked, edited, approved, skipped, sent, replied, booked and opted out.

## How the checks are measured (Round 2, 2026-11-11)

| Set | What it holds | Target |
|---|---|---|
| Bad drafts | 40 hand-written drafts, each with one planted fault: a digit, a date, a stray fill-in, a clinical word, advice, or too hard to read | every one blocked |
| Claude's drafts | Claude's draft of every reminder in the demo, recorded | at least 90% pass; every failure listed in the README |
| Comparison | Sonnet beside Haiku, and both beside the template | pass rate and cost reported |

## The evidence pack (Round 2, 2026-11-11)

- A page, built on every push, linking each acceptance criterion to the tests that prove it. CI fails if a finished milestone's criterion has none passing.

## Tooling (Round 2, 2026-11-11)

- **Rust** (axum, sqlx) on **Nudgewright** at a pinned release; **Postgres 17**.
- **React**, TypeScript and shadcn, in the Lab Report Assistant's clinical look.
- **HAPI FHIR** v8.4.0-1 in Docker, locally and in CI; **Mailpit**; **Playwright**; **nextest**; **GitHub Actions**.
