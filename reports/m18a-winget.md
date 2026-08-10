# M18a — WinGet local install verification and submission preflight

**Session:** attended verification session (Opus 5 — top tier per `CLAUDE.md`'s
routing table), 2026-08-10. Hand-carried paste; the M11d dispatcher remains
paused. §4 stops applied throughout.
**Scope:** prove the `packaging/winget/` manifests by installing them, then
preflight the `microsoft/winget-pkgs` submission. **The upstream PR is the
owner's act — nothing was submitted, and `wingetcreate submit` was never run.**
**Tree footprint:** this report only. **No §4.1 path touched. No version bump.
`~/.claude/**` never read or written.**
**Host toolchain:** WinGet v1.29.280, wingetcreate 1.12.13.0, gh 2.92.0
(2026-04-28), Windows PowerShell 5.1.26100.8875, Windows 11 Pro 10.0.26200.

> **Verdict: the manifests are correct and the install is byte-honest — but
> the directory as committed cannot be validated, installed, or submitted.**
> `packaging/winget/README.md` sits alongside the three YAML files, and WinGet
> parses *every* file in a manifest directory as YAML. Its line 4 starts with
> a backtick, which YAML cannot start a token with, so `winget validate
> --manifest packaging\winget` — the exact command `packaging/winget/README.md`
> and `reports/m18a-endgate.md` §3 both tell the reader to run, and which the
> end-gate recorded as succeeding — **fails on the committed tree**. Causation
> was isolated with a negative control. With that one file removed the
> manifests validate, install, put both commands on PATH, report
> `Publisher: Cipher Pine` / `1.7.0` / `MIT OR Apache-2.0`, install bytes that
> hash-match the CI-signed release artifact exactly, and uninstall clean.
> **The fix is one file move and it is the owner's to approve — this session
> committed nothing but this report (F1, §9).**

---

## 1. Preconditions (step 1)

Tip is `ff85d7e` exactly — the prompt's floor, not merely a descendant — and
the tree is clean.

```
$ git log --oneline -3
ff85d7e M18a: end-gate report
af12f13 M18a: WinGet manifests for CipherPine.QuotaPane 1.7.0
085a396 M18a: docs/gating.md — quota gates as a practice

$ git status --porcelain
(no output)

$ git rev-parse HEAD
ff85d7e2941d60431bc896b362753801d9008c40

$ git merge-base --is-ancestor ff85d7e HEAD && echo YES
YES
```

```
$ winget --version
v1.29.280
```

Same WinGet build the end-gate used, so nothing below is explained by a
version difference.

## 2. Local manifests were disabled; enabling required elevation (step 2)

Prior state, recorded before touching anything:

```
$ winget settings export
{"$schema":"https://aka.ms/winget-settings-export.schema.json","adminSettings":{"BypassCertificatePinningForMicrosoftStore":false,"ConfigurationProcessorPath":false,"InstallerHashOverride":false,"LocalArchiveMalwareScanOverride":false,"LocalManifestFiles":false,"ProxyCommandLineOptions":false},"userSettingsFile":"C:\\Users\\dayto\\AppData\\Local\\Packages\\Microsoft.DesktopAppInstaller_8wekyb3d8bbwe\\LocalState\\settings.json"}
```

**`LocalManifestFiles: false`** — disabled. The session shell is not elevated
(`IsInRole(Administrator)` → `False`), and the command is an admin setting:

```
$ winget settings --enable LocalManifestFiles
This command requires administrator privileges to execute.
exit: 25
```

Re-run elevated via `Start-Process -Verb RunAs -Wait` (owner approved the UAC
dialog — deviation 6):

```
$ powershell -Command "$p = Start-Process -FilePath 'winget' -ArgumentList 'settings','--enable','LocalManifestFiles' -Verb RunAs -Wait -PassThru; ..."
elevated-exit-code: 0

$ winget settings export
... "LocalManifestFiles":true ...
```

**Baseline before installing anything** — neither command on PATH, no package
installed, and `WinGet\Links` already on the user PATH from unrelated packages
(so the step-4 check has to prove the *shims* appeared, not that PATH changed):

```
=== Get-Command quotapane (pre-install) ===
(nothing)
=== Get-Command quotapane-cli (pre-install) ===
(nothing)
=== WinGet\Packages entries matching quota ===
(nothing)
=== WinGet\Links entries matching quota ===
(nothing)
=== user PATH contains WinGet\Links? ===
C:\Users\dayto\AppData\Local\Microsoft\WinGet\Links
=== winget list CipherPine.QuotaPane (pre-install) ===
No installed package found matching input criteria.
exit: -1978335212
```

`WinGet\Links` held exactly seven unrelated shims: `ffmpeg.exe`, `ffplay.exe`,
`ffprobe.exe`, `rg.exe`, `uv.exe`, `uvw.exe`, `uvx.exe`. That set is the
yardstick for step 6.

## 3. F1 — the committed directory cannot be installed, or even validated (step 3)

The prompt's command, run verbatim from the repo root:

```
$ winget install --manifest packaging\winget
An unexpected error occurred while executing the command:
[YAML:Scanner] while scanning for the next token [line 4; col 1] found character that cannot start any token [line 4; col 1]
0x8a150004 : Opening manifest failed
exit: -1978335228
```

Not an install failure — a *parse* failure, before any download. The same
directory fails validation too:

```
$ winget validate --manifest packaging\winget
Manifest validation failed.
[YAML:Scanner] while scanning for the next token [line 4; col 1] found character that cannot start any token [line 4; col 1]
exit: -1978335191
```

**This contradicts `reports/m18a-endgate.md` §3**, which records
`winget validate --manifest packaging\winget` → `Manifest validation succeeded.`
on the same WinGet build. It is not reproducible on the committed tree.

### Cause, isolated

`--manifest <dir>` deserializes **every** file in the directory, not just
`*.yaml`. `packaging/winget/README.md` line 4 col 1 is a backtick:

```
$ sed -n '4p' packaging/winget/README.md | od -c | head -2
0000000   `   e   n   -   U   S   `       d   e   f   a   u   l   t   L
0000020   o   c   a   l   e   .       `   I   n   s   t   a   l   l   e
```

In YAML, `` ` `` is a reserved indicator that cannot begin a token — exactly
the scanner's complaint, at exactly that line and column.

Confirmed with a controlled pair (identical YAML in both directories; the only
variable is the README):

```
=== TEST A: three .yaml files only ===
CipherPine.QuotaPane.installer.yaml
CipherPine.QuotaPane.locale.en-US.yaml
CipherPine.QuotaPane.yaml
Manifest validation succeeded.
exit: 0

=== TEST B (negative control): same three + README.md ===
CipherPine.QuotaPane.installer.yaml
CipherPine.QuotaPane.locale.en-US.yaml
CipherPine.QuotaPane.yaml
README.md
Manifest validation failed.
[YAML:Scanner] while scanning for the next token [line 4; col 1] found character that cannot start any token [line 4; col 1]
exit: -1978335191
```

The README is the sole cause. The three manifests are innocent.

### Why the end-gate saw it pass

Inference, from file mtimes: the three YAML files are stamped `22:38`,
`README.md` `22:39`. All four landed in one commit (`af12f13`), but the README
was written **after** the YAML files — so a validate run in that window would
have seen a directory holding only manifests and legitimately succeeded. The
recorded output is almost certainly true of the moment it was run and false of
the tree that was committed. Nothing was checked afterwards, because
validation had already "passed".

### What it breaks

Every command the repo tells a reader to run against that directory:

| Where | Command | Status |
|---|---|---|
| `packaging/winget/README.md:40` | `winget validate --manifest packaging\winget` | **fails** (proven) |
| `packaging/winget/README.md:48` | `winget install --manifest packaging\winget` | **fails** (proven) |
| `packaging/winget/README.md:68` | `wingetcreate submit --token <token> packaging\winget` | **expected to fail** (inferred — see §8) |
| `reports/m18a-endgate.md:183` | recorded as succeeding | not reproducible |
| `reports/m18a-endgate.md:355` | owner instruction to install before submitting | fails as written |

The README documenting the manifests is what stops the manifests from being
used. **Fix: move it out of the manifest directory** — `packaging/winget-README.md`,
or `packaging/README.md`, or a `packaging/winget/docs/` subdirectory. The YAML
needs no change. Authoring that move is outside this session's one-file commit
(§9, deviation 1) and is left to the owner.

### Deviation taken, so the rest of the verification could run

The three `.yaml` files were copied to a scratch directory and the install run
from there. **This is a deviation and is numbered 1 in §9.** It is also
precisely the layout `winget-pkgs` will hold (§7), so what follows tests the
bytes that would actually be published.

```
$ winget install --manifest <scratch>\mf-yaml-only
Found QuotaPane [CipherPine.QuotaPane] Version 1.7.0
This application is licensed to you by its owner.
Microsoft is not responsible for, nor does it grant any licenses to, third-party packages.
Downloading https://github.com/cipherpine/quotapane/releases/download/v1.7.0/quotapane-v1.7.0-x86_64-pc-windows-msvc.zip
Successfully verified installer hash
Extracting archive...
Successfully extracted archive
Starting package install...
Command line alias added: "quotapane"
Command line alias added: "quotapane-cli"
Successfully installed
exit: 0
```

Both portable aliases created — the thing `winget validate` structurally
cannot prove, and the reason this step exists.

## 4. Fresh-shell verification (step 4)

`Start-Process -UseNewEnvironment` is unusable on Windows PowerShell 5.1 here
— it strips the environment far enough that the child cannot start:

```
Internal Windows PowerShell error. Loading managed Windows PowerShell failed with error 8009001d.
```

So the fresh shell is a **new process that rebuilds `PATH` from the registry**
(`Machine` then `User`, via `[Environment]::GetEnvironmentVariable`), discarding
everything inherited — the same value a new logon shell computes. That is a
stronger check than a plain new process, not a weaker one, since the parent's
`PATH` is thrown away rather than trusted (deviation 2).

```
=== fresh shell: PID 10216, PSVersion 5.1.26100.8875, PATH rebuilt from registry ===
=== registry PATH entries containing WinGet ===
C:\Users\dayto\AppData\Local\Microsoft\WinGet\Links

=== Get-Command quotapane-cli ===
C:\Users\dayto\AppData\Local\Microsoft\WinGet\Links\quotapane-cli.exe
=== Get-Command quotapane ===
C:\Users\dayto\AppData\Local\Microsoft\WinGet\Links\quotapane.exe

=== quotapane-cli --version ===
quotapane-cli 1.7.0
exit: 0

=== quotapane --help ===
error: unrecognized argument: --help
usage: quotapane [--client-version <VER>] [--codex-user-agent <UA>] [--no-tray]
                 [--plain | --themed] [--pace-demo] [--agents-demo]
exit: 2

=== winget list CipherPine.QuotaPane ===
Name      Id                                               Version
------------------------------------------------------------------
QuotaPane ARP\User\X64\CipherPine.QuotaPane__DefaultSource 1.7.0
exit: 0

=== winget show CipherPine.QuotaPane ===
No package found matching input criteria.
exit: -1978335212
```

- **`quotapane-cli --version` → `quotapane-cli 1.7.0`.** As specified.
- **`quotapane --help` resolves and no window opened.** `quotapane` has no
  `--help` flag — `parse_args` rejects unknown arguments
  (`crates/usage-ui/src/main.rs:663`), so it prints its usage line to stderr
  and returns `ExitCode::from(2)` without constructing a window or a poller.
  The alias resolving and the binary running is what step 4 asks for, and it
  is what happened; the flag itself not existing is worth a line in
  `packaging/winget/README.md` one day, but it is not a manifest defect.
  Checked the source before running it, specifically so an always-on-top
  window would not be left on screen.
- **`winget list` finds it**, at `1.7.0`, under the synthesized ARP id
  `ARP\User\X64\CipherPine.QuotaPane__DefaultSource` — how WinGet records a
  package installed from a local manifest rather than a source.
- **`winget show CipherPine.QuotaPane` finds nothing, and that is expected,
  not a manifest bug.** `winget show` queries configured *sources*; this
  package is in no source — that is the whole point of the pending
  `winget-pkgs` submission. There is no state in which this command could
  succeed today.

### The metadata, rendered (deviation 3)

`winget show --manifest` renders the same fields from the manifest directly —
the only way to see what `winget show` will print once the package is
published:

```
$ winget show --manifest <scratch>\mf-yaml-only
Found QuotaPane [CipherPine.QuotaPane]
Version: 1.7.0
Publisher: Cipher Pine
Publisher Url: https://github.com/cipherpine
Publisher Support Url: https://github.com/cipherpine/quotapane/issues
Moniker: quotapane
Description:
  QuotaPane is a small, always-on-top desktop window that shows how much of
  your Claude and Codex subscription quota you have left. It reads the
  credential files the official claude and codex CLIs already wrote on your
  machine, read-only, and talks to nothing else. There is no account to
  create and nothing is phoned home.
  [... full Description as authored, four paragraphs, rendered intact ...]
Homepage: https://github.com/cipherpine/quotapane
License: MIT OR Apache-2.0
License Url: https://github.com/cipherpine/quotapane/blob/main/LICENSE-MIT
Copyright: Copyright (c) 2026 The QuotaPane Contributors
Copyright Url: https://github.com/cipherpine/quotapane/blob/main/LICENSE-MIT
Release Notes Url: https://github.com/cipherpine/quotapane/releases/tag/v1.7.0
Documentation:
  README: https://github.com/cipherpine/quotapane/blob/main/README.md
  Security policy: https://github.com/cipherpine/quotapane/blob/main/SECURITY.md
Tags:
  ai
  anthropic
  claude
  cli
  codex
  developer-tools
  monitoring
  openai
  quota
  rate-limit
  rust
  usage
Installer:
  Installer Type: portable (zip)
  Installer Url: https://github.com/cipherpine/quotapane/releases/download/v1.7.0/quotapane-v1.7.0-x86_64-pc-windows-msvc.zip
  Installer SHA256: 5b465e544dfaf62c80bfba6f6093c2a4c674bbce94d414747a9a3b95be86b64f
  Release Date: 2026-08-08
  Offline Distribution Supported: true
exit: 0
```

The four fields the prompt asked for, exactly as displayed:

| Field | As displayed | Expected | |
|---|---|---|---|
| Publisher | `Cipher Pine` | `Cipher Pine` | ✅ |
| Version | `1.7.0` | `1.7.0` | ✅ |
| License | `MIT OR Apache-2.0` | — | ✅ dual licence, per DECISIONS §1 |
| Homepage | `https://github.com/cipherpine/quotapane` | — | ✅ |

**`Publisher` renders as `Cipher Pine`.** No manifest bug on this axis.

## 5. Trust cross-check — the installed bytes are the signed bytes (step 5)

WinGet said `Successfully verified installer hash`, but that only proves the
download matched the manifest. The chain has to reach back to what CI signed.

Fresh download of both the archive and the release's own checksum file:

```
$ gh release download v1.7.0 --repo cipherpine/quotapane \
    --pattern 'quotapane-v1.7.0-x86_64-pc-windows-msvc.zip' --pattern 'SHA256SUMS' --dir dl

$ cat SHA256SUMS
5b465e544dfaf62c80bfba6f6093c2a4c674bbce94d414747a9a3b95be86b64f  quotapane-v1.7.0-x86_64-pc-windows-msvc.zip
cf4b22ada89803dfbfc91ee2aa594cd242cca386c5189edf438f648dc12cf7a4  quotapane-v1.7.0-x86_64-unknown-linux-gnu.tar.gz

$ sha256sum quotapane-v1.7.0-x86_64-pc-windows-msvc.zip
5b465e544dfaf62c80bfba6f6093c2a4c674bbce94d414747a9a3b95be86b64f *quotapane-v1.7.0-x86_64-pc-windows-msvc.zip

$ sha256sum -c SHA256SUMS --ignore-missing
quotapane-v1.7.0-x86_64-pc-windows-msvc.zip: OK
```

That value is character-for-character the manifest's `InstallerSha256`
(`5B465E54…B64F`, case aside). Then the exe-level compare:

```
=== contents of the freshly downloaded zip ===
LICENSE-APACHE
LICENSE-MIT
quotapane-cli.exe
quotapane.exe
README.md
TOOLCHAIN.txt

=== quotapane-cli.exe ===
installed (package dir) : 68106C1A6E13DDF270C1ECF11A11820F6FD3D3F6DB9261CA0D6541EB248DFC7F
from fresh zip download : 68106C1A6E13DDF270C1ECF11A11820F6FD3D3F6DB9261CA0D6541EB248DFC7F
PATH shim (symlink)     : 68106C1A6E13DDF270C1ECF11A11820F6FD3D3F6DB9261CA0D6541EB248DFC7F
sizes                   : installed=2165248  zip=2165248
MATCH: installed bytes == release artifact bytes

=== quotapane.exe ===
installed (package dir) : 5FA0F78BED701B2A70895B9647DF669416FA479189E4AB8714C7ED7F9EB0B6D7
from fresh zip download : 5FA0F78BED701B2A70895B9647DF669416FA479189E4AB8714C7ED7F9EB0B6D7
PATH shim (symlink)     : 5FA0F78BED701B2A70895B9647DF669416FA479189E4AB8714C7ED7F9EB0B6D7
sizes                   : installed=8514048  zip=8514048
MATCH: installed bytes == release artifact bytes
```

The prompt asked for `quotapane-cli.exe`; `quotapane.exe` was done too, since
the manifest ships both and half a proof is not one.

**Unbroken chain:** CI-produced `SHA256SUMS` (cosign keyless bundle, per M6) →
independent hash of a fresh download → identical to the manifest's
`InstallerSha256` → WinGet's own hash verification at install → installed exes
byte-identical to the exes inside that archive. WinGet installed the bytes CI
signed.

Also recorded: the two PATH entries are **symbolic links**, not copies or
stubs, so the shim and the target are necessarily the same bytes —

```
C:\...\WinGet\Links\quotapane-cli.exe  LinkType=SymbolicLink  Target=C:\...\Packages\CipherPine.QuotaPane__DefaultSource\quotapane-v1.7.0-x86_64-pc-windows-msvc\quotapane-cli.exe
C:\...\WinGet\Links\quotapane.exe      LinkType=SymbolicLink  Target=C:\...\Packages\CipherPine.QuotaPane__DefaultSource\quotapane-v1.7.0-x86_64-pc-windows-msvc\quotapane.exe
```

and the archive unpacks into the single versioned directory the manifest's
`RelativeFilePath` entries assume.

## 6. Uninstall and restore (step 6)

The obvious command does not work (deviation 4):

```
$ winget uninstall CipherPine.QuotaPane
No installed package found matching input criteria.
exit: -1978335212
```

A local-manifest install is recorded under the synthesized ARP id, not the
package identifier, so the identifier does not match on uninstall even though
`winget list CipherPine.QuotaPane` finds it. Matching by name works:

```
$ winget uninstall QuotaPane
Found QuotaPane [ARP\User\X64\CipherPine.QuotaPane__DefaultSource]
Starting package uninstall...
Successfully uninstalled
exit: 0
```

**This is an artifact of local-manifest installation, not a manifest defect** —
a package installed from the `winget` source correlates to its identifier
normally. Worth knowing before anyone tries to script a teardown.

Fresh shell (PATH again rebuilt from the registry), after:

```
=== fresh shell: PID 23424, PATH rebuilt from registry ===
=== Get-Command quotapane-cli (expect nothing) ===
(nothing)
=== Get-Command quotapane (expect nothing) ===
(nothing)

=== quotapane-cli --version (expect: not recognized) ===
'quotapane-cli' is not recognized as an internal or external command,
operable program or batch file.
exit: 1
=== quotapane --help (expect: not recognized) ===
'quotapane' is not recognized as an internal or external command,
operable program or batch file.
exit: 1

=== portable install dir (expect: False) ===
C:\Users\dayto\AppData\Local\Microsoft\WinGet\Packages\CipherPine.QuotaPane__DefaultSource -> False
=== PATH shims (expect: False, False) ===
C:\Users\dayto\AppData\Local\Microsoft\WinGet\Links\quotapane.exe -> False
C:\Users\dayto\AppData\Local\Microsoft\WinGet\Links\quotapane-cli.exe -> False

=== WinGet\Links now holds (should be the pre-install set) ===
ffmpeg.exe
ffplay.exe
ffprobe.exe
rg.exe
uv.exe
uvw.exe
uvx.exe

=== winget list CipherPine.QuotaPane ===
No installed package found matching input criteria.
exit: -1978335212
```

Both commands gone, the portable directory gone, and `WinGet\Links` back to
the exact seven-entry pre-install set from §2. Nothing orphaned.

Setting restored to its prior state:

```
$ powershell -Command "$p = Start-Process -FilePath 'winget' -ArgumentList 'settings','--disable','LocalManifestFiles' -Verb RunAs -Wait -PassThru; ..."
elevated-exit-code: 0

$ winget settings export
{"$schema":"https://aka.ms/winget-settings-export.schema.json","adminSettings":{"BypassCertificatePinningForMicrosoftStore":false,"ConfigurationProcessorPath":false,"InstallerHashOverride":false,"LocalArchiveMalwareScanOverride":false,"LocalManifestFiles":false,"ProxyCommandLineOptions":false},"userSettingsFile":"C:\\Users\\dayto\\AppData\\Local\\Packages\\Microsoft.DesktopAppInstaller_8wekyb3d8bbwe\\LocalState\\settings.json"}
```

Byte-identical to the §2 pre-session export. `LocalManifestFiles: false`.

## 7. Submission preflight (steps 7–8)

### wingetcreate

Not present:

```
$ wingetcreate --version
The term 'wingetcreate' is not recognized as the name of a cmdlet, function, script file, or operable program.
$ winget list --id Microsoft.WingetCreate --exact
No installed package found matching input criteria.
```

Installed, per the prompt's allowance — **this is the one install left in
place**:

```
$ winget install Microsoft.WingetCreate
Found Windows Package Manager Manifest Creator [Microsoft.WingetCreate] Version 1.12.13.0
This application is licensed to you by its owner.
Microsoft is not responsible for, nor does it grant any licenses to, third-party packages.
Successfully verified installer hash
Starting package install...
Successfully installed
exit: 0

$ wingetcreate --version
WingetCreateCLI 1.12.13.0+1aa9b6637ca872911b84639d89526f5fe67a06c3

$ (Get-Command wingetcreate).Source
C:\Users\dayto\AppData\Local\Microsoft\WindowsApps\wingetcreate.exe
```

### wingetcreate has no standalone validate verb (deviation 5)

Its full command list is `new`, `update`, `new-locale`, `update-locale`,
`submit`, `settings`, `token`, `cache`, `show`, `info`, `dsc`. Validation runs
**inside `submit`** — there is no way to invoke wingetcreate's validator
without invoking submission, and `wingetcreate submit` is a hard edge for this
session. It was not run.

`update` is not a substitute: it resolves the package from the `winget-pkgs`
repo by identifier, and `CipherPine.QuotaPane` is not published there.

What was run instead is the validation `winget-pkgs` CI performs first, against
the **real upstream directory layout**, built from the repo's own manifests:

```
=== upstream layout built from the repo manifests ===
manifests\c\CipherPine\QuotaPane\1.7.0\CipherPine.QuotaPane.installer.yaml
manifests\c\CipherPine\QuotaPane\1.7.0\CipherPine.QuotaPane.locale.en-US.yaml
manifests\c\CipherPine\QuotaPane\1.7.0\CipherPine.QuotaPane.yaml

=== winget validate --manifest <upstream layout path> ===
Manifest validation succeeded.
exit: 0
```

### The layout maps cleanly

```
=== filename convention check ===
CipherPine.QuotaPane.yaml -> present
CipherPine.QuotaPane.installer.yaml -> present
CipherPine.QuotaPane.locale.en-US.yaml -> present
extra files in the version folder: (none)

=== PackageIdentifier / PackageVersion consistency across the three files ===
CipherPine.QuotaPane.installer.yaml: id=CipherPine.QuotaPane version=1.7.0 type=installer manifestVersion=1.6.0
CipherPine.QuotaPane.locale.en-US.yaml: id=CipherPine.QuotaPane version=1.7.0 type=defaultLocale manifestVersion=1.6.0
CipherPine.QuotaPane.yaml: id=CipherPine.QuotaPane version=1.7.0 type=version manifestVersion=1.6.0
```

- Path `manifests/c/CipherPine/QuotaPane/1.7.0/` is derived correctly from the
  identifier: first letter of the publisher lowercased, then publisher, then
  package, then version.
- Filenames follow `<PackageIdentifier>[.installer|.locale.<lang>].yaml`.
- One `version`, one `installer`, one `defaultLocale`; identifier, version and
  manifest version agree across all three.
- **`extra files in the version folder: (none)`** — the README is what F1 is
  about, and it must not be copied upstream either.

Upstream reality, read-only:

```
$ gh api repos/microsoft/winget-pkgs/contents/manifests/c/CipherPine
{"message":"Not Found", ... "status":"404"}

$ gh api repos/microsoft/winget-pkgs/contents/manifests/c --jq '.[].name' | head -12
C-POPLO
C-PartnerSystemhausGmbH
CABS
CAI
CBackup
CCExtractor
CCF
CCL
CCPGames
CCRMA
CDESoftware
CE-Programming
```

**No identifier collision** — `CipherPine` does not exist upstream, so this is
a new publisher directory and a first submission. The sibling names confirm
the `manifests/c/<Publisher>/` convention.

```
$ gh api user --jq '.login'
cipherpine

$ gh api repos/cipherpine/winget-pkgs
{"message":"Not Found", ... "status":"404"}
```

The authenticated identity is `cipherpine`, which matches the org owning the
release URLs — the association a moderator looks for. **No fork of
`winget-pkgs` exists yet**, so the first submission will create one.

## 8. The submit command, and what the first-time flow will ask (step 9)

**Precondition, from F1:** move `packaging/winget/README.md` out of the
manifest directory first. `wingetcreate submit` takes a directory and loads
every file in it the same way `winget validate` and `winget install` do, so it
is expected to fail on the identical YAML scanner error. *Inferred, not
proven — `wingetcreate submit` was not run, per the hard edges.* Either move
the README, or point the command at a directory holding only the three YAML
files.

The one line, once the directory holds only manifests:

```powershell
wingetcreate submit --prtitle "New package: CipherPine.QuotaPane version 1.7.0" packaging\winget
```

`--prtitle` is optional; omitted, wingetcreate generates an equivalent title.
Do **not** pass `--token` on the command line — wingetcreate's own help warns
`Using this argument may result in the token being logged`; let it prompt.

What the first run will ask, in order:

1. **GitHub authorization.** With no cached token, wingetcreate starts a
   device flow: it prints a user code, opens `github.com/login/device`, and
   waits for the "Windows Package Manager Manifest Creator" app to be
   authorized. The token is then cached (manageable later via
   `wingetcreate token`). A PAT with `public_repo` scope works instead.
2. **A fork of `microsoft/winget-pkgs` under `cipherpine`** — confirmed above
   not to exist, so it will be created. This is the step that publishes under
   the owner's GitHub identity.
3. **The `winget-pkgs` contribution terms.** The PR body carries that repo's
   template, including the acknowledgement that the submission follows the
   contribution guidelines and that the publisher has the right to distribute
   the package. Agreeing to it on the publisher's behalf is the reason this
   session stopped short of submitting.
4. **The PR opens in a browser** unless `--no-open` is passed.
5. **Then `winget-pkgs` CI runs**, and it does more than schema validation: it
   downloads the installer and installs it in a sandbox. §3 and §5 above are
   a local dry run of exactly that, and both passed.

One thing worth knowing before running it, given this project's posture:

> **wingetcreate has telemetry on by default.** Its own `info` output:
> *"The Windows Package Manager Manifest Creator collects usage data in order
> to improve your experience… By default, telemetry is enabled but can be
> disabled by running `wingetcreate settings`."* QuotaPane's invariant 4 is
> about QuotaPane, not about Microsoft's tooling, so this is not an invariant
> breach — but a project whose headline is "no telemetry of any kind" may not
> want its publishing tool phoning home. `wingetcreate settings` opens the
> settings file; the fork-and-PR route in `packaging/winget/README.md` avoids
> the tool entirely.

## 9. Deviations, numbered

1. **The install did not run against `packaging\winget`.** It could not — F1,
   §3. It ran against a scratch copy of the three `.yaml` files, which is the
   layout `winget-pkgs` will hold. Nothing in the repo was modified to make it
   work: the failure is recorded rather than patched around, and the fix is
   left to the owner. Everything §3–§6 proves is true of the manifests as
   committed; only their *directory* is broken.
2. **"Fresh shell" is a new process with `PATH` rebuilt from the registry**,
   not `Start-Process -UseNewEnvironment` — that switch cannot launch
   PowerShell 5.1 on this host (`error 8009001d`). The substitute is stricter:
   the inherited `PATH` is discarded entirely. Noted because
   `WinGet\Links` was already on `PATH` before the install, so the check that
   matters is the shims appearing and disappearing, and it was made against
   registry state both times.
3. **`winget show CipherPine.QuotaPane` returned no package**, so the
   Publisher/Version/License/homepage fields were read from
   `winget show --manifest`. Expected behaviour — the package is in no source
   until the submission lands — not a manifest bug. Publisher is `Cipher Pine`
   as expected.
4. **`winget uninstall CipherPine.QuotaPane` did not match**; `winget uninstall
   QuotaPane` did. An artifact of local-manifest installs recording a
   synthesized ARP id, §6.
5. **wingetcreate's validator could not be invoked directly** — it exists only
   inside `submit`, which is a hard edge. Substituted `winget validate` against
   the real `manifests/c/CipherPine/QuotaPane/1.7.0/` layout, plus filename,
   identifier and cross-file consistency checks, §7.
6. **Two UAC prompts.** `winget settings --enable/--disable LocalManifestFiles`
   is an admin setting and the session shell is not elevated; the owner
   approved both dialogs. The install itself was run **unelevated**, on
   purpose, so it landed as a user-scope portable install — the realistic case.

Not deviations, but recorded: `quotapane` has no `--help` flag, so
`quotapane --help` prints its usage line and exits 2 (§4); and the
`jsonschema`/`pyyaml` modules are absent on this host, so no second,
independent schema validation was run beyond WinGet's own — installing pip
packages would have exceeded "nothing installed beyond steps 3 and 7".

## 10. What the owner must do next

Nothing here is accepted; §4.8 leaves the gates with you.

1. **Fix F1 before anything else.** Move `packaging/winget/README.md` out of
   the manifest directory. Until then `winget validate`, `winget install` and
   `wingetcreate submit` all fail against `packaging\winget`, including the
   three commands that README itself documents.
2. **Correct the two stale claims** produced by F1: `packaging/winget/README.md`
   §"How these were validated" step 3, and `reports/m18a-endgate.md` §3, both
   record a `winget validate` success that does not reproduce on the committed
   tree. The end-gate is a report of record and should not be rewritten
   silently; a correction note is the honest form.
3. **Everything else about the manifests is proven good** — installs, both
   aliases land on PATH, `Publisher: Cipher Pine`, `1.7.0`,
   `MIT OR Apache-2.0`, installed bytes hash-match the CI-signed release, and
   uninstall is clean. Item 3 of the end-gate's §9 ("prove the WinGet manifests
   by installing them") is discharged, with the F1 caveat.
4. **The submission remains yours**, §8. Nothing was submitted, no PR was
   opened, `wingetcreate submit` was never run, and no fork of `winget-pkgs`
   exists under `cipherpine`.
5. **`Microsoft.WingetCreate` 1.12.13.0 is installed and left in place**, per
   the prompt. It is the only thing this session added to the machine;
   `CipherPine.QuotaPane` was removed and `LocalManifestFiles` restored to
   `false`. Consider `wingetcreate settings` for its telemetry default (§8).
6. **Decide whether `packaging/winget/README.md` should mention** that
   `quotapane` has no `--help` and that `winget uninstall` needs the package
   name for local installs. Both are small, both cost someone ten minutes.
