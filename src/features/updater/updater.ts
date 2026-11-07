import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { create } from "zustand";
import { errorMessage } from "@/lib/commands";

export type UpdateState =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "none"; checkedAt: number }
  | { phase: "available"; update: Update; dismissed: boolean }
  | { phase: "installing"; update: Update; downloaded: number; total: number | null }
  | { phase: "error"; message: string };

interface UpdateStore {
  state: UpdateState;
  set: (state: UpdateState) => void;
}

export const useUpdateStore = create<UpdateStore>((set) => ({
  state: { phase: "idle" },
  set: (state) => {
    set({ state });
  },
}));

/**
 * Asks GitHub for a newer signed release. A startup check stays silent when
 * offline or failing; a manual check reports the error.
 */
export async function checkForUpdate({ manual }: { manual: boolean }): Promise<void> {
  const { state, set } = useUpdateStore.getState();
  if (state.phase === "checking" || state.phase === "installing") return;
  set({ phase: "checking" });
  try {
    const update = await check();
    set(
      update
        ? { phase: "available", update, dismissed: false }
        : { phase: "none", checkedAt: Date.now() },
    );
  } catch (err) {
    set(manual ? { phase: "error", message: errorMessage(err) } : { phase: "idle" });
  }
}

/** Downloads, verifies the signature, installs and restarts. */
export async function installUpdate(update: Update): Promise<void> {
  const { set } = useUpdateStore.getState();
  let downloaded = 0;
  let total: number | null = null;
  set({ phase: "installing", update, downloaded, total });
  try {
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? null;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
      }
      set({ phase: "installing", update, downloaded, total });
    });
    await relaunch();
  } catch (err) {
    set({ phase: "error", message: `The update could not be installed: ${errorMessage(err)}` });
  }
}
