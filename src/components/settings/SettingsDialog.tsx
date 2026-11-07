import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useId } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { checkForUpdate, useUpdateStore } from "@/features/updater/updater";
import { commands, errorMessage } from "@/lib/commands";
import { queryKeys } from "@/lib/queryClient";
import { useImportStore } from "@/store/importStore";
import { useUiStore } from "@/store/uiStore";

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-3 border-t pt-4 first:border-t-0 first:pt-0">
      <h3 className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
        {title}
      </h3>
      {children}
    </section>
  );
}

function UpdateStatus() {
  const state = useUpdateStore((s) => s.state);
  switch (state.phase) {
    case "idle":
      return null;
    case "checking":
      return <span>Checking…</span>;
    case "none":
      return <span>You have the latest version.</span>;
    case "available":
      return (
        <span>Version {state.update.version} is available. See the banner to install it.</span>
      );
    case "installing":
      return <span>Installing version {state.update.version}…</span>;
    case "error":
      return <span className="text-destructive">{state.message}</span>;
  }
}

export function SettingsDialog() {
  const open = useUiStore((s) => s.settingsOpen);
  const setOpen = useUiStore((s) => s.setSettingsOpen);
  const switchId = useId();
  const queryClient = useQueryClient();
  const busy = useImportStore((s) => s.state.phase === "running");
  const checking = useUpdateStore((s) => s.state.phase === "checking");

  const info = useQuery({ queryKey: ["app-info"], queryFn: commands.appInfo });
  const prefs = useQuery({ queryKey: queryKeys.preferences, queryFn: commands.getPreferences });
  const savePrefs = useMutation({
    mutationFn: commands.setPreferences,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.preferences });
    },
  });
  const backup = useMutation({ mutationFn: commands.backupNow });
  const exportEdits = useMutation({ mutationFn: commands.exportEdits });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>About, updates and your data.</DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4 text-sm">
          <Section title="About">
            <p>
              Memory Organiser {info.data ? `version ${info.data.version}` : ""}
              <br />
              <span className="text-muted-foreground">
                Mode: local only. Nothing leaves this machine except the update check, which sends
                nothing about you or your chats.
              </span>
            </p>
          </Section>

          <Section title="Updates">
            <div className="flex items-center justify-between gap-4">
              <Label htmlFor={switchId}>Check for updates when the app starts</Label>
              <Switch
                id={switchId}
                checked={prefs.data?.checkUpdatesOnStartup ?? true}
                disabled={!prefs.data || savePrefs.isPending}
                onCheckedChange={(checked) => {
                  savePrefs.mutate({ ...prefs.data, checkUpdatesOnStartup: checked });
                }}
              />
            </div>
            {savePrefs.isError && (
              <p className="text-xs text-destructive">{errorMessage(savePrefs.error)}</p>
            )}
            <div className="flex items-center gap-3">
              <Button
                size="sm"
                variant="outline"
                disabled={checking}
                onClick={() => {
                  void checkForUpdate({ manual: true });
                }}
              >
                Check for updates
              </Button>
              <span className="text-xs text-muted-foreground">
                <UpdateStatus />
              </span>
            </div>
          </Section>

          <Section title="Your data">
            <p className="text-xs text-muted-foreground">
              A backup is an encrypted copy of the whole database, kept next to it. Exported edits
              are your own decisions (chat states and exclusions) as a plain JSON file.{" "}
              <strong className="text-foreground">That file is not encrypted</strong> and includes
              chat titles.
            </p>
            <div className="flex flex-wrap gap-2">
              <Button
                size="sm"
                variant="outline"
                disabled={busy || backup.isPending}
                onClick={() => {
                  backup.mutate();
                }}
              >
                Back up now
              </Button>
              <Button
                size="sm"
                variant="outline"
                disabled={exportEdits.isPending}
                onClick={() => {
                  exportEdits.mutate();
                }}
              >
                Export my edits
              </Button>
            </div>
            <p className="text-xs text-muted-foreground" aria-live="polite">
              {busy && "Backups are unavailable while an import is running. "}
              {backup.isSuccess && `Backed up to ${backup.data}. `}
              {backup.isError && (
                <span className="text-destructive">{errorMessage(backup.error)} </span>
              )}
              {exportEdits.isSuccess && exportEdits.data && "Edits exported. "}
              {exportEdits.isError && (
                <span className="text-destructive">{errorMessage(exportEdits.error)}</span>
              )}
            </p>
          </Section>
        </div>
      </DialogContent>
    </Dialog>
  );
}
