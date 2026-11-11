import type { ReactNode } from "react"

import { Logo } from "@/components/logo"

/** A centred column under the logo: the login pages. */
export function CenteredLayout({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-svh flex-col items-center justify-center gap-6 bg-muted p-4">
      <Logo className="text-lg" />
      <div className="w-full max-w-sm">{children}</div>
    </div>
  )
}
