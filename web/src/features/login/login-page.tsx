import { zodResolver } from "@hookform/resolvers/zod"
import { useMutation } from "@tanstack/react-query"
import { MailCheckIcon } from "lucide-react"
import { useForm } from "react-hook-form"
import { z } from "zod"

import { api } from "@/api/client"
import type { LoginRequest } from "@/api/generated/LoginRequest"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Field, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { CenteredLayout } from "@/components/centered-layout"

const schema = z.object({
  email: z.email("Enter your email address."),
})

/** F1: asking for a login link. The answer is the same whether or not
 *  the address has an account. */
export function LoginPage() {
  const form = useForm<LoginRequest>({
    resolver: zodResolver(schema),
    defaultValues: { email: "" },
  })
  const request = useMutation({
    mutationFn: (values: LoginRequest) => api("POST", "/api/login", values),
  })

  return (
    <CenteredLayout>
      <Card>
        <CardHeader>
          <CardTitle>Log in</CardTitle>
          <CardDescription>
            We'll email you a link. There's no password to remember.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {request.isSuccess ? (
            <Alert>
              <MailCheckIcon />
              <AlertTitle>Check your email</AlertTitle>
              <AlertDescription>
                If {form.getValues("email")} has an account here, a login link is on its way. It
                works once, for 15 minutes.
              </AlertDescription>
            </Alert>
          ) : (
            <form onSubmit={form.handleSubmit((values) => request.mutate(values))} noValidate>
              <FieldGroup>
                <Field data-invalid={form.formState.errors.email ? true : undefined}>
                  <FieldLabel htmlFor="email">Your email</FieldLabel>
                  <Input
                    id="email"
                    type="email"
                    autoComplete="email"
                    autoFocus
                    aria-invalid={form.formState.errors.email ? true : undefined}
                    {...form.register("email")}
                  />
                  <FieldError errors={[form.formState.errors.email]} />
                </Field>
                {request.isError ? (
                  <Alert variant="destructive">
                    <AlertDescription>{request.error.message}</AlertDescription>
                  </Alert>
                ) : null}
                <Button type="submit" disabled={request.isPending}>
                  {request.isPending ? <Spinner data-icon="inline-start" /> : null}
                  Email me a login link
                </Button>
              </FieldGroup>
            </form>
          )}
        </CardContent>
      </Card>
    </CenteredLayout>
  )
}
