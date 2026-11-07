import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/lib/commands";
import { queryKeys } from "@/lib/queryClient";
import type { InboxFilter } from "@/store/uiStore";
import type { ChatState } from "@/types/app";

export function useConversations(filter: InboxFilter) {
  return useQuery({
    queryKey: [...queryKeys.conversations, filter],
    queryFn: () => commands.listConversations(filter === "all" ? null : filter),
    placeholderData: keepPreviousData,
  });
}

export function useStateCounts() {
  return useQuery({ queryKey: queryKeys.stateCounts, queryFn: commands.stateCounts });
}

export function useConversation(id: number | null, via: string | null) {
  return useQuery({
    queryKey: [...queryKeys.conversation, id, via],
    queryFn: () => commands.getConversation(id ?? 0, via),
    enabled: id !== null,
  });
}

export function useSearch(query: string) {
  const trimmed = query.trim();
  return useQuery({
    queryKey: [...queryKeys.search, trimmed],
    queryFn: () => commands.searchMessages(trimmed),
    enabled: trimmed.length > 0,
    // Keeps the old results on screen while the next keystroke's arrive.
    placeholderData: keepPreviousData,
  });
}

export function useSetState() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ ids, state }: { ids: number[]; state: ChatState }) =>
      commands.setConversationState(ids, state),
    onSuccess: () => {
      for (const key of [queryKeys.conversations, queryKeys.stateCounts, queryKeys.conversation]) {
        void queryClient.invalidateQueries({ queryKey: key });
      }
    },
  });
}
