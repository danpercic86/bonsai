/** T3.7 — AiAssetsPanel: the AI-asset inventory + drift overlay. IPC stubbed via
 *  vi.spyOn(mockIpc, …). Covers closed no-fetch, drift badge, managed instruction
 *  rows + sync chips, detected section, agent-asset groups + validation chips,
 *  drifted-row compare, New/edit editor entry points, Refresh, whole-fetch error,
 *  and Close. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within, act } from '@testing-library/react';
import { AiAssetsPanel } from './AiAssetsPanel';
import { ToastContext } from '../ToastContext';
import { mockIpc } from '../ipc/mock';
import type {
  AgentAsset,
  AgentAssetInventory,
  AiAsset,
  AiAssetInventory,
  ProfileStore,
  RepoChangedPayload,
} from '../ipc';
import {
  ECHO_TTL_MS,
  __resetEchoSuppression,
  armEcho,
} from './repoWorkspace/echoSuppression';

function aiAsset(over: Partial<AiAsset> = {}): AiAsset {
  return {
    id: 'x',
    agent: 'claude',
    label: 'X',
    kind: 'singleFile',
    path: 'X.md',
    managed: true,
    exists: true,
    files: [],
    ...over,
  };
}

const CANONICAL = aiAsset({ id: 'canon', label: 'CLAUDE.md', path: 'CLAUDE.md' });
const DRIFTED = aiAsset({ id: 'drift', label: 'AGENTS.md', path: 'AGENTS.md' });
const DETECTED = aiAsset({
  id: 'rules',
  label: 'Cursor rules',
  path: '.cursor/rules',
  kind: 'rulesDir',
  managed: false,
});

function inventory(): AiAssetInventory {
  return {
    assets: [CANONICAL, DRIFTED, DETECTED],
    drift: {
      canonicalId: 'canon',
      canonicalHash: 'h',
      inSync: false,
      entries: [
        { assetId: 'canon', exists: true, comparable: true, normalizedHash: 'h', inSync: true },
        { assetId: 'drift', exists: true, comparable: true, normalizedHash: 'g', inSync: false },
      ],
    },
  };
}

function agentAsset(over: Partial<AgentAsset> = {}): AgentAsset {
  return {
    kind: 'skill',
    name: 'brew',
    path: '.claude/skills/brew/SKILL.md',
    exists: true,
    frontmatter: [],
    body: '',
    complex: false,
    validation: { valid: true, issues: [] },
    ...over,
  };
}

const agentInventory: AgentAssetInventory = {
  assets: [
    agentAsset({ name: 'brew', kind: 'skill' }),
    agentAsset({
      name: 'deploy',
      kind: 'command',
      path: '.claude/commands/deploy.md',
      validation: { valid: false, issues: [{ severity: 'warning', message: 'x' }] },
    }),
  ],
};

const store: ProfileStore = { version: 1, profiles: [] };

function stubAll() {
  vi.spyOn(mockIpc, 'onRepoChanged').mockResolvedValue(() => {});
  const inv = vi.spyOn(mockIpc, 'listAiAssets').mockResolvedValue(inventory());
  vi.spyOn(mockIpc, 'listProfiles').mockResolvedValue(store);
  vi.spyOn(mockIpc, 'listAgentAssets').mockResolvedValue(agentInventory);
  return inv;
}

function renderPanel(props: Partial<React.ComponentProps<typeof AiAssetsPanel>> = {}) {
  const full = {
    open: true,
    onClose: vi.fn(),
    repoId: '/mock/repo',
    aiEnabled: true,
    ...props,
  };
  render(
    <ToastContext.Provider value={vi.fn()}>
      <AiAssetsPanel {...full} />
    </ToastContext.Provider>,
  );
  return full;
}

beforeEach(() => {
  vi.restoreAllMocks();
  __resetEchoSuppression();
});

describe('AiAssetsPanel', () => {
  it('P81 (AC8): drops the self-echo repo-changed within the window, refetches after it', async () => {
    const inv = vi.spyOn(mockIpc, 'listAiAssets').mockResolvedValue(inventory());
    vi.spyOn(mockIpc, 'listProfiles').mockResolvedValue(store);
    vi.spyOn(mockIpc, 'listAgentAssets').mockResolvedValue(agentInventory);
    let handler: ((p: RepoChangedPayload) => void) | null = null;
    vi.spyOn(mockIpc, 'onRepoChanged').mockImplementation((cb) => {
      handler = cb;
      return Promise.resolve(() => {});
    });
    renderPanel();
    await screen.findByText('1 file drifted');
    expect(inv).toHaveBeenCalledTimes(1);
    expect(handler).toBeTruthy();

    // Armed window active → the self-caused echo is a no-op.
    armEcho('/mock/repo');
    await act(async () => {
      handler?.({ repoId: '/mock/repo', reason: 'test' });
    });
    expect(inv).toHaveBeenCalledTimes(1);

    // Past the window → a genuine external change refetches.
    armEcho('/mock/repo', Date.now() - ECHO_TTL_MS - 100);
    await act(async () => {
      handler?.({ repoId: '/mock/repo', reason: 'test' });
    });
    await waitFor(() => expect(inv).toHaveBeenCalledTimes(2));
  });

  it('renders nothing and never fetches while closed', () => {
    const inv = stubAll();
    const { container } = render(
      <ToastContext.Provider value={vi.fn()}>
        <AiAssetsPanel open={false} onClose={vi.fn()} repoId="/mock/repo" aiEnabled />
      </ToastContext.Provider>,
    );
    expect(container).toBeEmptyDOMElement();
    expect(inv).not.toHaveBeenCalled();
  });

  it('shows the drifted badge and the managed rows with canonical/drifted chips', async () => {
    stubAll();
    renderPanel();
    expect(await screen.findByText('1 file drifted')).toBeInTheDocument();
    expect(screen.getByText('canonical')).toBeInTheDocument();
    expect(screen.getByText('drifted')).toBeInTheDocument();
    // The drifted managed row is a clickable button; canonical is not.
    expect(screen.getByRole('button', { name: /AGENTS\.md/ })).toBeInTheDocument();
  });

  it('lists a detected (unmanaged) asset in its own section', async () => {
    stubAll();
    renderPanel();
    await screen.findByText('1 file drifted');
    const detected = screen.getByText('Detected (not managed)').closest('section')!;
    expect(within(detected).getByText('Cursor rules')).toBeInTheDocument();
  });

  it('renders the three agent-asset groups with validation chips', async () => {
    stubAll();
    renderPanel();
    await screen.findByText('1 file drifted');
    expect(screen.getByText('brew')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'New skill' })).toBeInTheDocument();
    expect(screen.getByText('valid')).toBeInTheDocument();
    expect(screen.getByText('1 issue')).toBeInTheDocument();
  });

  it('clicking a drifted row opens the current-vs-canonical compare', async () => {
    stubAll();
    vi.spyOn(mockIpc, 'readAiAsset').mockImplementation(async (_r, path) => ({
      path,
      exists: true,
      content: `body of ${path}`,
    }));
    renderPanel();
    await screen.findByText('1 file drifted');
    fireEvent.click(screen.getByRole('button', { name: /AGENTS\.md/ }));
    expect(await screen.findByText('AGENTS.md vs canonical')).toBeInTheDocument();
  });

  it('New skill opens the agent-asset editor in create mode', async () => {
    stubAll();
    renderPanel();
    await screen.findByText('1 file drifted');
    fireEvent.click(screen.getByRole('button', { name: 'New skill' }));
    expect(await screen.findByRole('dialog', { name: 'New skill' })).toBeInTheDocument();
  });

  it('clicking an existing agent asset opens the editor in edit mode', async () => {
    stubAll();
    vi.spyOn(mockIpc, 'readAgentAsset').mockResolvedValue(agentAsset({ name: 'brew' }));
    renderPanel();
    await screen.findByText('1 file drifted');
    fireEvent.click(screen.getByRole('button', { name: /brew/ }));
    expect(await screen.findByRole('dialog', { name: 'Edit skill “brew”' })).toBeInTheDocument();
  });

  it('Refresh refetches the inventory', async () => {
    const inv = stubAll();
    renderPanel();
    await screen.findByText('1 file drifted');
    expect(inv).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    await waitFor(() => expect(inv).toHaveBeenCalledTimes(2));
  });

  it('a whole-fetch failure shows the error banner', async () => {
    vi.spyOn(mockIpc, 'onRepoChanged').mockResolvedValue(() => {});
    vi.spyOn(mockIpc, 'listAiAssets').mockRejectedValue({ kind: 'other', message: 'repo gone' });
    vi.spyOn(mockIpc, 'listProfiles').mockResolvedValue(store);
    vi.spyOn(mockIpc, 'listAgentAssets').mockResolvedValue(agentInventory);
    renderPanel();
    expect(await screen.findByText('repo gone')).toBeInTheDocument();
  });

  it('Close fires onClose', async () => {
    stubAll();
    const { onClose } = renderPanel();
    await screen.findByText('1 file drifted');
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
