import { QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query"
import type { ReactNode } from "react"

import { ApiError } from "@/api/client"
import { Toaster } from "@/components/ui/toast"
import { TooltipProvider } from "@/components/ui/tooltip"

function createQueryClient() {
  return new QueryClient({
    queryCache: new QueryCache({
      onError(error) {
        // A session that ends while the app is open: back to the login page.
        if (
          error instanceof ApiError &&
          error.isNotLoggedIn &&
          !location.pathname.startsWith("/login")
        ) {
          location.assign("/login")
        }
      },
    }),
    defaultOptions: {
      queries: {
        // A refusal won't change by asking again; a dropped connection might.
        retry: (count, error) => !(error instanceof ApiError) && count < 2,
        refetchOnWindowFocus: true,
      },
    },
  })
}

const queryClient = createQueryClient()

export function Providers({ children }: { children: ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <Toaster>{children}</Toaster>
      </TooltipProvider>
    </QueryClientProvider>
  )
}
