import { useMutation } from "@tanstack/react-query";
import { commands, errorMessage } from "@/lib/commands";
import { useImportStore } from "@/store/importStore";

export function useStartImport() {
  return useMutation({
    mutationFn: commands.importChooseAndStart,
    // Remember the state before starting. A fast import can emit its
    // progress or finished event before this call resolves, and that newer
    // state must not be overwritten.
    onMutate: () => ({ before: useImportStore.getState().state }),
    onSuccess: (started, _vars, context) => {
      const { state, set } = useImportStore.getState();
      if (started && state === context.before) {
        set({ phase: "running", progress: null, cancelling: false });
      }
    },
    onError: (err) => {
      useImportStore
        .getState()
        .set({ phase: "failed", message: errorMessage(err), cancelled: false });
    },
  });
}
