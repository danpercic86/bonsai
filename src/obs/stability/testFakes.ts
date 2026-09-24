/** Test-only fakes for the stability detector (imported by *.test.ts only). */

export interface FakeShiftEntry {
  entryType: 'layout-shift';
  startTime: number;
  value: number;
  hadRecentInput: boolean;
  sources: { node: Node | null }[];
}

type Callback = (list: { getEntries(): FakeShiftEntry[] }) => void;

/** A PerformanceObserver stand-in: records instances + observe options and lets
 *  a test deliver entries by hand. */
export class FakePerformanceObserver {
  static supportedEntryTypes: readonly string[] = ['layout-shift'];
  static instances: FakePerformanceObserver[] = [];
  options: { type?: string; buffered?: boolean } | null = null;
  disconnected = false;
  private readonly cb: Callback;
  constructor(cb: Callback) {
    this.cb = cb;
    FakePerformanceObserver.instances.push(this);
  }
  observe(options: { type?: string; buffered?: boolean }): void {
    this.options = options;
  }
  disconnect(): void {
    this.disconnected = true;
  }
  deliver(entries: FakeShiftEntry[]): void {
    this.cb({ getEntries: () => entries });
  }
}

export function shiftEntry(p: Partial<FakeShiftEntry> & { startTime: number }): FakeShiftEntry {
  return { entryType: 'layout-shift', value: 0.1, hadRecentInput: false, sources: [], ...p };
}
