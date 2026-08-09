/** T3.5 — DiffFileTree (scope navigator) + DiffImageView (image compare modes)
 *  from tiny inline fixtures. Both are pure presentational components. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DiffFileTree } from './DiffFileTree';
import type { DiffScope } from './DiffFileTree';
import { DiffImageView } from './DiffImageView';
import type { FileDiffHeader, ImageDiff, ImageSide } from '../ipc';

function header(path: string, over: Partial<FileDiffHeader> = {}): FileDiffHeader {
  return { path, origPath: null, status: 'modified', additions: 3, deletions: 1, binary: false, ...over };
}

const FILES = [
  header('src/app/main.ts'),
  header('src/app/util.ts', { status: 'added', additions: 10, deletions: 0 }),
  header('README.md'),
];

function renderTree(over: { files?: FileDiffHeader[]; listView?: 'tree' | 'flat'; scope?: DiffScope } = {}) {
  const onSelect = vi.fn();
  const utils = render(
    <DiffFileTree
      files={over.files ?? FILES}
      listView={over.listView ?? 'flat'}
      scope={over.scope ?? { kind: 'root' }}
      onSelect={onSelect}
    />,
  );
  return { ...utils, onSelect };
}

describe('DiffFileTree', () => {
  it('root row shows the total count and selects the root scope', () => {
    const { onSelect } = renderTree({ scope: { kind: 'file', path: 'README.md' } });
    const root = screen.getByRole('button', { name: /All files/ });
    expect(root).toHaveTextContent('3');
    fireEvent.click(root);
    expect(onSelect).toHaveBeenCalledWith({ kind: 'root' });
  });

  it('flat mode lists full paths with +/− counts; clicking selects the file', () => {
    const { onSelect } = renderTree();
    const row = screen.getByRole('button', { name: /src\/app\/main\.ts/ });
    expect(row).toHaveTextContent('+3');
    expect(row).toHaveTextContent('−1');
    fireEvent.click(row);
    expect(onSelect).toHaveBeenCalledWith({ kind: 'file', path: 'src/app/main.ts' });
  });

  it('the scope prop highlights the selected file row', () => {
    renderTree({ scope: { kind: 'file', path: 'README.md' } });
    expect(screen.getByRole('button', { name: /README\.md/ })).toHaveClass('diff-tree-selected');
    expect(screen.getByRole('button', { name: /main\.ts/ })).not.toHaveClass('diff-tree-selected');
  });

  it('tree mode: dir-name click selects the dir scope; chevron only collapses', () => {
    const { onSelect, container } = renderTree({ listView: 'tree' });
    // Leaves render basenames under their folder.
    expect(screen.getByText('main.ts')).toBeInTheDocument();
    const dirBtn = screen.getByTitle('src/app');
    fireEvent.click(dirBtn);
    expect(onSelect).toHaveBeenCalledWith({ kind: 'dir', prefix: 'src/app' });
    onSelect.mockClear();
    fireEvent.click(screen.getAllByRole('button', { name: 'Collapse folder' })[0]);
    expect(onSelect).not.toHaveBeenCalled();
    expect(container.querySelector('[aria-expanded="false"]')).toBeInTheDocument();
    expect(screen.queryByText('main.ts')).not.toBeInTheDocument();
  });

  it('rename rows show orig → new; binary rows show the bin badge', () => {
    renderTree({
      files: [
        header('new.ts', { status: 'renamed', origPath: 'old.ts' }),
        header('logo.png', { binary: true }),
      ],
    });
    expect(screen.getByTitle('old.ts → new.ts')).toBeInTheDocument();
    const bin = screen.getByRole('button', { name: /logo\.png/ });
    expect(bin).toHaveTextContent('bin');
    expect(bin).not.toHaveTextContent('+3');
  });
});

// --- DiffImageView -----------------------------------------------------------

// 1x1 transparent PNG (67 bytes) — tiny but decodable base64.
const PNG_B64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';

function side(byteLen = 67): ImageSide {
  return { base64: PNG_B64, mime: 'image/png', byteLen };
}

function imgDiff(over: Partial<ImageDiff> = {}): ImageDiff {
  return { path: 'logo.png', old: side(), new: side(2048), oldTooLarge: false, newTooLarge: false, ...over };
}

describe('DiffImageView', () => {
  it('side-by-side renders both panes with data: URLs and size labels', () => {
    render(<DiffImageView diff={imgDiff()} mode="sideBySide" />);
    const oldImg = screen.getByAltText('Old version');
    expect(oldImg).toHaveAttribute('src', `data:image/png;base64,${PNG_B64}`);
    expect(screen.getByAltText('New version')).toBeInTheDocument();
    expect(screen.getByText('67 B')).toBeInTheDocument();
    expect(screen.getByText('2.0 KB')).toBeInTheDocument();
  });

  it('an absent side renders Added/Deleted; over-cap renders the size message', () => {
    render(<DiffImageView diff={imgDiff({ old: null })} mode="sideBySide" />);
    expect(screen.getByText('Added')).toBeInTheDocument();
    render(<DiffImageView diff={imgDiff({ new: null })} mode="sideBySide" />);
    expect(screen.getByText('Deleted')).toBeInTheDocument();
    render(<DiffImageView diff={imgDiff({ new: null, newTooLarge: true })} mode="sideBySide" />);
    expect(screen.getByText(/Larger than 8 MB/)).toBeInTheDocument();
  });

  it('onion mode overlays both images with a crossfade slider', () => {
    render(<DiffImageView diff={imgDiff()} mode="onion" />);
    const slider = screen.getByRole('slider', { name: 'Crossfade old to new' });
    expect(slider).toHaveValue('0.5');
    expect(screen.getByAltText('New version')).toHaveStyle({ opacity: 0.5 });
    fireEvent.change(slider, { target: { value: '1' } });
    expect(screen.getByAltText('New version')).toHaveStyle({ opacity: 1 });
  });

  it('swipe mode renders the divider stage; drag hint present', () => {
    const { container } = render(<DiffImageView diff={imgDiff()} mode="swipe" />);
    expect(container.querySelector('.img-swipe-stage')).toBeInTheDocument();
    expect(screen.getByText(/Drag to compare/)).toBeInTheDocument();
  });

  it('onion/swipe degrade to side-by-side when a side is missing', () => {
    const { container } = render(<DiffImageView diff={imgDiff({ old: null })} mode="onion" />);
    expect(container.querySelector('.img-diff-sbs')).toBeInTheDocument();
    expect(container.querySelector('.img-diff-onion')).not.toBeInTheDocument();
    expect(screen.getByText('Added')).toBeInTheDocument();
  });
});
