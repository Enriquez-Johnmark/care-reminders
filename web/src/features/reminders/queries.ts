import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { api } from "@/api/client"
import type { ApproveRequest } from "@/api/generated/ApproveRequest"
import type { EditedView } from "@/api/generated/EditedView"
import type { EditRequest } from "@/api/generated/EditRequest"
import type { EntryView } from "@/api/generated/EntryView"
import type { FetchView } from "@/api/generated/FetchView"
import type { QueueRow } from "@/api/generated/QueueRow"
import type { ReminderView } from "@/api/generated/ReminderView"
import type { SkipRequest } from "@/api/generated/SkipRequest"
import { reminderKeys } from "@/app/query-keys"

export function useQueue() {
  return useQuery({
    queryKey: reminderKeys.queue(),
    queryFn: () => api<QueueRow[]>("GET", "/api/queue"),
    // The worker chooses and drafts in the background; show new reminders
    // as they arrive.
    refetchInterval: 5000,
  })
}

export function useReminder(id: string) {
  return useQuery({
    queryKey: reminderKeys.detail(id),
    queryFn: () => api<ReminderView>("GET", `/api/reminders/${id}`),
  })
}

export function useTimeline(followUp: string | undefined) {
  return useQuery({
    queryKey: reminderKeys.timeline(followUp ?? ""),
    queryFn: () => api<EntryView[]>("GET", `/api/follow-ups/${followUp}/timeline`),
    enabled: Boolean(followUp),
  })
}

/** A review step on one reminder; afterwards everything about reminders is
 *  read again, since the step changes its status, draft and timeline. */
function useReviewStep<Body, Result = void>(path: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (body: Body) => api<Result>("POST", path, body),
    onSettled: () => queryClient.invalidateQueries({ queryKey: reminderKeys.all() }),
  })
}

export function useApprove(id: string) {
  return useReviewStep<ApproveRequest>(`/api/reminders/${id}/approve`)
}

export function useEdit(id: string) {
  return useReviewStep<EditRequest, EditedView>(`/api/reminders/${id}/edit`)
}

export function useTemplate(id: string) {
  return useReviewStep<undefined, EditedView>(`/api/reminders/${id}/template`)
}

export function useRedraft(id: string) {
  return useReviewStep<undefined, EditedView>(`/api/reminders/${id}/redraft`)
}

export function useSkip(id: string) {
  return useReviewStep<SkipRequest>(`/api/reminders/${id}/skip`)
}

export function useFetchFromServer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => api<FetchView>("POST", "/api/fhir-server/fetch"),
    onSettled: () => queryClient.invalidateQueries({ queryKey: reminderKeys.all() }),
  })
}
