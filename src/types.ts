export interface Profile {
  id: string;
  name: string;
  browserType: string;
  userDataDir: string;
  proxyId: string | null;
  notes: string | null;
  status: "stopped" | "running";
  createdAt: number;
  updatedAt: number;
  lastUsedAt: number | null;
  pinned: boolean;
  /** Re-launch automatically after an unexpected exit. */
  restartOnCrash: boolean;
  /** Folder this profile lives in; null = unfiled (root's pseudo-view). */
  folderId: string | null;
  /** Identity overrides; null = browser defaults. */
  fingerprint: Fingerprint | null;
  /** Page opened when the browser starts; null = browser default. */
  startupUrl: string | null;
}

/** Per-profile identity overrides. Unset fields keep the browser's value. */
export interface Fingerprint {
  language?: string | null;
  userAgent?: string | null;
  timezone?: string | null;
  platform?: string | null;
  geolocation?: Geolocation | null;
  fonts?: string[] | null;
  screen?: Screen | null;
  /** Block WebRTC from revealing the host IP. Calls will not connect. */
  disableWebRTC?: boolean;
}

/** Reported screen size in CSS pixels. */
export interface Screen {
  width: number;
  height: number;
}

export interface Geolocation {
  latitude: number;
  longitude: number;
  accuracy?: number;
}

export interface CreateProfileInput {
  name: string;
  browserType: string;
  proxyId: string | null;
  notes: string | null;
  restartOnCrash?: boolean;
  folderId?: string | null;
  fingerprint?: Fingerprint | null;
  startupUrl?: string | null;
}

export interface UpdateProfileInput {
  name?: string;
  browserType?: string;
  proxyId?: string | null;
  notes?: string | null;
  restartOnCrash?: boolean;
  folderId?: string | null;
  fingerprint?: Fingerprint | null;
  startupUrl?: string | null;
}

/** A user-created folder grouping profiles (file-manager style, nestable). */
export interface Folder {
  id: string;
  name: string;
  parentId: string | null;
  position: number;
  createdAt: number;
  /** Number of profiles directly inside (filled by the backend). */
  profileCount: number;
}

/** Per-profile RAM/CPU usage of the running browser tree (Linux only). */
export interface ResourceUsage {
  profileId: string;
  pid: number;
  /** Resident memory of the whole process tree, in KiB. */
  memoryKb: number;
  /** CPU % since the previous sample; null on the first sample. */
  cpuPercent: number | null;
}

/** Aggregate usage statistics per profile (from launch history). */
export interface UsageStat {
  profileId: string;
  sessions: number;
  seconds: number;
}

/** Returned by delete_profile so the UI can offer "Undo". */
export interface DeletedProfileInfo {
  id: string;
  name: string;
}

export interface Proxy {
  id: string;
  label: string;
  protocol: "http" | "socks5";
  host: string;
  port: number;
  username: string | null;
  createdAt: number;
}

export interface CreateProxyInput {
  label: string;
  protocol: string;
  host: string;
  port: number;
  username?: string | null;
  password?: string | null;
}

export interface UpdateProxyInput {
  label?: string;
  protocol?: string;
  host?: string;
  port?: number;
  username?: string | null;
  password?: string | null;
}

export interface ProxyTestResult {
  success: boolean;
  message: string;
  latencyMs: number | null;
  ip: string | null;
}

export interface BrowserInfo {
  browserType: string;
  name: string;
  executablePath: string;
  version: string | null;
}

export interface LaunchResult {
  profileId: string;
  success: boolean;
  message: string;
}

export interface ImportResult {
  importedProfiles: number;
  importedProxies: number;
  importedCredentials: number;
  skippedProfiles: number;
  skippedProxies: number;
  skippedCredentials: number;
  conflicts: string[];
}

export interface AppSettings {
  historyRetentionDays: number;
  historyMaxEntries: number;
  autoBackup: boolean;
  autoBackupKeep: number;
  defaultBrowserType: string;
  /** Memory-trim launch mode: append curated RAM-saving flags. */
  lightweightBrowsers: boolean;
}

export interface Credential {
  id: string;
  profileId: string;
  platform: string;
  label: string;
  username: string | null;
  password: string | null;
  seedPhrase: string | null;
  evmAddress: string | null;
  notes: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface CreateCredentialInput {
  platform: string;
  label?: string;
  username?: string | null;
  password?: string | null;
  seedPhrase?: string | null;
  evmAddress?: string | null;
  notes?: string | null;
}

export interface UpdateCredentialInput {
  label?: string;
  username?: string | null;
  evmAddress?: string | null;
  notes?: string | null;
  /** Absent = keep, null = clear, value = replace (backend double-Option). */
  password?: string | null;
  seedPhrase?: string | null;
}
