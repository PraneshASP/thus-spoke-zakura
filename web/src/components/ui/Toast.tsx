import * as RadixToast from '@radix-ui/react-toast';
import { AlertTriangle, Check } from 'lucide-react';
import { useCallback, useMemo, useState, type ReactNode } from 'react';
import { ToastContext, type ToastApi, type ToastMessage, type ToastTone } from './toast-context';
import { cn } from '@/lib/cn';

let nextId = 0;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [messages, setMessages] = useState<ToastMessage[]>([]);

  const push = useCallback((tone: ToastTone, title: string, detail?: string) => {
    setMessages((current) => [...current, { id: nextId++, tone, title, detail }]);
  }, []);

  const api = useMemo<ToastApi>(
    () => ({
      success: (title, detail) => push('success', title, detail),
      error: (title, detail) => push('error', title, detail),
    }),
    [push],
  );

  return (
    <ToastContext value={api}>
      <RadixToast.Provider swipeDirection="right" duration={6000}>
        {children}
        {messages.map((message) => (
          <RadixToast.Root
            key={message.id}
            duration={message.tone === 'error' ? 10000 : 5000}
            onOpenChange={(open) => {
              if (!open) setMessages((current) => current.filter((m) => m.id !== message.id));
            }}
            className={cn(
              'xp-raised animate-slide-in flex items-start gap-2.5 rounded-xs p-3 text-[12px] shadow-(--shadow-float)',
              'data-[state=closed]:animate-out data-[state=closed]:fade-out',
              message.tone === 'success'
                ? 'bg-positive-soft text-positive'
                : 'bg-negative-soft text-negative',
            )}
          >
            {message.tone === 'success' ? (
              <Check className="mt-px size-3.5 shrink-0" aria-hidden />
            ) : (
              <AlertTriangle className="mt-px size-3.5 shrink-0" aria-hidden />
            )}
            <div>
              <RadixToast.Title className="font-semibold">{message.title}</RadixToast.Title>
              {message.detail && (
                <RadixToast.Description className="mt-1 leading-relaxed opacity-90">
                  {message.detail}
                </RadixToast.Description>
              )}
            </div>
          </RadixToast.Root>
        ))}
        <RadixToast.Viewport className="fixed top-6 right-6 z-100 flex w-[min(380px,calc(100vw-48px))] flex-col gap-2 outline-none" />
      </RadixToast.Provider>
    </ToastContext>
  );
}
