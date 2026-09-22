import { Keyboard, X } from "lucide-react";

interface Props {
  onClose: () => void;
}

const SHORTCUTS: Array<{ keys: string[]; action: string }> = [
  { keys: ["Ctrl", "K"], action: "Command palette" },
  { keys: ["/"], action: "Focus search" },
  { keys: ["N"], action: "New profile" },
  { keys: ["P"], action: "Toggle proxy manager" },
  { keys: ["H"], action: "Toggle launch history" },
  { keys: ["T"], action: "Toggle dark / light theme" },
  { keys: ["R"], action: "Refresh profiles" },
  { keys: ["?"], action: "Show this help" },
  { keys: ["Esc"], action: "Close any open dialog" },
];

export function ShortcutsHelp({ onClose }: Props) {
  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-4">
      <div className="w-full max-w-sm rounded-xl border border-neutral-200 bg-white p-6 shadow-xl dark:border-neutral-800 dark:bg-neutral-900">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-base font-semibold">
            <Keyboard className="h-4 w-4" /> Keyboard Shortcuts
          </h2>
          <button onClick={onClose} className="text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-300">
            <X className="h-5 w-5" />
          </button>
        </div>

        <ul className="space-y-2">
          {SHORTCUTS.map((s) => (
            <li key={s.action} className="flex items-center justify-between text-sm">
              <span className="text-neutral-600 dark:text-neutral-300">{s.action}</span>
              <span className="flex gap-1">
                {s.keys.map((k) => (
                  <kbd
                    key={k}
                    className="rounded-md border border-neutral-300 bg-neutral-100 px-2 py-0.5 font-mono text-xs shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
                  >
                    {k}
                  </kbd>
                ))}
              </span>
            </li>
          ))}
        </ul>

        <p className="mt-4 text-xs text-neutral-400">
          Tip: WM-level keybinds (e.g. niri) can also launch profiles directly via
          <code className="mx-1 rounded bg-neutral-100 px-1 py-0.5 dark:bg-neutral-800">
            mbm --launch &lt;profile&gt;
          </code>
          — see docs/niri-keybinds.kdl.
        </p>
      </div>
    </div>
  );
}
