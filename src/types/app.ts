/** Mirrors `AppError` in src-tauri/src/error.rs. */
export interface AppError {
  kind: string;
  message: string;
}

export interface AppInfo {
  name: string;
  version: string;
}
