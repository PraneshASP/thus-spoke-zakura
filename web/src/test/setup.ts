import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

/**
 * jsdom does not implement matchMedia. Anything that respects reduced motion
 * reads it during render, so it belongs here rather than in each test file.
 */
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }),
});

// jsdom has no Web Locks API. Faucet tests replace this with a queued lock when
// they need to exercise cross-tab ordering.
Object.defineProperty(window.navigator, 'locks', {
  configurable: true,
  value: {
    request: (name: string, callback: (lock: Lock) => unknown) =>
      Promise.resolve(callback({ name, mode: 'exclusive' })),
  },
});
