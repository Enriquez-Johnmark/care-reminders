import type { ReactNode } from "react"

type PageHeaderProps = {
  title: string
  description?: ReactNode
  /** Buttons shown beside the title. */
  children?: ReactNode
}

/** The title row at the top of every page. */
export function PageHeader({ title, description, children }: PageHeaderProps) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-4">
      <div className="flex flex-col gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
        {description ? <p className="text-sm text-muted-foreground">{description}</p> : null}
      </div>
      {children ? <div className="flex flex-wrap gap-2">{children}</div> : null}
    </div>
  )
}
