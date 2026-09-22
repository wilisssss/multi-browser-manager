import { useEffect, useState } from "react";
import { History, Loader2, X } from "lucide-react";
import { getLaunchHistory, type HistoryEntry } from "../lib/tauri-api";

interface Props {
  onClose: () => void;
  onRefreshKey: number; // bump to refetch
}

function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleString();
}

function formatDuration(from: number, to: number): string {
  const secs = Math.max(0, Math.round(to - from));
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ${secs % 60}s`;
  return `${Math.floor(mins / 60)}h ${mins % 60}m`;
}

export function HistoryModal({ onClose, onRefreshKey }: Props) {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getLaunchHistory(100)
      .then((list) => {
        if (!cancelled) {
          setEntries(list);
          setError(null);
        }
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [onRefreshKey]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4">
      <div className="flex max-h-[85vh] w-full max-w-2xl flex-col rounded-xl border border-neutral-200 bg-white shadow-xl dark:border-neutral-800 dark:bg-neutral-900">
        <div className="flex items-center justify-between border-b border-neutral-200 px-6 py-4 dark:border-neutral-800">
          <h2 className="flex items-center gap-2 text-base font-semibold">
            <History className="h-4 w-4" /> Launch History
          </h2>
          <button onClick={onClose} className="text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-300">
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-6 py-4">
          {loading ? (
            <div className="flex items-center justify-center py-12 text-neutral-500">
              <Loader2 className="mr-2 h-5 w-5 animate-spin" /> Loading...
            </div>
          ) : error ? (
            <div className="rounded-lg border border-red-300 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-200">
              {error}
            </div>
          ) : entries.length === 0 ? (
            <p className="py-12 text-center text-sm text-neutral-500">
              No launches recorded yet.
            </p>
          ) : (
            <ul className="space-y-1.5">
              {entries.map((entry) => (
                <li
                  key={entry.id}
                  className="flex items-center justify-between gap-3 rounded-lg border border-neutral-200 px-4 py-2.5 text-sm dark:border-neutral-800"
                >
                  <div className="min-w-0">
                    <p className="truncate font-medium">
                      {entry.profileName ?? (
                        <span className="italic text-neutral-400">deleted profile</span>
                      )}
                    </p>
                    <p className="text-xs text-neutral-500">
                      {formatTime(entry.launchedAt)}
                      {entry.pid ? ` · pid ${entry.pid}` : ""}
                    </p>
                  </div>
                  <span
                    className={`shrink-0 rounded-md px-2 py-1 text-xs ${
                      entry.closedAt
                        ? "bg-neutral-100 text-neutral-600 dark:bg-neutral-800 dark:text-neutral-300"
                        : "bg-green-100 text-green-700 dark:bg-green-950 dark:text-green-400"
                    }`}
                  >
                    {entry.closedAt
                      ? `${formatDuration(entry.launchedAt, entry.closedAt)}`
                      : "running"}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
