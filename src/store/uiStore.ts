import { create } from "zustand";
import type { ChatState } from "@/types/app";

export type InboxFilter = ChatState | "all";

interface Selection {
  conversationId: number;
  /** Show the branch through this message instead of the main path. */
  via: string | null;
  /** Scroll to and highlight this message. */
  focus: string | null;
}

interface UiStore {
  filter: InboxFilter;
  query: string;
  selection: Selection | null;
  paletteOpen: boolean;
  setFilter: (filter: InboxFilter) => void;
  setQuery: (query: string) => void;
  select: (selection: Selection | null) => void;
  setPaletteOpen: (open: boolean) => void;
}

export const useUiStore = create<UiStore>((set) => ({
  filter: "untriaged",
  query: "",
  selection: null,
  paletteOpen: false,
  setFilter: (filter) => {
    set({ filter });
  },
  setQuery: (query) => {
    set({ query });
  },
  select: (selection) => {
    set({ selection });
  },
  setPaletteOpen: (paletteOpen) => {
    set({ paletteOpen });
  },
}));

/** Id of the search box, so shortcuts and the palette can focus it. */
export const SEARCH_INPUT_ID = "inbox-search";

export function focusSearch(): void {
  const el = document.getElementById(SEARCH_INPUT_ID);
  if (el instanceof HTMLInputElement) {
    el.focus();
    el.select();
  }
}
