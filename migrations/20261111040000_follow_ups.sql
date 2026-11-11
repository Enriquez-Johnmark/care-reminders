-- Each follow-up: one patient due for one kind of care, chosen by one rule
-- version, from one thing in their record. The same source never makes a
-- second follow-up under the same rule (B4). A stop ends its reminders.

create table follow_ups (
    id            uuid primary key,
    tenant_id     uuid not null,
    patient_id    uuid not null,
    rule_id       uuid not null,
    rule_version  integer not null,
    kind          text not null check (kind in ('after-result', 'screening-recall', 'after-discharge')),
    source        text not null,
    reason        text not null,
    since         date not null,
    due_by        date not null,
    selected_at   timestamptz not null,
    stopped_at    timestamptz,
    stop          text check (stop in ('booked', 'replied', 'opted-out')),
    unique (tenant_id, patient_id, rule_id, source),
    unique (tenant_id, id),
    foreign key (tenant_id, patient_id) references patients (tenant_id, id) on delete cascade,
    foreign key (tenant_id, rule_id, rule_version) references rule_versions (tenant_id, rule_id, version),
    check ((stopped_at is null) = (stop is null))
);
