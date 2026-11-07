import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle } from "lucide-react";
import { useId, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { usePreferences, useTaggingStats } from "@/features/tagging/queries";
import { commands, errorMessage } from "@/lib/commands";
import { count } from "@/lib/format";
import { queryKeys } from "@/lib/queryClient";
import { cn } from "@/lib/utils";
import { useImportStore } from "@/store/importStore";
import { useTaggingStore } from "@/store/taggingStore";
import type { Estimate, Preferences, TagScope } from "@/types/app";

function usd(n: number): string {
  return n < 0.01 && n > 0 ? "under $0.01" : `$${n.toFixed(2)}`;
}

function ModeChoice({ prefs }: { prefs: Preferences }) {
  const queryClient = useQueryClient();
  const [confirming, setConfirming] = useState(false);
  const save = useMutation({
    mutationFn: commands.setPreferences,
    onSuccess: () => {
      setConfirming(false);
      void queryClient.invalidateQueries({ queryKey: queryKeys.preferences });
    },
  });

  const option = (value: "off" | "api", title: string, detail: string) => (
    <button
      type="button"
      role="radio"
      aria-checked={prefs.aiMode === value}
      onClick={() => {
        if (value === "api" && prefs.aiMode !== "api") setConfirming(true);
        else save.mutate({ ...prefs, aiMode: value });
      }}
      className={cn(
        "flex-1 rounded-md border p-2 text-left",
        prefs.aiMode === value ? "border-foreground/40 bg-secondary" : "hover:bg-accent",
      )}
    >
      <span className="block text-sm font-medium">{title}</span>
      <span className="block text-xs text-muted-foreground">{detail}</span>
    </button>
  );

  return (
    <div className="flex flex-col gap-2">
      <div className="flex gap-2" role="radiogroup" aria-label="AI mode">
        {option("off", "Local only", "Nothing leaves this machine. No tagging.")}
        {option("api", "Anthropic API", "Chat text is sent to Anthropic for tagging.")}
      </div>
      {confirming && (
        <div className="flex flex-col gap-2 rounded-md border border-amber-500/50 bg-amber-500/10 p-3 text-xs">
          <p className="flex gap-2">
            <AlertTriangle className="size-4 shrink-0 text-amber-500" />
            <span>
              In API mode, the full text of every chat you tag (including pasted attachments) is
              sent to Anthropic&apos;s API. Chats you mark &ldquo;Never send to AI&rdquo; in the
              reader are always skipped. Nothing is sent until you start a run.
            </span>
          </p>
          <div className="flex gap-2">
            <Button
              size="sm"
              onClick={() => {
                save.mutate({ ...prefs, aiMode: "api" });
              }}
            >
              Turn on API mode
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                setConfirming(false);
              }}
            >
              Cancel
            </Button>
          </div>
        </div>
      )}
      {save.isError && <p className="text-xs text-destructive">{errorMessage(save.error)}</p>}
    </div>
  );
}

function ApiKey() {
  const queryClient = useQueryClient();
  const inputId = useId();
  const [key, setKey] = useState("");
  const present = useQuery({ queryKey: queryKeys.apiKey, queryFn: commands.apiKeyPresent });
  const done = () => {
    setKey("");
    void queryClient.invalidateQueries({ queryKey: queryKeys.apiKey });
  };
  const save = useMutation({ mutationFn: commands.setApiKey, onSuccess: done });
  const clear = useMutation({ mutationFn: commands.clearApiKey, onSuccess: done });

  if (present.data) {
    return (
      <div className="flex items-center justify-between gap-2 text-xs">
        <span className="text-muted-foreground">API key saved in the system keychain.</span>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            clear.mutate();
          }}
        >
          Remove key
        </Button>
      </div>
    );
  }
  return (
    <form
      className="flex flex-col gap-1.5"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate(key);
      }}
    >
      <Label htmlFor={inputId}>Anthropic API key</Label>
      <div className="flex gap-2">
        <Input
          id={inputId}
          type="password"
          autoComplete="off"
          placeholder="sk-ant-…"
          value={key}
          onChange={(e) => {
            setKey(e.target.value);
          }}
          className="h-8"
        />
        <Button size="sm" type="submit" disabled={!key || save.isPending}>
          Save
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        Stored in the system keychain, never in a file.
      </p>
      {(save.isError || present.isError) && (
        <p className="text-xs text-destructive">{errorMessage(save.error ?? present.error)}</p>
      )}
    </form>
  );
}

function EstimateLine({ e }: { e: Estimate }) {
  return (
    <p className="text-xs text-muted-foreground">
      {count(e.conversations)} chats: {count(e.toSend)} to send
      {e.cached > 0 && `, ${count(e.cached)} from cache`}
      {e.alreadyTagged > 0 && `, ${count(e.alreadyTagged)} already tagged`}. About{" "}
      {count(Math.round(e.inputTokens / 1000))}k tokens in and{" "}
      {count(Math.round(e.outputTokens / 1000))}k out on {e.model}:{" "}
      <strong className="text-foreground">{usd(e.costUsd)}</strong>
      {e.calibrated ? " (based on your earlier runs)" : " (rough until a run has measured it)"}.
    </p>
  );
}

function RunControls({ ready }: { ready: boolean }) {
  const [estimate, setEstimate] = useState<{ scope: TagScope; e: Estimate } | null>(null);
  const tagging = useTaggingStore((s) => s.state);
  const importing = useImportStore((s) => s.state.phase === "running");
  const stats = useTaggingStats();
  const estimateMutation = useMutation({
    mutationFn: async (scope: TagScope) => ({ scope, e: await commands.taggingEstimate(scope) }),
    onSuccess: setEstimate,
  });
  const start = useMutation({
    mutationFn: commands.taggingStart,
    onSuccess: () => {
      setEstimate(null);
      useTaggingStore.getState().set({ phase: "running", progress: null, cancelling: false });
    },
  });
  const busy = tagging.phase === "running" || importing;

  return (
    <div className="flex flex-col gap-2">
      {stats.data && stats.data.taggedConversations > 0 && (
        <p className="text-xs text-muted-foreground">
          {count(stats.data.taggedConversations)} chats tagged into {count(stats.data.themes)}{" "}
          themes. Spent so far: {usd(stats.data.totalCostUsd)}.
        </p>
      )}
      <div className="flex flex-wrap gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={!ready || busy || estimateMutation.isPending}
          onClick={() => {
            estimateMutation.mutate("sample");
          }}
        >
          Tag a sample of 20 chats…
        </Button>
        <Button
          size="sm"
          variant="outline"
          disabled={!ready || busy || estimateMutation.isPending}
          onClick={() => {
            estimateMutation.mutate("all");
          }}
        >
          Tag all chats…
        </Button>
      </div>
      {estimate && (
        <div className="flex flex-col gap-2 rounded-md border p-3">
          <EstimateLine e={estimate.e} />
          <div className="flex gap-2">
            <Button
              size="sm"
              disabled={busy || start.isPending || estimate.e.toSend + estimate.e.cached === 0}
              onClick={() => {
                start.mutate(estimate.scope);
              }}
            >
              {estimate.e.toSend + estimate.e.cached === 0
                ? "Nothing to do"
                : `Start (${usd(estimate.e.costUsd)})`}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                setEstimate(null);
              }}
            >
              Cancel
            </Button>
          </div>
        </div>
      )}
      {(estimateMutation.isError || start.isError) && (
        <p className="text-xs text-destructive">
          {errorMessage(estimateMutation.error ?? start.error)}
        </p>
      )}
    </div>
  );
}

export function AiSection() {
  const prefs = usePreferences();
  const key = useQuery({ queryKey: queryKeys.apiKey, queryFn: commands.apiKeyPresent });
  if (!prefs.data) return null;
  const api = prefs.data.aiMode === "api";
  return (
    <div className="flex flex-col gap-3">
      <ModeChoice prefs={prefs.data} />
      {api && <ApiKey />}
      {api && <RunControls ready={key.data === true} />}
    </div>
  );
}
