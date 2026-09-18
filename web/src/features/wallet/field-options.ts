import type { SelectOption } from '@/components/ui/Select';
import type { Account } from '@/lib/api';

export const POOL_OPTIONS: ReadonlyArray<SelectOption<'orchard' | 'transparent'>> = [
  { value: 'orchard', label: 'Orchard (shielded)' },
  { value: 'transparent', label: 'Transparent (public)' },
];

export function accountOptions(accounts: Account[]): ReadonlyArray<SelectOption<string>> {
  return accounts.map((account) => ({ value: String(account.id), label: account.name }));
}
