import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/lib/commands";
import { queryKeys } from "@/lib/queryClient";
import type { MemoryStatus } from "@/types/app";

export function usePreferences() {
  return useQuery({ queryKey: queryKeys.preferences, queryFn: commands.getPreferences });
}

export function useThemes() {
  return useQuery({ queryKey: queryKeys.themes, queryFn: commands.listThemes });
}

export function useThemeSegments(themeId: number | null) {
  return useQuery({
    queryKey: [...queryKeys.themeSegments, themeId],
    queryFn: () => commands.themeSegments(themeId ?? 0),
    enabled: themeId !== null,
  });
}

export function useConversationSegments(conversationId: number | null) {
  return useQuery({
    queryKey: [...queryKeys.conversationSegments, conversationId],
    queryFn: () => commands.conversationSegments(conversationId ?? 0),
    enabled: conversationId !== null,
  });
}

export function useMemory() {
  return useQuery({ queryKey: queryKeys.memory, queryFn: commands.listMemory });
}

export function useTaggingStats() {
  return useQuery({ queryKey: queryKeys.taggingStats, queryFn: commands.taggingStats });
}

export function useSetMemoryStatus() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, status }: { id: number; status: MemoryStatus }) =>
      commands.setMemoryStatus(id, status),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.memory });
      void queryClient.invalidateQueries({ queryKey: queryKeys.taggingStats });
    },
  });
}

export function useSetExcluded() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ ids, excluded }: { ids: number[]; excluded: boolean }) =>
      commands.setConversationExcluded(ids, excluded),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.conversation });
    },
  });
}
