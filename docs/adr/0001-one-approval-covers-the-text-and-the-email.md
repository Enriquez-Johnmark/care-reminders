# 1. One approval covers the text and the email

Date: 2026-11-11. Status: accepted.

## Context

Nudgewright's outbox binds an approval to the content hash of the email it sends, and sends it exactly once. Here the coordinator approves a **reminder** on the review page, and the product also records that approval for the follow-up's timeline and its stops. The two records must never disagree: an approved reminder whose email isn't in the outbox would never go, and an email in the outbox without the product's approval would bypass the review rules (D1–D3).

## Decision

- The coordinator's approval runs **one database transaction** that stores the content hash of the reminder's exact text, with who approved it and when, and puts the email in the outbox.
- The engine's `approve` runs in its own transaction (v0.4.5 has no version that takes the caller's), so the email is approved **right after** the commit. If that step is interrupted, a **sweep** in the worker approves it, but only while the reminder's approved text still stands.
- The product's **pre-send check** refuses an email whose reminder text changed after approval, or whose follow-up has stopped.

## Consequences

- The engine's guarantee (approved exact text, sent once) covers the reminder as the coordinator saw it, without changing the engine.
- Between the commit and the engine's approval there is a short window where an approved reminder's email is still a draft; the sweep closes it, and nothing unapproved can be sent in it.
- This is the same pattern as Lab Report Assistant's ADR 0001. With two products using it, moving "approve inside the caller's transaction" into Nudgewright becomes worth proposing to the owner; until then it stays here.
