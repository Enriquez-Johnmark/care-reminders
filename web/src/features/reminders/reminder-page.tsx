import { ChevronLeftIcon, CircleAlertIcon, CircleCheckIcon, TriangleAlertIcon } from "lucide-react"
import { useState } from "react"
import { Link, useParams } from "react-router"

import type { ReminderView } from "@/api/generated/ReminderView"
import { useMe } from "@/app/queries"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { SaveError } from "@/components/save-error"
import { Section } from "@/components/section"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { formatDate, formatDateTime } from "@/lib/dates"
import { actionLabel, checkLabel, kindLabel, sourceLabel } from "@/lib/labels"
import {
  useApprove,
  useEdit,
  useRedraft,
  useReminder,
  useTemplate,
  useTimeline,
} from "@/features/reminders/queries"
import { SkipDialog } from "@/features/reminders/skip-dialog"

/** One reminder: why the patient is due, the draft as the patient will read
 *  it beside the clinic's template, what the checks found, and the
 *  coordinator's actions. Every rule (who may approve, what blocks a draft)
 *  is the API's; this page only asks. */
export function ReminderPage() {
  const { id = "" } = useParams()
  const reminder = useReminder(id)
  const me = useMe()
  const [editing, setEditing] = useState(false)

  if (reminder.isPending) return <Skeleton className="h-96 w-full" />
  if (reminder.isError) return <QueryError error={reminder.error} />
  const data = reminder.data
  const isCoordinator = me.data?.role === "coordinator"

  return (
    <>
      <Link
        to="/"
        className="-mb-3 inline-flex w-fit items-center gap-1 text-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
      >
        <ChevronLeftIcon aria-hidden className="size-4" />
        Reminders
      </Link>
      <PageHeader
        title={data.patient_name}
        description={`${kindLabel(data.kind)} · reminder ${data.number} of 3 · due by ${formatDate(data.due_by)}`}
      />
      <section aria-label="Reminder" className="overflow-hidden rounded-lg border bg-card">
        <Why reminder={data} />
        <DraftStatus reminder={data} />
        {editing ? (
          <Editor key={data.draft.id} reminder={data} onDone={() => setEditing(false)} />
        ) : (
          <Texts reminder={data} />
        )}
        {isCoordinator && !editing && !data.finished ? (
          <Actions reminder={data} onEdit={() => setEditing(true)} />
        ) : null}
        {!isCoordinator && me.data ? (
          <p className="border-t bg-muted/50 px-4 py-3 text-sm text-muted-foreground">
            Only a care coordinator can approve or change this reminder.
          </p>
        ) : null}
      </section>
      <Timeline followUp={data.follow_up} />
    </>
  )
}

/** Why the patient is due, for staff only; the reminder never says. */
function Why({ reminder }: { reminder: ReminderView }) {
  return (
    <div className="flex flex-col gap-1 border-b px-4 py-3 text-sm">
      <p>
        <span className="font-medium">Why:</span> {reminder.reason}
      </p>
      <p className="text-muted-foreground">
        Chosen by “{reminder.rule}”, version {reminder.rule_version}. The reminder never says why.
      </p>
      {reminder.contact_another_way ? (
        <p className="font-medium text-destructive">
          This patient asked not to be emailed: contact them another way.
        </p>
      ) : null}
    </div>
  )
}

function DraftStatus({ reminder }: { reminder: ReminderView }) {
  const draft = reminder.draft
  return (
    <div className="flex flex-col gap-3 border-b px-4 py-3">
      <p className="text-sm text-muted-foreground">
        <span className="font-medium text-foreground">{sourceLabel(draft.source)}</span>
        {draft.grade > 0 ? <> · reading grade {draft.grade.toFixed(1)}</> : null}
      </p>
      {reminder.finished ? (
        <Notice tone="info">
          <CircleCheckIcon />
          <AlertTitle>{reminder.finished === "skipped" ? "Skipped" : "Sent"}</AlertTitle>
          <AlertDescription>This reminder is finished.</AlertDescription>
        </Notice>
      ) : null}
      {reminder.approved_at ? (
        <Notice tone="info">
          <CircleCheckIcon />
          <AlertTitle>Approved</AlertTitle>
          <AlertDescription>
            This exact text was approved on {formatDateTime(reminder.approved_at)}.
          </AlertDescription>
        </Notice>
      ) : null}
      {draft.problems.length > 0 ? (
        <Notice tone="failed">
          <CircleAlertIcon />
          <AlertTitle>This draft failed a check, so it can't be approved</AlertTitle>
          <AlertDescription>
            <ul className="list-disc pl-4">
              {draft.problems.map((problem) => (
                <li key={`${problem.check}-${problem.detail}`}>
                  <span className="font-medium">{checkLabel(problem.check)}:</span> {problem.detail}
                </li>
              ))}
            </ul>
          </AlertDescription>
        </Notice>
      ) : null}
      {draft.warnings.length > 0 ? (
        <Notice tone="caution">
          <TriangleAlertIcon />
          <AlertTitle>Worth a look</AlertTitle>
          <AlertDescription>
            <ul className="list-disc pl-4">
              {draft.warnings.map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
          </AlertDescription>
        </Notice>
      ) : null}
    </div>
  )
}

/** A notice inside the sheet: a tinted band, not a box inside a box. */
function Notice({
  tone,
  children,
}: {
  tone: "info" | "failed" | "caution"
  children: React.ReactNode
}) {
  const styles = {
    info: "border-0 bg-muted",
    failed: "border-0 bg-destructive/[0.06]",
    caution: "border-0 bg-warning/10",
  } as const
  const variant = tone === "failed" ? "destructive" : tone === "caution" ? "warning" : "default"
  return (
    <Alert variant={variant} className={styles[tone]}>
      {children}
    </Alert>
  )
}

/** The draft as the patient will read it, beside the clinic's template. */
function Texts({ reminder }: { reminder: ReminderView }) {
  return (
    <div className="grid gap-px bg-border md:grid-cols-2">
      <div className="flex flex-col gap-2 bg-card p-4">
        <h2 className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
          As the patient will read it
        </h2>
        {reminder.preview ? (
          <p className="leading-relaxed whitespace-pre-line">{reminder.preview}</p>
        ) : (
          <>
            <p className="leading-relaxed whitespace-pre-line text-muted-foreground">
              {reminder.draft.text || "No text."}
            </p>
            {reminder.preview_problem ? (
              <p className="text-sm text-destructive">{reminder.preview_problem}</p>
            ) : null}
          </>
        )}
      </div>
      <div className="flex flex-col gap-2 bg-card p-4">
        <h2 className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
          The clinic's template
        </h2>
        <p className="leading-relaxed whitespace-pre-line text-muted-foreground">
          {reminder.template}
        </p>
      </div>
    </div>
  )
}

const FILL_INS = ["{first_name}", "{clinic_name}", "{clinic_phone}", "{due_by}", "{opt_out_link}"]

/** The coordinator's own text, checked like any draft (D4). */
function Editor({ reminder, onDone }: { reminder: ReminderView; onDone: () => void }) {
  const [text, setText] = useState(reminder.draft.text || reminder.template)
  const edit = useEdit(reminder.id)
  return (
    <div className="flex flex-col gap-3 p-4">
      <label htmlFor="reminder-text" className="text-sm font-medium">
        Edit the reminder
      </label>
      <Textarea
        id="reminder-text"
        rows={6}
        value={text}
        onChange={(event) => setText(event.target.value)}
      />
      <p className="text-sm text-muted-foreground">
        Fill-ins, filled from the clinic's data: {FILL_INS.join(", ")}. It must give{" "}
        {"{clinic_phone}"} and {"{opt_out_link}"}, and no date, number or reason of its own.
      </p>
      {edit.isError ? <SaveError error={edit.error} /> : null}
      <div className="flex flex-wrap gap-2">
        <Button
          onClick={() => edit.mutate({ text }, { onSuccess: onDone })}
          disabled={edit.isPending}
        >
          {edit.isPending ? <Spinner data-icon="inline-start" /> : null}
          Save the text
        </Button>
        <Button variant="outline" onClick={onDone} disabled={edit.isPending}>
          Cancel
        </Button>
      </div>
    </div>
  )
}

function Actions({ reminder, onEdit }: { reminder: ReminderView; onEdit: () => void }) {
  const approve = useApprove(reminder.id)
  const template = useTemplate(reminder.id)
  const redraft = useRedraft(reminder.id)
  const draft = reminder.draft
  const error = approve.error ?? template.error ?? redraft.error
  const busy = approve.isPending || template.isPending || redraft.isPending
  return (
    <div className="flex flex-col gap-3 border-t bg-muted/50 px-4 py-3">
      {error ? <SaveError error={error} /> : null}
      <div className="flex flex-wrap gap-2">
        {draft.problems.length === 0 && !reminder.approved_at ? (
          <Button
            onClick={() => approve.mutate({ draft_id: draft.id, content_hash: draft.content_hash })}
            disabled={busy}
          >
            {approve.isPending ? <Spinner data-icon="inline-start" /> : null}
            Approve this text
          </Button>
        ) : null}
        <Button variant="outline" onClick={onEdit} disabled={busy}>
          Edit
        </Button>
        {draft.source !== "template" ? (
          <Button variant="outline" onClick={() => template.mutate(undefined)} disabled={busy}>
            Use the template
          </Button>
        ) : null}
        <Button
          variant="outline"
          onClick={() => redraft.mutate(undefined)}
          disabled={busy || reminder.new_drafts_left === 0}
          title={
            reminder.new_drafts_left === 0
              ? "No more new drafts: edit it or use the template"
              : undefined
          }
        >
          {redraft.isPending ? <Spinner data-icon="inline-start" /> : null}
          {reminder.new_drafts_left === 0
            ? "No new drafts left"
            : `Ask for a new draft (${reminder.new_drafts_left} left)`}
        </Button>
        <SkipDialog id={reminder.id} />
      </div>
    </div>
  )
}

function Timeline({ followUp }: { followUp: string }) {
  const timeline = useTimeline(followUp)
  return (
    <Section title="Timeline">
      {timeline.isPending ? (
        <Skeleton className="h-24 w-full" />
      ) : timeline.isError ? (
        <QueryError error={timeline.error} />
      ) : (
        <ol className="flex flex-col gap-2 text-sm">
          {timeline.data.map((entry, index) => (
            <li key={`${entry.at}-${index}`} className="flex flex-wrap gap-x-3">
              <span className="text-muted-foreground tabular-nums">{formatDateTime(entry.at)}</span>
              <span className="font-medium">{actionLabel(entry.action)}</span>
              <span className="text-muted-foreground">by {entry.actor}</span>
              {typeof entry.details.reason === "string" ? (
                <span>{entry.details.reason}</span>
              ) : null}
            </li>
          ))}
        </ol>
      )}
    </Section>
  )
}
