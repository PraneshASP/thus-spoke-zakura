import * as RadixDialog from '@radix-ui/react-dialog';
import { X } from 'lucide-react';
import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';

/**
 * Wraps Radix Dialog rather than reimplementing it. The previous modal was a
 * bare div: no focus trap, no Escape handling, no focus restoration, and no
 * `role="dialog"`/`aria-modal`. All of that is handled here by construction.
 */
export function Dialog({
  open,
  onOpenChange,
  eyebrow,
  title,
  description,
  children,
  className,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  eyebrow: string;
  title: string;
  description?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="bg-ink/35 animate-pop fixed inset-0 z-40" />
        <RadixDialog.Content
          className={cn(
            'fixed top-1/2 left-1/2 z-50 w-[min(430px,calc(100vw-36px))] -translate-x-1/2 -translate-y-1/2',
            'xp-window animate-pop rounded-xs shadow-(--shadow-modal)',
            'max-h-[calc(100vh-36px)] overflow-hidden overflow-y-auto',
            className,
          )}
        >
          <div className="xp-caption flex items-center justify-between gap-4 px-3 py-1.5">
            <RadixDialog.Title className="text-[13px] font-semibold tracking-wide">
              {title}
            </RadixDialog.Title>
            <RadixDialog.Close
              aria-label="Close dialog"
              className="xp-raised bg-raised text-ink xp-press grid size-5 cursor-pointer place-items-center rounded-xs"
            >
              <X className="size-3" />
            </RadixDialog.Close>
          </div>
          <div className="p-5">
            <span className="text-accent mb-1 block font-mono text-[11px] font-medium tracking-[0.13em]">
              {eyebrow}
            </span>
            {description ? (
              <RadixDialog.Description className="text-ink-muted mb-5 text-[12px]">
                {description}
              </RadixDialog.Description>
            ) : (
              <RadixDialog.Description className="sr-only">{title}</RadixDialog.Description>
            )}
            {children}
          </div>
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
