import { Folder as FolderIcon, Pencil, Move, Trash2 } from "lucide-react";
import { ask } from "@tauri-apps/plugin-dialog";
import { deleteFolder } from "../lib/tauri-api";
import type { Folder } from "../types";

interface Props {
  folder: Folder;
  onOpen: (id: string) => void;
  onRename: (folder: Folder) => void;
  onMove: (folder: Folder) => void;
  onDeleted: () => void;
  onError: (message: string) => void;
}

/**
 * One folder tile in the folder grid. Clicking the tile opens the folder;
 * the icon buttons manage it. Deleting re-parents the contents one level up
 * (backend guarantee) — the dialog says so.
 */
export function FolderCard({ folder, onOpen, onRename, onMove, onDeleted, onError }: Props) {
  const handleDelete = async () => {
    const ok = await ask(
      `Delete folder "${folder.name}"?\n\nIts subfolders and profiles move up one level — nothing is deleted.`,
      { title: "Delete folder", kind: "warning", okLabel: "Delete", cancelLabel: "Cancel" },
    );
    if (!ok) return;
    try {
      await deleteFolder(folder.id);
      onDeleted();
    } catch (e) {
      onError(String(e));
    }
  };

  return (
    <div className="group flex flex-col rounded-xl border border-neutral-200 bg-white p-5 shadow-sm transition-all hover:border-neutral-300 hover:shadow dark:border-neutral-800 dark:bg-neutral-900 dark:hover:border-neutral-700 dark:shadow-none">
      <button
        onClick={() => onOpen(folder.id)}
        className="mb-4 flex min-w-0 items-center gap-3 text-left"
        title={`Open "${folder.name}"`}
      >
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-blue-100 text-blue-600 dark:bg-blue-950 dark:text-blue-400">
          <FolderIcon className="h-5 w-5" />
        </div>
        <div className="min-w-0">
          <h3 className="truncate text-sm font-semibold leading-snug" title={folder.name}>
            {folder.name}
          </h3>
          <p className="mt-0.5 text-xs text-neutral-500">
            {folder.profileCount} profile{folder.profileCount === 1 ? "" : "s"}
          </p>
        </div>
      </button>

      <div className="mt-auto flex items-center justify-end gap-x-1.5 border-t border-neutral-100 pt-3.5 dark:border-neutral-800">
        <button
          onClick={() => onRename(folder)}
          title="Rename"
          className="inline-flex h-8 w-8 items-center justify-center rounded-lg text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700 dark:hover:bg-neutral-800 dark:hover:text-neutral-200"
        >
          <Pencil className="h-4 w-4" />
        </button>
        <button
          onClick={() => onMove(folder)}
          title="Move to another folder"
          className="inline-flex h-8 w-8 items-center justify-center rounded-lg text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700 dark:hover:bg-neutral-800 dark:hover:text-neutral-200"
        >
          <Move className="h-4 w-4" />
        </button>
        <button
          onClick={handleDelete}
          title="Delete (contents move up one level)"
          className="inline-flex h-8 w-8 items-center justify-center rounded-lg text-neutral-400 hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-950 dark:hover:text-red-400"
        >
          <Trash2 className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}
