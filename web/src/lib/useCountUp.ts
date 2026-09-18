import { useEffect, useRef, useState, useSyncExternalStore } from 'react';

const DURATION = 620;
const easeOut = (t: number) => 1 - Math.pow(1 - t, 3);

const REDUCED = '(prefers-reduced-motion: reduce)';

function subscribe(onChange: () => void): () => void {
  const query = window.matchMedia(REDUCED);
  query.addEventListener('change', onChange);
  return () => query.removeEventListener('change', onChange);
}

/** Tracks the user's motion preference as an external store, not as state. */
function useReducedMotion(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => window.matchMedia(REDUCED).matches,
    () => false,
  );
}

/**
 * Animates a zatoshi value toward its new target so a balance visibly moves
 * rather than silently swapping. Interpolation runs through Number, which is
 * exact for every value below 2^53 — total supply is 2.1e15 zatoshi, so the
 * displayed figure never loses precision, and the final frame snaps to the
 * exact bigint regardless.
 */
export function useCountUp(target: bigint): bigint {
  const reduced = useReducedMotion();
  const [value, setValue] = useState(target);
  const from = useRef(target);

  useEffect(() => {
    const start = from.current;
    if (reduced || start === target) {
      from.current = target;
      return;
    }

    const startNumber = Number(start);
    const delta = Number(target) - startNumber;
    let startedAt = 0;
    let frame = 0;

    const step = (now: number) => {
      startedAt ||= now;
      const progress = Math.min((now - startedAt) / DURATION, 1);
      if (progress >= 1) {
        from.current = target;
        setValue(target);
        return;
      }
      setValue(BigInt(Math.round(startNumber + delta * easeOut(progress))));
      frame = requestAnimationFrame(step);
    };

    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [target, reduced]);

  return reduced ? target : value;
}
