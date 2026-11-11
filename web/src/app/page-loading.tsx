import { Spinner } from "@/components/ui/spinner"

/** Shown while a page's code loads on the first visit. */
export function PageLoading() {
  return (
    <div className="flex min-h-svh items-center justify-center" role="status" aria-label="Loading">
      <Spinner className="size-6" />
    </div>
  )
}
