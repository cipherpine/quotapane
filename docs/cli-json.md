# `quotapane-cli --json` — the output contract

This documents every key `quotapane-cli --json` emits today, with its
type and whether it can be `null`. It is the reference for anyone
parsing QuotaPane's output in a script.

Only `--json` output is a contract. The text summary is for humans and
may be reworded at any time; `--debug-raw` prints a provider's wire
response, which is the provider's shape, not ours; and `--statusline`
emits a one-line human-format surface (`5h 12% · 7d 83%! · resets 2h10m`)
that is **not** covered by this contract and may be reworded in any
release.

## Stability policy

Keys are never renamed or removed within a major version. New keys
may be added in any release and are announced in the CHANGELOG.
Consumers must ignore keys they do not recognize.

In practice that means: read the keys you need by name, tolerate keys
you have never seen, and do not depend on key order or on whitespace.

## Top-level shape

| Invocation | stdout |
|---|---|
| `--once --json` | one snapshot **object**, pretty-printed over several lines |
| `--once --json --provider all` | an **array** of snapshot objects, pretty-printed |
| `--watch <SECS> --json` | **NDJSON**: one cycle per line, compact, no internal newlines |
| `--watch <SECS> --json --provider all` | NDJSON, each line a compact array |

With a single provider that failed to poll, stdout is empty — the
error goes to stderr and the exit code is non-zero. With
`--provider all`, providers that polled successfully are still
emitted; a provider that failed is simply absent from the array.

Array order is the provider order QuotaPane polled in: `claude` before
`codex`.

## The snapshot object

| Key | Type | Nullable | Meaning |
|---|---|---|---|
| `provider` | string | no | Which provider this snapshot describes. See [enumerated values](#enumerated-values). |
| `taken_at_unix_secs` | number (integer) | no | Unix timestamp, in seconds, of when the poll completed. |
| `windows` | array of [window](#the-window-object) | no | The headline quota windows — the small set describing the subscription as a whole. May be empty. |
| `per_model` | array of [window](#the-window-object) | no | Quota windows scoped to a single model, when the provider reports any. May be empty; the key is always present. |
| `reset_credits` | [object](#the-reset-credits-object) | **yes** | Reset credits the provider reports. `null` for a provider with no such concept (Claude). |
| `source` | string | no | Which data path produced the snapshot. See [enumerated values](#enumerated-values). |

`per_model` carries every bucket the provider reported, including
untouched ones (`used_fraction` of `0.0` or `null`). The window hides
those rows for readability; the JSON does not — a script reading it
sees the full truth.

## The window object

The same shape in `windows` and in `per_model`: a per-model row *is* a
quota window, just scoped to one model.

| Key | Type | Nullable | Meaning |
|---|---|---|---|
| `label` | string | no | The window's display label, from the provider verbatim — e.g. `"5h"`, `"7d"`, `"7d-opus"`, `"GPT-5.3-Codex-Max"`. Not a stable identifier: providers rename these. Do not parse it. |
| `used_fraction` | number (float) | **yes** | Fraction of the window consumed, `0.0`–`1.0`. `null` when the provider did not report usage for this window. |
| `resets_in_secs` | number (integer) | **yes** | Seconds until the window resets. `null` when unknown — including when the provider's reset timestamp failed validation, which is reported as unknown rather than guessed. |
| `duration_secs` | number (integer) | **yes** | The window's **total** length in seconds — `18000` for a five-hour window, `604800` for a weekly one. `null` when the provider neither stated nor implied it. Never derived from `label`. |

`null` means "not known", never "zero". A `used_fraction` of `0.0` is
a measured zero; `null` is the absence of a measurement. Gates should
treat the two differently — `--fail-at` skips `null` windows rather
than reading them as `0` or as `100`.

## The reset credits object

Present as an object only when the provider reports reset credits
(Codex); otherwise the `reset_credits` key is `null`.

| Key | Type | Nullable | Meaning |
|---|---|---|---|
| `available` | number (integer) | no | Credits the account owns. |
| `applicable_now` | number (integer) | **yes** | Credits usable right now, when the provider says. Normally `0` while not rate-limited. `null` when the provider did not report it. |

Owning a credit and being able to spend one are different facts, which
is why they are two keys rather than one number.

## Enumerated values

| Key | Values today |
|---|---|
| `provider` | `"claude_subscription"`, `"codex_subscription"` |
| `source` | `"usage_endpoint"`, `"rate_limit_headers"` |

`source` is `"usage_endpoint"` in every snapshot the shipped providers
currently produce; `"rate_limit_headers"` is defined for a header
fallback path that is not currently emitted. Treat both as possible
and treat an unrecognized value as "some other source" rather than an
error — the same rule as unrecognized keys.

## Examples

`--once --json` (single provider, pretty-printed):

```json
{
  "provider": "codex_subscription",
  "taken_at_unix_secs": 1784000000,
  "windows": [
    {
      "label": "5h",
      "used_fraction": 0.42,
      "resets_in_secs": 900,
      "duration_secs": 18000
    }
  ],
  "per_model": [
    {
      "label": "GPT-5.3-Codex-Max",
      "used_fraction": null,
      "resets_in_secs": null,
      "duration_secs": null
    }
  ],
  "reset_credits": {
    "available": 1,
    "applicable_now": null
  },
  "source": "usage_endpoint"
}
```

`--watch 300 --json` — the same object, one compact line per cycle:

```
{"provider":"codex_subscription","taken_at_unix_secs":1784000000,"windows":[{"label":"5h","used_fraction":0.42,"resets_in_secs":900,"duration_secs":18000}],"per_model":[{"label":"GPT-5.3-Codex-Max","used_fraction":null,"resets_in_secs":null,"duration_secs":null}],"reset_credits":{"available":1,"applicable_now":null},"source":"usage_endpoint"}
```

Reading a watch stream line by line, with `jq`:

```sh
quotapane-cli --watch 300 --json | while read -r line; do
  echo "$line" | jq -r '.windows[] | "\(.label) \(.used_fraction // "unknown")"'
done
```

## Exit codes

Parsing the JSON is only half the contract; the exit code is the other
half, and it is what a gate should branch on.

| Code | Meaning |
|---|---|
| `0` | success; with `--fail-at`, all windows under the threshold |
| `1` | a provider or credential error |
| `2` | usage error |
| `3` | `--fail-at` tripped: a window reached the threshold |

## What is never in this output

No credential material, in any key, ever — the types that carry a
snapshot cannot hold a secret by construction, and that is enforced in
review as a security invariant (`SECURITY.md`). If you are pasting
`--json` output into an issue, it is safe. `--debug-raw` output is
not: it is the provider's raw response, which is why it redacts
account identifiers by default.
