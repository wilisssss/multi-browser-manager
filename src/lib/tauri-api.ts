import { invoke } from "@tauri-apps/api/core";
import type {
  BrowserInfo,
  CreateProfileInput,
  CreateProxyInput,
  Group,
  ImportResult,
  LaunchResult,
  Profile,
  Proxy,
  ProxyTestResult,
  UpdateProfileInput,
  UpdateProxyInput,
  AppSettings,
  WindowRule,
  Credential,
  CreateCredentialInput,
  UpdateCredentialInput,
} from "../types";

// Profile
export const getProfiles = () => invoke<Profile[]>("get_profiles");
export const createProfile = (input: CreateProfileInput) =>
  invoke<Profile>("create_profile", { input });
export const updateProfile = (id: string, input: UpdateProfileInput) =>
  invoke<Profile>("update_profile", { id, input });
export const deleteProfile = (id: string) => invoke<void>("delete_profile", { id });
export const duplicateProfile = (id: string) =>
  invoke<Profile>("duplicate_profile", { id });
export const togglePin = (id: string) => invoke<boolean>("toggle_pin", { id });

// Groups / tags
export const getGroups = () => invoke<Group[]>("get_groups");
export const createGroup = (name: string) => invoke<Group>("create_group", { name });
export const setProfileGroups = (profileId: string, groupIds: string[]) =>
  invoke<void>("set_profile_groups", { profileId, groupIds });

// Process
export const launchProfile = (id: string) => invoke<LaunchResult>("launch_profile", { id });
export const stopProfile = (id: string) => invoke<void>("stop_profile", { id });
export const bulkLaunch = (ids: string[]) => invoke<LaunchResult[]>("bulk_launch", { ids });
export const bulkStop = (ids: string[]) => invoke<LaunchResult[]>("bulk_stop", { ids });
export const getRunningProfiles = () => invoke<string[]>("get_running_profiles");

// Launch history
export interface HistoryEntry {
  id: number;
  profileName: string | null;
  launchedAt: number;
  closedAt: number | null;
  pid: number | null;
}
export const getLaunchHistory = (limit = 50) =>
  invoke<HistoryEntry[]>("get_launch_history", { limit });

// Browser
export const detectBrowsers = () => invoke<BrowserInfo[]>("detect_browsers");

// Proxy
export const getProxies = () => invoke<Proxy[]>("get_proxies");
export const createProxy = (input: CreateProxyInput) => invoke<Proxy>("create_proxy", { input });
export const updateProxy = (id: string, input: UpdateProxyInput) =>
  invoke<Proxy>("update_proxy", { id, input });
export const deleteProxy = (id: string) => invoke<void>("delete_proxy", { id });
export const testProxy = (id: string) => invoke<ProxyTestResult>("test_proxy", { id });

// Backup
export const exportProfiles = (path: string) => invoke<void>("export_profiles", { path });
export const importProfiles = (path: string) => invoke<ImportResult>("import_profiles", { path });

// Settings
export const getSettings = () => invoke<AppSettings>("get_settings");
export const updateSettings = (settings: AppSettings) =>
  invoke<AppSettings>("update_settings", { settings });
export const getBackupDir = () => invoke<string>("get_backup_dir");

// Window rules / workspaces
export const getWindowRules = () => invoke<WindowRule[]>("get_window_rules");

// Credentials (per-profile accounts; secrets live in the OS keychain)
export const getCredentials = (profileId: string) =>
  invoke<Credential[]>("get_credentials", { profileId });
export const createCredential = (profileId: string, input: CreateCredentialInput) =>
  invoke<Credential>("create_credential", { profileId, input });
export const updateCredential = (id: string, input: UpdateCredentialInput) =>
  invoke<Credential>("update_credential", { id, input });
export const deleteCredential = (id: string) => invoke<void>("delete_credential", { id });
