import { create } from "zustand";
import type { ImportProgress, ImportSummary } from "@/types/app";

type ImportState =
  | { phase: "idle" }
  | { phase: "running"; progress: ImportProgress | null; cancelling: boolean }
  | { phase: "done"; summary: ImportSummary }
  | { phase: "failed"; message: string; cancelled: boolean };

interface ImportStore {
  state: ImportState;
  set: (state: ImportState) => void;
}

export const useImportStore = create<ImportStore>((set) => ({
  state: { phase: "idle" },
  set: (state) => {
    set({ state });
  },
}));
