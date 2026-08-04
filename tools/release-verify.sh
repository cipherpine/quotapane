#!/usr/bin/env bash
# tools/release-verify.sh — the six-step outsider verification plus the six
# negative controls, rules R1-R4 built in, as one deterministic run.
#
#   R1  digest-match BOTH attestation subjects against the bytes on disk
#   R2  every negative control asserts its SPECIFIC error and artifacts are
#       restored (and proven restored) before the next control runs
#   R3  tamper controls prove the bytes actually changed (before/after digest)
#   R4  the wrong-repo control targets a repo confirmed to exist, so its
#       failure cannot be a "no such repo" false pass
#
# Usage:   tools/release-verify.sh <tag> [owner/repo]
# Example: tools/release-verify.sh v1.4.1
#
# Requires: gh (authenticated), cosign >= 3.1 (bundle-aware), sha256sum,
# unzip, tar, python3. Talks only to GitHub via gh/cosign. Never reads
# credential files; runs shipped binaries with --version/--help only.
# Identity regex uses [.] (not \.) so Git Bash cannot mangle it — see the
# README's Verify-a-release section for why.
#
# This file is part of the release-verify standard (prompts/ release specs).
set -u
TAG="${1:?usage: release-verify.sh <tag> [owner/repo]}"
REPO="${2:-cipherpine/quotapane}"
WRONG_REPO="cli/cli"
WIN="quotapane-${TAG}-x86_64-pc-windows-msvc.zip"
LIN="quotapane-${TAG}-x86_64-unknown-linux-gnu.tar.gz"
ISSUER="https://token.actions.githubusercontent.com"
IDENT="^https://github[.]com/${REPO//\//\/}/[.]github/workflows/release[.]yml@refs/tags/"

FAILURES=0
note()  { printf '%s\n' "$*"; }
pass()  { printf 'PASS  %s\n' "$*"; }
fail()  { printf 'FAIL  %s\n' "$*"; FAILURES=$((FAILURES+1)); }
digest(){ sha256sum "$1" | cut -d' ' -f1; }

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

# ---- step 1: fresh download into an empty directory -------------------------
if gh release download "$TAG" -R "$REPO" -D . >/dev/null 2>&1 \
   && [ -f SHA256SUMS ] && [ -f SHA256SUMS.sigstore.json ] \
   && [ -f "$WIN" ] && [ -f "$LIN" ]; then
  pass "step 1: downloaded 4 assets for $TAG"
else
  fail "step 1: download incomplete (need SHA256SUMS, bundle, $WIN, $LIN)"; echo "RESULT: FAIL"; exit 1
fi
mkdir pristine && cp SHA256SUMS SHA256SUMS.sigstore.json "$WIN" "$LIN" pristine/
D_WIN0="$(digest "$WIN")"; D_LIN0="$(digest "$LIN")"; D_SUMS0="$(digest SHA256SUMS)"

verify_checksums(){ sha256sum --ignore-missing -c SHA256SUMS >/dev/null 2>&1; }
verify_blob(){ cosign verify-blob SHA256SUMS --bundle SHA256SUMS.sigstore.json \
  --certificate-identity-regexp "$IDENT" --certificate-oidc-issuer "$ISSUER" 2>&1; }
verify_att(){ gh attestation verify "$1" -R "$2" 2>&1; }

# ---- steps 2-4 ---------------------------------------------------------------
if verify_checksums; then pass "step 2: sha256sum -c SHA256SUMS"; else fail "step 2: checksum mismatch"; fi
OUT="$(verify_blob)" && grep -q "Verified OK" <<<"$OUT" \
  && pass "step 3: cosign verify-blob (Verified OK)" || fail "step 3: cosign verify-blob"
verify_att "$WIN" "$REPO" >/dev/null && pass "step 4a: attestation $WIN" || fail "step 4a: attestation $WIN"
verify_att "$LIN" "$REPO" >/dev/null && pass "step 4b: attestation $LIN" || fail "step 4b: attestation $LIN"

# ---- R1: both subjects digest-matched ---------------------------------------
SUBJ_JSON="$(gh attestation verify "$WIN" -R "$REPO" --format json 2>/dev/null)"
R1_OK="$(SUBJ_JSON="$SUBJ_JSON" python3 -c "
import json, os, sys
want = {sys.argv[1], sys.argv[2]}
try:
    data = json.loads(os.environ['SUBJ_JSON'])
except Exception as e:
    print(f'MISMATCH (parse error: {e})'); raise SystemExit
subs = set()
for att in data:
    stmt = att.get('verificationResult', {}).get('statement') or att.get('statement') or {}
    for s in stmt.get('subject', []):
        d = s.get('digest', {}).get('sha256')
        if d: subs.add(d)
print('OK' if want <= subs and len(subs) >= 2 else f'MISMATCH attested={sorted(subs)}')
" "$D_WIN0" "$D_LIN0")" || R1_OK="MISMATCH (python failed)"
[ "$R1_OK" = "OK" ] && pass "R1: both attestation subjects digest-match disk" \
  || fail "R1: $R1_OK"

# ---- step 5: extract + inventory --------------------------------------------
mkdir x_win x_lin
unzip -q "$WIN" -d x_win && tar -xzf "$LIN" -C x_lin
INV_OK=1
for f in quotapane.exe quotapane-cli.exe LICENSE-MIT LICENSE-APACHE README.md TOOLCHAIN.txt; do
  find x_win -name "$f" | grep -q . || { INV_OK=0; note "  missing in zip: $f"; }
done
for f in quotapane quotapane-cli LICENSE-MIT LICENSE-APACHE README.md TOOLCHAIN.txt; do
  find x_lin -name "$f" | grep -q . || { INV_OK=0; note "  missing in tar: $f"; }
done
TW="$(find x_win -name TOOLCHAIN.txt | head -1)"; TL="$(find x_lin -name TOOLCHAIN.txt | head -1)"
cmp -s "$TW" "$TL" || { INV_OK=0; note "  TOOLCHAIN.txt differs between archives"; }
[ "$INV_OK" = 1 ] && pass "step 5: inventory + identical TOOLCHAIN.txt" || fail "step 5: inventory"

# ---- step 6: run the shipped CLI (platform-appropriate; no credentials) -----
BIN=""; STEP6="ran"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) BIN="$(find x_win -name quotapane-cli.exe | head -1)";;
  Linux)                BIN="$(find x_lin -name quotapane-cli    | head -1)"; chmod +x "$BIN" 2>/dev/null;;
esac
if [ -n "$BIN" ]; then
  # An rc tag points at the exact commit that ships as the final version, so
  # the crate version has no -rc suffix; assert against the base version.
  BASEVER="${TAG#v}"; BASEVER="${BASEVER%%-rc.*}"
  V="$("$BIN" --version 2>&1)" && grep -q "$BASEVER" <<<"$V" && "$BIN" --help >/dev/null 2>&1 \
    && pass "step 6: shipped CLI runs ($V, matches $BASEVER)" || fail "step 6: shipped CLI ($V, wanted $BASEVER)"
else
  STEP6="skipped"
  note "step 6: SKIP — no binary for this host platform (record as skipped, not passed)"
fi

# ---- negative controls -------------------------------------------------------
restore(){ cp "pristine/$1" "$1"; [ "$(digest "$1")" = "$2" ] \
  && pass "  restored: $1 digest back to pristine" || fail "  restore of $1 did not match pristine"; }

# NC1: one-byte corruption of the zip @ offset 5000
printf '\x00' | dd of="$WIN" bs=1 seek=5000 conv=notrunc >/dev/null 2>&1
D_TAMP="$(digest "$WIN")"
[ "$D_TAMP" != "$D_WIN0" ] && pass "NC1/R3: zip bytes changed ($D_WIN0 -> $D_TAMP)" \
  || fail "NC1/R3: tamper did not change bytes"
OUT="$(cd . && sha256sum --ignore-missing -c SHA256SUMS 2>&1)"; RC=$?
[ $RC -ne 0 ] && grep -q "$WIN: FAILED" <<<"$OUT" \
  && pass "NC1: checksum names the tampered file specifically" \
  || fail "NC1: expected '$WIN: FAILED', got rc=$RC"
verify_att "$WIN" "$REPO" >/dev/null 2>&1 && fail "NC1: attestation passed on tampered zip" \
  || pass "NC1: attestation refuses tampered zip"
restore "$WIN" "$D_WIN0"

# NC2: wrong certificate identity
OUT="$(cosign verify-blob SHA256SUMS --bundle SHA256SUMS.sigstore.json \
  --certificate-identity-regexp '^https://github[.]com/someone-else/some-other-repo/' \
  --certificate-oidc-issuer "$ISSUER" 2>&1)"; RC=$?
[ $RC -ne 0 ] && grep -qi "certificateidentity\|none of the expected identities" <<<"$OUT" \
  && pass "NC2: wrong identity rejected on the identity check specifically" \
  || fail "NC2: expected CertificateIdentity error, got rc=$RC"

# NC3: wrong OIDC issuer (distinct error from NC2)
OUT="$(cosign verify-blob SHA256SUMS --bundle SHA256SUMS.sigstore.json \
  --certificate-identity-regexp "$IDENT" \
  --certificate-oidc-issuer "https://accounts.google.com" 2>&1)"; RC=$?
[ $RC -ne 0 ] && grep -qi "issuer" <<<"$OUT" \
  && pass "NC3: wrong issuer rejected on the issuer check specifically" \
  || fail "NC3: expected issuer error, got rc=$RC"

# NC4: tamper SHA256SUMS itself (first byte)
FIRST="$(head -c1 SHA256SUMS)"; SUB=$([ "$FIRST" = "9" ] && echo 0 || echo 9)
printf '%s' "$SUB" | dd of=SHA256SUMS bs=1 seek=0 conv=notrunc >/dev/null 2>&1
D_TAMP="$(digest SHA256SUMS)"
[ "$D_TAMP" != "$D_SUMS0" ] && pass "NC4/R3: SHA256SUMS bytes changed ($D_SUMS0 -> $D_TAMP)" \
  || fail "NC4/R3: tamper did not change bytes"
OUT="$(verify_blob)"; RC=$?
[ $RC -ne 0 ] && grep -qi "signature" <<<"$OUT" \
  && pass "NC4: cosign rejects tampered SHA256SUMS on the signature check" \
  || fail "NC4: expected signature error, got rc=$RC"
restore SHA256SUMS "$D_SUMS0"

# NC5: wrong repo — R4: confirm the target exists FIRST
if gh api "repos/$WRONG_REPO" --jq .full_name >/dev/null 2>&1; then
  pass "NC5/R4: wrong-repo target $WRONG_REPO confirmed to exist"
  OUT="$(verify_att "$WIN" "$WRONG_REPO")"; RC=$?
  [ $RC -ne 0 ] && grep -qi "attestation" <<<"$OUT" \
    && pass "NC5: real repo, real digest-miss (no attestations for our artifact)" \
    || fail "NC5: expected no-attestations failure, got rc=$RC"
else
  fail "NC5/R4: could not confirm $WRONG_REPO exists — control is meaningless, marked FAILED"
fi

# NC6: unrelated file has no attestation
head -c 4096 /dev/urandom > unrelated.bin
verify_att unrelated.bin "$REPO" >/dev/null 2>&1 && fail "NC6: attestation passed on unrelated file" \
  || pass "NC6: unrelated file has no attestation"
rm -f unrelated.bin

# ---- post-control sweep: everything pristine, steps 2-4 re-run --------------
SWEEP=1
[ "$(digest "$WIN")" = "$D_WIN0" ] || SWEEP=0; [ "$(digest "$LIN")" = "$D_LIN0" ] || SWEEP=0
[ "$(digest SHA256SUMS)" = "$D_SUMS0" ] || SWEEP=0
verify_checksums || SWEEP=0
grep -q "Verified OK" <<<"$(verify_blob)" || SWEEP=0
verify_att "$WIN" "$REPO" >/dev/null && verify_att "$LIN" "$REPO" >/dev/null || SWEEP=0
[ "$SWEEP" = 1 ] && pass "sweep: all artifacts pristine, steps 2-4 re-run clean" \
  || fail "sweep: post-control state is not pristine"

# ---- verdict ------------------------------------------------------------------
if [ "$FAILURES" -eq 0 ]; then
  if [ "$STEP6" = "ran" ]; then
    echo "RESULT: PASS — $TAG: six steps, six controls, R1-R4"
  else
    echo "RESULT: PASS — $TAG: five of six steps (step 6 skipped: no binary for this host), six controls, R1-R4"
  fi
  exit 0
else
  echo "RESULT: FAIL — $FAILURES check(s) failed for $TAG"
  exit 1
fi
