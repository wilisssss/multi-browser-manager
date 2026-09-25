import { memo } from "react";
import { Globe, Loader2, Plus } from "lucide-react";
import { ProfileCard } from "./ProfileCard";
import type { Profile } from "../types";

interface Props {
  profiles: Profile[];
  loading: boolean;
  /** O(1) proxy lookup shared by every card (built once per proxies change). */
  proxyById: Map<string, import("../types").Proxy>;
  /** Live RAM/CPU per running profile id (feature 2), empty when not sampled. */
  usageById?: Record<string, { memoryKb: number; cpuPercent: number | null }>;
  /** Total browser seconds per profile id (feature 5). */
  usageSeconds?: Record<string, number>;
  onLaunch: (id: string) => Promise<void>;
  onStop: (id: string) => Promise<void>;
  onEdit: (profile: Profile) => void;
  onDelete: (id: string) => Promise<unknown>;
  onDuplicate: (id: string) => Promise<unknown>;
  onTogglePin: (id: string) => Promise<unknown>;
  onCredentials: (profile: Profile) => void;
  onError: (message: string) => void;
  onCreate: () => void;
}

export const ProfileList = memo(function ProfileList({
  profiles,
  loading,
  proxyById,
  usageById,
  usageSeconds,
  onLaunch,
  onStop,
  onEdit,
  onDelete,
  onDuplicate,
  onTogglePin,
  onCredentials,
  onError,
  onCreate,
}: Props) {
  if (loading) {
    return (
      <div className="flex flex-1 items-center justify-center py-24 text-neutral-500">
        <Loader2 className="mr-2 h-5 w-5 animate-spin" /> Loading profiles...
      </div>
    );
  }

  if (profiles.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-neutral-300 py-20 text-center dark:border-neutral-800">
        <Globe className="h-10 w-10 text-neutral-700" />
        <p className="text-sm text-neutral-400">No profiles found.</p>
        <button
          onClick={onCreate}
          className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-2 text-sm font-medium text-white hover:bg-blue-500"
        >
          <Plus className="h-4 w-4" /> Create your first profile
        </button>
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
      {profiles.map((profile) => (
        <ProfileCard
          key={profile.id}
          profile={profile}
          proxy={profile.proxyId ? proxyById.get(profile.proxyId) ?? null : null}
          usage={usageById?.[profile.id] ?? null}
          totalSeconds={usageSeconds?.[profile.id] ?? null}
          onLaunch={onLaunch}
          onStop={onStop}
          onEdit={onEdit}
          onDelete={onDelete}
          onDuplicate={onDuplicate}
          onTogglePin={onTogglePin}
          onCredentials={onCredentials}
          onError={onError}
        />
      ))}
    </div>
  );
});
