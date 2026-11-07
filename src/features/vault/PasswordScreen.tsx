import { useMutation, useQueryClient } from "@tanstack/react-query";
import { LockKeyhole } from "lucide-react";
import { useId, useState, type SyntheticEvent } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { commands, errorMessage } from "@/lib/commands";
import { queryKeys } from "@/lib/queryClient";

const MIN_LENGTH = 8;

interface PasswordScreenProps {
  mode: "create" | "unlock";
}

export function PasswordScreen({ mode }: PasswordScreenProps) {
  const queryClient = useQueryClient();
  const passwordId = useId();
  const confirmId = useId();
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);

  const submit = useMutation({
    mutationFn: (pw: string) =>
      mode === "create" ? commands.vaultCreate(pw) : commands.vaultUnlock(pw),
    onSuccess: () => {
      setPassword("");
      setConfirm("");
      void queryClient.invalidateQueries({ queryKey: queryKeys.vaultStatus });
    },
  });

  const onSubmit = (e: SyntheticEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLocalError(null);
    if (mode === "create") {
      if (password.length < MIN_LENGTH) {
        setLocalError(`Use at least ${MIN_LENGTH} characters.`);
        return;
      }
      if (password !== confirm) {
        setLocalError("The passwords do not match.");
        return;
      }
    }
    submit.mutate(password);
  };

  const error = localError ?? (submit.error ? errorMessage(submit.error) : null);

  return (
    <div className="flex h-full items-center justify-center p-6">
      <form onSubmit={onSubmit} className="flex w-full max-w-sm flex-col gap-4">
        <div className="flex flex-col items-center gap-2 text-center">
          <LockKeyhole className="size-8 text-muted-foreground" aria-hidden />
          <h1 className="text-lg font-semibold">
            {mode === "create" ? "Set a password" : "Unlock Memory Organiser"}
          </h1>
          <p className="text-sm text-muted-foreground">
            {mode === "create"
              ? "Your chats are stored encrypted with this password."
              : "Enter your password to decrypt your chats."}
          </p>
        </div>

        <div className="flex flex-col gap-2">
          <Label htmlFor={passwordId}>Password</Label>
          <Input
            id={passwordId}
            type="password"
            autoFocus
            autoComplete={mode === "create" ? "new-password" : "current-password"}
            value={password}
            onChange={(e) => {
              setPassword(e.target.value);
            }}
          />
        </div>

        {mode === "create" && (
          <div className="flex flex-col gap-2">
            <Label htmlFor={confirmId}>Confirm password</Label>
            <Input
              id={confirmId}
              type="password"
              autoComplete="new-password"
              value={confirm}
              onChange={(e) => {
                setConfirm(e.target.value);
              }}
            />
          </div>
        )}

        {mode === "create" && (
          <p className="rounded-md border border-destructive/40 p-3 text-xs text-muted-foreground">
            There is no way to recover this password. If you forget it, your organised data cannot
            be decrypted and your edits are lost. Keep it somewhere safe.
          </p>
        )}

        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}

        <Button type="submit" disabled={submit.isPending || password.length === 0}>
          {submit.isPending
            ? mode === "create"
              ? "Creating…"
              : "Unlocking…"
            : mode === "create"
              ? "Create encrypted store"
              : "Unlock"}
        </Button>
      </form>
    </div>
  );
}
