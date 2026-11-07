import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { events, isAppError } from "@/lib/commands";
import { queryKeys } from "@/lib/queryClient";
import { useImportStore } from "@/store/importStore";
import type { ImportProgress, ImportSummary } from "@/types/app";

/** Subscribes once to import events from Rust and mirrors them into the store. */
export function useImportEvents(): void {
  const queryClient = useQueryClient();

  useEffect(() => {
    const { set } = useImportStore.getState();
    const unlisten = Promise.all([
      listen<ImportProgress>(events.importProgress, (e) => {
        const current = useImportStore.getState().state;
        set({
          phase: "running",
          progress: e.payload,
          cancelling: current.phase === "running" && current.cancelling,
        });
      }),
      listen<ImportSummary>(events.importFinished, (e) => {
        set({ phase: "done", summary: e.payload });
        void queryClient.invalidateQueries({ queryKey: queryKeys.libraryStats });
      }),
      listen<unknown>(events.importFailed, (e) => {
        const err = e.payload;
        set({
          phase: "failed",
          message: isAppError(err) ? err.message : "The import failed.",
          cancelled: isAppError(err) && err.kind === "cancelled",
        });
      }),
    ]);
    return () => {
      void unlisten.then((fns) => {
        fns.forEach((fn) => {
          fn();
        });
      });
    };
  }, [queryClient]);
}
