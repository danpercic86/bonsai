// P91 §7 — group 3 "What Bonsai records": the frozen privacy statement.
//
// §F6/§6.5: the heading was "What a log file contains" until ¶7 was added. ¶7 is
// not about a log file, so under the old heading the honest paragraph read as if
// it were filed in the wrong place. The heading and the catalog `group` string in
// `catalog/dev.ts` must change ATOMICALLY — settings search matches the group
// string, and a mismatch makes the block unreachable by search while it still
// renders.
//
// A prose block, catalogued as `control: 'group'` (no value row). ALWAYS visible,
// never inside the disabled fieldset — it is the text that decides whether the
// user turns Dev mode on. The `role="group"` is named by the group's own heading.
// Copy rules (§7): no "redaction", "telemetry", "IPC", "hashing" or "salt".

import { SettingsGroup } from './SettingsGroup';

const GROUP_ID = 'dev-privacy';

export function SettingsDevPrivacySection({ rawNames }: { rawNames: boolean }) {
  return (
    <SettingsGroup id={GROUP_ID} title="What Bonsai records">
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
          <strong>A log file never contains — in any mode:</strong> your commit messages, your
          search text or anything else you type, the contents of your files or diffs, the name and
          email address you commit under, or any password, access token or key. Bonsai drops these
          where the file is written, so the rule holds in both modes.
        </p>
        <p>
          <strong>Names are replaced by default.</strong> Your branches, tags, files, remotes and
          repository appear as <span className="mono">ref#3</span>,{' '}
          <span className="mono">path#7</span>, <span className="mono">remote#1</span>,{' '}
          <span className="mono">repo#1</span>, and commit IDs are shortened. The same name keeps
          the same number inside one file, so a problem can still be followed — and the numbers
          change next time, so two files cannot be matched up.
        </p>
        <p>
          <strong>If you turn on &ldquo;Include raw repository names&rdquo;</strong>, those
          placeholders stop: new log files contain the folder your repository lives in, your real
          branch, tag and file names, your remote addresses (with any username or password removed)
          and full commit IDs. That is the whole difference — raw names add identifiers, never
          anything you typed, and the &ldquo;never contains&rdquo; list above is unchanged.
        </p>
        <p>
          <strong>Log files stay on this computer until you delete them.</strong> They are kept when
          you turn Dev mode off, so you can export one afterwards. Bonsai keeps the 10 most recent
          sessions and removes the oldest automatically, and &ldquo;Delete logs and usage
          counts&rdquo; below removes every one of them — including the session being recorded
          right now.
        </p>
        {/* ¶7 (§F6 §6.2, SIGNED — implement verbatim). Two sentences about what is
            recorded, not one: counts, `sessions` and `first_seen` are always-on,
            but the duration histograms fill ONLY during Dev-mode sessions, and a
            single sentence claiming both would be an overclaim in the app's only
            privacy surface. The quoted label must stay byte-identical to the row
            label in `SettingsDevLogsSection.tsx`, and the copy names the FOLDER,
            never `usage.json` — deleting that file alone is undone by its `.bak`
            on the next load. */}
        <p>
          <strong>Bonsai also keeps a small usage count.</strong> From the first time you open
          it, and whether or not Dev mode is on, it records the date the count started, how many
          times you have opened the app, and a day-by-day tally of what it did — how many
          repositories it opened, how many commits it made. With Dev mode on it also records how
          long its own actions took. The day-by-day detail is kept for 90 days, and a running
          total after that. It holds only Bonsai&apos;s own action names — never a repository,
          branch, file, or anything you typed — it stays in a{' '}
          <span className="mono">metrics</span> folder beside your log files, and it never leaves
          this computer or goes into an export. &ldquo;Delete logs and usage counts&rdquo; below
          clears all of it.
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
