import { useEffect } from "react";
import { commands } from "@/lib/commands";
import { checkForUpdate } from "./updater";

/** Checks once at startup, in the background, unless switched off. */
export function useStartupUpdateCheck(): void {
  useEffect(() => {
    let cancelled = false;
    void commands
      .getPreferences()
      .then((prefs) => {
        if (!cancelled && prefs.checkUpdatesOnStartup) void checkForUpdate({ manual: false });
      })
      .catch(() => {
        // Preferences fall back to defaults in Rust; a bridge failure here
        // just means no startup check this time.
      });
    return () => {
      cancelled = true;
    };
  }, []);
}
