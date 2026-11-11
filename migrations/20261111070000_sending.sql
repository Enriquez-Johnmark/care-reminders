-- Sending (E1–E5): each clinic's time zone, which sets its sending hours and
-- its day, and the outbox message each approved reminder became. A patient
-- who asked not to be emailed is contacted another way instead (E5).

alter table clinic_settings add column time_zone text not null default 'America/New_York';

alter table reminders drop constraint reminders_outcome_check;
alter table reminders add constraint reminders_outcome_check
    check (outcome in ('sent', 'skipped', 'contact-another-way'));

create table reminder_messages (
    message_id   uuid primary key,
    tenant_id    uuid not null,
    reminder_id  uuid not null,
    -- One email per approved draft; an edit after approval makes a new draft.
    draft_id     uuid not null,
    created_at   timestamptz not null,
    unique (tenant_id, draft_id),
    foreign key (tenant_id, reminder_id) references reminders (tenant_id, id) on delete cascade,
    foreign key (tenant_id, draft_id) references drafts (tenant_id, id) on delete cascade
);
