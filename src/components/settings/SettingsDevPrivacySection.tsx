// P91 §7 — group 3 "What a log file contains": the frozen privacy statement.
//
// A prose block, catalogued as `control: 'group'` (no value row). ALWAYS visible,
// never inside the disabled fieldset — it is the text that decides whether the
// user turns Dev mode on. The `role="group"` is named by the group's own heading.
// Copy rules (§7): no "redaction", "telemetry", "IPC", "hashing" or "salt".

import { SettingsGroup } from './SettingsGroup';

const GROUP_ID = 'dev-privacy';

export function SettingsDevPrivacySection({ rawNames }: { rawNames: boolean }) {
  return (
    <SettingsGroup id={GROUP_ID} title="What a log file contains">
      <div
        className="dev-privacy"
        role="group"
        aria-labelledby={`${GROUP_ID}-title`}
        data-setting-id="dev.privacy-note"
      >
        <p>
          Bonsai writes log files <strong>only to this computer</strong>. It never uploads them, and
          nothing here sends anything over the network.
        </p>
        <p>
          <strong>A log file contains:</strong> what you clicked, which commands the app ran, how
          long they took, whether they succeeded, how often the screen redrew, and problems Bonsai
          flagged along the way.
        </p>
        <p>
          <strong>A log file never contains — in any mode:</strong> your commit messages, the
          contents of your files or diffs, your name or email address, your search text, or any
          password, access token or key.
        </p>
        <p>
          <strong>Names are replaced by default.</strong> Your branches, tags, files and repository
          appear as <span className="mono">ref#3</span>, <span className="mono">path#7</span>,{' '}
          <span className="mono">repo#1</span>. The same name keeps the same number inside one file,
          so a problem can still be followed — and the numbers change next time, so two files cannot
          be matched up.
        </p>
        <p>
          <strong>If you turn on &ldquo;Include raw repository names&rdquo;</strong>, that
          replacement stops: new log files contain your real branch, tag, file and repository names.
          Everything in the &ldquo;never contains&rdquo; list above stays excluded.
        </p>
        <p>
          <strong>Log files stay on this computer until you delete them.</strong> They are kept when
          you turn Dev mode off, so you can export one afterwards. Bonsai keeps the 10 most recent
          sessions and removes the oldest automatically, and &ldquo;Delete all log files&rdquo;
          below removes every one of them — including the session being recorded right now.
        </p>
        <p>
          Log files are plain text, one record per line. You can open one in any text editor and
          read it before you send it anywhere.
        </p>
        {rawNames ? (
          <p className="settings-group-note dev-warning-bar">
            <span className="dev-warning-glyph" aria-hidden="true">
              ⚠
            </span>
            Right now: raw names are included.
          </p>
        ) : (
          <p className="settings-group-note">Right now: names are replaced.</p>
        )}
      </div>
    </SettingsGroup>
  );
}
