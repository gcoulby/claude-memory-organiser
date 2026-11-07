import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { events, isAppError } from "@/lib/commands";
import { queryKeys } from "@/lib/queryClient";
import { useTaggingStore } from "@/store/taggingStore";
import type { TagProgress, TagSummary } from "@/types/app";

const DERIVED = [
  queryKeys.themes,
  queryKeys.themeSegments,
  queryKeys.conversationSegments,
  queryKeys.memory,
  queryKeys.taggingStats,
];

/** Mirrors tagging events into the store, and refreshes derived views as results land. */
export function useTaggingEvents(): void {
  const queryClient = useQueryClient();

  useEffect(() => {
    const { set } = useTaggingStore.getState();
    const refresh = () => {
      for (const key of DERIVED) void queryClient.invalidateQueries({ queryKey: key });
    };
    const unlisten = Promise.all([
      listen<TagProgress>(events.taggingProgress, (e) => {
        const current = useTaggingStore.getState().state;
        set({
          phase: "running",
          progress: e.payload,
          cancelling: current.phase === "running" && current.cancelling,
        });
        refresh();
      }),
      listen<TagSummary>(events.taggingFinished, (e) => {
        set({ phase: "done", summary: e.payload });
        refresh();
      }),
      listen<unknown>(events.taggingFailed, (e) => {
        set({
          phase: "failed",
          message: isAppError(e.payload) ? e.payload.message : "Tagging failed.",
        });
        refresh();
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
