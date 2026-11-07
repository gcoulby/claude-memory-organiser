import { invoke } from "@tauri-apps/api/core";
import type { AppError, AppInfo } from "@/types/app";

/** A failed command. `kind` comes from Rust, or is "bridge" when the call never reached it. */
export class CommandError extends Error {
  readonly kind: string;

  constructor({ kind, message }: AppError) {
    super(message);
    this.name = "CommandError";
    this.kind = kind;
  }
}

function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    typeof value.kind === "string" &&
    "message" in value &&
    typeof value.message === "string"
  );
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw new CommandError(isAppError(err) ? err : { kind: "bridge", message: String(err) });
  }
}

export const commands = {
  appInfo: () => call<AppInfo>("app_info"),
};
