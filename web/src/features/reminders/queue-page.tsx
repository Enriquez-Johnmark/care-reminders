import { CloudDownloadIcon } from "lucide-react"
import { Link } from "react-router"

import type { FetchView } from "@/api/generated/FetchView"
import type { QueueRow } from "@/api/generated/QueueRow"
import { useMe } from "@/app/queries"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { SaveError } from "@/components/save-error"
import { Status } from "@/components/status"
import { Button } from "@/components/ui/button"
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { formatDate } from "@/lib/dates"
import { kindLabel, statusLabel } from "@/lib/labels"
import { useFetchFromServer, useQueue } from "@/features/reminders/queries"

/** The reminders waiting for the care coordinator: drafts that passed every
 *  check first, then blocked ones, then approved ones (the API's order). */
export function QueuePage() {
  const queue = useQueue()
  const me = useMe()
  const isAdmin = me.data?.role === "administrator"
  const fetch = useFetchFromServer()

  return (
    <>
      <PageHeader
        title="Reminders"
        description={
          me.data?.role === "coordinator"
            ? "Each patient the clinic's rules found due for care, with the reminder to approve, change or skip."
            : "Each patient the clinic's rules found due for care. Only a care coordinator can approve a reminder."
        }
      >
        {isAdmin && me.data?.fhir_server ? (
          <Button onClick={() => fetch.mutate()} disabled={fetch.isPending}>
            {fetch.isPending ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <CloudDownloadIcon data-icon="inline-start" />
            )}
            Fetch from the FHIR server
          </Button>
        ) : null}
      </PageHeader>
      {fetch.isError ? <SaveError error={fetch.error} /> : null}
      {fetch.isSuccess ? <Fetched outcome={fetch.data} /> : null}
      {queue.isPending ? (
        <Skeleton className="h-64 w-full" />
      ) : queue.isError ? (
        <QueryError error={queue.error} />
      ) : queue.data.length === 0 ? (
        <Empty>
          <EmptyHeader>
            <EmptyTitle>No reminders waiting</EmptyTitle>
            <EmptyDescription>
              Patients appear here when the clinic's rules find them due for care, after each fetch
              from the FHIR server.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <>
          <Summary rows={queue.data} />
          <div className="overflow-hidden rounded-lg border bg-card">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Patient</TableHead>
                  <TableHead className="hidden md:table-cell">Why</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className="hidden sm:table-cell">Due by</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {queue.data.map((row) => {
                  const status = statusLabel(row.status)
                  return (
                    <TableRow key={row.id}>
                      <TableCell className="font-medium whitespace-normal">
                        <Link
                          to={`/reminders/${row.id}`}
                          className="underline-offset-4 hover:underline"
                        >
                          {row.patient_name}
                        </Link>
                        <span className="block font-normal text-muted-foreground">
                          {kindLabel(row.kind)} · reminder {row.number} of 3
                          {row.contact_another_way ? " · contact another way" : ""}
                        </span>
                      </TableCell>
                      <TableCell className="hidden whitespace-normal text-muted-foreground md:table-cell">
                        {row.reason}
                      </TableCell>
                      <TableCell>
                        <Status tone={status.variant}>{status.label}</Status>
                      </TableCell>
                      <TableCell className="hidden text-muted-foreground tabular-nums sm:table-cell">
                        {formatDate(row.due_by)}
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </div>
        </>
      )}
    </>
  )
}

/** How many reminders are ready, blocked and approved. */
function Summary({ rows }: { rows: QueueRow[] }) {
  const count = (status: string) => rows.filter((row) => row.status === status).length
  const parts = [
    [count("waiting"), "ready to review"],
    [count("blocked"), "blocked by a check"],
    [count("approved"), "approved"],
  ].filter(([n]) => n !== 0)
  return (
    <p className="text-sm text-muted-foreground">
      {parts.map(([n, label], index) => (
        <span key={label}>
          {index > 0 ? " · " : null}
          <span className="font-medium text-foreground tabular-nums">{n}</span> {label}
        </span>
      ))}
    </p>
  )
}

/** What a fetch did. */
function Fetched({ outcome }: { outcome: FetchView }) {
  return (
    <div role="status" className="rounded-lg border bg-card px-4 py-3 text-sm">
      <p>
        From <span className="font-medium">{outcome.server}</span>:{" "}
        <span className="tabular-nums">{outcome.new}</span> new patients,{" "}
        <span className="tabular-nums">{outcome.updated}</span> updated;{" "}
        <span className="font-medium tabular-nums">{outcome.follow_ups_chosen}</span> follow-ups
        chosen, <span className="tabular-nums">{outcome.follow_ups_booked}</span> stopped as booked.
      </p>
      {outcome.refused.length > 0 ? (
        <ul className="mt-2 flex flex-col gap-1 text-muted-foreground">
          {outcome.refused.map((refused) => (
            <li key={refused}>{refused}</li>
          ))}
        </ul>
      ) : null}
    </div>
  )
}
