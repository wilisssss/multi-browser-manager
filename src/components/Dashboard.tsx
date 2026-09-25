import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  AppWindow,
  Command as CommandIcon,
  Download,
  Globe,
  History as HistoryIcon,
  Keyboard,
  Moon,
  Play,
  Plus,
  RefreshCw,
  Search,
  Settings as SettingsIcon,
  Settings2,
  Sun,
  Square,
  Upload,
  X,
} from "lucide-react";
import { useProfiles } from "../hooks/useProfiles";
import { useProxies } from "../hooks/useProxies";
import { useGroups } from "../hooks/useGroups";
import { useUpdater } from "../hooks/useUpdater";
import { ProfileList } from "./ProfileList";
import { ProfileForm } from "./ProfileForm";
import { ProxyManager } from "./ProxyManager";
import { HistoryModal } from "./HistoryModal";
import { ShortcutsHelp } from "./ShortcutsHelp";
import { CommandPalette, type PaletteCommand } from "./CommandPalette";
import { SettingsModal } from "./SettingsModal";
import { WindowRulesModal } from "./WindowRulesModal";
import { CredentialsModal } from "./CredentialsModal";
import { Select } from "./Select";
import { exportProfiles, importProfiles, bulkLaunch, bulkStop, setProfileGroups, togglePin, getSettings, updateSettings } from "../lib/tauri-api";
import { getVersion } from "@tauri-apps/api/app";
import { save, open } from "@tauri-apps/plugin-dialog";
import type { Profile } from "../types";
import type { Theme } from "../hooks/useTheme";

interface Props {
  theme: Theme;
  toggleTheme: () => void;
}

/** Sort orders for the profile grid. Pinned profiles always stay on top. */
type SortKey =
  | "name-asc"
  | "name-desc"
  | "last-used-desc"
  | "last-used-asc"
  | "created-desc"
  | "created-asc"
  | "status";

const SORT_STORAGE_KEY = "mbm.sort";

function loadSort(): SortKey {
  const stored = localStorage.getItem(SORT_STORAGE_KEY);
  const valid: SortKey[] = [
    "name-asc",
    "name-desc",
    "last-used-desc",
    "last-used-asc",
    "created-desc",
    "created-asc",
    "status",
  ];
  return valid.includes(stored as SortKey) ? (stored as SortKey) : "name-asc";
}

export function Dashboard({ theme, toggleTheme }: Props) {
  const profilesState = useProfiles();
  const proxiesState = useProxies();
  const groupsState = useGroups();
  const updater = useUpdater();

  const [search, setSearch] = useState("");
  // Debounced mirror of `search`: typing filters after a short pause instead of
  // re-filtering (and re-rendering every card) on each keystroke.
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<"all" | "running" | "stopped">("all");
  const [browserFilter, setBrowserFilter] = useState<string>("all");
  const [tagFilter, setTagFilter] = useState<string>("all");
  const [sortBy, setSortBy] = useState<SortKey>(loadSort);
  const [appVersion, setAppVersion] = useState<string>("");

  // Remember the chosen sort across sessions.
  useEffect(() => {
    localStorage.setItem(SORT_STORAGE_KEY, sortBy);
  }, [sortBy]);

  // Displayed next to the app title.
  useEffect(() => {
    getVersion()
      .then(setAppVersion)
      .catch(() => {});
  }, []);

  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<Profile | null>(null);
  const [proxyManagerOpen, setProxyManagerOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [rulesOpen, setRulesOpen] = useState(false);
  const [credentialsProfile, setCredentialsProfile] = useState<Profile | null>(null);
  const [appSettings, setAppSettings] = useState<import("../types").AppSettings | null>(null);
  const [historyKey, setHistoryKey] = useState(0);
  const [busy, setBusy] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  const anyModalOpen =
    formOpen ||
    proxyManagerOpen ||
    historyOpen ||
    helpOpen ||
    paletteOpen ||
    settingsOpen ||
    rulesOpen ||
    credentialsProfile !== null;

  // Settings are loaded once and kept in sync with the settings modal.
  useEffect(() => {
    getSettings()
      .then(setAppSettings)
      .catch(() => {});
  }, []);

  /** Persists one tag → workspace mapping change. */
  const handleWorkspaceChange = useCallback(async (groupId: string, workspace: number | null) => {
    setAppSettings((prev) => {
      if (!prev) return prev;
      const groupWorkspaces = { ...prev.groupWorkspaces };
      if (workspace === null) delete groupWorkspaces[groupId];
      else groupWorkspaces[groupId] = workspace;
      const next = { ...prev, groupWorkspaces };
      // Fire-and-forget persist; failures resurface via reload on next open.
      updateSettings(next).catch(() => {});
      return next;
    });
  }, []);

  // 150 ms debounce on the search box.
  useEffect(() => {
    const t = setTimeout(() => setDebouncedSearch(search), 150);
    return () => clearTimeout(t);
  }, [search]);

  // O(1) proxy lookup per card instead of an O(P×Q) Array.find per render.
  const proxyById = useMemo(
    () => new Map(proxiesState.proxies.map((p) => [p.id, p])),
    [proxiesState.proxies],
  );

  const browserTypes = useMemo(() => {
    const set = new Set(profilesState.profiles.map((p) => p.browserType));
    return Array.from(set).sort();
  }, [profilesState.profiles]);

  const filtered = useMemo(() => {
    const q = debouncedSearch.trim().toLowerCase();
    const matched = profilesState.profiles.filter((p) => {
      if (q && !p.name.toLowerCase().includes(q) && !(p.notes ?? "").toLowerCase().includes(q)) {
        return false;
      }
      if (statusFilter !== "all" && p.status !== statusFilter) return false;
      if (browserFilter !== "all" && p.browserType !== browserFilter) return false;
      if (tagFilter !== "all" && !p.groups.some((g) => g.id === tagFilter)) return false;
      return true;
    });

    // Pinned profiles always float to the top, whatever the sort order.
    const byName = (a: Profile, b: Profile) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    const byTime = (key: "lastUsedAt" | "createdAt", desc: boolean) => (a: Profile, b: Profile) => {
      const av = a[key];
      const bv = b[key];
      // Never-used profiles sink to the bottom regardless of direction.
      if (av == null && bv == null) return byName(a, b);
      if (av == null) return 1;
      if (bv == null) return -1;
      return desc ? bv - av : av - bv;
    };

    const comparators: Record<SortKey, (a: Profile, b: Profile) => number> = {
      "name-asc": byName,
      "name-desc": (a, b) => byName(b, a),
      "last-used-desc": byTime("lastUsedAt", true),
      "last-used-asc": byTime("lastUsedAt", false),
      "created-desc": byTime("createdAt", true),
      "created-asc": byTime("createdAt", false),
      status: (a, b) => {
        // Running first, then alphabetical.
        if (a.status !== b.status) return a.status === "running" ? -1 : 1;
        return byName(a, b);
      },
    };
    const cmp = comparators[sortBy];
    return matched.sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
      return cmp(a, b);
    });
  }, [profilesState.profiles, debouncedSearch, statusFilter, browserFilter, tagFilter, sortBy]);

  const runningCount = profilesState.profiles.filter((p) => p.status === "running").length;

  // Refresh when the backend signals external changes (e.g. `mbm --launch` from a WM keybind).
  useEffect(() => {
    const unlistenPromise = listen("profiles-changed", () => profilesState.refresh());
    return () => {
      unlistenPromise.then((fn) => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Global keyboard shortcuts (ignored while typing or when a modal is open).
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Ctrl+K toggles the command palette from anywhere, even while typing.
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((v) => !v);
        return;
      }
      if (e.key === "Escape") {
        setFormOpen(false);
        setProxyManagerOpen(false);
        setHistoryOpen(false);
        setHelpOpen(false);
        setPaletteOpen(false);
        setSettingsOpen(false);
        setRulesOpen(false);
        setCredentialsProfile(null);
        return;
      }
      const target = e.target as HTMLElement;
      const typing =
        ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName) ||
        target.isContentEditable ||
        Boolean(target.closest?.("[data-shortcut-ignore]"));
      if (typing || e.metaKey || e.ctrlKey || e.altKey) return;

      switch (e.key) {
        case "/":
          e.preventDefault();
          searchRef.current?.focus();
          break;
        case "n":
          if (!anyModalOpen) openCreate();
          break;
        case "p":
          setProxyManagerOpen((v) => !v);
          break;
        case "h":
          setHistoryOpen((v) => !v);
          break;
        case "t":
          toggleTheme();
          break;
        case "r":
          profilesState.refresh();
          break;
        case "?":
          setHelpOpen(true);
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [anyModalOpen, toggleTheme]);

  const handleExport = async () => {
    const path = await save({
      title: "Export profiles",
      defaultPath: "mbm-backup.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    try {
      setBusy(true);
      await exportProfiles(path);
    } catch (e) {
      profilesState.setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleImport = async () => {
    const path = await open({
      title: "Import profiles",
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path || Array.isArray(path)) return;
    try {
      setBusy(true);
      const result = await importProfiles(path);
      await Promise.all([profilesState.refresh(), proxiesState.refresh()]);
      if (result.conflicts.length > 0) {
        profilesState.setError(
          `Imported ${result.importedProfiles} profiles, ${result.importedProxies} proxies, ` +
            `${result.importedCredentials} credentials. ` +
            `Skipped: ${result.skippedProfiles} profiles, ${result.skippedProxies} proxies, ` +
            `${result.skippedCredentials} credentials.`,
        );
      }
    } catch (e) {
      profilesState.setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const launchAllStopped = async () => {
    const ids = filtered.filter((p) => p.status === "stopped").map((p) => p.id);
    if (ids.length === 0) return;
    setBusy(true);
    try {
      const results = await bulkLaunch(ids);
      await profilesState.refresh();
      reportBulkFailures(results, "launch");
    } catch (e) {
      profilesState.setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const stopAllRunning = async () => {
    const ids = filtered.filter((p) => p.status === "running").map((p) => p.id);
    if (ids.length === 0) return;
    setBusy(true);
    try {
      const results = await bulkStop(ids);
      await profilesState.refresh();
      reportBulkFailures(results, "stop");
    } catch (e) {
      profilesState.setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  /** Surfaces per-profile bulk failures that the backend collected. */
  const reportBulkFailures = (results: { profileName?: string; profileId: string; success: boolean; message: string }[], action: string) => {
    const failures = results.filter((r) => !r.success);
    if (failures.length === 0) return;
    const names = failures
      .map((f) => {
        const p = profilesState.profiles.find((x) => x.id === f.profileId);
        return p?.name ?? f.profileId;
      })
      .join(", ");
    const firstError = failures[0].message;
    profilesState.setError(
      `Failed to ${action} ${failures.length} profile(s): ${names}. First error: ${firstError}`,
    );
  };

  const openCreate = useCallback(() => {
    setEditing(null);
    setFormOpen(true);
    // ProfileForm fetches the browser list itself — no double probe here.
  }, []);

  const openEdit = useCallback((profile: Profile) => {
    setEditing(profile);
    setFormOpen(true);
  }, []);

  const handleTogglePin = useCallback(
    async (id: string) => {
      await togglePin(id);
      await profilesState.refresh();
    },
    [profilesState.refresh],
  );

  // Command palette entries. Rebuilt per render on purpose — the array is tiny
  // and the closures need the latest handlers.
  const paletteCommands: PaletteCommand[] = [
    ...profilesState.profiles.map((p) => ({
      id: `profile-${p.id}`,
      label: p.name,
      section: "Profiles",
      hint: p.status === "running" ? "running · stop" : "stopped · launch",
      keywords: [p.browserType, p.notes ?? "", ...p.groups.map((g) => g.name)]
        .filter(Boolean)
        .join(" "),
      icon: p.status === "running" ? Square : Play,
      action: () =>
        p.status === "running" ? profilesState.stop(p.id) : profilesState.launch(p.id),
    })),
    {
      id: "new-profile",
      label: "New profile",
      section: "Actions",
      hint: "N",
      icon: Plus,
      action: openCreate,
    },
    {
      id: "proxies",
      label: "Manage proxies",
      section: "Actions",
      hint: "P",
      icon: Settings2,
      action: () => setProxyManagerOpen(true),
    },
    {
      id: "history",
      label: "Launch history",
      section: "Actions",
      hint: "H",
      icon: HistoryIcon,
      action: () => {
        setHistoryKey((k) => k + 1);
        setHistoryOpen(true);
      },
    },
    {
      id: "launch-all",
      label: "Launch all (current filter)",
      section: "Actions",
      icon: Play,
      action: launchAllStopped,
    },
    {
      id: "stop-all",
      label: "Stop all (current filter)",
      section: "Actions",
      icon: Square,
      action: stopAllRunning,
    },
    {
      id: "refresh",
      label: "Refresh profiles",
      section: "Actions",
      hint: "R",
      icon: RefreshCw,
      action: profilesState.refresh,
    },
    {
      id: "check-updates",
      label: "Check for updates",
      section: "Actions",
      icon: Download,
      action: updater.checkForUpdates,
    },
    {
      id: "export",
      label: "Export backup…",
      section: "Actions",
      icon: Download,
      action: handleExport,
    },
    {
      id: "import",
      label: "Import backup…",
      section: "Actions",
      icon: Upload,
      action: handleImport,
    },
    {
      id: "theme",
      label: theme === "dark" ? "Switch to light theme" : "Switch to dark theme",
      section: "Actions",
      hint: "T",
      icon: theme === "dark" ? Sun : Moon,
      action: toggleTheme,
    },
    {
      id: "shortcuts",
      label: "Keyboard shortcuts",
      section: "Actions",
      hint: "?",
      icon: Keyboard,
      action: () => setHelpOpen(true),
    },
    {
      id: "window-rules",
      label: "Window rules & workspaces",
      section: "Actions",
      icon: AppWindow,
      action: () => setRulesOpen(true),
    },
    {
      id: "settings",
      label: "Settings",
      section: "Actions",
      icon: SettingsIcon,
      action: () => setSettingsOpen(true),
    },
  ];

  const updaterBanner =
    updater.status.state === "up-to-date"
      ? { tone: "ok" as const, text: updater.status.message }
      : updater.status.state === "available"
        ? { tone: "info" as const, text: `Update ${updater.status.version} available.` }
        : updater.status.state === "error"
          ? { tone: "err" as const, text: updater.status.message }
          : null;

  return (
    <div className="mx-auto flex min-h-screen max-w-7xl flex-col px-6 py-6">
      {/* Header */}
      <header className="mb-6 flex items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-blue-600">
            <Globe className="h-5 w-5 text-white" />
          </div>
          <div>
            <h1 className="text-lg font-semibold">
              Multi Browser Manager
              {appVersion && (
                <span className="ml-2 rounded-md bg-neutral-200/80 px-1.5 py-0.5 align-middle text-[11px] font-medium text-neutral-600 dark:bg-neutral-800 dark:text-neutral-300">
                  v{appVersion}
                </span>
              )}
            </h1>
            <p className="text-xs text-neutral-500">
              {profilesState.profiles.length} profiles · {runningCount} running
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={() => setPaletteOpen(true)}
            title="Command palette (Ctrl+K)"
            className="rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 dark:text-neutral-300 dark:hover:bg-neutral-800"
          >
            <CommandIcon className="h-4 w-4" />
          </button>
          <button
            onClick={() => setRulesOpen(true)}
            title="Window rules & workspaces"
            className="rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 dark:text-neutral-300 dark:hover:bg-neutral-800"
          >
            <AppWindow className="h-4 w-4" />
          </button>
          <button
            onClick={() => setSettingsOpen(true)}
            title="Settings"
            className="rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 dark:text-neutral-300 dark:hover:bg-neutral-800"
          >
            <SettingsIcon className="h-4 w-4" />
          </button>
          <button
            onClick={() => {
              setHistoryKey((k) => k + 1);
              setHistoryOpen(true);
            }}
            title="Launch history (H)"
            className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 dark:text-neutral-300 dark:hover:bg-neutral-800"
          >
            <HistoryIcon className="h-4 w-4" />
          </button>
          <button
            onClick={() => setHelpOpen(true)}
            title="Keyboard shortcuts (?)"
            className="rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 dark:text-neutral-300 dark:hover:bg-neutral-800"
          >
            <Keyboard className="h-4 w-4" />
          </button>
          <button
            onClick={updater.checkForUpdates}
            disabled={updater.status.state === "checking" || updater.status.state === "downloading"}
            title={`Check for updates${appVersion ? ` (current: v${appVersion})` : ""}`}
            className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800"
          >
            {updater.status.state === "checking" || updater.status.state === "downloading" ? (
              <RefreshCw className="h-4 w-4 animate-spin" />
            ) : (
              <Download className="h-4 w-4" />
            )}
            Check update
          </button>
          <button
            onClick={toggleTheme}
            title="Toggle theme (T)"
            className="rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 dark:text-neutral-300 dark:hover:bg-neutral-800"
          >
            {theme === "dark" ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
          </button>
          <button
            onClick={handleImport}
            disabled={busy}
            className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800"
          >
            <Upload className="h-4 w-4" /> Import
          </button>
          <button
            onClick={handleExport}
            disabled={busy}
            className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800"
          >
            <Download className="h-4 w-4" /> Export
          </button>
          <button
            onClick={() => setProxyManagerOpen(true)}
            title="Proxy manager (P)"
            className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 dark:text-neutral-300 dark:hover:bg-neutral-800"
          >
            <Settings2 className="h-4 w-4" /> Proxies
          </button>
          <button
            onClick={openCreate}
            title="New profile (N)"
            className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-2 text-sm font-medium text-white hover:bg-blue-500"
          >
            <Plus className="h-4 w-4" /> New Profile
          </button>
        </div>
      </header>

      {/* Banners */}
      {profilesState.error && (
        <div className="mb-4 flex items-start justify-between gap-3 rounded-lg border border-red-300 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-200">
          <span>{profilesState.error}</span>
          <button onClick={() => profilesState.setError(null)}>
            <X className="h-4 w-4" />
          </button>
        </div>
      )}

      {updaterBanner && (
        <div
          className={`mb-4 flex items-start justify-between gap-3 rounded-lg border px-4 py-3 text-sm ${
            updaterBanner.tone === "err"
              ? "border-red-300 bg-red-50 text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-200"
              : updaterBanner.tone === "info"
                ? "border-blue-300 bg-blue-50 text-blue-700 dark:border-blue-900 dark:bg-blue-950/50 dark:text-blue-200"
                : "border-green-300 bg-green-50 text-green-700 dark:border-green-900 dark:bg-green-950/50 dark:text-green-200"
          }`}
        >
          <span>{updaterBanner.text}</span>
          <span className="flex shrink-0 items-center gap-2">
            {updater.status.state === "available" && (
              <button
                onClick={updater.install}
                className="rounded-md bg-blue-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-blue-500"
              >
                Install & restart
              </button>
            )}
            <button onClick={updater.dismiss}>
              <X className="h-4 w-4" />
            </button>
          </span>
        </div>
      )}

      {/* Toolbar */}
      <div className="mb-5 flex flex-wrap items-center gap-3">
        <div className="relative min-w-56 flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
          <input
            ref={searchRef}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search profiles... (press / to focus)"
            className="w-full rounded-lg border border-neutral-300 bg-white py-2 pl-9 pr-3 text-sm outline-none placeholder:text-neutral-400 focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
          />
        </div>

        <Select
          className="w-40"
          value={statusFilter}
          onChange={(v) => setStatusFilter(v as typeof statusFilter)}
          options={[
            { value: "all", label: "All status" },
            { value: "running", label: "Running" },
            { value: "stopped", label: "Stopped" },
          ]}
        />

        <Select
          className="w-44"
          value={browserFilter}
          onChange={setBrowserFilter}
          options={[
            { value: "all", label: "All browsers" },
            ...browserTypes.map((t) => ({ value: t, label: t })),
          ]}
        />

        <Select
          className="w-40"
          value={tagFilter}
          onChange={setTagFilter}
          options={[
            { value: "all", label: "All tags" },
            ...groupsState.groups.map((g) => ({ value: g.id, label: g.name })),
          ]}
        />

        <Select
          className="w-56"
          value={sortBy}
          onChange={(v) => setSortBy(v as SortKey)}
          options={[
            { value: "name-asc", label: "Sort: Name A–Z" },
            { value: "name-desc", label: "Sort: Name Z–A" },
            { value: "last-used-desc", label: "Sort: Last used (newest)" },
            { value: "last-used-asc", label: "Sort: Last used (oldest)" },
            { value: "created-desc", label: "Sort: Created (newest)" },
            { value: "created-asc", label: "Sort: Created (oldest)" },
            { value: "status", label: "Sort: Status" },
          ]}
        />

        <button
          onClick={launchAllStopped}
          disabled={busy}
          title="Launch every stopped profile in view"
          className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800"
        >
          <Play className="h-4 w-4" /> Launch all
        </button>
        <button
          onClick={stopAllRunning}
          disabled={busy}
          title="Stop every running profile in view"
          className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800"
        >
          <Square className="h-4 w-4" /> Stop all
        </button>
        <button
          onClick={profilesState.refresh}
          title="Refresh (R)"
          className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 dark:text-neutral-300 dark:hover:bg-neutral-800"
        >
          <RefreshCw className="h-4 w-4" /> Refresh
        </button>
      </div>

      {/* Profile grid */}
      <ProfileList
        profiles={filtered}
        loading={profilesState.loading}
        proxyById={proxyById}
        onLaunch={profilesState.launch}
        onStop={profilesState.stop}
        onEdit={openEdit}
        onDelete={profilesState.remove}
        onDuplicate={profilesState.duplicate}
        onTogglePin={handleTogglePin}
        onCredentials={setCredentialsProfile}
        onError={profilesState.setError}
        onCreate={openCreate}
      />

      {/* Modals */}
      {formOpen && (
        <ProfileForm
          profile={editing}
          proxies={proxiesState.proxies}
          groups={groupsState.groups}
          defaultBrowserType={appSettings?.defaultBrowserType ?? "chromium"}
          onCreateGroup={groupsState.create}
          onClose={() => setFormOpen(false)}
          onSubmit={async (input, groupIds) => {
            const saved = editing
              ? await profilesState.update(editing.id, input)
              : await profilesState.create(input);
            await setProfileGroups(saved.id, groupIds);
            await profilesState.refresh();
            setFormOpen(false);
          }}
        />
      )}

      {proxyManagerOpen && (
        <ProxyManager proxiesState={proxiesState} onClose={() => setProxyManagerOpen(false)} />
      )}

      {historyOpen && <HistoryModal onClose={() => setHistoryOpen(false)} onRefreshKey={historyKey} />}

      {helpOpen && <ShortcutsHelp onClose={() => setHelpOpen(false)} />}

      {settingsOpen && (
        <SettingsModal
          onClose={() => setSettingsOpen(false)}
          onSaved={(s) => setAppSettings(s)}
        />
      )}

      {rulesOpen && (
        <WindowRulesModal
          groups={groupsState.groups}
          settings={appSettings}
          onWorkspaceChange={handleWorkspaceChange}
          onClose={() => setRulesOpen(false)}
        />
      )}

      {credentialsProfile && (
        <CredentialsModal profile={credentialsProfile} onClose={() => setCredentialsProfile(null)} />
      )}

      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} commands={paletteCommands} />
    </div>
  );
}
