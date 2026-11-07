import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Lock, Settings, X } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { usePreferences } from "@/features/tagging/queries";
import { commands, errorMessage } from "@/lib/commands";
import { count } from "@/lib/format";
import { queryKeys } from "@/lib/queryClient";
import { cn } from "@/lib/utils";
import { useImportStore } from "@/store/importStore";
import { useJobRunning } from "@/store/jobs";
import { useTaggingStore } from "@/store/taggingStore";
import { useUiStore } from "@/store/uiStore";

function Dismissable({ children, onDismiss }: { children: ReactNode; onDismiss: () => void }) {
  return (
    <span className="flex min-w-0 items-center gap-1">
      <span className="truncate">{children}</span>
      <button
        type="button"
        aria-label="Dismiss"
        className="shrink-0 rounded-sm p-0.5 hover:bg-accent"
        onClick={onDismiss}
      >
        <X className="size-3" />
      </button>
    </span>
  );
}

function CancelButton({ onCancel }: { onCancel: () => void }) {
  return (
    <Button variant="ghost" size="sm" className="h-5 px-1.5 text-xs" onClick={onCancel}>
      Cancel
    </Button>
  );
}

function ImportStatus() {
  const state = useImportStore((s) => s.state);
  const set = useImportStore((s) => s.set);
  const idle = () => {
    set({ phase: "idle" });
  };

  switch (state.phase) {
    case "idle":
      return null;
    case "running": {
      const p = state.progress;
      const pct = p && p.bytesTotal > 0 ? Math.floor((p.bytesDone / p.bytesTotal) * 100) : null;
      return (
        <span className="flex items-center gap-2">
          <span>
            {state.cancelling ? "Cancelling import…" : "Importing"}
            {p ? ` ${p.file}: ${count(p.conversations)} chats` : "…"}
            {pct !== null ? ` (${pct}%)` : ""}
          </span>
          {!state.cancelling && (
            <CancelButton
              onCancel={() => {
                set({ ...state, cancelling: true });
                void commands.importCancel();
              }}
            />
          )}
        </span>
      );
    }
    case "done": {
      const s = state.summary;
      return (
        <Dismissable onDismiss={idle}>
          Import finished: {s.conversationsAdded} new, {s.conversationsUpdated} updated,{" "}
          {s.conversationsUnchanged} unchanged
          {s.conversationsSkipped > 0 ? `, ${s.conversationsSkipped} unreadable` : ""} (
          {(s.durationMs / 1000).toFixed(1)}s)
        </Dismissable>
      );
    }
    case "failed":
      return (
        <Dismissable onDismiss={idle}>
          <span className={state.cancelled ? "" : "text-destructive"}>{state.message}</span>
        </Dismissable>
      );
  }
}

function TaggingStatus() {
  const state = useTaggingStore((s) => s.state);
  const set = useTaggingStore((s) => s.set);
  const idle = () => {
    set({ phase: "idle" });
  };

  switch (state.phase) {
    case "idle":
      return null;
    case "running": {
      const p = state.progress;
      return (
        <span className="flex items-center gap-2">
          <span>
            {state.cancelling ? "Finishing chats already sent…" : "Tagging"}
            {p ? ` ${count(p.done)} of ${count(p.total)}, $${p.costUsd.toFixed(2)}` : "…"}
            {p && p.failed > 0 ? `, ${count(p.failed)} failed` : ""}
          </span>
          {!state.cancelling && (
            <CancelButton
              onCancel={() => {
                set({ ...state, cancelling: true });
                void commands.taggingCancel();
              }}
            />
          )}
        </span>
      );
    }
    case "done": {
      const s = state.summary;
      return (
        <Dismissable onDismiss={idle}>
          {s.cancelled ? "Tagging cancelled" : "Tagging finished"}: {count(s.tagged)} tagged
          {s.fromCache > 0 ? `, ${count(s.fromCache)} from cache` : ""}
          {s.skipped > 0 ? `, ${count(s.skipped)} already done` : ""}
          {s.failed > 0 ? `, ${count(s.failed)} failed` : ""}, ${s.costUsd.toFixed(2)}
          {s.errors.length > 0 && (
            <span className="text-destructive" title={s.errors.join("\n")}>
              {" "}
              (hover for errors)
            </span>
          )}
        </Dismissable>
      );
    }
    case "failed":
      return (
        <Dismissable onDismiss={idle}>
          <span className="text-destructive">{state.message}</span>
        </Dismissable>
      );
  }
}

function ModeBadge() {
  const prefs = usePreferences();
  const api = prefs.data?.aiMode === "api";
  return (
    <button
      type="button"
      title={
        api ? "Chat text is sent to Anthropic when you run tagging" : "Nothing leaves this machine"
      }
      onClick={() => {
        useUiStore.getState().setSettingsOpen(true);
      }}
      className={cn(
        "rounded-sm px-1.5 py-0.5",
        api ? "bg-amber-500/20 text-amber-200" : "bg-secondary text-secondary-foreground",
      )}
    >
      {api ? "API mode" : "Local only"}
    </button>
  );
}

export function StatusBar() {
  const queryClient = useQueryClient();
  const info = useQuery({ queryKey: ["app-info"], queryFn: commands.appInfo });
  const running = useJobRunning();
  const importIdle = useImportStore((s) => s.state.phase === "idle");
  const taggingIdle = useTaggingStore((s) => s.state.phase === "idle");
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
      <ModeBadge />
      <span className="flex min-w-0 flex-1 items-center gap-4">
        <ImportStatus />
        <TaggingStatus />
        {importIdle && taggingIdle && <span>No jobs running</span>}
      </span>
      {lock.isError && <span className="text-destructive">{errorMessage(lock.error)}</span>}
      <button
        type="button"
        className="flex items-center gap-1 rounded-sm px-1 hover:bg-accent"
        onClick={() => {
          useUiStore.getState().setSettingsOpen(true);
        }}
      >
        <Settings className="size-3" /> Settings
      </button>
      <button
        type="button"
        className="flex items-center gap-1 rounded-sm px-1 hover:bg-accent disabled:opacity-50"
        disabled={running || lock.isPending}
        title={running ? "Wait for the running job to finish" : "Lock"}
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
