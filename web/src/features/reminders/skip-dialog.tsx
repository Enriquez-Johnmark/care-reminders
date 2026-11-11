import { useState } from "react"

import { SaveError } from "@/components/save-error"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { useSkip } from "@/features/reminders/queries"

/** D6: skipping a reminder, with the reason; it is never sent. */
export function SkipDialog({ id }: { id: string }) {
  const [isOpen, setIsOpen] = useState(false)
  const [reason, setReason] = useState("")
  const skip = useSkip(id)
  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        setIsOpen(open)
        if (!open) skip.reset()
      }}
    >
      <DialogTrigger render={<Button variant="outline" />}>Skip this reminder</DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Skip this reminder</DialogTitle>
          <DialogDescription>
            It won't be sent. The next reminder, if any, follows on its own day.
          </DialogDescription>
        </DialogHeader>
        <label htmlFor="skip-reason" className="text-sm font-medium">
          Why? (for the activity log)
        </label>
        <Textarea
          id="skip-reason"
          rows={3}
          value={reason}
          onChange={(event) => setReason(event.target.value)}
        />
        {skip.isError ? <SaveError error={skip.error} /> : null}
        <DialogFooter>
          <DialogClose render={<Button variant="outline" />}>Cancel</DialogClose>
          <Button
            onClick={() => skip.mutate({ reason }, { onSuccess: () => setIsOpen(false) })}
            disabled={skip.isPending || !reason.trim()}
          >
            {skip.isPending ? <Spinner data-icon="inline-start" /> : null}
            Skip it
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
