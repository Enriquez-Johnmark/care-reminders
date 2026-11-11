import { useQuery } from "@tanstack/react-query"

import { api } from "@/api/client"
import type { Me } from "@/api/generated/Me"

export const meKeys = {
  me: () => ["me"] as const,
}

/** The logged-in person and their clinic; fails with a 401 ApiError when nobody is logged in. */
export function useMe() {
  return useQuery({
    queryKey: meKeys.me(),
    queryFn: () => api<Me>("GET", "/api/me"),
    staleTime: 5 * 60 * 1000,
  })
}
