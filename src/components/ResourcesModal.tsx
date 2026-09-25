import { useEffect, useState } from "react";
import { Activity, Loader2, X } from "lucide-react";
import { getResourceUsage } from "../lib/tauri-api";
import type { ResourceUsage } from "../types";

interface Props {
  /** profileId → display name. */
  names: Record<string, string>;
  onClose: () => void;
}

function formatMemory(kb: number): string {
  if (kb >= 1024 * 1024) return `${(kb / 1024 / 1024).toFixed(1)} GiB`;
  if (kb >= 1024) return `${(kb / 1024).toFixed(0)} MiB`;
  return `${kb} KiB`;
}

/**
 * Live RAM/CPU per running browser (feature 2). Polls the backend every
 * 3 s — /proc sampling is cheap, but the CPU% is a delta between samples,
 * so a slower cadence only smooths the number, never loses data.
 */
export function ResourcesModal({ names, onClose }: Props) {
  const [usage, setUsage] = useState<ResourceUsage[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const list = await getResourceUsage();
        if (!cancelled) {
          setUsage(list.sort((a, b) => b.memoryKb - a.memoryKb));
          setError(null);
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    };
    poll();
    const timer = setInterval(poll, 3000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-4">
      <div className="w-full max-w-md rounded-xl border border-neutral-200 bg-white p-6 shadow-xl dark:border-neutral-800 dark:bg-neutral-900">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-base font-semibold">
            <Activity className="h-4 w-4" /> Resource usage
          </h2>
          <button onClick={onClose} className="text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-300">
            <X className="h-5 w-5" />
          </button>
        </div>

        {usage === null && !error && (
          <div className="flex justify-center py-10 text-neutral-400">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        )}

        {error && (
          <p className="rounded-lg border border-red-300 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-200">
            {error}
          </p>
        )}

        {usage !== null && usage.length === 0 && (
          <p className="py-6 text-center text-sm text-neutral-500">
            No browsers running right now.
          </p>
        )}

        {usage !== null && usage.length > 0 && (
          <div className="space-y-2">
            {usage.map((u) => (
              <div
                key={u.profileId}
                className="flex items-center justify-between rounded-lg border border-neutral-200 px-3 py-2 text-sm dark:border-neutral-800"
              >
                <span className="min-w-0 truncate font-medium" title={names[u.profileId] ?? u.profileId}>
                  {names[u.profileId] ?? u.profileId}
                </span>
                <span className="ml-3 flex shrink-0 items-center gap-3 font-mono text-xs text-neutral-500 dark:text-neutral-400">
                  <span title="Resident memory (whole process tree)">🧠 {formatMemory(u.memoryKb)}</span>
                  <span className="w-14 text-right" title="CPU since the previous sample">
                    {u.cpuPercent == null ? "—" : `${u.cpuPercent.toFixed(1)}%`}
                  </span>
                </span>
              </div>
            ))}
            <p className="pt-1 text-xs text-neutral-400 dark:text-neutral-500">
              Refreshes every 3 s. Memory covers the whole browser process tree.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
