import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Data only changes through our own commands, which invalidate as needed.
      staleTime: Infinity,
      refetchOnWindowFocus: false,
      retry: false,
    },
  },
});

export const queryKeys = {
  vaultStatus: ["vault-status"] as const,
  libraryStats: ["library-stats"] as const,
  conversations: ["conversations"] as const,
  stateCounts: ["state-counts"] as const,
  conversation: ["conversation"] as const,
  search: ["search"] as const,
  preferences: ["preferences"] as const,
};
