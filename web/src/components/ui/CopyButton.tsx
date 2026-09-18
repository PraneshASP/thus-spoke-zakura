import { useEffect, useState, type ReactNode } from 'react';
import { Check } from 'lucide-react';

/**
 * Copying gave no feedback in the previous UI, so there was no way to tell a
 * successful copy from a no-op. The confirmation is also announced politely
 * for screen readers.
 */
export function CopyButton({
  value,
  label,
  icon,
}: {
  value: string;
  label: string;
  icon: ReactNode;
}) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 1600);
    return () => clearTimeout(timer);
  }, [copied]);

  return (
    <button
      type="button"
      aria-label={label}
      className="text-ink-muted hover:text-accent cursor-pointer rounded transition-colors"
      onClick={() => {
        void navigator.clipboard.writeText(value).then(
          () => setCopied(true),
          () => setCopied(false),
        );
      }}
    >
      {copied ? <Check className="text-accent size-4" /> : icon}
      <span className="sr-only" aria-live="polite">
        {copied ? 'Copied' : ''}
      </span>
    </button>
  );
}
