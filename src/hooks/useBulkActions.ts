import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { listen } from "@tauri-apps/api/event";
import { bulkLaunch, bulkStop } from "../lib/tauri-api";
import type { Profile } from "../types";

/** Progress of an in-flight bulk action, fed by backend `bulk-progress` events. */
export interface BulkProgress {
  action: "launch" | "stop";
  done: number;
  total: number;
}

interface Options {
  profiles: Profile[];
  /** Profiles currently visible after search/filters. */
  filtered: Profile[];
  refresh: () => Promise<unknown>;
  setError: (message: string | null) => void;
  setBusy: Dispatch<SetStateAction<boolean>>;
}

/** Bulk launch/stop of the visible profiles, with live backend progress. */
export function useBulkActions({ profiles, filtered, refresh, setError, setBusy }: Options) {
  const [bulk, setBulk] = useState<BulkProgress | null>(null);

  // Live progress for bulk launch/stop (fires once per profile completed).
  useEffect(() => {
    const unlistenPromise = listen<BulkProgress & { profileId: string; success: boolean }>(
      "bulk-progress",
      (event) => {
        const { action, done, total } = event.payload;
        setBulk({ action, done, total });
      },
    );
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  const launchAllStopped = async () => {
    const ids = filtered.filter((p) => p.status === "stopped").map((p) => p.id);
    if (ids.length === 0) return;
    setBusy(true);
    setBulk({ action: "launch", done: 0, total: ids.length });
    try {
      const results = await bulkLaunch(ids);
      await refresh();
      reportBulkFailures(results, "launch");
    } catch (e) {
      setError(String(e));
    } finally {
      setBulk(null);
      setBusy(false);
    }
  };

  const stopAllRunning = async () => {
    const ids = filtered.filter((p) => p.status === "running").map((p) => p.id);
    if (ids.length === 0) return;
    setBusy(true);
    setBulk({ action: "stop", done: 0, total: ids.length });
    try {
      const results = await bulkStop(ids);
      await refresh();
      reportBulkFailures(results, "stop");
    } catch (e) {
      setError(String(e));
    } finally {
      setBulk(null);
      setBusy(false);
    }
  };

  /** Surfaces per-profile bulk failures that the backend collected. */
  const reportBulkFailures = (
    results: { profileId: string; success: boolean; message: string }[],
    action: string,
  ) => {
    const failures = results.filter((r) => !r.success);
    if (failures.length === 0) return;
    const names = failures
      .map((f) => {
        const p = profiles.find((x) => x.id === f.profileId);
        return p?.name ?? f.profileId;
      })
      .join(", ");
    const firstError = failures[0].message;
    setError(
      `Failed to ${action} ${failures.length} profile(s): ${names}. First error: ${firstError}`,
    );
  };

  return { bulk, launchAllStopped, stopAllRunning };
}
