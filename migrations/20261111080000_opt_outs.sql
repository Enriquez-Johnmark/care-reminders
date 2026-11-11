-- Opt-outs from the reminder's own link (F3): one patient, one kind of
-- reminder. Rules keep that kind stopped at every later fetch.

create table opt_outs (
    tenant_id     uuid not null,
    patient_id    uuid not null,
    kind          text not null check (kind in ('after-result', 'screening-recall', 'after-discharge')),
    opted_out_at  timestamptz not null,
    primary key (tenant_id, patient_id, kind),
    foreign key (tenant_id, patient_id) references patients (tenant_id, id) on delete cascade
);
