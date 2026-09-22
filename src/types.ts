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
  groups: Group[];
}

export interface CreateProfileInput {
  name: string;
  browserType: string;
  proxyId: string | null;
  notes: string | null;
}

export interface UpdateProfileInput {
  name?: string;
  browserType?: string;
  proxyId?: string | null;
  notes?: string | null;
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
