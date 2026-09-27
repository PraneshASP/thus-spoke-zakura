import { useMutation, useQueryClient, type UseMutationResult } from '@tanstack/react-query';
import {
  api,
  idempotencyKey,
  type Activity,
  type MiningJob,
  type PaymentUri,
  type Pool,
} from '@/lib/api';
import { queryKeys } from './queries';

/** Everything a successful money movement invalidates. */
function walletKeys() {
  return [[...queryKeys.accounts], ['activity'], [...queryKeys.status], ['blocks'], ['send-quote']];
}

function useInvalidateWallet() {
  const queryClient = useQueryClient();
  return async () => {
    await Promise.all(
      walletKeys().map((key) =>
        queryClient.invalidateQueries({ queryKey: key, refetchType: 'active' }),
      ),
    );
  };
}

function operationKey<T>(kind: string, fingerprint: (variables: T) => string) {
  const storageKey = (variables: T) => `ths:${kind}:${fingerprint(variables)}`;
  return {
    keyFor(variables: T) {
      const storage = storageKey(variables);
      let key = sessionStorage.getItem(storage);
      if (!key) {
        key = idempotencyKey();
        sessionStorage.setItem(storage, key);
      }
      return key;
    },
    clear(variables: T) {
      sessionStorage.removeItem(storageKey(variables));
    },
  };
}

/** Exactly one of `to_account` or `to_address` is set. */
export interface SendVariables {
  from_account: number;
  to_account?: number;
  to_address?: string;
  source_pool: Pool;
  destination_pool: Pool;
  amount_zatoshi: bigint;
  memo?: string;
}

export function useSend(): UseMutationResult<Activity, Error, SendVariables> {
  const invalidate = useInvalidateWallet();
  const operation = operationKey('send', (variables: SendVariables) =>
    JSON.stringify([
      variables.from_account,
      variables.to_account ?? null,
      variables.to_address ?? null,
      variables.source_pool,
      variables.destination_pool,
      variables.amount_zatoshi.toString(),
      variables.memo ?? null,
    ]),
  );
  return useMutation({
    mutationFn: (variables: SendVariables) =>
      api.send({ ...variables, idempotency_key: operation.keyFor(variables) }),
    onSuccess: async (_activity, variables) => {
      operation.clear(variables);
      await invalidate();
    },
  });
}

/** Resolves a pasted `zcash:` URI; it moves no funds, so nothing is invalidated. */
export function useParsePaymentUri(): UseMutationResult<PaymentUri, Error, string> {
  return useMutation({ mutationFn: (uri: string) => api.parsePaymentUri(uri) });
}

export interface FaucetVariables {
  account_id: number;
  pool: Pool;
  amount_zatoshi: bigint;
}

export function useFaucet(): UseMutationResult<Activity, Error, FaucetVariables> {
  const invalidate = useInvalidateWallet();
  const operation = operationKey(
    'faucet',
    (variables: FaucetVariables) =>
      `${variables.account_id}:${variables.pool}:${variables.amount_zatoshi}`,
  );
  return useMutation({
    mutationFn: (variables: FaucetVariables) =>
      api.faucet({ ...variables, idempotency_key: operation.keyFor(variables) }),
    onSuccess: async (_activity, variables) => {
      operation.clear(variables);
      await invalidate();
    },
  });
}

export function useMine(): UseMutationResult<{ blocks: number }, Error, number> {
  const invalidate = useInvalidateWallet();
  return useMutation({
    mutationFn: (blocks: number) => api.mine(blocks),
    onSuccess: invalidate,
  });
}

export function useStartMining(): UseMutationResult<MiningJob, Error, number> {
  const queryClient = useQueryClient();
  const operation = operationKey('mine', (blocks: number) => String(blocks));
  return useMutation({
    mutationFn: (blocks: number) => {
      const state = queryClient.getQueryData<{ job: MiningJob | null }>(queryKeys.mining)?.job
        ?.state;
      if (state === 'completed' || state === 'failed') operation.clear(blocks);
      return api.startMining(blocks, operation.keyFor(blocks));
    },
    retry: false,
    onSuccess: async (job, blocks) => {
      operation.clear(blocks);
      queryClient.setQueryData(queryKeys.mining, { job });
      await queryClient.invalidateQueries({ queryKey: queryKeys.mining });
    },
    onError: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.mining });
    },
  });
}
