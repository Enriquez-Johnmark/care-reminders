import { useQuery } from "@tanstack/react-query"
import { useSearchParams } from "react-router"

import { api } from "@/api/client"
import type { EntryView } from "@/api/generated/EntryView"
import { activityKeys } from "@/app/query-keys"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Input } from "@/components/ui/input"
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { formatDateTime } from "@/lib/dates"
import { actionChoices, actionLabel } from "@/lib/labels"

/** G3: the clinic's activity log, newest first, for the administrator; the
 *  filters live in the address so a filtered view can be shared. */
export function ActivityPage() {
  const [params, setParams] = useSearchParams()
  const actor = params.get("actor") ?? ""
  const action = params.get("action") ?? ""
  const activity = useQuery({
    queryKey: activityKeys.list(actor, action),
    queryFn: () =>
      api<EntryView[]>("GET", `/api/activity?${new URLSearchParams({ actor, action }).toString()}`),
  })
  const setFilter = (name: string, value: string) =>
    setParams((old) => {
      const next = new URLSearchParams(old)
      if (value) next.set(name, value)
      else next.delete(name)
      return next
    })

  return (
    <>
      <PageHeader
        title="Activity"
        description="Every fetch, choice, draft, check and review step: who did it and when."
      />
      <div className="flex flex-wrap gap-3">
        <div className="flex flex-col gap-1.5">
          <label htmlFor="actor" className="text-sm font-medium">
            Person
          </label>
          <Input
            id="actor"
            type="email"
            spellCheck={false}
            placeholder="carla@family.example.test…"
            defaultValue={actor}
            onBlur={(event) => setFilter("actor", event.target.value.trim())}
            onKeyDown={(event) => {
              if (event.key === "Enter") setFilter("actor", event.currentTarget.value.trim())
            }}
          />
        </div>
        <div className="flex flex-col gap-1.5">
          <label htmlFor="action" className="text-sm font-medium">
            Step
          </label>
          <NativeSelect
            id="action"
            value={action}
            onChange={(event) => setFilter("action", event.target.value)}
          >
            <NativeSelectOption value="">Every step</NativeSelectOption>
            {actionChoices.map(([code, label]) => (
              <NativeSelectOption key={code} value={code}>
                {label}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </div>
      </div>
      {activity.isPending ? (
        <Skeleton className="h-64 w-full" />
      ) : activity.isError ? (
        <QueryError error={activity.error} />
      ) : activity.data.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {actor || action ? "Nothing matches these filters." : "Nothing has happened yet."}
        </p>
      ) : (
        <div className="overflow-hidden rounded-lg border bg-card">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>When</TableHead>
                <TableHead>Step</TableHead>
                <TableHead>Who</TableHead>
                <TableHead>Details</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {activity.data.map((entry, index) => (
                <TableRow key={`${entry.at}-${index}`}>
                  <TableCell className="text-muted-foreground tabular-nums">
                    {formatDateTime(entry.at)}
                  </TableCell>
                  <TableCell>{actionLabel(entry.action)}</TableCell>
                  <TableCell>{entry.actor}</TableCell>
                  <TableCell className="whitespace-normal">
                    <span className="text-muted-foreground">{summary(entry)}</span>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </>
  )
}

/** One line about an entry, from its details. */
function summary(entry: EntryView) {
  const details = entry.details
  const text = (key: string) => (typeof details[key] === "string" ? (details[key] as string) : "")
  const number = (key: string) => (typeof details[key] === "number" ? (details[key] as number) : 0)
  switch (entry.action) {
    case "fhir.fetched":
      return `${number("new")} new, ${number("updated")} updated, ${number("refused_count")} refused; ${number("follow_ups_chosen")} follow-ups chosen`
    case "fhir.fetch_failed":
    case "reminder.skipped":
      return text("reason")
    case "follow_up.selected":
      return text("reason")
    case "reminder.checked":
      return details.blocked === true ? "Blocked by a check" : "Passed every check"
    default:
      return ""
  }
}
