# reports/ — floor end-gate reports, in-tree

Every floor session's end-gate report is written here as
`m<NN>[-release]-endgate.md` and committed as part of the session's
final push — the report travels with the work it describes instead of
through a copy-paste. The top tier reads it from the repository and
verifies it against the tree before acceptance; the terminal summary
remains a courtesy copy.

Rules:
- The in-tree report is the report of record. If it and the terminal
  paste ever disagree, the in-tree bytes govern.
- Reports state facts (SHAs, counts, verbatim grep results, script
  output) — no token material, ever (§4.4).
- `dispatch/` holds local dispatcher logs and is gitignored.
