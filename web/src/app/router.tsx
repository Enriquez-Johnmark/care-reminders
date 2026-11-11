import { createBrowserRouter } from "react-router"

import { AppShell } from "@/app/app-shell"
import { NotFoundPage } from "@/app/not-found-page"
import { PageLoading } from "@/app/page-loading"

/** Each page is loaded when first visited, so the first screen stays small. */
export const router = createBrowserRouter([
  {
    path: "/login",
    HydrateFallback: PageLoading,
    lazy: async () => ({ Component: (await import("@/features/login/login-page")).LoginPage }),
  },
  {
    path: "/login/:token",
    HydrateFallback: PageLoading,
    lazy: async () => ({
      Component: (await import("@/features/login/login-link-page")).LoginLinkPage,
    }),
  },
  {
    Component: AppShell,
    HydrateFallback: PageLoading,
    children: [
      {
        index: true,
        lazy: async () => ({
          Component: (await import("@/features/reminders/queue-page")).QueuePage,
        }),
      },
      {
        path: "reminders/:id",
        lazy: async () => ({
          Component: (await import("@/features/reminders/reminder-page")).ReminderPage,
        }),
      },
      {
        path: "activity",
        lazy: async () => ({
          Component: (await import("@/features/activity/activity-page")).ActivityPage,
        }),
      },
      { path: "*", Component: NotFoundPage },
    ],
  },
])
