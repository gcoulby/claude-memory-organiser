import { Download, X } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { installUpdate, useUpdateStore } from "@/features/updater/updater";
import { useJobRunning } from "@/store/jobs";

/**
 * Offers an available update. Never installs without a click, and holds
 * off while a job is running so a restart cannot interrupt it.
 */
export function UpdateBanner() {
  const state = useUpdateStore((s) => s.state);
  const set = useUpdateStore((s) => s.set);
  const busy = useJobRunning();
  const [showNotes, setShowNotes] = useState(false);

  if (state.phase === "installing") {
    const pct = state.total ? Math.floor((state.downloaded / state.total) * 100) : null;
    return (
      <div className="flex shrink-0 items-center gap-3 border-b bg-secondary px-4 py-2 text-sm">
        <Download className="size-4" />
        Installing version {state.update.version}
        {pct !== null ? ` (${pct}%)` : "…"} The app will restart when it is done.
      </div>
    );
  }
  if (state.phase !== "available" || state.dismissed) return null;
  const { update } = state;

  return (
    <div className="shrink-0 border-b bg-secondary px-4 py-2 text-sm">
      <div className="flex items-center gap-3">
        <Download className="size-4 shrink-0" />
        <span className="flex-1">
          Version {update.version} is available (you have {update.currentVersion}).
        </span>
        {update.body && (
          <Button
            size="sm"
            variant="ghost"
            className="h-7"
            onClick={() => {
              setShowNotes(!showNotes);
            }}
          >
            {showNotes ? "Hide notes" : "Release notes"}
          </Button>
        )}
        <Button
          size="sm"
          className="h-7"
          disabled={busy}
          title={busy ? "Available once the running job finishes" : undefined}
          onClick={() => {
            void installUpdate(update);
          }}
        >
          {busy ? "Install when the job finishes" : "Install and restart"}
        </Button>
        <button
          type="button"
          aria-label="Not now"
          className="rounded-sm p-1 hover:bg-accent"
          onClick={() => {
            set({ ...state, dismissed: true });
          }}
        >
          <X className="size-4" />
        </button>
      </div>
      {showNotes && update.body && (
        <pre className="mt-2 max-h-48 overflow-auto font-sans text-xs whitespace-pre-wrap text-muted-foreground">
          {update.body}
        </pre>
      )}
    </div>
  );
}
