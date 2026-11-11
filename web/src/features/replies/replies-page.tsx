import { useQuery } from "@tanstack/react-query"

import { api } from "@/api/client"
import type { ReplyView } from "@/api/generated/ReplyView"
import { replyKeys } from "@/app/query-keys"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Skeleton } from "@/components/ui/skeleton"
import { formatDateTime } from "@/lib/dates"

/** F2: patients' replies, newest first. Each one stopped its follow-up's
 *  reminders; the coordinator answers by phone or email. A reply is the
 *  patient's own words, untrusted, so it is shown as plain text only. */
export function RepliesPage() {
  const replies = useQuery({
    queryKey: replyKeys.list(),
    queryFn: () => api<ReplyView[]>("GET", "/api/replies"),
    refetchInterval: 15_000,
  })

  return (
    <>
      <PageHeader
        title="Replies"
        description="What patients wrote back. A reply stops that follow-up's reminders, so answer them yourself."
      />
      {replies.isPending ? (
        <Skeleton className="h-48 w-full" />
      ) : replies.isError ? (
        <QueryError error={replies.error} />
      ) : replies.data.length === 0 ? (
        <p className="text-sm text-muted-foreground">No replies yet.</p>
      ) : (
        <ul className="flex flex-col gap-3">
          {replies.data.map((reply) => (
            <li key={reply.id} className="rounded-lg border bg-card px-4 py-3">
              <p className="flex flex-wrap items-baseline justify-between gap-x-4 text-sm">
                <span className="font-medium">{reply.patient_name}</span>
                <span className="text-muted-foreground tabular-nums">
                  {formatDateTime(reply.received_at)}
                </span>
              </p>
              <p className="text-sm text-muted-foreground">{reply.from_address}</p>
              <p className="mt-2 text-sm whitespace-pre-wrap">{reply.text}</p>
            </li>
          ))}
        </ul>
      )}
    </>
  )
}
