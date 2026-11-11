import { useMutation, useQueryClient } from "@tanstack/react-query"
import { Link, useNavigate, useParams } from "react-router"

import { api } from "@/api/client"
import { meKeys } from "@/app/queries"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
import { CenteredLayout } from "@/components/centered-layout"

/** F1: the page a login link opens. Only pressing the button uses the
 *  link, so an email scanner that opens it can't use it up. */
export function LoginLinkPage() {
  const { token = "" } = useParams()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const logIn = useMutation({
    mutationFn: () => api("POST", `/api/login/${encodeURIComponent(token)}`),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: meKeys.me() })
      navigate("/", { replace: true })
    },
  })

  return (
    <CenteredLayout>
      <Card>
        <CardHeader>
          <CardTitle>Log in</CardTitle>
          <CardDescription>Press the button to finish logging in on this device.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {logIn.isError ? (
            <>
              <Alert variant="destructive">
                <AlertDescription>{logIn.error.message}</AlertDescription>
              </Alert>
              <Button variant="outline" render={<Link to="/login" />} nativeButton={false}>
                Get a new login link
              </Button>
            </>
          ) : (
            <Button onClick={() => logIn.mutate()} disabled={logIn.isPending}>
              {logIn.isPending ? <Spinner data-icon="inline-start" /> : null}
              Log in
            </Button>
          )}
        </CardContent>
      </Card>
    </CenteredLayout>
  )
}
