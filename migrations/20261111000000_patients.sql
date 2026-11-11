-- Each clinic's patients, as last fetched from its FHIR server. The record
-- the rules read is kept whole; the columns are what staff pages and sending
-- need. Erasing a clinic's account removes them with it.

create table patients (
    id          uuid primary key,
    tenant_id   uuid not null references accounts (tenant_id) on delete cascade,
    fhir_id     text not null,
    name        text not null,
    given_name  text not null,
    email       text,
    no_email    boolean not null,
    record      jsonb not null,
    fetched_at  timestamptz not null,
    updated_at  timestamptz not null,
    unique (tenant_id, fhir_id),
    unique (tenant_id, id)
);
