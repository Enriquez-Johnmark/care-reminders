# Showcase

Screenshots and a captioned walkthrough (2 min 19 s) of Care Reminders at a made-up clinic, Example Family Practice, on Tuesday 10 November 2026 at 10 am New York time. The administrator fetches the 25 made-up patients from the clinic's FHIR server (HAPI FHIR, running on this computer), and the clinic's rules choose 40 follow-ups. The care coordinator reviews and approves Claude's drafts. One reminder goes out, and its patient opens the opt-out page and stops that kind of reminder. A second reminder to the same patient waits for the daily cap. A patient who asked not to be emailed shows as "contact another way", an edit naming a condition is blocked, a reply stops a follow-up, and the activity log shows every step.

The drafts are Claude Sonnet's, made with the owner's API key on 2026-11-11 and replayed from `evalset/recorded/` (the same drafts `evalset/report.md` measures). Everything is made up: the clinic, the patients, their care and the `example.test` addresses. The emails arrive in Mailpit, a test inbox on this computer; nothing is sent anywhere.

| File | Shows |
|---|---|
| `walkthrough.mp4` | The whole loop, from fetching the patients to a reply and the activity log |
| `desktop-1-queue.png` | The coordinator's queue after the rules chose 40 follow-ups: drafts that passed every check first |
| `desktop-2-review.png` | Reviewing a reminder: why the patient is due and which rule version chose them, Claude's words as the patient will read them, beside the clinic's template |
| `desktop-3-sent.png` | After approval: the exact text approved, and sent in the sending hours |
| `desktop-4-email.png` | The patient's email in the test inbox: the clinic, its phone, a date and an opt-out link, and nothing about the test or the reason |
| `desktop-5-opt-out.png` | The opt-out page: opening it changes nothing; only the button opts out |
| `desktop-6-blocked-edit.png` | The coordinator's edit naming a condition, blocked by the same checks Claude's drafts go through |
| `desktop-7-replies.png` | A patient's reply, which stopped that follow-up |
| `desktop-8-activity.png` | The administrator's activity log |
| `phone-1-queue.png`, `phone-2-review.png` | The queue and a review page on a phone |

Mailpit stamps each email with the real time it arrived, while the app runs on the demo day, so the two dates differ.

## Making them again

```bash
docker compose up -d
pnpm --dir web install && pnpm --dir web build
scripts/showcase.sh
```

The script builds a test build (`--features failpoints`), starts a fresh database and a server on port 3004 whose clock starts at the demo time, and drafts from the recordings (`serve --recorded`), so it needs no API key and costs nothing. It also starts HAPI FHIR on this computer (`docker compose --profile fhir`, port 8090) and loads the made-up patients into it; nothing is sent to a public FHIR server. The video needs ffmpeg.
