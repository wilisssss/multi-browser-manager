export interface Group {
  id: string;
  name: string;
  color: string | null;
}

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
  /** Extra launch args, space-separated with double-quote support. */
  extraArgs: string | null;
  /** Re-launch automatically after an unexpected exit. */
  restartOnCrash: boolean;
  /** Graceful-stop window in seconds; null = 3 s default. */
  stopTimeoutSecs: number | null;
  groups: Group[];
}

export interface CreateProfileInput {
  name: string;
  browserType: string;
  proxyId: string | null;
  notes: string | null;
  extraArgs?: string | null;
  restartOnCrash?: boolean;
  stopTimeoutSecs?: number | null;
}

export interface UpdateProfileInput {
  name?: string;
  browserType?: string;
  proxyId?: string | null;
  notes?: string | null;
  extraArgs?: string | null;
  restartOnCrash?: boolean;
  stopTimeoutSecs?: number | null;
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
  groupWorkspaces: Record<string, number>;
  /** Memory-trim launch mode: append curated RAM-saving flags. */
  lightweightBrowsers: boolean;
}

export interface WindowRule {
  profileId: string;
  profileName: string;
  appId: string;
  status: string;
  groupIds: string[];
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
