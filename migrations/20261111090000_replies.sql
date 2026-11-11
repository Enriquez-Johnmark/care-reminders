-- Patients' replies to reminders (F2), found from the signed reply code in
-- the address they replied to. A reply stops its follow-up and is shown to
-- the coordinator. A redelivered email is stored once.

create table replies (
    id            uuid primary key,
    tenant_id     uuid not null,
    follow_up_id  uuid not null,
    email_key     text not null,
    from_address  text not null,
    text          text not null,
    received_at   timestamptz not null,
    unique (tenant_id, email_key),
    foreign key (tenant_id, follow_up_id) references follow_ups (tenant_id, id) on delete cascade
);
