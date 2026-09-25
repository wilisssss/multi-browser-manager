import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Activity,
  AppWindow,
  Download,
  History as HistoryIcon,
  Keyboard,
  Moon,
  Play,
  Plus,
  RefreshCw,
  Settings as SettingsIcon,
  Settings2,
  Square,
  Sun,
  Undo2,
  Upload,
} from "lucide-react";
import { useProfiles } from "../hooks/useProfiles";
import { useProxies } from "../hooks/useProxies";
import { useGroups } from "../hooks/useGroups";
import { useUpdater } from "../hooks/useUpdater";
import { useProfileFilters } from "../hooks/useProfileFilters";
import { ProfileList } from "./ProfileList";
import { ProfileForm } from "./ProfileForm";
import { ProxyManager } from "./ProxyManager";
import { HistoryModal } from "./HistoryModal";
import { ShortcutsHelp } from "./ShortcutsHelp";
import { CommandPalette, type PaletteCommand } from "./CommandPalette";
import { SettingsModal } from "./SettingsModal";
import { WindowRulesModal } from "./WindowRulesModal";
import { CredentialsModal } from "./CredentialsModal";
import { ResourcesModal } from "./ResourcesModal";
import { PassphraseModal } from "./PassphraseModal";
import { DashboardHeader } from "./DashboardHeader";
import { Toolbar } from "./Toolbar";
import { Banners } from "./Banners";
import {
  exportProfiles,
  exportProfilesEncrypted,
  importProfiles,
  bulkLaunch,
  bulkStop,
  setProfileGroups,
  togglePin,
  getSettings,
  updateSettings,
  getUsageStats,
  getResourceUsage,
  restoreProfile,
} from "../lib/tauri-api";
import { getVersion } from "@tauri-apps/api/app";
import { save, open } from "@tauri-apps/plugin-dialog";
import type { Profile } from "../types";
import type { Theme } from "../hooks/useTheme";

interface Props {
  theme: Theme;
  toggleTheme: () => void;
}

/** Progress of an in-flight bulk action, fed by backend `bulk-progress` events. */
interface BulkProgress {
  action: "launch" | "stop";
  done: number;
  total: number;
}

/**
 * A4: one discriminated-union state instead of eight booleans. Only the top
 * modal exists at a time, so Escape always closes exactly one modal (L13) and
 * Ctrl+K can replace whatever is open with the palette.
 */
type Modal =
  | { kind: "profileForm"; editing: Profile | null }
  | { kind: "proxyManager" }
  | { kind: "history" }
  | { kind: "help" }
  | { kind: "settings" }
  | { kind: "rules" }
  | { kind: "credentials"; profile: Profile }
  | { kind: "resources" }
  | { kind: "palette" }
  | { kind: "exportPassphrase"; fileName: string }
  | { kind: "importPassphrase"; fileName: string }
  | null;

/** How long the "Profile deleted — Undo" banner stays actionable. */
const UNDO_WINDOW_MS = 30_000;

export function Dashboard({ theme, toggleTheme }: Props) {
  const profilesState = useProfiles();
  const proxiesState = useProxies();
  const groupsState = useGroups();
  const updater = useUpdater();

  const [modal, setModal] = useState<Modal>(null);
  const [historyKey, setHistoryKey] = useState(0);
  const [appSettings, setAppSettings] = useState<import("../types").AppSettings | null>(null);
  const [appVersion, setAppVersion] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [bulk, setBulk] = useState<BulkProgress | null>(null);
  /** F6: last deleted profile + deadline, for the undo banner. */
  const [undoTrash, setUndoTrash] = useState<{ id: string; name: string; until: number } | null>(null);
  /** F5: total browser seconds per profile id. */
  const [usageSeconds, setUsageSeconds] = useState<Record<string, number>>({});
  /** F2: live RAM/CPU per running profile id (polls only while something runs). */
  const [liveUsage, setLiveUsage] = useState<Record<string, { memoryKb: number; cpuPercent: number | null }>>({});

  // Displayed next to the app title.
  useEffect(() => {
    getVersion()
      .then(setAppVersion)
      .catch(() => {});
  }, []);

  // Settings are loaded once and kept in sync with the settings modal.
  useEffect(() => {
    getSettings()
      .then(setAppSettings)
      .catch(() => {});
  }, []);

  const runningCount = profilesState.profiles.filter((p) => p.status === "running").length;

  // Refresh when the backend signals external changes (e.g. `mbm --launch` from a WM keybind).
  useEffect(() => {
    const unlistenPromise = listen("profiles-changed", () => profilesState.refresh());
    return () => {
      unlistenPromise.then((fn) => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Usage statistics (F5): refetch together with the profile list — the
  // cheapest correct trigger, since launches/stops change both.
  useEffect(() => {
    getUsageStats()
      .then((stats) => {
        const map: Record<string, number> = {};
        for (const s of stats) map[s.profileId] = s.seconds;
        setUsageSeconds(map);
      })
      .catch(() => {});
  }, [profilesState.profiles]);

  // Live resource usage (F2): poll while any browser runs (or the panel is
  // open), pause entirely otherwise.
  useEffect(() => {
    const shouldPoll = runningCount > 0 || modal?.kind === "resources";
    if (!shouldPoll) {
      setLiveUsage({});
      return;
    }
    let cancelled = false;
    const poll = () =>
      getResourceUsage()
        .then((list) => {
          if (cancelled) return;
          const map: Record<string, { memoryKb: number; cpuPercent: number | null }> = {};
          for (const u of list) map[u.profileId] = { memoryKb: u.memoryKb, cpuPercent: u.cpuPercent };
          setLiveUsage(map);
        })
        .catch(() => {});
    poll();
    const timer = setInterval(poll, 3000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [runningCount, modal?.kind]);

  // F6: expire the undo banner after its window.
  useEffect(() => {
    if (!undoTrash) return;
    const remaining = undoTrash.until - Date.now();
    if (remaining <= 0) {
      setUndoTrash(null);
      return;
    }
    const t = setTimeout(() => setUndoTrash(null), remaining);
    return () => clearTimeout(t);
  }, [undoTrash]);

  /** Persists one tag → workspace mapping change. */
  const handleWorkspaceChange = useCallback(
    async (groupId: string, workspace: number | null) => {
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
    },
    [],
  );

  // O(1) proxy lookup per card instead of an O(P×Q) Array.find per render.
  const proxyById = useMemo(
    () => new Map(proxiesState.proxies.map((p) => [p.id, p])),
    [proxiesState.proxies],
  );

  const filters = useProfileFilters({
    profiles: profilesState.profiles,
    groups: groupsState.groups,
    usageSeconds,
  });
  const {
    search,
    setSearch,
    statusFilter,
    setStatusFilter,
    browserFilter,
    setBrowserFilter,
    tagFilter,
    setTagFilter,
    sortBy,
    setSortBy,
    browserTypes,
    filtered,
    searchRef,
  } = filters;

  const anyModalOpen = modal !== null;

  // Live progress for bulk launch/stop (fires once per profile completed).
  useEffect(() => {
    const unlistenPromise = listen<BulkProgress & { profileId: string; success: boolean }>(
      "bulk-progress",
      (event) => {
        const { action, done, total } = event.payload;
        setBulk({ action, done, total });
      },
    );
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  // Global keyboard shortcuts (ignored while typing or when a modal is open).
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Ctrl+K toggles the command palette from anywhere, even while typing —
      // and replaces whatever modal is open (A4).
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setModal((m) => (m?.kind === "palette" ? null : { kind: "palette" }));
        return;
      }
      if (e.key === "Escape") {
        // Only the (single) top modal closes — L13.
        setModal((m) => (m === null ? m : null));
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
          setModal((m) => (m?.kind === "proxyManager" ? null : { kind: "proxyManager" }));
          break;
        case "h":
          setModal((m) => (m?.kind === "history" ? null : { kind: "history" }));
          break;
        case "t":
          toggleTheme();
          break;
        case "r":
          profilesState.refresh();
          break;
        case "?":
          setModal({ kind: "help" });
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [anyModalOpen, toggleTheme]);

  const handleExport = async (passphrase?: string) => {
    const fileName = "mbm-backup.json";
    const path = await save({
      title: "Export profiles",
      defaultPath: fileName,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    if (passphrase === undefined) {
      // First step: ask whether to encrypt (empty = plaintext).
      setModal({ kind: "exportPassphrase", fileName: path });
      return;
    }
    try {
      setBusy(true);
      if (passphrase) {
        await exportProfilesEncrypted(path, passphrase);
      } else {
        await exportProfiles(path);
      }
      setModal(null);
    } catch (e) {
      profilesState.setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleImport = async (passphrase?: string) => {
    const path = await open({
      title: "Import profiles",
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path || Array.isArray(path)) return;
    if (passphrase === undefined) {
      // Try plaintext first; an encrypted file errors with a clear message and
      // we reopen this flow with the passphrase prompt (import retry).
      try {
        setBusy(true);
        await importProfiles(path);
        await Promise.all([profilesState.refresh(), proxiesState.refresh()]);
        return;
      } catch (e) {
        const message = String(e);
        if (message.toLowerCase().includes("passphrase") || message.toLowerCase().includes("encrypted")) {
          setModal({ kind: "importPassphrase", fileName: path });
          return;
        }
        profilesState.setError(message);
        return;
      } finally {
        setBusy(false);
      }
    }
    try {
      setBusy(true);
      const result = await importProfiles(path, passphrase || undefined);
      await Promise.all([profilesState.refresh(), proxiesState.refresh()]);
      setModal(null);
      const summary =
        `Imported ${result.importedProfiles} profiles, ${result.importedProxies} proxies, ` +
        `${result.importedCredentials} credentials. ` +
        `Skipped: ${result.skippedProfiles} profiles, ${result.skippedProxies} proxies, ` +
        `${result.skippedCredentials} credentials.`;
      // Surface skipped/conflict info even on success — those are expected
      // outcomes, not errors, but the user must see them.
      profilesState.setError(result.conflicts.length > 0 ? summary : null);
    } catch (e) {
      // Wrong passphrase etc. — stay in the prompt so the user can retry.
      throw e;
    } finally {
      setBusy(false);
    }
  };

  const launchAllStopped = async () => {
    const ids = filtered.filter((p) => p.status === "stopped").map((p) => p.id);
    if (ids.length === 0) return;
    setBusy(true);
    setBulk({ action: "launch", done: 0, total: ids.length });
    try {
      const results = await bulkLaunch(ids);
      await profilesState.refresh();
      reportBulkFailures(results, "launch");
    } catch (e) {
      profilesState.setError(String(e));
    } finally {
      setBulk(null);
      setBusy(false);
    }
  };

  const stopAllRunning = async () => {
    const ids = filtered.filter((p) => p.status === "running").map((p) => p.id);
    if (ids.length === 0) return;
    setBusy(true);
    setBulk({ action: "stop", done: 0, total: ids.length });
    try {
      const results = await bulkStop(ids);
      await profilesState.refresh();
      reportBulkFailures(results, "stop");
    } catch (e) {
      profilesState.setError(String(e));
    } finally {
      setBulk(null);
      setBusy(false);
    }
  };

  /** Surfaces per-profile bulk failures that the backend collected. */
  const reportBulkFailures = (
    results: { profileId: string; success: boolean; message: string }[],
    action: string,
  ) => {
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
    setModal({ kind: "profileForm", editing: null });
    // ProfileForm fetches the browser list itself — no double probe here.
  }, []);

  const openEdit = useCallback((profile: Profile) => {
    setModal({ kind: "profileForm", editing: profile });
  }, []);

  /** Delete → trash (F6) with a 30-second undo banner. */
  const deleteWithUndo = useCallback(
    async (id: string) => {
      const info = await profilesState.remove(id);
      setUndoTrash({ id: info.id, name: info.name, until: Date.now() + UNDO_WINDOW_MS });
    },
    [profilesState.remove],
  );

  const handleUndoDelete = useCallback(async () => {
    if (!undoTrash) return;
    const { id } = undoTrash;
    setUndoTrash(null);
    try {
      await restoreProfile(id);
      await profilesState.refresh();
    } catch (e) {
      profilesState.setError(String(e));
    }
  }, [undoTrash, profilesState.refresh]);

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
      action: () => setModal({ kind: "proxyManager" }),
    },
    {
      id: "history",
      label: "Launch history",
      section: "Actions",
      hint: "H",
      icon: HistoryIcon,
      action: () => {
        setHistoryKey((k) => k + 1);
        setModal({ kind: "history" });
      },
    },
    {
      id: "resources",
      label: "Resource usage",
      section: "Actions",
      icon: Activity,
      action: () => setModal({ kind: "resources" }),
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
      label: "Export backup...",
      section: "Actions",
      icon: Download,
      action: () => handleExport(),
    },
    {
      id: "import",
      label: "Import backup...",
      section: "Actions",
      icon: Upload,
      action: () => handleImport(),
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
      action: () => setModal({ kind: "help" }),
    },
    {
      id: "window-rules",
      label: "Window rules & workspaces",
      section: "Actions",
      icon: AppWindow,
      action: () => setModal({ kind: "rules" }),
    },
    {
      id: "settings",
      label: "Settings",
      section: "Actions",
      icon: SettingsIcon,
      action: () => setModal({ kind: "settings" }),
    },
  ];

  return (
    <div className="mx-auto flex min-h-screen max-w-7xl flex-col px-6 py-6">
      <DashboardHeader
        theme={theme}
        profileCount={profilesState.profiles.length}
        runningCount={runningCount}
        appVersion={appVersion}
        updaterBusy={updater.status.state === "checking" || updater.status.state === "downloading"}
        onOpenPalette={() => setModal({ kind: "palette" })}
        onOpenRules={() => setModal({ kind: "rules" })}
        onOpenSettings={() => setModal({ kind: "settings" })}
        onOpenHistory={() => {
          setHistoryKey((k) => k + 1);
          setModal({ kind: "history" });
        }}
        onOpenHelp={() => setModal({ kind: "help" })}
        onCheckUpdates={updater.checkForUpdates}
        onToggleTheme={toggleTheme}
        onImport={() => handleImport()}
        onExport={() => handleExport()}
        onOpenProxies={() => setModal({ kind: "proxyManager" })}
        onCreate={openCreate}
        busy={busy}
      />

      <Banners
        error={profilesState.error}
        onDismissError={() => profilesState.setError(null)}
        status={updater.status}
        onInstallUpdate={updater.install}
        onDismissUpdate={updater.dismiss}
      />

      {/* F6: undo banner for a just-deleted profile. */}
      {undoTrash && (
        <div className="mb-4 flex items-center justify-between gap-3 rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950/50 dark:text-amber-200">
          <span>
            Profile <strong>{undoTrash.name}</strong> moved to trash. Its data is kept for 30 days.
          </span>
          <span className="flex shrink-0 items-center gap-3">
            <button
              onClick={handleUndoDelete}
              className="flex items-center gap-1.5 rounded-lg border border-amber-400 px-3 py-1.5 text-xs font-medium hover:bg-amber-100 dark:border-amber-700 dark:hover:bg-amber-900/40"
            >
              <Undo2 className="h-3.5 w-3.5" /> Undo
            </button>
            <button onClick={() => setUndoTrash(null)} className="text-amber-600 hover:text-amber-800 dark:text-amber-300">
              ✕
            </button>
          </span>
        </div>
      )}

      <Toolbar
        search={search}
        onSearch={setSearch}
        searchRef={searchRef}
        statusFilter={statusFilter}
        onStatusFilter={setStatusFilter}
        browserFilter={browserFilter}
        onBrowserFilter={setBrowserFilter}
        browserTypes={browserTypes}
        tagFilter={tagFilter}
        onTagFilter={setTagFilter}
        groups={groupsState.groups}
        sortBy={sortBy}
        onSortBy={setSortBy}
        bulk={bulk}
        onLaunchAll={launchAllStopped}
        onStopAll={stopAllRunning}
        onRefresh={profilesState.refresh}
        onOpenResources={() => setModal({ kind: "resources" })}
      />

      {/* Profile grid */}
      <ProfileList
        profiles={filtered}
        loading={profilesState.loading}
        proxyById={proxyById}
        usageById={liveUsage}
        usageSeconds={usageSeconds}
        onLaunch={profilesState.launch}
        onStop={profilesState.stop}
        onEdit={openEdit}
        onDelete={deleteWithUndo}
        onDuplicate={profilesState.duplicate}
        onTogglePin={handleTogglePin}
        onCredentials={(p) => setModal({ kind: "credentials", profile: p })}
        onError={profilesState.setError}
        onCreate={openCreate}
      />

      {/* Modals — A4: one union state, exactly one open at a time. */}
      {modal?.kind === "profileForm" && (
        <ProfileForm
          profile={modal.editing}
          proxies={proxiesState.proxies}
          groups={groupsState.groups}
          defaultBrowserType={appSettings?.defaultBrowserType ?? "chromium"}
          onCreateGroup={groupsState.create}
          onClose={() => setModal(null)}
          onSubmit={async (input, groupIds) => {
            const saved = modal.editing
              ? await profilesState.update(modal.editing.id, input)
              : await profilesState.create(input);
            await setProfileGroups(saved.id, groupIds);
            await profilesState.refresh();
            setModal(null);
          }}
        />
      )}

      {modal?.kind === "proxyManager" && (
        <ProxyManager proxiesState={proxiesState} onClose={() => setModal(null)} />
      )}

      {modal?.kind === "history" && <HistoryModal onClose={() => setModal(null)} onRefreshKey={historyKey} />}

      {modal?.kind === "help" && <ShortcutsHelp onClose={() => setModal(null)} />}

      {modal?.kind === "settings" && (
        <SettingsModal
          onClose={() => setModal(null)}
          onSaved={(s) => setAppSettings(s)}
        />
      )}

      {modal?.kind === "rules" && (
        <WindowRulesModal
          groups={groupsState.groups}
          settings={appSettings}
          onWorkspaceChange={handleWorkspaceChange}
          onClose={() => setModal(null)}
        />
      )}

      {modal?.kind === "credentials" && (
        <CredentialsModal profile={modal.profile} onClose={() => setModal(null)} />
      )}

      {modal?.kind === "resources" && (
        <ResourcesModal
          names={Object.fromEntries(profilesState.profiles.map((p) => [p.id, p.name]))}
          onClose={() => setModal(null)}
        />
      )}

      {modal?.kind === "exportPassphrase" && (
        <PassphraseModal
          mode="export"
          fileName={modal.fileName}
          onSubmit={(pass) => handleExport(pass)}
          onClose={() => setModal(null)}
        />
      )}

      {modal?.kind === "importPassphrase" && (
        <PassphraseModal
          mode="import"
          fileName={modal.fileName}
          onSubmit={(pass) => handleImport(pass)}
          onClose={() => setModal(null)}
        />
      )}

      {modal?.kind === "palette" && (
        <CommandPalette open onClose={() => setModal(null)} commands={paletteCommands} />
      )}
    </div>
  );
}
