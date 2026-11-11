import { cn } from "@/lib/utils"

type Tone = "default" | "success" | "warning" | "destructive" | "secondary"

const dot: Record<Tone, string> = {
  default: "bg-primary",
  success: "bg-success",
  warning: "bg-warning",
  destructive: "bg-destructive",
  secondary: "bg-muted-foreground/50",
}

/** A status in words with a small dot of its colour: the words carry the
 *  meaning, so the colour is never the only signal, and the accent isn't
 *  spent as a solid pill on every row. */
export function Status({ tone, children }: { tone: Tone; children: string }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-2 text-sm whitespace-nowrap",
        tone === "destructive" ? "font-medium text-destructive" : "text-foreground"
      )}
    >
      <span aria-hidden className={cn("size-2 shrink-0 rounded-full", dot[tone])} />
      {children}
    </span>
  )
}
