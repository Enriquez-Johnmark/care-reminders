-- Why a patient's stored record may be out of date: they weren't on the
-- clinic's FHIR server at the last fetch, or the reader refused their newer
-- record. Null while the record is current. Rules skip such patients rather
-- than remind on stale data.

alter table patients add column not_current text;
