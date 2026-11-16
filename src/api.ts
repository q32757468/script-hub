import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Snapshot } from "./domain";
export const desktop = isTauri();
export function call<T = void>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!desktop)
    return Promise.reject(
      new Error("请在桌面应用中使用此功能：pnpm tauri dev"),
    );
  return invoke<T>(command, args);
}
export const emptySnapshot: Snapshot = {
  config: {
    version: 1,
    settings: { launchAtLogin: false },
    tasks: [],
    files: [],
  },
  root: "~/.script-hub",
  configError: null,
  runtimeError: null,
  appError: null,
  runs: [],
};
