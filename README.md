# Care Reminders

Finds a clinic's patients who are due for follow-up care and reminds them, with a person at the clinic approving every reminder: Claude drafts, code checks, the approved text is sent once, reminders stop when the patient replies, books or opts out, and every step is audited.

**Status:** building. M1 (data and fetching) is done, with its 6 acceptance criteria proven ([the evidence page](docs/evidence/m1.html)); M2 (rules and follow-ups) is next. See [`AGENTS.md`](AGENTS.md) for the phases and [`docs/architecture.md`](docs/architecture.md) for the build order.

## What works so far

- **Made-up data:** 25 made-up patients of Example Family Practice, a made-up US primary-care clinic, as FHIR R4 bundles in [`demo/patients/`](demo/patients/). They have lab results, hospital stays, screenings and appointments, and five tricky cases are planted on purpose: a patient who already booked, one who opted out of screening reminders, one who asked not to be emailed, one with a sensitive result, and one discharged too long ago to be due. Every resource carries the HL7 "test health data" label (`HTEST`), and CI fails if a file doesn't, or if anything outside the research documents names a real company (`a1_*`, `a3_*`).
- **Fetching from a FHIR server:** each clinic's patients are fetched from its own FHIR server, by request (`fetch`) and every hour (`work`). Each patient's resources go through a strict reader; one it refuses is skipped with the reason while the others are saved, and nothing is saved twice. Every request has a time limit and every answer a size limit, next pages are followed only on the same server, and a slow, oversized or non-FHIR answer saves nothing and says why. A patient who is gone from the server, or whose newer record was refused, is marked as not current. Every fetch is in the audit log (`h2_*`, `h3_*`). CI runs this against a real HAPI FHIR server (v8.4.0-1).
- **Each clinic sees only its own patients** (`a4_*`).
- **The evidence pack** links each of the 43 acceptance criteria to the tests that prove it (`j1_*`).

## Try it

```bash
docker compose up -d && docker compose --profile fhir up -d   # Postgres, Mailpit and HAPI FHIR on port 8090
cp .env.example .env
cargo run -- migrate
cargo run -- clinic add --name "Example Family Practice" --email admin@family.example.test
cargo run -- load-made-up --url http://127.0.0.1:8090/fhir    # only a server on this computer is accepted
cargo run -- clinic set-fhir-server --clinic <the printed ID> --url http://127.0.0.1:8090/fhir
cargo run -- fetch --clinic <the ID>
cargo nextest run
```

The second of four healthcare projects, after [Lab Report Assistant](https://github.com/Enriquez-Johnmark/lab-report-assistant). Made-up clinic, made-up patients: this repository contains no real patient data and names no real company.

## Licence

All rights reserved ([LICENSE](LICENSE)). The source is published to be read, not reused.
