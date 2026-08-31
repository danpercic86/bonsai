// P80 — mock forge account index (extracted from handlers/forge.ts to keep that
// file focused). Holds the multi-account state that mirrors the backend settings
// index + the sentinel-driven provider/host selection. Never carries a token.
import { query as urlParam } from '../repoState';
import {
  FORGE_ACCOUNT_GITHUB,
  FORGE_ACCOUNT_GITHUB_2,
  FORGE_ACCOUNT_LONG,
  FORGE_MULTI_OWNER,
  FORGE_REPO_CONTEXT,
} from '../../fixtures/forge';
import type { AccountSource, ForgeAccount, ForgeKind } from '../../types';

// P64b/c/d: a `?forge=gitlab|bitbucket|azure` sentinel selects the provider so
// the panel exercises a non-GitHub forge; `?forge=unsupported` ⇒ 'unknown'.
export const FORGE_KIND: ForgeKind =
  urlParam('forge') === 'gitlab'
    ? 'gitLab'
    : urlParam('forge') === 'bitbucket'
      ? 'bitbucket'
      : urlParam('forge') === 'azure'
        ? 'azureDevOps'
        : urlParam('forge') === 'unsupported'
          ? 'unknown'
          : 'gitHub';

// Host + web URL matching the detected provider.
export const FORGE_HOST: Record<ForgeKind, string> = {
  gitHub: FORGE_REPO_CONTEXT.host,
  gitLab: 'gitlab.com',
  bitbucket: 'bitbucket.org',
  azureDevOps: 'dev.azure.com',
  unknown: 'git.example.com',
};

// Azure DevOps needs a 3-part org/project/repo identity; sample project for it.
export const FORGE_PROJECT: string | null =
  FORGE_KIND === 'azureDevOps' ? 'sample-project' : null;

// `?forge=expired` seeds a token-present-but-viewer-cold host + a one-shot
// authFailed on the first list; `?forge=multi` seeds TWO github.com accounts.
export const FORGE_EXPIRED = urlParam('forge') === 'expired';
export const FORGE_MULTI = urlParam('forge') === 'multi';

/** The mock account index: accounts + host defaults + per-repo overrides. */
class AccountStore {
  accounts: ForgeAccount[] = FORGE_MULTI
    ? [{ ...FORGE_ACCOUNT_GITHUB }, { ...FORGE_ACCOUNT_GITHUB_2 }]
    : urlParam('forge') === 'auth'
      ? [{ ...FORGE_ACCOUNT_GITHUB }, { ...FORGE_ACCOUNT_LONG }]
      : FORGE_EXPIRED
        ? [{ ...FORGE_ACCOUNT_GITHUB, login: null, avatarUrl: null }]
        : [];

  hostDefaults: Record<string, string> =
    FORGE_MULTI || urlParam('forge') === 'auth' || FORGE_EXPIRED
      ? { 'github.com': FORGE_ACCOUNT_GITHUB.accountId }
      : {};

  repoOverrides: Record<string, string> = {};

  /** The `accountId` for a host/login (mirrors the backend `account_id`). */
  accountId(kind: ForgeKind, host: string, login: string | null): string {
    const base = `${kind}:${host.toLowerCase()}`;
    return login ? `${base}:${login.toLowerCase()}` : base;
  }

  /** The owner/namespace used for the owner-match resolution step. */
  repoOwner(): string {
    return FORGE_MULTI ? FORGE_MULTI_OWNER : FORGE_REPO_CONTEXT.owner;
  }

  /** P80 §4 resolution: per-repo override → owner match → host default →
   *  single → first. Pure. */
  resolveAccount(repoId: string): { account: ForgeAccount | null; source: AccountSource } {
    const host = FORGE_HOST[FORGE_KIND];
    const onHost = this.accounts.filter((a) => a.host === host);
    if (onHost.length === 0) return { account: null, source: 'none' };
    const pinned = this.repoOverrides[repoId];
    if (pinned) {
      const a = onHost.find((x) => x.accountId === pinned);
      if (a) return { account: a, source: 'override' };
    }
    const owner = this.repoOwner().toLowerCase();
    if (owner) {
      const matches = onHost.filter((a) => (a.login ?? '').toLowerCase() === owner);
      if (matches.length === 1) return { account: matches[0], source: 'ownerMatch' };
    }
    const def = this.hostDefaults[host];
    if (def) {
      const a = onHost.find((x) => x.accountId === def);
      if (a) return { account: a, source: 'hostDefault' };
    }
    if (onHost.length === 1) return { account: onHost[0], source: 'single' };
    return { account: onHost[0], source: 'hostDefault' };
  }

  /** Insert-or-replace an account keyed by accountId. Sets the host default when
   *  none exists. Never stores a token. */
  upsertAccount(host: string, kind: ForgeKind, login: string | null, avatarUrl: string | null): void {
    const id = this.accountId(kind, host, login);
    this.accounts = this.accounts.filter((a) => a.accountId !== id);
    const isHostDefault = !this.hostDefaults[host] || this.hostDefaults[host] === id;
    this.accounts.unshift({ accountId: id, host, kind, login, avatarUrl, connected: true, isHostDefault });
    if (!this.hostDefaults[host]) this.hostDefaults[host] = id;
    this.syncHostDefaultFlags();
  }

  /** Remove an account by accountId, cleaning references. Idempotent. */
  removeAccountById(id: string): void {
    const rec = this.accounts.find((a) => a.accountId === id);
    this.accounts = this.accounts.filter((a) => a.accountId !== id);
    for (const k of Object.keys(this.repoOverrides)) {
      if (this.repoOverrides[k] === id) delete this.repoOverrides[k];
    }
    if (rec && this.hostDefaults[rec.host] === id) {
      delete this.hostDefaults[rec.host];
      const next = this.accounts.find((a) => a.host === rec.host);
      if (next) this.hostDefaults[rec.host] = next.accountId;
    }
    this.syncHostDefaultFlags();
  }

  /** Sign out every account on a host (P79 clear-token-for-host). */
  removeAccountsForHost(host: string): void {
    const ids = new Set(this.accounts.filter((a) => a.host === host).map((a) => a.accountId));
    this.accounts = this.accounts.filter((a) => a.host !== host);
    delete this.hostDefaults[host];
    for (const k of Object.keys(this.repoOverrides)) {
      if (ids.has(this.repoOverrides[k])) delete this.repoOverrides[k];
    }
    this.syncHostDefaultFlags();
  }

  /** Set a host default and recompute the flags. */
  setHostDefault(host: string, id: string): void {
    this.hostDefaults[host] = id;
    this.syncHostDefaultFlags();
  }

  /** Recompute each account's `isHostDefault` from `hostDefaults`. */
  syncHostDefaultFlags(): void {
    this.accounts = this.accounts.map((a) => ({
      ...a,
      isHostDefault: this.hostDefaults[a.host] === a.accountId,
    }));
  }
}

export const accountStore = new AccountStore();
