import { useState } from "react";
import { FolderPlus, Loader2, X, Move } from "lucide-react";
import { createFolder, renameFolder, moveFolder, moveProfilesToFolder } from "../lib/tauri-api";
import type { Folder } from "../types";

/** Shared modal shell (same styling as the other modals). */
function ModalShell({
  title,
  icon,
  onClose,
  children,
}: {
  title: string;
  icon: React.ReactNode;
  onClose: () => void;
  children: React.ReactNode;
}) {
  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-4">
      <div className="w-full max-w-md rounded-xl border border-neutral-200 bg-white p-6 shadow-xl dark:border-neutral-800 dark:bg-neutral-900">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-base font-semibold">
            {icon} {title}
          </h2>
          <button onClick={onClose} className="text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-300">
            <X className="h-5 w-5" />
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

interface NameModalProps {
  /** `null` = create a new folder in `parentId`; set = rename that folder. */
  editing: Folder | null;
  parentId: string | null;
  onClose: () => void;
  onError: (message: string) => void;
}

/** Create / rename folder prompt (same dialog, mode picked by `editing`). */
export function FolderNameModal({ editing, parentId, onClose, onError }: NameModalProps) {
  const [name, setName] = useState(editing?.name ?? "");
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setSaving(true);
    try {
      if (editing) await renameFolder(editing.id, name.trim());
      else await createFolder(name.trim(), parentId);
      onClose();
    } catch (err) {
      onError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <ModalShell
      title={editing ? "Rename folder" : "New folder"}
      icon={<FolderPlus className="h-4 w-4" />}
      onClose={onClose}
    >
      <form onSubmit={handleSubmit} className="space-y-4">
        <input
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Folder name"
          className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-700 dark:bg-neutral-900"
        />
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg px-3 py-2 text-sm text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-300">
            Cancel
          </button>
          <button
            type="submit"
            disabled={saving || !name.trim()}
            className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50"
          >
            {saving && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
            {editing ? "Rename" : "Create"}
          </button>
        </div>
      </form>
    </ModalShell>
  );
}

interface PickerProps {
  /** Folder being moved (excludes itself + its subtree from the targets). */
  folder: Folder | null;
  /** Profile ids being moved (single or bulk). */
  profileIds: string[] | null;
  /** Where the moved item(s) currently live, to grey out in the list. */
  currentParentId: string | null;
  folders: Folder[];
  onClose: () => void;
  onError: (message: string) => void;
}

/** "Root / A / B" label with depth-based indentation for the option list. */
function targetLabel(id: string | null, byId: Map<string, Folder>): { label: string; depth: number } {
  if (id === null) return { label: "Root (no folder)", depth: 0 };
  const parts: string[] = [];
  let depth = 0;
  let cur: string | null = id;
  while (cur) {
    const f = byId.get(cur);
    if (!f) break;
    parts.unshift(f.name);
    depth++;
    cur = f.parentId;
  }
  return { label: parts.join(" / "), depth };
}

/**
 * Folder picker for moving things. Targets: every folder except (for a
 * folder move) the folder itself and its descendants, plus the root.
 */
export function FolderPickerModal({ folder, profileIds, currentParentId, folders, onClose, onError }: PickerProps) {
  const [saving, setSaving] = useState<string | null>(null);
  const byId = new Map(folders.map((f) => [f.id, f]));

  // Self + descendants are not valid targets for a folder move.
  const excluded = new Set<string>();
  if (folder) {
    const stack = [folder.id];
    while (stack.length) {
      const id = stack.pop()!;
      excluded.add(id);
      for (const f of folders) if (f.parentId === id) stack.push(f.id);
    }
  }

  const targets: (string | null)[] = [
    null,
    ...folders.map((f) => f.id).filter((id) => !excluded.has(id)),
  ];

  const move = async (target: string | null) => {
    setSaving(target ?? "root");
    try {
      if (folder) await moveFolder(folder.id, target);
      else if (profileIds) await moveProfilesToFolder(profileIds, target);
      onClose();
    } catch (e) {
      onError(String(e));
    } finally {
      setSaving(null);
    }
  };

  const title = folder ? `Move "${folder.name}"` : `Move ${profileIds?.length ?? 0} profile(s)`;

  return (
    <ModalShell title={title} icon={<Move className="h-4 w-4" />} onClose={onClose}>
      <p className="mb-3 text-xs text-neutral-500">Choose the destination folder:</p>
      <div className="max-h-80 space-y-1 overflow-y-auto">
        {targets.map((t) => {
          const { label, depth } = targetLabel(t, byId);
          const isCurrent = currentParentId === t;
          const busy = saving === (t ?? "root");
          return (
            <button
              key={t ?? "root"}
              onClick={() => move(t)}
              disabled={saving !== null}
              className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm hover:bg-neutral-100 disabled:opacity-50 dark:hover:bg-neutral-800 ${
                isCurrent ? "text-neutral-400" : ""
              }`}
              style={{ paddingLeft: `${12 + depth * 16}px` }}
            >
              {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <FolderPlus className="h-3.5 w-3.5 opacity-50" />}
              <span className="truncate">{label}</span>
            </button>
          );
        })}
      </div>
    </ModalShell>
  );
}
