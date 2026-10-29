# Changelog

## Unreleased

- Accept both current `subscription_type` / `plans` billing responses and legacy nested `plan` responses, restoring credits, model discovery, authentication checks, and generation validation when `plan` is absent.

## v0.10.1 — v6 catalogue, resumable jobs, and signed downloads

- Added Turnstile provider detection, numeric `--token-provider`, and the website's hCaptcha fallback. Captcha calls have absolute deadlines; `--quiet` keeps failures machine-readable.
- Release builds now run checks before packaging; crates.io publication failures are surfaced.

**Models and request validation:**

- The out-of-box default is now v6 (`chirp-hawk`), with v6-wild (`chirp-hawk-wild`) and v6-mini (`chirp-goose`) available. The v6 remaster key is `chirp-halibut`.
- Pre-v6 flag values remain parseable for compatibility. Suno retired those models on September 9, 2026, so the CLI now checks the account catalogue and rejects absent or unavailable models before submission.
- Limits are validated as UTF-16 units to match the web forms: title 100, custom prompt/lyrics 5000, tags 1000, excluded styles 1000, and Simple description 3000.
- `generate` and `describe` gained `--dry-run`, `--max-mode`, and a UUID `--request-id`. Dry runs are offline previews and explicitly report that live model validation and submission have not occurred.
- Suno documents standard v6 generation as 10 credits for two songs. Max Mode uses more credits; the account response remains authoritative.

**Recovery and headless operation:**

- Generation receipts are reserved before submission and saved under the data directory. `suno jobs` lists their state, IDs, payload hash, and recovery action without storing lyrics, credentials, or tokens.
- Reusing `--request-id` with an identical submitted payload returns the saved clip IDs. Payload mismatches and outcome-unknown submissions fail closed instead of issuing another paid request.
- `suno status <ids> --wait --download DIR` resumes existing work and never submits a generation. `--download` implies `--wait`; downloaded clip objects include `local_path`.
- Global `--headless` allows invisible Chrome for a required captcha with no headed fallback. Global `--no-browser` is HTTP-only and returns `captcha_required` when a browser challenge is required.
- Each captcha process has an isolated temporary Chrome profile, preventing cross-command profile locks. JSON errors suppress progress chatter.
- `suno auth --cookie-stdin` and `suno auth --jwt-stdin` keep credentials out of process arguments.

**Downloads:**

- Downloads use Suno's signed preparation flow instead of stale playback URLs. `--format` supports MP3, WAV, M4A, and MP4; `--source` supports `auto`, `studio`, and `library`; `--video` remains a compatibility shortcut.
- Auto source selection uses Studio when the account advertises the `studio` feature. Otherwise it authorizes the library item once and prepares a short-lived signed URL. Expired preparation URLs are refreshed without repeating authorization.
- Downloads validate their file container before the atomic rename; HTML error pages cannot become completed audio files.
- MP3 lyric embedding remains available. A failed multi-download exits nonzero with completed paths, failed IDs, and a retry argv in `error.details`; successful downloads keep `{downloaded, failed}`.

**Agent workflows and framework:**

- New offline `suno prompt` makes tempo, beat unit, meter, groove, voice, delivery, instruments, arrangement, and exclusions explicit. Original editable examples include comic folk, work song, solo lament, and electronic music. Returned preview/generation argv uses a stable request UUID.
- Prompting and songwriting guides cite current official sources, remove obsolete model/cost guidance and unsupported rigid rules, and recommend auditioning a pair before batching.
- Updated to agent-cli-framework cdb6add: Clap-derived syntax, scoped discovery through `agent-info --command` (or `info --command`), compact fallible JSON, single failure envelope with diagnostic details, kernel file locks, and verified standalone updates.
- `--force` now bypasses only duplicate locking. `--allow-placeholders` explicitly sends literal scaffold markers. `delete --confirm` complements the existing `--yes`.

**Migration:**

- Existing installs with an old persisted override should run `suno config set default_model v6`.
- Refresh installed agent instructions with `suno skill install` after updating the binary.

The v6 catalogue and limits were live verified. The current generate and download request shapes are supported by web-bundle and independent implementation evidence plus local tests; live v6 generation, MP3/WAV/M4A/MP4 decoding, Studio and Library downloads, Library MP3 authorization, and request-ID replay passed. Strict headless captcha challenges required the existing offscreen fallback on the tested account.

## v0.8.0 — the composer and the renderer agree about the artifact

One invariant now holds end to end: the file named by the emitted generate command exists, is directly consumable by `--lyrics-file`, contains no unresolved instructions, and reflects every selected control.

**Breaking** (agents reading `write --json`):

- `write --out FILE` writes the **lyric block only**. It previously wrote a composite document (title, Style Prompt, `---` rules, Suno Tags, and in priming mode the Prime-Stack Map and research artefact) that the emitted workflow then handed to `--lyrics-file` — so headers, tags and research metadata were sent to Suno as lyrics and embedded in the MP3. The composite document moved to the new `--project-out FILE`.
- The `generate` string field is replaced by `next_action: {argv, command}`. `argv` is authoritative; `command` is shell-escaped display text. It is `null` when no `--out` file exists, instead of advertising a hardcoded `song.txt` that was never written.
- `write --mode priming` requires `--target`, `--objective` and `--domain` (exit 3 when missing) — priming is consent-based and every run must be auditable. `--domain` and `--subtlety` values are validated.
- `guide` lost the `write` alias (it competed with the `suno write` command).

**Fixes:**

- `--mood` / `--vocal` / `--bpm` / `--instrumental` now drive the Style Prompt, the `[Mood:]`/`[Energy:]`/vocal meta-tags and `suno_tags` from one resolved-controls struct. `--mood "dark and brooding"` no longer emitted `[Mood: Uplifting]` and an "uplifting" tag alongside it.
- `--instrumental` is coherent: no `<...>` fill instructions, `--instrumental` in the emitted command, no vocal-only tags.
- Titles and paths in the emitted command are shell-escaped (`She Said "Go"` produced invalid shell).
- `generate` refuses lyrics containing unresolved `<...>` scaffold placeholders (exit 3, naming the line numbers) so an unfilled draft cannot spend generation credits. `--force` overrides.
- The emitted command stopped pinning `--model v4.5-all` and instead used the configured default. At the time of this v0.8.0 release, that default was v5.5; v0.10.1 later moved it to v6.
- New fields: `placeholders_remaining`, `ready_to_generate`, `missing_requirements`, `project_written`.

**Discovery:**

- `write` leads the command list in `--help`; the root example is the full write → fill → generate → download flow, with a one-liner distinguishing write/generate/describe/lyrics. README Quick Start mirrors it.
- `write --help` no longer claims plain text on stdout while the framework sends JSON when piped: shell redirection gets the envelope, `--out` gets the lyrics file.
- `agent-info` gained the `write` output schema, workflow, and mode-specific required fields.

## v0.6.0 — framework conformance, captcha preflight, real config

**Breaking** (agents pinned to the 0.5.x contract must update):

- Exit codes remapped to the [agent-cli-framework](https://github.com/paperfoot/agent-cli-framework) contract: 0 success, 1 transient, 2 config/auth (auth was 3), 3 bad input incl. not-found (was 5), 4 rate limited. Code 5 removed.
- `list --json` data is now `{clips, next_cursor, has_more}` (was a bare clip array); `list --page` replaced by `--cursor <token>` (the old page numbers never worked against feed/v3's opaque cursors).
- `generate --variation` removed — it was parsed and silently ignored.
- `download --json` data is now `{downloaded, failed}` with `partial_success` status when some clips fail (was a bare path array).
- `delete`/`auth`/`set`/`publish`/`config` now emit success envelopes in JSON mode.

**Captcha & generation:**

- Captcha preflight: every gated command asks `/api/c/check` first and skips the Chrome solver entirely when the account isn't captcha-gated (most aren't).
- `extend`/`cover`/`remaster` now route through the same captcha pipeline as `generate`/`describe` (they previously posted `token: null` and failed on captcha-enforced accounts), and gained `--token`/`--no-captcha`.
- Fixed the bare `__client` cookie being dropped on the solver's cookie replay — the root cause of "hcaptcha never finished loading" on sub-threshold accounts.
- `--wait` now exits non-zero when generation fails (moderation rejections exit 3); previously failed clips exited 0.
- New model `v4.5-all` (chirp-auk-turbo, Suno's "best free model"); `extend` gained `--model`; new `--audio-influence` slider on generate/cover.
- Documented the then-observed v5.5 credit cost. Suno's current v6 documentation supersedes that historical value.

**Tooling:**

- `suno doctor` — auth/JWT/Chrome/API/credits/captcha health checks.
- `suno skill install|status` — agent skill for Claude Code, Codex CLI, Gemini CLI (replaces `install-skill`, which remains a hidden alias).
- `suno update` is distribution-aware: brew- and cargo-owned binaries are never self-replaced; the owner channel's upgrade command is returned instead.
- Config layer is now real: TOML file + `SUNO_*` env vars actually drive polling, default model, and download dir (`config show|set|path|check`).
- Duplicate-run guard on generate/describe/cover/remaster/update (`--force` bypasses).
- Vendored framework conformance probe + schemas under `conformance/`, run in CI; integration test suite under `tests/`.
- Fixed a UTF-8 panic when tables truncated multi-byte (CJK/emoji) prompts.

## v0.5.x

- v0.5.7 — fix captcha desktop viewport
- v0.5.6 — fix captcha cookie replay
- v0.5.5 — auth hardening and release cleanup
- v0.5.4 — auto-solve hCaptcha via piloted Chrome (CDP)
- v0.5.3 — add `suno auth --refresh` + clearer captcha-rollout error
- v0.5.2 — add in-process JWT refresh retry
- v0.5.1 — rename package to `suno`, add `install-skill` command
- v0.5.0 — fix cover/remaster endpoints, add persona + info commands, framework alignment

## Earlier

- v0.4.0 — zero-friction auth: `suno auth --login`
- v0.3.0 — audit fixes, set metadata, publish, timed lyrics, ID3 embedding
- v0.2.0 — search, delete, slug filenames, renamed commands
- v0.1.0 — initial release
