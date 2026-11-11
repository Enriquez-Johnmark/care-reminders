# Spec: Care Reminders

Decided round by round with the owner; the single source for spec decisions. The brief is `brief.md`. Status: **in progress** (Round 1 decided 2026-11-11).

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
