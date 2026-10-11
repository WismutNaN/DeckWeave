// SPDX-License-Identifier: GPL-3.0-or-later
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface HostSnapshot {
  lifecycle: "running" | "stopping";
  mode: "installed" | "portable";
  dataRoot: string;
  version: string;
  notice: string | null;
}

export const native = isTauri();
export const getSnapshot = () => invoke<HostSnapshot>("host_snapshot");
export const getAutostart = () => invoke<boolean>("autostart_enabled");
export const setAutostart = (enabled: boolean) =>
  invoke<void>("set_autostart", { enabled });
export const exitApp = () => invoke<void>("quit_app");
export const onHostChanged = (refresh: () => void) =>
  listen("host-changed", refresh);
