import { useQuery } from "@tanstack/react-query";
import { AlertTriangle } from "lucide-react";
import type { ReactNode } from "react";
import { EmptyState } from "@/components/layout/EmptyState";
import { commands } from "@/lib/commands";
import { queryKeys } from "@/lib/queryClient";
import { PasswordScreen } from "./PasswordScreen";

export function VaultGate({ children }: { children: ReactNode }) {
  const status = useQuery({ queryKey: queryKeys.vaultStatus, queryFn: commands.vaultStatus });

  if (status.isPending) {
    return <div className="h-full" aria-busy />;
  }
  if (status.isError) {
    return (
      <EmptyState
        icon={AlertTriangle}
        title="Could not check the encrypted store"
        description={status.error.message}
      />
    );
  }
  switch (status.data) {
    case "new":
      return <PasswordScreen mode="create" />;
    case "locked":
      return <PasswordScreen mode="unlock" />;
    case "unlocked":
      return <>{children}</>;
  }
}
