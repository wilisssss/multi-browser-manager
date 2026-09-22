import { useCallback, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdaterStatus =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "up-to-date"; message: string }
  | { state: "available"; version: string; notes: string }
  | { state: "downloading" }
  | { state: "installed" }
  | { state: "error"; message: string };

/**
 * Auto-updater flow:
 *  1. `check()` queries the configured endpoint (see tauri.conf.json → plugins.updater)
 *  2. If an update exists, the UI offers download + install
 *  3. After install the app relaunches via the process plugin
 *
 * Note: on Linux the updater supports AppImage bundles. The endpoint and pubkey
 * live in src-tauri/tauri.conf.json; releases must be signed with the private
 * key matching the configured pubkey.
 */
export function useUpdater() {
  const [status, setStatus] = useState<UpdaterStatus>({ state: "idle" });
  const [update, setUpdate] = useState<Update | null>(null);

  const checkForUpdates = useCallback(async () => {
    setStatus({ state: "checking" });
    try {
      const found = await check();
      if (found) {
        setUpdate(found);
        setStatus({
          state: "available",
          version: found.version,
          notes: found.body ?? "",
        });
      } else {
        setStatus({ state: "up-to-date", message: "You're on the latest version." });
      }
    } catch (e) {
      const msg = String(e);
      setStatus({
        state: "error",
        message: msg.includes("404") || msg.includes("Empty")
          ? "Updater not configured — set the release endpoint in tauri.conf.json."
          : `Update check failed: ${msg}`,
      });
    }
  }, []);

  const install = useCallback(async () => {
    if (!update) return;
    setStatus({ state: "downloading" });
    try {
      await update.downloadAndInstall();
      setStatus({ state: "installed" });
      await relaunch();
    } catch (e) {
      setStatus({ state: "error", message: `Install failed: ${String(e)}` });
    }
  }, [update]);

  const dismiss = useCallback(() => setStatus({ state: "idle" }), []);

  return { status, checkForUpdates, install, dismiss };
}
