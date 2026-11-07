import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Lock, X } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { commands, errorMessage } from "@/lib/commands";
import { queryKeys } from "@/lib/queryClient";
import { useImportStore } from "@/store/importStore";

function ImportStatus() {
  const state = useImportStore((s) => s.state);
  const set = useImportStore((s) => s.set);

  switch (state.phase) {
    case "idle":
      return <span>No jobs running</span>;
    case "running": {
      const p = state.progress;
      const pct = p && p.bytesTotal > 0 ? Math.floor((p.bytesDone / p.bytesTotal) * 100) : null;
      return (
        <span className="flex items-center gap-2">
          <span>
            {state.cancelling ? "Cancelling import…" : "Importing"}
            {p ? ` ${p.file}: ${p.conversations.toLocaleString("en-GB")} chats` : "…"}
            {pct !== null ? ` (${pct}%)` : ""}
          </span>
          {!state.cancelling && (
            <Button
              variant="ghost"
              size="sm"
              className="h-5 px-1.5 text-xs"
              onClick={() => {
                set({ ...state, cancelling: true });
                void commands.importCancel();
              }}
            >
              Cancel
            </Button>
          )}
        </span>
      );
    }
    case "done": {
      const s = state.summary;
      return (
        <Dismissable
          onDismiss={() => {
            set({ phase: "idle" });
          }}
        >
          Import finished: {s.conversationsAdded} new, {s.conversationsUpdated} updated,{" "}
          {s.conversationsUnchanged} unchanged
          {s.conversationsSkipped > 0 ? `, ${s.conversationsSkipped} unreadable` : ""} (
          {(s.durationMs / 1000).toFixed(1)}s)
        </Dismissable>
      );
    }
    case "failed":
      return (
        <Dismissable
          onDismiss={() => {
            set({ phase: "idle" });
          }}
        >
          <span className={state.cancelled ? "" : "text-destructive"}>{state.message}</span>
        </Dismissable>
      );
  }
}

function Dismissable({ children, onDismiss }: { children: ReactNode; onDismiss: () => void }) {
  return (
    <span className="flex items-center gap-1">
      <span>{children}</span>
      <button
        type="button"
        aria-label="Dismiss"
        className="rounded-sm p-0.5 hover:bg-accent"
        onClick={onDismiss}
      >
        <X className="size-3" />
      </button>
    </span>
  );
}

export function StatusBar() {
  const queryClient = useQueryClient();
  const info = useQuery({ queryKey: ["app-info"], queryFn: commands.appInfo });
  const running = useImportStore((s) => s.state.phase === "running");
  const lock = useMutation({
    mutationFn: commands.vaultLock,
    onSuccess: () => {
      // Drop everything read from the database, then re-check the vault.
      queryClient.removeQueries({
        predicate: (q) => q.queryKey[0] !== queryKeys.vaultStatus[0],
      });
      void queryClient.invalidateQueries({ queryKey: queryKeys.vaultStatus });
    },
  });

  return (
    <footer className="flex h-7 shrink-0 items-center gap-4 border-t px-3 text-xs text-muted-foreground">
      {/* Placeholder until settings exist: the mode must always be visible. */}
      <span className="rounded-sm bg-secondary px-1.5 py-0.5 text-secondary-foreground">
        Local only
      </span>
      <span className="min-w-0 flex-1 truncate">
        <ImportStatus />
      </span>
      {lock.isError && <span className="text-destructive">{errorMessage(lock.error)}</span>}
      <button
        type="button"
        className="flex items-center gap-1 rounded-sm px-1 hover:bg-accent disabled:opacity-50"
        disabled={running || lock.isPending}
        title={running ? "Wait for the import to finish" : "Lock"}
        onClick={() => {
          lock.mutate();
        }}
      >
        <Lock className="size-3" /> Lock
      </button>
      <span>
        {info.isError ? (
          <span className="text-destructive">Backend unavailable</span>
        ) : info.data ? (
          `v${info.data.version}`
        ) : (
          "…"
        )}
      </span>
    </footer>
  );
}
