import { CircleAlertIcon } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"

/** Shown when a page's data couldn't be loaded. */
export function QueryError({ error }: { error: Error }) {
  return (
    <Alert variant="destructive">
      <CircleAlertIcon />
      <AlertTitle>This page couldn't be loaded</AlertTitle>
      <AlertDescription>{error.message}</AlertDescription>
    </Alert>
  )
}
