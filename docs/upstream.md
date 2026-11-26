# Orbe upstream

| Field | Value |
|---|---|
| Upstream | `godotengine/godot`, branch `4.7` |
| Last observed | `ed1daf0bf001b61586d9930840f2f1394092c079` (tag `4.7.2-stable`) |
| Last evaluated | `ed1daf0bf001b61586d9930840f2f1394092c079` |
| Last integrated | `ed1daf0bf001b61586d9930840f2f1394092c079` (the first mirror commit) on 2026-11-26 |
| Skipped | none |
| Adapted | application id, binary name and desktop entry (`.github/rebrand.toml`); every `Godot Engine` renamed to Orbe; NDL editor themes; Orbe's own logo, icon and splash |
| Conflicting | none |

Orbe shares no history with upstream: it begins from upstream's tree at `4.7.2-stable`, renamed.
Upstream's own commits are not pushed here.

## How upstream arrives renamed

`scripts/rebrand.py` renames upstream's names to Orbe's, in file contents and in paths, from the
rules in `.github/rebrand.toml`: literal replacements in order, spans to keep (links, upstream's
repository, copyright holders, the export templates' package names), files to skip (`thirdparty/`,
the licence notices) and lines that keep Orbe's own value (`website`, `docs` in `version.py`). A
line marked `rebrand: keep` is left alone, for compatibility code that has to name the old product.

Each weekly sync (`.github/workflows/upstream-sync.yml`) records upstream's new head, renamed, as
one mirror commit: its only parent is the previous mirror commit, its subject is
`Upstream <sha> with this app's names`, and an `Upstream-Commit:` trailer names the upstream commit
it renames. The sync merges that commit into a branch and opens a pull request, so upstream's
changes arrive already speaking Orbe, and a conflict is a real one. Files in
`.github/upstream-keep` keep Orbe's version. No mirror branch is pushed, so merge sync pull
requests with a merge commit: a squash would drop the mirror commit the next sync starts from.

The first mirror commit is upstream `ed1daf0b` renamed.

The sync follows upstream's `4.7` branch (fixes only). Moving to the next release is a deliberate
change of `UPSTREAM_BRANCH` in the workflow, with the migration notes read first.

`python3 scripts/rebrand.py check` lists anything still using an upstream name.
