# AGENTS.md

**Care Reminders** finds a clinic's patients who are due for follow-up care, such as a visit after an abnormal result or a screening recall. Claude drafts each reminder; code checks it; a person at the clinic approves the exact text; it is sent once; reminders stop when the patient replies, books or opts out; and every step is in an append-only audit log. It works for any clinic; the demo uses a **made-up clinic with made-up patients**, and the repository names no real company. Working name; it may be renamed.

It is the second of four healthcare projects, each in its own repository: [Lab Report Assistant](https://github.com/Enriquez-Johnmark/lab-report-assistant) (done), care follow-up reminders (this one), a care-team message assistant and a record-access audit.

## Phases

Each gate needs the owner's explicit "yes"; an approval for one phase never carries into the next.

| Phase | Status |
| --- | --- |
| 1. Research | done (2026-11-11, `docs/research/README.md`) |
| 2. Brief | done, approved 2026-11-11 (`docs/product/brief.md`) |
| 3. Spec & acceptance criteria | done, approved 2026-11-11 (`docs/product/spec.md`, 43 criteria) |
| 4. Architecture | done, approved 2026-11-11 (`docs/architecture.md`) |
| 5. Build | **in progress**, approved 2026-11-11: M1 (data and fetching), M2 (rules and follow-ups) and M3 (drafts and checks) done (`docs/evidence/m3.html`); M4 (logins, review, activity log) next |

**No application code exists or gets written until the build is approved.** Research, docs and throwaway prototypes that answer a specific question are in scope; prototypes live in `prototypes/<question>/` and are deleted once their answer is recorded.

## Owner decisions

Every decision needing the owner's judgement goes through the `grilling` skill: numbered rounds of questions, each with a recommended answer, worded so "yes" accepts it. Look facts up yourself, using sub-agents for research; put only decisions to the owner. Record every approved decision, dated, in `docs/decisions.md`. Record hard-to-reverse, non-obvious trade-offs in `docs/adr/`, and the project's words in `GLOSSARY.md`; both are created when first needed.

## Evidence

Research claims carry a source (URL, publisher, date) and a confidence (H/M/L). Label vendor claims as vendor claims. When evidence is missing, write "not found"; a gap stays a gap. Keep facts, hypotheses, recommendations and owner-approved decisions visibly distinct.

## Rules

- **No real company or patient.** No real company's name, branding or content, and no real patient data, anywhere in the repository, except that research documents may name real companies as cited sources (decided 2026-11-11). Patients and their care are made up, and say so.
- **A person approves every reminder.** Nothing is sent without approval, and the approval binds the exact text. Every draft, check, edit, approval, send, reply and opt-out is logged.
- **Reminders never give clinical advice,** and reveal no more about a patient's care than the clinic allows; code checks this before a person sees a draft.
- **Model output is untrusted.** Parsed into strict types and checked by rules before a person sees it.
- **API key only.** The Claude API through the owner's key (`OWNER_ANTHROPIC_API_KEY`), never a Claude subscription; no data to free model tiers that train on inputs.
- **No compliance claims.** The README never says "HIPAA compliant" or "certified"; it says what the code enforces and how it is tested.
- **Engine:** Nudgewright (decided 2026-11-10), a git dependency pinned to a release; an engine change is made and released there, never patched here.
- **Read the `rust-best-practices` skill** (`.claude/skills/rust-best-practices`) before writing or reviewing Rust.
- **React:** read the `vercel-react-best-practices`, `vercel-composition-patterns`, `shadcn`, `frontend-design`, `web-design-guidelines` and `ui-taste` skills (`.claude/skills/`) before writing or reviewing React. Third-party skills are read in full before they're added, recorded in `skills-lock.json` with source and hash, and may not run unpinned commands.
- **Tests:** test first, using the `tdd` skill (one criterion's test, see it fail, then make it pass); every acceptance criterion has a test whose name starts with its ID.
- **Report test results only from commands actually run.**
- **Time:** read only through the engine's injected `Clock`; direct system-clock calls are banned with clippy `disallowed-methods`.
- **Clinic isolation:** every repository function takes a `ClinicId` (the engine's `TenantId`), and every query filters on it; the exceptions are in `docs/architecture.md`.
- **Small commits** on `main`, each one behaviour (a test plus the code that makes it pass) or one small refactor or doc change, aiming for under ~150 changed lines excluding `.sqlx/` and `Cargo.lock`; messages imperative; no AI attribution trailers such as "Co-Authored-By" (owner's choice). Commits are signed with the owner's SSH key, so GitHub shows them as verified.

## Working here

- **Before every commit:** `cargo fmt --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings` and `cargo nextest run --all-features --locked` pass. If SQL changed, run `cargo sqlx prepare -- --all-targets --all-features` and commit `.sqlx/`.
- **Cloud sessions:** `scripts/cloud-session-start.sh` runs at session start (via `.claude/settings.json`): commit signing, Postgres and Mailpit, the pinned Rust and the migrations. On a local machine it does nothing.
- **Locally:** `docker compose up -d`; `cp .env.example .env` once; `cargo run -- migrate`; then `cargo nextest run` (or `cargo test`).
