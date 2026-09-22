import { useEffect, useState } from "react";
import { Loader2, Plus, X } from "lucide-react";
import { detectBrowsers } from "../lib/tauri-api";
import { Select } from "./Select";
import type { BrowserInfo, CreateProfileInput, Profile, Proxy } from "../types";

interface Props {
  profile: Profile | null;
  proxies: Proxy[];
  groups: import("../types").Group[];
  /** Preselected browser type for new profiles (from settings). */
  defaultBrowserType?: string;
  onCreateGroup: (name: string) => Promise<import("../types").Group>;
  onClose: () => void;
  /** Second argument = tag ids to assign to the saved profile. */
  onSubmit: (input: CreateProfileInput, groupIds: string[]) => Promise<void>;
}

export function ProfileForm({ profile, proxies, groups, defaultBrowserType, onCreateGroup, onClose, onSubmit }: Props) {
  const [browsers, setBrowsers] = useState<BrowserInfo[]>([]);
  const [name, setName] = useState(profile?.name ?? "");
  const [browserType, setBrowserType] = useState(profile?.browserType ?? defaultBrowserType ?? "");
  const [proxyId, setProxyId] = useState<string>(profile?.proxyId ?? "");
  const [notes, setNotes] = useState(profile?.notes ?? "");
  const [selectedGroups, setSelectedGroups] = useState<Set<string>>(
    () => new Set(profile?.groups.map((g) => g.id) ?? []),
  );
  const [newTag, setNewTag] = useState("");
  const [addingTag, setAddingTag] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    detectBrowsers()
      .then((list) => {
        setBrowsers(list);
        if (!profile && list.length > 0) {
          setBrowserType((current) => current || list[0].browserType);
        }
      })
      .catch((e) => setError(String(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toggleGroup = (id: string) => {
    setSelectedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleAddTag = async () => {
    const trimmed = newTag.trim();
    if (!trimmed) return;
    setAddingTag(true);
    try {
      const group = await onCreateGroup(trimmed);
      setSelectedGroups((prev) => new Set(prev).add(group.id));
      setNewTag("");
    } catch (err) {
      setError(String(err));
    } finally {
      setAddingTag(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !browserType) {
      setError("Name and browser are required.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await onSubmit(
        {
          name: name.trim(),
          browserType,
          proxyId: proxyId || null,
          notes: notes.trim() || null,
        },
        Array.from(selectedGroups),
      );
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4">
      <div className="w-full max-w-md rounded-xl border border-neutral-200 bg-white p-6 shadow-xl dark:border-neutral-800 dark:bg-neutral-900">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-base font-semibold">
            {profile ? "Edit Profile" : "New Profile"}
          </h2>
          <button onClick={onClose} className="text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-300">
            <X className="h-5 w-5" />
          </button>
        </div>

        {error && (
          <div className="mb-3 rounded-lg border border-red-300 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-200">
            {error}
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">Name</label>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Client A – Marketing"
              autoFocus
              className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-950"
            />
          </div>

          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">Browser</label>
            {browsers.length === 0 ? (
              <p className="text-xs text-neutral-500">
                No Chromium browsers detected on this machine.
              </p>
            ) : (
              <Select
                value={browserType}
                onChange={setBrowserType}
                options={browsers.map((b) => ({
                  value: b.browserType,
                  label: b.name + (b.version ? ` — ${b.version}` : ""),
                }))}
                placeholder="Choose a browser"
              />
            )}
          </div>

          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">
              Proxy (optional)
            </label>
            <Select
              value={proxyId}
              onChange={setProxyId}
              options={[
                { value: "", label: "No proxy" },
                ...proxies.map((p) => ({
                  value: p.id,
                  label: `${p.label} (${p.protocol}://${p.host}:${p.port})`,
                })),
              ]}
            />
          </div>

          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">
              Tags (optional)
            </label>
            {groups.length > 0 && (
              <div className="mb-2 flex flex-wrap gap-1.5">
                {groups.map((g) => {
                  const active = selectedGroups.has(g.id);
                  const color = g.color ?? "#3b82f6";
                  return (
                    <button
                      key={g.id}
                      type="button"
                      onClick={() => toggleGroup(g.id)}
                      className={`inline-flex items-center gap-1 rounded-md border px-2 py-1 text-xs transition-colors ${
                        active
                          ? "border-transparent"
                          : "border-neutral-300 text-neutral-600 hover:border-neutral-400 dark:border-neutral-700 dark:text-neutral-300 dark:hover:border-neutral-600"
                      }`}
                      style={
                        active
                          ? { backgroundColor: `${color}26`, color, borderColor: color }
                          : undefined
                      }
                    >
                      {g.name}
                    </button>
                  );
                })}
              </div>
            )}
            <div className="flex gap-2">
              <input
                value={newTag}
                onChange={(e) => setNewTag(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    void handleAddTag();
                  }
                }}
                placeholder="New tag name"
                className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-950"
              />
              <button
                type="button"
                onClick={handleAddTag}
                disabled={addingTag || !newTag.trim()}
                title="Create tag"
                className="flex shrink-0 items-center rounded-lg border border-neutral-300 px-3 text-neutral-600 hover:bg-neutral-100 disabled:opacity-50 dark:border-neutral-700 dark:text-neutral-300 dark:hover:bg-neutral-800"
              >
                {addingTag ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
              </button>
            </div>
          </div>

          <div>
            <label className="mb-1 block text-xs font-medium text-neutral-500 dark:text-neutral-400">
              Notes (optional)
            </label>
            <textarea
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              rows={3}
              className="w-full resize-none rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-950"
            />
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded-lg px-4 py-2 text-sm text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={saving}
              className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50"
            >
              {saving && <Loader2 className="h-4 w-4 animate-spin" />}
              {profile ? "Save changes" : "Create profile"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
