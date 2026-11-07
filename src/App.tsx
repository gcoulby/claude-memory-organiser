import { QueryClientProvider } from "@tanstack/react-query";
import { AppShell } from "@/components/layout/AppShell";
import { VaultGate } from "@/features/vault/VaultGate";
import { queryClient } from "@/lib/queryClient";

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <VaultGate>
        <AppShell />
      </VaultGate>
    </QueryClientProvider>
  );
}
