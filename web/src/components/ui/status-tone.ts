import type { BadgeProps } from './Badge';

/** Maps a server activity status onto a badge tone. */
export function statusTone(status: string): NonNullable<BadgeProps['tone']> {
  if (status === 'confirmed') return 'positive';
  if (status === 'broadcast') return 'warning';
  return 'neutral';
}
