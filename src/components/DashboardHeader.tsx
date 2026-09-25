import {
  AppWindow,
  Command as CommandIcon,
  Download,
  Globe,
  History as HistoryIcon,
  Keyboard,
  Minus,
  Moon,
  Plus,
  RefreshCw,
  Settings as SettingsIcon,
  Settings2,
  Square,
  Sun,
  Upload,
  X,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Theme } from "../hooks/useTheme";

interface Props {
  theme: Theme;
  profileCount: number;
  runningCount: number;
  appVersion: string;
  /** True while the updater is checking or downloading. */
  updaterBusy: boolean;
  onOpenPalette: () => void;
  onOpenRules: () => void;
  onOpenSettings: () => void;
  onOpenHistory: () => void;
  onOpenHelp: () => void;
  onCheckUpdates: () => void;
  onToggleTheme: () => void;
  onImport: () => void;
  onExport: () => void;
  onOpenProxies: () => void;
  onCreate: () => void;
  busy: boolean;
}

const iconBtn =
  "rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800";
const labeledBtn =
  "flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800";
const windowBtn =
  "flex h-10 w-10 items-center justify-center text-neutral-500 transition-colors hover:bg-neutral-200/70 dark:hover:bg-neutral-800 dark:text-neutral-300";

/** App title, counts and the row of header actions. */
export function DashboardHeader(props: Props) {
  const {
    theme,
    profileCount,
    runningCount,
    appVersion,
    updaterBusy,
    onOpenPalette,
    onOpenRules,
    onOpenSettings,
    onOpenHistory,
    onOpenHelp,
    onCheckUpdates,
    onToggleTheme,
    onImport,
    onExport,
    onOpenProxies,
    onCreate,
    busy,
  } = props;

  return (
    <header className="mb-6 flex items-center justify-between gap-4">
      {/* The window is borderless (CSD) — this top strip is the drag region,
          and the buttons cover minimize / maximize / close (a WM that ignores
          super-drag left the dashboard impossible to move or close before). */}
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
        <div className="mr-1 flex items-center">
          <button onClick={() => getCurrentWindow().minimize()} title="Minimize" className={windowBtn}>
            <Minus className="h-4 w-4" />
          </button>
          <button onClick={() => getCurrentWindow().toggleMaximize()} title="Maximize / restore" className={windowBtn}>
            <Square className="h-3.5 w-3.5" />
          </button>
          {/* close() emits CloseRequested; the Rust window handler prevents the
              destroy and hides to tray instead, so "X" = hide, matching the
              documented tray behavior. */}
          <button onClick={() => getCurrentWindow().close()} title="Hide to tray" className={`${windowBtn} hover:bg-red-500 hover:text-white`}>
            <X className="h-4 w-4" />
          </button>
        </div>
        <button onClick={onOpenPalette} title="Command palette (Ctrl+K)" className={iconBtn}>
          <CommandIcon className="h-4 w-4" />
        </button>
        <button onClick={onOpenRules} title="Window rules & workspaces" className={iconBtn}>
          <AppWindow className="h-4 w-4" />
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
        <button
          onClick={onCheckUpdates}
          disabled={updaterBusy}
          title={`Check for updates${appVersion ? ` (current: v${appVersion})` : ""}`}
          className={iconBtn}
        >
          {updaterBusy ? <RefreshCw className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
        </button>
        <button onClick={onToggleTheme} title="Toggle theme (T)" className={iconBtn}>
          {theme === "dark" ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
        </button>
        <button onClick={onImport} disabled={busy} className={labeledBtn}>
          <Upload className="h-4 w-4" /> Import
        </button>
        <button onClick={onExport} disabled={busy} className={labeledBtn}>
          <Download className="h-4 w-4" /> Export
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
