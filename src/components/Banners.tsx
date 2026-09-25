import { X } from "lucide-react";
import type { UpdaterStatus } from "../hooks/useUpdater";

interface Props {
  error: string | null;
  onDismissError: () => void;
  status: UpdaterStatus;
  onInstallUpdate: () => void;
  onDismissUpdate: () => void;
}

/** Error banner + auto-updater banner, stacked below the header. */
export function Banners({ error, onDismissError, status, onInstallUpdate, onDismissUpdate }: Props) {
  const updaterBanner =
    status.state === "up-to-date"
      ? { tone: "ok" as const, text: status.message }
      : status.state === "available"
        ? { tone: "info" as const, text: `Update ${status.version} available.` }
        : status.state === "error"
          ? { tone: "err" as const, text: status.message }
          : null;

  if (!error && !updaterBanner) return null;

  return (
    <>
      {error && (
        <div className="mb-4 flex items-start justify-between gap-3 rounded-lg border border-red-300 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-200">
          <span>{error}</span>
          <button onClick={onDismissError}>
            <X className="h-4 w-4" />
          </button>
        </div>
      )}

      {updaterBanner && (
        <div
          className={`mb-4 flex items-start justify-between gap-3 rounded-lg border px-4 py-3 text-sm ${
            updaterBanner.tone === "err"
              ? "border-red-300 bg-red-50 text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-200"
              : updaterBanner.tone === "info"
                ? "border-blue-300 bg-blue-50 text-blue-700 dark:border-blue-900 dark:bg-blue-950/50 dark:text-blue-200"
                : "border-green-300 bg-green-50 text-green-700 dark:border-green-900 dark:bg-green-950/50 dark:text-green-200"
          }`}
        >
          <span>{updaterBanner.text}</span>
          <span className="flex shrink-0 items-center gap-2">
            {status.state === "available" && (
              <button
                onClick={onInstallUpdate}
                className="rounded-md bg-blue-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-blue-500"
              >
                Install & restart
              </button>
            )}
            <button onClick={onDismissUpdate}>
              <X className="h-4 w-4" />
            </button>
          </span>
        </div>
      )}
    </>
  );
}
