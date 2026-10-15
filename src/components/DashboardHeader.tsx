import {
  Command as CommandIcon,
  Globe,
  History as HistoryIcon,
  Keyboard,
  Moon,
  Plus,
  Settings as SettingsIcon,
  Settings2,
  Sun,
} from "lucide-react";
import type { Theme } from "../hooks/useTheme";

interface Props {
  theme: Theme;
  profileCount: number;
  runningCount: number;
  appVersion: string;
  onOpenPalette: () => void;
  onOpenSettings: () => void;
  onOpenHistory: () => void;
  onOpenHelp: () => void;
  onToggleTheme: () => void;
  onOpenProxies: () => void;
  onCreate: () => void;
}

const iconBtn =
  "rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800";
const labeledBtn =
  "flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800";

/** App title, counts and the row of header actions. */
export function DashboardHeader(props: Props) {
  const {
    theme,
    profileCount,
    runningCount,
    appVersion,
    onOpenPalette,
    onOpenSettings,
    onOpenHistory,
    onOpenHelp,
    onToggleTheme,
    onOpenProxies,
    onCreate,
  } = props;

  return (
    <header className="mb-6 flex items-center justify-between gap-4">
      {/* The window is borderless (CSD) — the title strip doubles as the
          drag region. Window controls (minimize/maximize/close) are left to
          the compositor (niri): no in-app buttons needed. */}
      <div className="flex min-w-0 items-center gap-3" data-tauri-drag-region>
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-blue-600" data-tauri-drag-region>
          <Globe className="h-5 w-5 text-white" />
        </div>
        <div className="min-w-0" data-tauri-drag-region>
          <h1 className="truncate text-lg font-semibold" data-tauri-drag-region>
            Multi Browser Manager
            {appVersion && (
              <span className="ml-2 rounded-md bg-neutral-200/80 px-1.5 py-0.5 align-middle text-[11px] font-medium text-neutral-600 dark:bg-neutral-800 dark:text-neutral-300">
                v{appVersion}
              </span>
            )}
          </h1>
          <p className="text-xs text-neutral-500" data-tauri-drag-region>
            {profileCount} profiles · {runningCount} running
          </p>
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-2">
        <button onClick={onOpenPalette} title="Command palette (Ctrl+K)" className={iconBtn}>
          <CommandIcon className="h-4 w-4" />
        </button>
        <button onClick={onOpenSettings} title="Settings" className={iconBtn}>
          <SettingsIcon className="h-4 w-4" />
        </button>
        <button onClick={onOpenHistory} title="Launch history (H)" className={iconBtn}>
          <HistoryIcon className="h-4 w-4" />
        </button>
        <button onClick={onOpenHelp} title="Keyboard shortcuts (?)" className={iconBtn}>
          <Keyboard className="h-4 w-4" />
        </button>
        <button onClick={onToggleTheme} title="Toggle theme (T)" className={iconBtn}>
          {theme === "dark" ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
        </button>
        <button onClick={onOpenProxies} title="Proxy manager (P)" className={labeledBtn}>
          <Settings2 className="h-4 w-4" /> Proxies
        </button>
        <button
          onClick={onCreate}
          title="New profile (N)"
          className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-2 text-sm font-medium text-white hover:bg-blue-500"
        >
          <Plus className="h-4 w-4" /> New Profile
        </button>
      </div>
    </header>
  );
}
