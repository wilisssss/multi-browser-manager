import { Chrome, Compass, Globe, Loader2, Play, Pencil, Copy, Trash2, Square, Pin, PinOff, KeyRound } from "lucide-react";
import { memo, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { formatLastUsed } from "../lib/format";
import type { Profile, Proxy } from "../types";

interface Props {
  profile: Profile;
  proxy: Proxy | null;
  onLaunch: (id: string) => Promise<void>;
  onStop: (id: string) => Promise<void>;
  onEdit: (profile: Profile) => void;
  onDelete: (id: string) => Promise<unknown>;
  onDuplicate: (id: string) => Promise<unknown>;
  onTogglePin: (id: string) => Promise<unknown>;
  onCredentials: (profile: Profile) => void;
  onError: (message: string) => void;
}

const BROWSER_ICONS: Record<string, typeof Globe> = {
  chrome: Chrome,
  chromium: Compass,
  brave: Globe,
  edge: Globe,
};

export const ProfileCard = memo(function ProfileCard({
  profile,
  proxy,
  onLaunch,
  onStop,
  onEdit,
  onDelete,
  onDuplicate,
  onTogglePin,
  onCredentials,
  onError,
}: Props) {
  const [busy, setBusy] = useState(false);
  const running = profile.status === "running";

  const Icon = BROWSER_ICONS[profile.browserType] ?? Globe;

  const handleToggle = async () => {
    setBusy(true);
    try {
      if (running) {
        await onStop(profile.id);
      } else {
        await onLaunch(profile.id);
      }
    } catch (e) {
      // Surface validation errors (e.g. "already running", proxy missing)
      // instead of letting them become unhandled rejections.
      onError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className={`group flex flex-col rounded-xl border p-5 shadow-sm transition-all dark:shadow-none ${
        running
          ? "border-green-500/60 bg-green-50/40 dark:border-green-800/60 dark:bg-green-950/20"
          : "border-neutral-200 bg-white hover:border-neutral-300 hover:shadow dark:border-neutral-800 dark:bg-neutral-900 dark:hover:border-neutral-700"
      }`}
    >
      <div className="mb-4 flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <div
            className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-lg ${
              running
                ? "bg-green-100 text-green-600 dark:bg-green-950 dark:text-green-400"
                : "bg-neutral-100 text-neutral-500 dark:bg-neutral-800 dark:text-neutral-400"
            }`}
          >
            <Icon className="h-5 w-5" />
          </div>
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold leading-snug" title={profile.name}>
              {profile.name}
            </h3>
            <p className="mt-0.5 text-xs capitalize text-neutral-500">{profile.browserType}</p>
          </div>
        </div>

        {/* Status dot */}
        <span
          className={`mt-2 h-2.5 w-2.5 shrink-0 rounded-full ${
            running ? "bg-green-500" : "bg-neutral-300 dark:bg-neutral-600"
          }`}
          title={running ? "Running" : "Stopped"}
        />
      </div>

      {/* Tags */}
      {profile.groups.length > 0 && (
        <div className="mb-3 flex flex-wrap gap-1.5">
          {profile.groups.map((g) => {
            const color = g.color ?? "#3b82f6";
            return (
              <span
                key={g.id}
                className="inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-xs"
                style={{ backgroundColor: `${color}22`, color }}
              >
                {g.name}
              </span>
            );
          })}
        </div>
      )}

      {/* Proxy badge */}
      {proxy && (
        <div className="mb-3 inline-flex max-w-full items-center gap-1.5 self-start rounded-md bg-neutral-100 px-2.5 py-1.5 text-xs text-neutral-700 dark:bg-neutral-800 dark:text-neutral-300">
          <Globe className="h-3 w-3 shrink-0" />
          <span className="truncate">
            {proxy.protocol}://{proxy.host}:{proxy.port}
          </span>
        </div>
      )}

      {profile.notes && (
        <p className="mb-3 line-clamp-2 text-xs leading-relaxed text-neutral-500" title={profile.notes}>
          {profile.notes}
        </p>
      )}

      <p className="mb-4 text-xs text-neutral-400 dark:text-neutral-500">
        Last used: {formatLastUsed(profile.lastUsedAt)}
      </p>

      {/* Actions — flex-wrap so the row can never overflow the card box. */}
      <div className="mt-auto flex flex-wrap items-center gap-x-1.5 gap-y-2 border-t border-neutral-100 pt-3.5 dark:border-neutral-800">
        <button
          onClick={handleToggle}
          disabled={busy}
          className={`flex min-w-0 flex-1 items-center justify-center gap-1.5 rounded-lg px-3 py-2 text-xs font-medium transition-colors disabled:opacity-50 ${
            running
              ? "bg-neutral-200 text-neutral-800 hover:bg-neutral-300 dark:bg-neutral-800 dark:text-neutral-200 dark:hover:bg-neutral-700"
              : "bg-blue-600 text-white hover:bg-blue-500"
          }`}
        >
          {busy ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : running ? (
            <Square className="h-3.5 w-3.5" />
          ) : (
            <Play className="h-3.5 w-3.5" />
          )}
          {running ? "Stop" : "Launch"}
        </button>

        <button
          onClick={() => onCredentials(profile)}
          title="Credentials (social accounts, wallets)"
          className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700 dark:hover:bg-neutral-800 dark:hover:text-neutral-200"
        >
          <KeyRound className="h-4 w-4" />
        </button>

        <button
          onClick={() => onTogglePin(profile.id)}
          title={profile.pinned ? "Unpin" : "Pin to top"}
          className={`inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg hover:bg-neutral-100 dark:hover:bg-neutral-800 ${
            profile.pinned
              ? "text-amber-500"
              : "text-neutral-400 hover:text-neutral-700 dark:hover:text-neutral-200"
          }`}
        >
          {profile.pinned ? <Pin className="h-4 w-4" /> : <PinOff className="h-4 w-4" />}
        </button>
        <button
          onClick={() => onEdit(profile)}
          title="Edit"
          className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700 dark:hover:bg-neutral-800 dark:hover:text-neutral-200"
        >
          <Pencil className="h-4 w-4" />
        </button>
        <button
          onClick={() => onDuplicate(profile.id)}
          title="Duplicate"
          className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700 dark:hover:bg-neutral-800 dark:hover:text-neutral-200"
        >
          <Copy className="h-4 w-4" />
        </button>
        <button
          onClick={async () => {
            const ok = await ask(
              `Delete profile "${profile.name}"?\n\nIts isolated data folder (cookies, sessions, cache) will be permanently wiped from disk.`,
              { title: "Delete profile", kind: "warning", okLabel: "Delete", cancelLabel: "Cancel" },
            );
            if (ok) await onDelete(profile.id);
          }}
          title="Delete"
          className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-neutral-400 hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-950 dark:hover:text-red-400"
        >
          <Trash2 className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
});
