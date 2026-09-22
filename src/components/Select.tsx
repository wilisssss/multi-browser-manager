import { useEffect, useRef, useState } from "react";
import { Check, ChevronDown } from "lucide-react";

export interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps {
  value: string;
  onChange: (value: string) => void;
  options: SelectOption[];
  placeholder?: string;
  className?: string;
}

/**
 * Custom dropdown replacing the native <select>. WebKitGTK renders the native
 * popup with the GTK theme, which stays white on dark UIs — this one is plain
 * HTML/CSS and follows our dark/light palette everywhere.
 *
 * Root carries data-shortcut-ignore so global keyboard shortcuts (/, n, p, …)
 * don't fire while the menu is open.
 */
export function Select({ value, onChange, options, placeholder, className = "" }: SelectProps) {
  const [open, setOpen] = useState(false);
  const [highlight, setHighlight] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);

  const selected = options.find((o) => o.value === value);

  useEffect(() => {
    if (!open) return;
    const onMouseDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        setOpen(false);
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setHighlight((h) => Math.min(h + 1, options.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setHighlight((h) => Math.max(h - 1, 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        const opt = options[highlight];
        if (opt) {
          onChange(opt.value);
          setOpen(false);
        }
      }
    };
    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keydown", onKeyDown, true);
    };
  }, [open, highlight, options, onChange]);

  const openMenu = () => {
    setHighlight(Math.max(0, options.findIndex((o) => o.value === value)));
    setOpen(true);
  };

  return (
    <div ref={rootRef} className={`relative ${className}`} data-shortcut-ignore>
      <button
        type="button"
        onClick={() => (open ? setOpen(false) : openMenu())}
        className="flex w-full items-center justify-between gap-2 rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm text-left hover:border-neutral-400 dark:border-neutral-700 dark:bg-neutral-800 dark:hover:border-neutral-600"
      >
        <span
          className={`truncate ${selected ? "text-neutral-900 dark:text-neutral-100" : "text-neutral-400 dark:text-neutral-500"}`}
        >
          {selected?.label ?? placeholder ?? "Select..."}
        </span>
        <ChevronDown
          className={`h-4 w-4 shrink-0 text-neutral-400 transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>

      {open && (
        <ul
          role="listbox"
          className="absolute z-30 mt-1 max-h-60 w-full overflow-y-auto rounded-lg border border-neutral-200 bg-white py-1 shadow-xl dark:border-neutral-700 dark:bg-neutral-900"
        >
          {options.map((opt, i) => {
            const isSelected = opt.value === value;
            return (
              <li key={opt.value}>
                <button
                  type="button"
                  role="option"
                  aria-selected={isSelected}
                  onMouseEnter={() => setHighlight(i)}
                  onClick={() => {
                    onChange(opt.value);
                    setOpen(false);
                  }}
                  className={`flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm ${
                    i === highlight
                      ? "bg-blue-600 text-white"
                      : "text-neutral-700 dark:text-neutral-200"
                  }`}
                >
                  <span className="truncate">{opt.label}</span>
                  {isSelected && <Check className="h-3.5 w-3.5 shrink-0" />}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
