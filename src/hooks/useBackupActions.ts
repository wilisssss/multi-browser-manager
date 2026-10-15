import { save, open } from "@tauri-apps/plugin-dialog";
import { exportProfiles, exportProfilesEncrypted, importProfiles } from "../lib/tauri-api";
import type { Modal } from "../components/modalTypes";

interface Options {
  refresh: () => Promise<unknown>;
  refreshProxies: () => Promise<unknown>;
  setError: (message: string | null) => void;
  setModal: (modal: Modal) => void;
  setBusy: (busy: boolean) => void;
}

/** Profile export/import flows, including the passphrase prompts. */
export function useBackupActions({ refresh, refreshProxies, setError, setModal, setBusy }: Options) {
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
      setError(String(e));
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
        await Promise.all([refresh(), refreshProxies()]);
        return;
      } catch (e) {
        const message = String(e);
        if (message.toLowerCase().includes("passphrase") || message.toLowerCase().includes("encrypted")) {
          setModal({ kind: "importPassphrase", fileName: path });
          return;
        }
        setError(message);
        return;
      } finally {
        setBusy(false);
      }
    }
    try {
      setBusy(true);
      const result = await importProfiles(path, passphrase || undefined);
      await Promise.all([refresh(), refreshProxies()]);
      setModal(null);
      const summary =
        `Imported ${result.importedProfiles} profiles, ${result.importedProxies} proxies, ` +
        `${result.importedCredentials} credentials. ` +
        `Skipped: ${result.skippedProfiles} profiles, ${result.skippedProxies} proxies, ` +
        `${result.skippedCredentials} credentials.`;
      // Surface skipped/conflict info even on success — those are expected
      // outcomes, not errors, but the user must see them.
      setError(result.conflicts.length > 0 ? summary : null);
    } catch (e) {
      // Wrong passphrase etc. — stay in the prompt so the user can retry.
      throw e;
    } finally {
      setBusy(false);
    }
  };

  return { handleExport, handleImport };
}
