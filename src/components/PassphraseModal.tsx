import { useEffect, useRef, useState } from "react";
import { KeyRound, Loader2, X } from "lucide-react";

interface Props {
  /** "export" = optional passphrase (empty → plaintext), "import" = required. */
  mode: "export" | "import";
  /** Shown in the header so the user knows which file is being processed. */
  fileName: string;
  /** Backend error that triggered this modal (import retry flow). */
  error?: string | null;
  onSubmit: (passphrase: string) => Promise<void>;
  onClose: () => void;
}

/**
 * Passphrase prompt for encrypted backups (feature 3). Export: an optional
 * passphrase encrypts the file; leaving it empty keeps the plaintext export.
 * Import: encrypted files require the passphrase — wrong ones retry here
 * instead of failing into the error banner.
 */
export function PassphraseModal({ mode, fileName, error, onSubmit, onClose }: Props) {
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLocalError(null);
    if (mode === "export" && passphrase && passphrase !== confirm) {
      setLocalError("Passphrases do not match.");
      return;
    }
    if (mode === "export" && passphrase && passphrase.length < 8) {
      setLocalError("Passphrase must be at least 8 characters (or empty for plaintext).");
      return;
    }
    if (mode === "import" && !passphrase) {
      setLocalError("This backup is encrypted — a passphrase is required.");
      return;
    }
    setBusy(true);
    try {
      await onSubmit(passphrase);
    } catch (err) {
      // Wrong passphrase etc. — stay open so the user can retry.
      setLocalError(String(err).replace(/^Error:\s*/i, "").replace(/^"|"$/g, ""));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/70 p-4">
      <div className="w-full max-w-sm rounded-xl border border-neutral-200 bg-white p-6 shadow-xl dark:border-neutral-800 dark:bg-neutral-900">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-base font-semibold">
            <KeyRound className="h-4 w-4" />
            {mode === "export" ? "Encrypt backup" : "Encrypted backup"}
          </h2>
          <button onClick={onClose} className="text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-300">
            <X className="h-5 w-5" />
          </button>
        </div>

        <p className="mb-1 break-all text-xs text-neutral-500 dark:text-neutral-400">{fileName}</p>
        {mode === "export" ? (
          <p className="mb-4 text-xs text-neutral-500 dark:text-neutral-400">
            Enter a passphrase to encrypt the backup (AES-256-GCM). Leave empty to
            export it as plain JSON.
          </p>
        ) : (
          <p className="mb-4 text-xs text-neutral-500 dark:text-neutral-400">
            This backup file is encrypted. Enter the passphrase it was exported with.
          </p>
        )}

        {(localError || error) && (
          <div className="mb-3 rounded-lg border border-red-300 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-200">
            {localError ?? error}
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-3">
          <input
            ref={inputRef}
            type="password"
            value={passphrase}
            onChange={(e) => setPassphrase(e.target.value)}
            placeholder="Passphrase"
            className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-950"
          />
          {mode === "export" && passphrase && (
            <input
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              placeholder="Repeat passphrase"
              className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-blue-600 dark:border-neutral-800 dark:bg-neutral-950"
            />
          )}

          <div className="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="rounded-lg px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={busy}
              className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50"
            >
              {busy && <Loader2 className="h-4 w-4 animate-spin" />}
              {mode === "export"
                ? passphrase
                  ? "Export encrypted"
                  : "Export plaintext"
                : "Unlock"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
