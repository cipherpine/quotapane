# Gating on quota

`quotapane-cli --fail-at <N>` exits **3** when any window has reached `N`
percent used. That one number is enough to build a gate — something that
checks your remaining quota *before* it matters and lets your own script
decide what to do about it.

QuotaPane never acts on your behalf. It reads your quota, prints it, and
sets an exit code. Every recipe below is your script deciding; none of
them is QuotaPane deciding.

Exit codes are the whole interface here, and they are documented once, in
[`cli-json.md`](cli-json.md#exit-codes). The short version, because every
recipe branches on it:

- **0** — under the threshold.
- **3** — at or over it.
- **1** — QuotaPane could not tell you (signed out, expired token, the
  endpoint was unreachable). This is not the same as "you have room",
  and a gate that treats it as such will happily start the run it exists
  to prevent.

Two practical notes before the recipes:

- Pass `--client-version <VER>` with a real `claude-code` version. The
  default `0.0.0` lands in a bucket the endpoint throttles aggressively,
  which is exactly the wrong behaviour for something that runs on a
  schedule.
- With `--provider all`, a provider that fails to poll makes the whole
  run exit 1 even when the other one answered. If you only care about one
  provider, name it — the gate is cleaner.

---

## 1. Pre-flight before a long agent run

The one-liner from the README:

```sh
quotapane-cli --once --provider all --fail-at 85 || exit 1
```

`||` fires on *any* non-zero exit, so this refuses to start on both "you
are over 85%" (3) and "I could not read your quota" (1). For a pre-flight
that is usually right: the point is to avoid a run that dies halfway, and
"I don't know" is not a good enough reason to spend an hour finding out.

When you do want to tell the two apart — proceed on an unreadable quota,
stop on a real one — branch on the code:

```sh
quotapane-cli --once --provider all --fail-at 85 --client-version 2.1.90
case $? in
  0) ;;  # room to work
  3) echo "quota: at or over 85% — not starting" >&2; exit 1 ;;
  *) echo "quota: could not be read; starting anyway" >&2 ;;
esac
```

```powershell
quotapane-cli --once --provider all --fail-at 85 --client-version 2.1.90
switch ($LASTEXITCODE) {
    0 { }  # room to work
    3 { Write-Error 'quota: at or over 85% — not starting'; exit 1 }
    default { Write-Warning 'quota: could not be read; starting anyway' }
}
```

`--fail-at` checks **every** window of every provider that polled —
headline windows and per-model buckets both — and trips on the worst one.
That is deliberate: a per-model bucket at 97% will end your run just as
dead as a headline window will, even though the text summary does not
print it.

## 2. A CI stage that refuses to start under quota pressure

**This only works on a self-hosted runner.** QuotaPane reads the
credential files the `claude` and `codex` CLIs wrote on *that machine*; a
hosted runner is a fresh VM with nobody signed in, so the step below can
only ever exit 1 there. Use it on a runner that is your own workstation
or a long-lived box you have signed in on.

```yaml
- name: Quota pre-flight
  run: quotapane-cli --once --provider all --fail-at 85 --client-version 2.1.90

- name: The expensive stage
  run: ./run-the-long-thing.sh
```

A failed step ends the job, so the expensive stage never starts. If you
would rather record the pressure and continue, let the step report and
gate the next one on it:

```yaml
- name: Quota pre-flight
  id: quota
  continue-on-error: true
  run: quotapane-cli --once --provider all --fail-at 85 --client-version 2.1.90

- name: The expensive stage
  if: steps.quota.outcome == 'success'
  run: ./run-the-long-thing.sh
```

This fragment is generic on purpose — it is not wired into this repo's
own CI, and you should paste it into yours rather than the other way
around.

## 3. A background heartbeat into a log

`--watch <SECS>` polls on an interval until interrupted, and with
`--json` each cycle is one compact line (NDJSON). That is a log you can
read back later with `jq`, or plot, or grep for the moment things went
sideways:

```sh
quotapane-cli --watch 300 --json --provider all --client-version 2.1.90 \
  >> "$HOME/quota.ndjson"
```

```powershell
quotapane-cli --watch 300 --json --provider all --client-version 2.1.90 |
    Add-Content -Path "$HOME\quota.ndjson"
```

`--watch` is a long-running process, not a cron job — the scheduler's
role is to *start* it once, at login or boot, not to run it every five
minutes. On Linux that is a user systemd unit or an `@reboot` crontab
line; on Windows, a Scheduled Task with the trigger "At log on" and the
action pointed at `quotapane-cli.exe`.

`SECS` has a floor of **180**, the same floor the window's own poller
respects. Scripted polling gets no faster path to the endpoint than the
GUI does.

The file grows by one line per cycle forever, so rotate it. Reading the
last hour back:

```sh
tail -n 12 ~/quota.ndjson | jq -r '.[] | "\(.provider) \(.windows[0].label) \(.windows[0].used_fraction)"'
```

## 4. A `pre-push` hook that warns and never blocks

A hook that blocks your push because a *quota* is high would be
infuriating. This one tells you and gets out of the way — the value is
knowing before you kick off the agent run that follows the push, not
being stopped.

`.git/hooks/pre-push`:

```sh
#!/bin/sh
# Warn — never block — when quota is tight. `exit 0` unconditionally.
quotapane-cli --once --provider all --fail-at 90 --client-version 2.1.90 >/dev/null 2>&1
if [ $? -eq 3 ]; then
  echo "quota: at or over 90% — pushing anyway, but a long agent run may not finish" >&2
fi
exit 0
```

Make it executable (`chmod +x .git/hooks/pre-push`). On Windows this same
script is the one to use: Git for Windows runs hooks under its own
bundled `sh`, so the POSIX version is the portable one. If you would
rather keep the logic in PowerShell, call out to it from the hook:

```sh
#!/bin/sh
pwsh -NoProfile -File "$(git rev-parse --show-toplevel)/scripts/quota-warn.ps1"
exit 0
```

```powershell
# scripts/quota-warn.ps1
quotapane-cli --once --provider all --fail-at 90 --client-version 2.1.90 |
    Out-Null
if ($LASTEXITCODE -eq 3) {
    Write-Warning 'quota: at or over 90% — pushing anyway, but a long agent run may not finish'
}
exit 0
```

Note the unconditional `exit 0` in both. A hook that forgets it stops
being a warning.

## 5. Claude Code's own status line

`--statusline` is the odd one out: it is not a gate, and it does not
poll. Claude Code's status line feature pipes a JSON document to its
configured command on every render, and that document already carries
your quota numbers — so QuotaPane formats them and sends nothing. No
credential file is opened, no HTTP client is built, no request is made.
That is what makes it safe to run on every keystroke's worth of redraw.

Add this to `~/.claude/settings.json` (or a project `.claude/settings.json`):

```json
{
  "statusLine": {
    "type": "command",
    "command": "quotapane-cli --statusline",
    "refreshInterval": 60
  }
}
```

You get one line:

```
5h 12% · 7d 83%! · resets 2h10m
```

One segment per window the payload carried, session window first; a `!`
on any segment at or over 80%; and a countdown on the window closest to
running out. `refreshInterval` is optional and re-runs the command every
N seconds on top of the event-driven updates — worth setting, because
the countdown is the one part of that line that goes stale on its own.

**The caveat:** `rate_limits` is not always in the payload. Claude Code's
own documentation says it appears only for Claude.ai subscribers (Pro or
Max) and only after the first API response of a session, and
[anthropics/claude-code#40094](https://github.com/anthropics/claude-code/issues/40094)
reports it missing for further plan and auth combinations. When it is
absent, `--statusline` prints nothing and exits 0 — your status line is
simply empty for those sessions. That is deliberate: a status line that
errored, or printed a placeholder, would be worse than one that says
nothing.

Two more things worth knowing:

- The line is a **human-readable surface**, not a contract. Unlike
  `--json` it may be reworded in any release; see
  [`cli-json.md`](cli-json.md).
- The status line command only runs after you have accepted the
  workspace trust dialog for that directory, the same gate hooks go
  through.

---

## Choosing a threshold

There is no correct number, but there is a way to pick one: a gate should
fire with enough room left to finish whatever it is guarding. If a run
typically burns 10% of a window, an 85% gate leaves you a run's worth of
margin and one to spare; a 95% gate does not.

Watch the 5-hour window for gating short work and the 7-day window for
gating anything you would be annoyed to lose overnight — `--fail-at`
already covers both, so you are choosing the number, not the window.
