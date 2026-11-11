import { useMutation } from "@tanstack/react-query"
import { Navigate, NavLink, Outlet, useLocation } from "react-router"

import { api, ApiError } from "@/api/client"
import { useMe } from "@/app/queries"
import { Logo } from "@/components/logo"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { cn } from "@/lib/utils"

type NavItem = { to: string; label: string }

const reminders: NavItem = { to: "/", label: "Reminders" }
const replies: NavItem = { to: "/replies", label: "Replies" }
const activity: NavItem = { to: "/activity", label: "Activity" }

/** The logged-in frame: a letterhead across the top, as on the clinic's own
 *  paper (the product, then the clinic and who is logged in), with the
 *  places beneath it, and the page below. */
export function AppShell() {
  const me = useMe()
  // Only an ended session means logging in again; anything else (a deploy, a
  // dropped connection) is retried while the person stays where they are.
  if (me.error instanceof ApiError && me.error.isNotLoggedIn)
    return <Navigate to="/login" replace />
  const items =
    me.data?.role === "administrator" ? [reminders, replies, activity] : [reminders, replies]
  return (
    <div className="flex min-h-svh flex-col">
      <header className="border-b bg-card">
        <div className="mx-auto flex w-full max-w-6xl flex-wrap items-center justify-between gap-x-6 gap-y-2 px-4 pt-3 md:px-8">
          <Logo />
          <Account
            clinicName={me.data?.clinic_name}
            who={me.data ? (me.data.name ?? me.data.email) : undefined}
          />
        </div>
        <nav aria-label="Places" className="mx-auto w-full max-w-6xl px-4 md:px-8">
          <ul className="-mb-px flex gap-6 overflow-x-auto">
            {items.map((item) => (
              <NavItemLink key={item.to} item={item} />
            ))}
          </ul>
        </nav>
      </header>
      <main className="mx-auto flex w-full max-w-6xl flex-1 flex-col gap-6 px-4 py-6 md:px-8 md:py-8">
        <Outlet />
      </main>
    </div>
  )
}

function NavItemLink({ item }: { item: NavItem }) {
  const { pathname } = useLocation()
  // A reminder belongs to the queue it was opened from.
  const isActive =
    item.to === "/"
      ? pathname === "/" || pathname.startsWith("/reminders")
      : pathname.startsWith(item.to)
  return (
    <li>
      <NavLink
        to={item.to}
        aria-current={isActive ? "page" : undefined}
        className={cn(
          "block border-b-2 py-3 text-sm whitespace-nowrap transition-colors",
          isActive
            ? "border-primary font-medium text-foreground"
            : "border-transparent text-muted-foreground hover:text-foreground"
        )}
      >
        {item.label}
      </NavLink>
    </li>
  )
}

function Account({ clinicName, who }: { clinicName?: string; who?: string }) {
  const logOut = useMutation({
    mutationFn: () => api("POST", "/api/logout"),
    onSettled: () => location.assign("/login"),
  })
  return (
    <div className="flex items-center gap-4">
      <div className="flex flex-col text-sm leading-tight sm:items-end">
        {clinicName ? (
          <span className="font-medium">{clinicName}</span>
        ) : (
          <Skeleton className="h-4 w-32" />
        )}
        {who ? <span className="text-xs text-muted-foreground">{who}</span> : null}
      </div>
      <Button
        variant="outline"
        size="sm"
        onClick={() => logOut.mutate()}
        disabled={logOut.isPending}
      >
        Log out
      </Button>
    </div>
  )
}
