import { ApiError } from "@/api/client"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"

/** Why the API refused a form, with each problem it found. */
export function SaveError({ error }: { error: Error }) {
  const problems = error instanceof ApiError ? error.problems : []
  return (
    <Alert variant="destructive">
      <AlertTitle>{error.message}</AlertTitle>
      {problems.length > 0 ? (
        <AlertDescription>
          <ul className="list-disc pl-4">
            {problems.map((problem) => (
              <li key={problem}>{problem}</li>
            ))}
          </ul>
        </AlertDescription>
      ) : null}
    </Alert>
  )
}
