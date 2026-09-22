import { useEffect, useRef, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { Search } from "lucide-react";

export interface PaletteCommand {
  id: string;
  label: string;
  section: string;
  /** Right-side hint, e.g. a keyboard shortcut or profile status. */
  hint?: string;
  /** Extra searchable text (notes, tags, browser type). */
  keywords?: string;
  icon: LucideIcon;
  action: () => void;
}

interface Props {
  open: boolean;
  onClose: () => void;
  commands: PaletteCommand[];
}

/**
 * Fuzzy-searchable command palette (Ctrl+K). Keyboard-first: arrows to move,
 * Enter to run, Esc to close. Sections are rendered in command order.
 */
export function CommandPalette({ open, onClose, commands }: Props) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setIndex(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  const filtered = query.trim()
    ? commands.filter((c) => {
        const q = query.trim().toLowerCase();
        return (
          c.label.toLowerCase().includes(q) ||
          c.section.toLowerCase().includes(q) ||
          (c.keywords ?? "").toLowerCase().includes(q)
        );
      })
    : commands;

  // Clamp the selection when the filtered list shrinks.
  useEffect(() => {
    setIndex((i) => Math.min(i, Math.max(0, filtered.length - 1)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filtered.length]);

  // Keep the selected row visible while navigating.
  useEffect(() => {
    listRef.current
      ?.querySelector('[data-selected="true"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [index, filtered.length]);

  if (!open) return null;

  const run = (cmd: PaletteCommand) => {
    onClose();
    cmd.action();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setIndex((i) => (filtered.length === 0 ? 0 : (i + 1) % filtered.length));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setIndex((i) =>
        filtered.length === 0 ? 0 : (i - 1 + filtered.length) % filtered.length,
      );
    } else if (e.key === "Enter") {
      e.preventDefault();
      const cmd = filtered[index];
      if (cmd) run(cmd);
    }
  };

  let lastSection = "";

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[12vh]"
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
      onKeyDown={onKeyDown}
      data-shortcut-ignore
    >
      <div className="w-full max-w-xl overflow-hidden rounded-xl border border-neutral-200 bg-white shadow-2xl dark:border-neutral-800 dark:bg-neutral-900">
        <div className="relative border-b border-neutral-200 dark:border-neutral-800">
          <Search className="pointer-events-none absolute left-4 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Type a command or profile name..."
            className="w-full bg-transparent py-3.5 pl-11 pr-4 text-sm outline-none placeholder:text-neutral-400"
          />
        </div>

        <div ref={listRef} className="max-h-80 overflow-y-auto p-2">
          {filtered.length === 0 ? (
            <p className="py-8 text-center text-sm text-neutral-400">No matching commands.</p>
          ) : (
            filtered.map((cmd, i) => {
              const showHeader = cmd.section !== lastSection;
              lastSection = cmd.section;
              const Icon = cmd.icon;
              const selected = i === index;
              return (
                <div key={cmd.id}>
                  {showHeader && (
                    <p className="px-2 pb-1 pt-3 text-[11px] font-semibold uppercase tracking-wider text-neutral-400">
                      {cmd.section}
                    </p>
                  )}
                  <button
                    data-selected={selected}
                    onMouseEnter={() => setIndex(i)}
                    onClick={() => run(cmd)}
                    className={`flex w-full items-center gap-3 rounded-lg px-2 py-2 text-left text-sm ${
                      selected
                        ? "bg-blue-600 text-white"
                        : "text-neutral-700 hover:bg-neutral-100 dark:text-neutral-200 dark:hover:bg-neutral-800"
                    }`}
                  >
                    <Icon className={`h-4 w-4 shrink-0 ${selected ? "" : "text-neutral-400"}`} />
                    <span className="flex-1 truncate">{cmd.label}</span>
                    {cmd.hint && (
                      <span
                        className={`shrink-0 rounded-md border px-1.5 py-0.5 font-mono text-[11px] ${
                          selected
                            ? "border-blue-400 text-blue-100"
                            : "border-neutral-300 text-neutral-400 dark:border-neutral-700"
                        }`}
                      >
                        {cmd.hint}
                      </span>
                    )}
                  </button>
                </div>
              );
            })
          )}
        </div>

        <div className="flex items-center gap-3 border-t border-neutral-200 px-4 py-2 text-[11px] text-neutral-400 dark:border-neutral-800">
          <span>
            <kbd className="rounded border border-neutral-300 px-1 font-mono dark:border-neutral-700">↑↓</kbd>{" "}
            navigate
          </span>
          <span>
            <kbd className="rounded border border-neutral-300 px-1 font-mono dark:border-neutral-700">↵</kbd> run
          </span>
          <span>
            <kbd className="rounded border border-neutral-300 px-1 font-mono dark:border-neutral-700">esc</kbd> close
          </span>
        </div>
      </div>
    </div>
  );
}
