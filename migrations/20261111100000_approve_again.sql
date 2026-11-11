-- An approved text whose email was refused (say, the patient's address
-- changed) can be approved again, making a new email for the same draft.

alter table reminder_messages drop constraint reminder_messages_tenant_id_draft_id_key;
create index reminder_messages_draft on reminder_messages (tenant_id, draft_id);
