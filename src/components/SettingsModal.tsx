import { useEffect, useState } from "react";
import { Loader2, Settings as SettingsIcon, X } from "lucide-react";
import { getBackupDir, getSettings, updateSettings } from "../lib/tauri-api";
import type { AppSettings } from "../types";

interface Props {
  onClose: () => void;
  /** Notifies the dashboard so it uses fresh values (e.g. default browser). */
  onSaved: (settings: AppSettings) => void;
}

export function SettingsModal({ onClose, onSaved }: Props) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [backupDir, setBackupDir] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getSettings()
      .then(setSettings)
      .catch((e) => setError(String(e)));
    getBackupDir()
      .then(setBackupDir)
      .catch(() => {});
  }, []);

  const set = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) =>
    setSettings((s) => (s ? { ...s, [key]: value } : s));

  const handleSave = async () => {
    if (!settings) return;
    setSaving(true);
    setError(null);
    try {
      const saved = await updateSettings(settings);
      onSaved(saved);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-4">
      <div className="w-full max-w-md rounded-xl border border-neutral-200 bg-white p-6 shadow-xl dark:border-neutral-800 dark:bg-neutral-900">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-base font-semibold">
            <SettingsIcon className="h-4 w-4" /> Settings
          </h2>
          <button onClick={onClose} className="text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-300">
            <X className="h-5 w-5" />
          </button>
        </div>

        {!settings ? (
          <div className="flex justify-center py-10 text-neutral-400">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : (
          <div className="space-y-5">
            {/* General */}
            <div>
              <label className="mb-1 block text-sm font-medium">Default browser for new profiles</label>
              <input
                value={settings.defaultBrowserType}
                onChange={(e) => set("defaultBrowserType", e.target.value)}
                placeholder="chromium"
                className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-700 dark:bg-neutral-900"
              />
            </div>

            {/* History */}
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="mb-1 block text-sm font-medium">History retention (days)</label>
                <input
                  type="number"
                  min={1}
                  max={3650}
                  value={settings.historyRetentionDays}
                  onChange={(e) => set("historyRetentionDays", Number(e.target.value))}
                  className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-700 dark:bg-neutral-900"
                />
              </div>
              <div>
                <label className="mb-1 block text-sm font-medium">History max entries</label>
                <input
                  type="number"
                  min={10}
                  max={100000}
                  value={settings.historyMaxEntries}
                  onChange={(e) => set("historyMaxEntries", Number(e.target.value))}
                  className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-700 dark:bg-neutral-900"
                />
              </div>
            </div>

            {/* Backups */}
            <div className="space-y-3">
              <label className="flex items-center gap-2 text-sm font-medium">
                <input
                  type="checkbox"
                  checked={settings.autoBackup}
                  onChange={(e) => set("autoBackup", e.target.checked)}
                  className="h-4 w-4 accent-blue-600"
                />
                Daily automatic backup (profiles & proxies, no passwords)
              </label>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="mb-1 block text-sm font-medium">Snapshots to keep</label>
                  <input
                    type="number"
                    min={1}
                    max={100}
                    value={settings.autoBackupKeep}
                    onChange={(e) => set("autoBackupKeep", Number(e.target.value))}
                    className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-700 dark:bg-neutral-900"
                  />
                </div>
              </div>
              {backupDir && (
                <p className="break-all font-mono text-xs text-neutral-400">Folder: {backupDir}</p>
              )}
            </div>

            {error && <p className="text-sm text-red-600 dark:text-red-400">{error}</p>}

            <div className="flex justify-end gap-2">
              <button
                onClick={onClose}
                className="rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"
              >
                Cancel
              </button>
              <button
                onClick={handleSave}
                disabled={saving}
                className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50"
              >
                {saving && <Loader2 className="h-4 w-4 animate-spin" />} Save
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
