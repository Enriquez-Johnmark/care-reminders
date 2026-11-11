-- The care coordinator's review: who may approve, each approval with the
-- exact text's hash, and why a reminder was skipped.

-- Members of a clinic who may approve reminders (D1). The administrator is
-- the account's owner and is never listed here.
create table coordinators (
    member_id   uuid primary key,
    tenant_id   uuid not null,
    name        text not null check (length(name) between 1 and 100),
    created_at  timestamptz not null,
    foreign key (member_id, tenant_id) references account_members (id, tenant_id) on delete cascade
);

-- An approval binds the draft's exact text by its hash (D3).
create table approvals (
    draft_id      uuid primary key,
    tenant_id     uuid not null,
    reminder_id   uuid not null,
    content_hash  text not null,
    approved_by   uuid not null,
    approved_at   timestamptz not null,
    foreign key (tenant_id, draft_id) references drafts (tenant_id, id) on delete cascade,
    foreign key (tenant_id, reminder_id) references reminders (tenant_id, id) on delete cascade
);

alter table reminders add column skip_reason text;
