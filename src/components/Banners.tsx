import { X } from "lucide-react";

interface Props {
  error: string | null;
  onDismissError: () => void;
}

/** Error banner, stacked below the header. */
export function Banners({ error, onDismissError }: Props) {
  if (!error) return null;

  return (
    <div className="mb-4 flex items-start justify-between gap-3 rounded-lg border border-red-300 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-200">
      <span>{error}</span>
      <button onClick={onDismissError}>
        <X className="h-4 w-4" />
      </button>
    </div>
  );
}
