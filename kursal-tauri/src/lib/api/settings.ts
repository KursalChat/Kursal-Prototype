import { invoke } from "@tauri-apps/api/core";

export type PeerRotationInterval = "6h" | "12h" | "30h" | "7d" | "manual";

export type AppLockMethod = "none" | "password" | "biometric";
export interface AppLockConfig {
  enabled: boolean;
  method: AppLockMethod;
}

export interface RelayConfig {
  enabled: boolean;
  maxConnections: number;
  maxConnectionsPerIp: number;
}

export type AutoAcceptMode = "nobody" | "verified" | "all";
export interface AutoAcceptConfig {
  mode: AutoAcceptMode;
  sizeCapBytes: number;
}

export type AutoDownloadScope = "per_contact" | "all_contacts";
export interface AutoDownloadConfig {
  scope: AutoDownloadScope;
  limitBytes: number;
}

export interface SharedFileEntry {
  id: string;
  filepath: string;
  sizeBytes: number;
  recipientId: string;
  sharedAt: number;
  lastAccessedAt: number | null;
}

export interface ContactUsage {
  contactId: string;
  dbBytes: number;
  filesBytes: number;
}
export interface StorageUsage {
  logsBytes: number;
  dbBytes: number;
  filesBytes: number;
  perContact: ContactUsage[];
}

export interface LocalApiConfig {
  enabled: boolean;
  hostOnNetwork: boolean;
  port: number;
}

// Peer rotation
export const getPeerRotationInterval = (): Promise<PeerRotationInterval> =>
  invoke("get_peer_rotation_interval");
export const setPeerRotationInterval = (
  interval: PeerRotationInterval,
): Promise<void> => invoke("set_peer_rotation_interval", { interval });

// App lock
export const getAppLockConfig = (): Promise<AppLockConfig> =>
  invoke("get_app_lock_config");
export const setAppLock = (
  enabled: boolean,
  method: AppLockMethod,
  password: string | null,
): Promise<void> => invoke("set_app_lock", { enabled, method, password });
export const verifyAppLock = (password: string): Promise<boolean> =>
  invoke("verify_app_lock", { password });

// Typing indicators
export const getTypingIndicatorsEnabled = (): Promise<boolean> =>
  invoke("get_typing_indicators_enabled");
export const setTypingIndicatorsEnabled = (enabled: boolean): Promise<void> =>
  invoke("set_typing_indicators_enabled", { value: enabled });

// Blocked contacts
export const listBlockedContacts = (): Promise<
  import("$lib/types").ContactResponse[]
> => invoke("list_blocked_contacts");

// Destructive
export const clearMessageHistory = (contactId: string | null): Promise<void> =>
  invoke("clear_message_history", { contactId });
export const deleteAllLocalData = (): Promise<void> =>
  invoke("delete_all_local_data");

// Relay
export const getRelayConfig = (): Promise<RelayConfig> =>
  invoke("get_relay_config");
export const setRelayConfig = (config: RelayConfig): Promise<void> =>
  invoke("set_relay_config", { config });

// Listening port
export const getListeningPort = (): Promise<number | null> =>
  invoke("get_listening_port");
export const setListeningPort = (port: number | null): Promise<void> =>
  invoke("set_listening_port", { port });

// Nearby share
export const getNearbyShareEnabled = (): Promise<boolean> =>
  invoke("get_nearby_share_enabled");
export const setNearbyShareEnabled = (enabled: boolean): Promise<void> =>
  invoke("set_nearby_share_enabled", { value: enabled });

// Shared files
export const listSharedFiles = (): Promise<SharedFileEntry[]> =>
  invoke("list_shared_files");
export const revokeSharedFile = (id: string): Promise<void> =>
  invoke("revoke_shared_file", { id });
export const revokeSharedFilesBulk = (ids: string[]): Promise<void> =>
  invoke("revoke_shared_files_bulk", { ids });

// Auto-accept
export const getAutoAcceptConfig = (): Promise<AutoAcceptConfig> =>
  invoke("get_auto_accept_config");
export const setAutoAcceptConfig = (config: AutoAcceptConfig): Promise<void> =>
  invoke("set_auto_accept_config", { config });

// Auto-download storage
export const getAutoDownloadConfig = (): Promise<AutoDownloadConfig> =>
  invoke("get_auto_download_config");
export const setAutoDownloadConfig = (
  config: AutoDownloadConfig,
): Promise<void> => invoke("set_auto_download_config", { config });

// Storage usage
export const getStorageUsage = (): Promise<StorageUsage> =>
  invoke("get_storage_usage");

// Auto-updater
export const getUpdaterEnabled = (): Promise<boolean> =>
  invoke("get_updater_enabled");
export const setUpdaterEnabled = (value: boolean): Promise<void> =>
  invoke("set_updater_enabled", { value });

// Local API
export const getLocalApiConfig = (): Promise<LocalApiConfig> =>
  invoke("get_local_api_config");
export const setLocalApiConfig = (config: LocalApiConfig): Promise<void> =>
  invoke("set_local_api_config", { config });
export const generateLocalApiToken = (): Promise<string> =>
  invoke("generate_local_api_token");
