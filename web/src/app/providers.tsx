import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState, type ReactNode } from 'react';

export function AppProviders({ children }: { children: ReactNode }) {
  // Created in state so React StrictMode's double-invoke does not discard the
  // cache on mount.
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            // The chain is local and pushes SSE updates, so background polling
            // would only add load; invalidation drives refetches instead.
            refetchOnWindowFocus: false,
            staleTime: 5_000,
            retry: (failureCount, error) =>
              !(error instanceof Error && error.name === 'ApiError') && failureCount < 2,
          },
        },
      }),
  );

  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}
