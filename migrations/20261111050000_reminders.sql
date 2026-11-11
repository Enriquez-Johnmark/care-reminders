-- Each clinic's reminder settings, each follow-up's reminders (at most
-- three), and every draft of each reminder with what the checks found.

create table clinic_settings (
    tenant_id  uuid primary key references accounts (tenant_id) on delete cascade,
    phone      text not null default '',
    tone       text not null default 'warm' check (tone in ('warm', 'plain'))
);

create table reminders (
    id            uuid primary key,
    tenant_id     uuid not null,
    follow_up_id  uuid not null,
    number        smallint not null check (number between 1 and 3),
    planned_on    date not null,
    created_at    timestamptz not null,
    -- Finished once sent or skipped; only then can the next one be drafted.
    finished_at   timestamptz,
    outcome       text check (outcome in ('sent', 'skipped')),
    unique (tenant_id, follow_up_id, number),
    unique (tenant_id, id),
    foreign key (tenant_id, follow_up_id) references follow_ups (tenant_id, id) on delete cascade,
    check ((finished_at is null) = (outcome is null))
);

create table drafts (
    id           uuid primary key,
    tenant_id    uuid not null,
    reminder_id  uuid not null,
    attempt      integer not null,
    source       text not null check (source in ('model', 'template', 'coordinator')),
    model        text,
    text         text not null,
    problems     jsonb not null,
    warnings     jsonb not null,
    grade        double precision not null,
    created_at   timestamptz not null,
    unique (tenant_id, reminder_id, attempt),
    unique (tenant_id, id),
    foreign key (tenant_id, reminder_id) references reminders (tenant_id, id) on delete cascade
);
