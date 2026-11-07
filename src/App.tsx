import { QueryClientProvider } from "@tanstack/react-query";
import { AppShell } from "@/components/layout/AppShell";
import { UpdateBanner } from "@/components/updater/UpdateBanner";
import { useStartupUpdateCheck } from "@/features/updater/useStartupUpdateCheck";
import { VaultGate } from "@/features/vault/VaultGate";
import { queryClient } from "@/lib/queryClient";

export default function App() {
  // Outside the vault gate, so an update can be offered before unlocking.
  useStartupUpdateCheck();
  return (
    <QueryClientProvider client={queryClient}>
      <div className="flex h-full flex-col">
        <UpdateBanner />
        <div className="min-h-0 flex-1">
          <VaultGate>
            <AppShell />
          </VaultGate>
        </div>
      </div>
    </QueryClientProvider>
  );
}
