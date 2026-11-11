-- Each clinic's follow-up rules and every version of them. A change adds a
-- version and never rewrites one, so a follow-up can always show the
-- version that selected it.

create table rules (
    id          uuid primary key,
    tenant_id   uuid not null references accounts (tenant_id) on delete cascade,
    kind        text not null check (kind in ('after-result', 'screening-recall', 'after-discharge')),
    name        text not null,
    version     integer not null,
    unique (tenant_id, name),
    unique (tenant_id, id)
);

create table rule_versions (
    tenant_id   uuid not null,
    rule_id     uuid not null,
    version     integer not null,
    settings    jsonb not null,
    created_at  timestamptz not null,
    created_by  text not null,
    primary key (tenant_id, rule_id, version),
    foreign key (tenant_id, rule_id) references rules (tenant_id, id) on delete cascade
);
