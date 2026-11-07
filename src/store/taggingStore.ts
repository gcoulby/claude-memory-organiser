import { create } from "zustand";
import type { TagProgress, TagSummary } from "@/types/app";

type TaggingState =
  | { phase: "idle" }
  | { phase: "running"; progress: TagProgress | null; cancelling: boolean }
  | { phase: "done"; summary: TagSummary }
  | { phase: "failed"; message: string };

interface TaggingStore {
  state: TaggingState;
  set: (state: TaggingState) => void;
}

export const useTaggingStore = create<TaggingStore>((set) => ({
  state: { phase: "idle" },
  set: (state) => {
    set({ state });
  },
}));
