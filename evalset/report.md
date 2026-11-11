# Measurement

Claude's draft of every reminder the made-up demo can ask for (3 kinds of follow-up × reminders 1–3 × 2 tones = 18), each put through the checks every draft must pass before a coordinator can approve it (`src/checks.rs`), beside the clinic's fixed template. Replayed from `evalset/recorded/`; `cargo run -- eval --record` asks Claude again with the owner's key. Costs are at list price. The model sees only the kind, which reminder and the tone, never a patient.

| Drafter | Pass rate | Passed | Average reading grade | Cost per draft |
|---|---|---|---|---|
| claude-sonnet-5-5 | 100% | 18 of 18 | 1.8 | US$0.00246 |
| claude-haiku-5-5 | 72% | 13 of 18 | 2.0 | US$0.00011 |
| the clinic's template | 100% | 18 of 18 | 2.7 | free |

## claude-sonnet-5-5: drafts that failed

None.

## claude-haiku-5-5: drafts that failed

- **after-result, reminder 3, warm**
  - numbers-and-dates: "third" is a number, date or time of the draft's own; only {due_by} gives a date
- **after-result, reminder 3, plain**
  - numbers-and-dates: "third" is a number, date or time of the draft's own; only {due_by} gives a date
- **screening-recall, reminder 3, plain**
  - numbers-and-dates: "third" is a number, date or time of the draft's own; only {due_by} gives a date
- **after-discharge, reminder 3, warm**
  - numbers-and-dates: "third" is a number, date or time of the draft's own; only {due_by} gives a date
- **after-discharge, reminder 3, plain**
  - numbers-and-dates: "third" is a number, date or time of the draft's own; only {due_by} gives a date

## the clinic's template: drafts that failed

None.
