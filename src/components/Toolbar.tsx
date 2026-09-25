import { Play, Activity, RefreshCw, Search, Square } from "lucide-react";
import { Select } from "./Select";
import { SORT_OPTIONS, type SortKey } from "../hooks/useProfileFilters";
import type { Group } from "../types";

interface Props {
  search: string;
  onSearch: (v: string) => void;
  searchRef: React.RefObject<HTMLInputElement | null>;
  statusFilter: "all" | "running" | "stopped";
  onStatusFilter: (v: "all" | "running" | "stopped") => void;
  browserFilter: string;
  onBrowserFilter: (v: string) => void;
  browserTypes: string[];
  tagFilter: string;
  onTagFilter: (v: string) => void;
  groups: Group[];
  sortBy: SortKey;
  onSortBy: (v: SortKey) => void;
  /** Live progress of a bulk action (from backend `bulk-progress` events). */
  bulk: { action: "launch" | "stop"; done: number; total: number } | null;
  /** Live RAM/CPU panel (feature 2). */
  onOpenResources: () => void;
  onLaunchAll: () => void;
  onStopAll: () => void;
  onRefresh: () => void;
}

/** Search box, filter/sort dropdowns and bulk-action buttons. */
export function Toolbar(props: Props) {
  const {
    search,
    onSearch,
    searchRef,
    statusFilter,
    onStatusFilter,
    browserFilter,
    onBrowserFilter,
    browserTypes,
    tagFilter,
    onTagFilter,
    groups,
    sortBy,
    onSortBy,
    bulk,
    onOpenResources,
    onLaunchAll,
    onStopAll,
    onRefresh,
  } = props;
  const busy = bulk !== null;
  const bulkLabel = (action: "launch" | "stop") =>
    bulk && bulk.action === action ? `${action === "launch" ? "Launching" : "Stopping"} ${bulk.done}/${bulk.total}` : null;

  return (
    <div className="mb-5 flex flex-wrap items-center gap-3">
      <div className="relative min-w-56 flex-1">
        <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
        <input
          ref={searchRef}
          value={search}
          onChange={(e) => onSearch(e.target.value)}
          placeholder="Search profiles... (press / to focus)"
          className="w-full rounded-lg border border-neutral-300 bg-white py-2 pl-9 pr-3 text-sm outline-none placeholder:text-neutral-400 focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
        />
      </div>

      <Select
        className="w-40"
        value={statusFilter}
        onChange={(v) => onStatusFilter(v as typeof statusFilter)}
        options={[
          { value: "all", label: "All status" },
          { value: "running", label: "Running" },
          { value: "stopped", label: "Stopped" },
        ]}
      />

      <Select
        className="w-44"
        value={browserFilter}
        onChange={onBrowserFilter}
        options={[
          { value: "all", label: "All browsers" },
          ...browserTypes.map((t) => ({ value: t, label: t })),
        ]}
      />

      <Select
        className="w-40"
        value={tagFilter}
        onChange={onTagFilter}
        options={[
          { value: "all", label: "All tags" },
          ...groups.map((g) => ({ value: g.id, label: g.name })),
        ]}
      />

      <Select className="w-56" value={sortBy} onChange={(v) => onSortBy(v as SortKey)} options={SORT_OPTIONS} />

      <button
        onClick={onLaunchAll}
        disabled={busy}
        title="Launch every stopped profile in view"
        className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800"
      >
        <Play className="h-4 w-4" /> {bulkLabel("launch") ?? "Launch all"}
      </button>
      <button
        onClick={onStopAll}
        disabled={busy}
        title="Stop every running profile in view"
        className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800"
      >
        <Square className="h-4 w-4" /> {bulkLabel("stop") ?? "Stop all"}
      </button>
      <button
        onClick={onRefresh}
        disabled={busy}
        title="Refresh (R)"
        className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800"
      >
        <RefreshCw className="h-4 w-4" /> Refresh
      </button>
      <button
        onClick={onOpenResources}
        title="Resource usage of running browsers"
        className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200/70 disabled:opacity-50 dark:text-neutral-300 dark:hover:bg-neutral-800"
      >
        <Activity className="h-4 w-4" /> Resources
      </button>
    </div>
  );
}
