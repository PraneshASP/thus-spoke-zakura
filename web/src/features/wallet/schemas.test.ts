import { describe, expect, it } from 'vitest';
import { sendSchema } from './schemas';

const base = {
  from_account: '1',
  to_account: '2',
  source_pool: 'orchard' as const,
  destination_pool: 'orchard' as const,
  amount: '1',
};

describe('sendSchema', () => {
  it('rejects a send to the same account', () => {
    // The dialog used to default both sides to the account it was opened from,
    // so this was reachable with two clicks and cost a fee to discover.
    const result = sendSchema.safeParse({ ...base, from_account: '3', to_account: '3' });
    expect(result.success).toBe(false);
    if (!result.success) {
      const issue = result.error.issues.find((candidate) => candidate.path[0] === 'to_account');
      expect(issue?.message).toMatch(/different account/i);
    }
  });

  it('accepts a transfer between two different accounts', () => {
    const result = sendSchema.safeParse(base);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.amount).toBe(100_000_000n);
    }
  });
});
