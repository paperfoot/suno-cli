<div align="center">

# suno

**Write, generate, resume, and download Suno music from your terminal — v6 support**

<br />

[![Star this repo](https://img.shields.io/github/stars/paperfoot/suno-cli?style=for-the-badge&logo=github&label=%E2%AD%90%20Star%20this%20repo&color=yellow)](https://github.com/paperfoot/suno-cli/stargazers)
&nbsp;&nbsp;
[![Follow @longevityboris](https://img.shields.io/badge/Follow_%40longevityboris-000000?style=for-the-badge&logo=x&logoColor=white)](https://x.com/longevityboris)

<br />

[![License: MIT](https://img.shields.io/badge/License-MIT-blue?style=for-the-badge)](LICENSE)
&nbsp;
[![Rust](https://img.shields.io/badge/Rust-2024-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
&nbsp;
[![crates.io](https://img.shields.io/crates/v/suno?style=for-the-badge)](https://crates.io/crates/suno)
&nbsp;
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?style=for-the-badge)](https://github.com/paperfoot/suno-cli/pulls)

---

A single Rust binary that talks directly to Suno's web API. It supports the current v6 family, custom lyrics, style controls, voice personas, covers, remasters, resumable jobs, and signed downloads. Authentication can come from an existing browser session, stdin, or a stored session.

[Install](#install) | [Quick Start](#quick-start) | [Commands](#commands) | [Features](#features) | [Contributing](#contributing)

</div>

## Why

Suno has no official API. The web UI works, but you can't script it, pipe lyrics from a file, batch-generate, or integrate it into a music production workflow.

This CLI fixes that. Auto-auth from your browser, every generation parameter exposed as a flag, dual JSON/table output for both humans and AI agents. Downloads auto-embed synced lyrics into MP3 files.

## Give the agent explicit musical direction

```bash
suno guide prompting
suno prompt --preset comic-folk --title "The Missing Pie" --lyrics-file song.txt
suno agent-info --command generate
```

`prompt` is offline and free. It assembles BPM, beat unit, meter, groove, voice, delivery, instruments, arrangement, production, and exclusions. Missing choices stay visible. It returns a preview argv and a generation argv with a stable request UUID. Presets are editable examples; they do not impose slider settings. Audition one generated pair before committing to a larger batch.

The built-in guides link to current Suno sources and distinguish documented controls from practical suggestions. A request for 88 BPM or a low adult voice still needs verification in the rendered audio.

## Install

### Homebrew (macOS/Linux)

```bash
brew tap paperfoot/tap
brew install suno
```

### Cargo (any platform)

```bash
cargo install suno
```

### Pre-built binaries

Download from [GitHub Releases](https://github.com/paperfoot/suno-cli/releases) — binaries for macOS (Apple Silicon + Intel), Linux (x86_64 + ARM), and Windows.

### Updating

`suno update` is distribution-aware: it detects how the binary was installed and never overwrites a package-manager-owned install.

```bash
suno update --check    # see what's available (JSON when piped)
suno update            # standalone installs: self-replace from GitHub Releases
                       # brew/cargo installs: prints the right upgrade command instead
```

| Install source | What `suno update` does |
|---|---|
| Homebrew | Never self-replaces — tells you to run `brew upgrade paperfoot/tap/suno` |
| Cargo | Never self-replaces — tells you to run `cargo install --locked --force suno` |
| Standalone binary | Downloads the latest GitHub release over HTTPS and swaps it in |
| Unrecognized | Returns instructions without replacement; latest_version stays null unless --check queried GitHub |

Standalone updates select the exact platform archive, verify its SHA256 against GitHub release metadata or checksums.txt, extract only the expected executable, check its version, then replace the binary. Unknown locations return installation instructions unless explicitly declared standalone with SUNO_INSTALL_SOURCE.

After updating, run `suno skill install` to refresh the agent skill.

## Quick Start

```bash
# 1. Authenticate (auto-extracts from Chrome/Arc/Brave/Firefox/Edge)
suno auth --login

# 2. Verify the setup end to end (auth, JWT freshness, Chrome, API reach, credits)
suno doctor

# 3. Check your credits
suno credits

# 4. Write a song — the composer scaffolds it, you fill the <...> lyric slots
suno write --genre "indie rock" --theme "late-night city drives" --vocal male --out song.txt

# 5. Generate the audio from the file you just filled (write prints this exact command)
suno generate \
  --title "Night Drive" \
  --tags "Indie rock, jangly and nostalgic, 110 BPM, warm male vocals, clean guitars, driving drums" \
  --lyrics-file song.txt \
  --wait --download ./songs/

# 6. Generate with your voice persona
suno generate \
  --title "My Song" \
  --tags "pop, warm" \
  --persona e483d2f0-50ca-4a09-8a74-b9e074646377 \
  --lyrics "[Verse]\nHello from the CLI"

# 7. Or skip the composer and let Suno write the lyrics from a description
suno describe --prompt "a chill lo-fi track about rainy mornings" --wait
```

Suno documents standard v6 generation as 10 credits for two songs. Max Mode and prompts with many image or video inputs cost more. `suno lyrics` does not render audio. Check `suno credits` and `suno models` for the account's current billing and catalogue.

## Write a song

`suno write` is the way to compose. It assembles a Suno-ready song scaffold from a genre grammar compiled into the binary — a Style Prompt line, a meta-tagged `[Verse]`/`[Chorus]` skeleton with inline `<...>` lyric placeholders, and a Suno Tags line — then hands you the exact `suno generate` command to run. The grammar is executable, so you never hand-assemble a style prompt: run one command, fill the `<...>` slots, generate.

```bash
# 1. Scaffold the song (free, no credits) — --out writes the lyric block to a file
suno write --genre "indie rock" --theme "late-night city drives" --vocal male --viral --out song.txt

# 2. Fill the <...> lyric lines in song.txt, then run the command `write` printed
suno generate --title "..." --tags "..." --lyrics-file song.txt --wait --download ./songs/
```

`--out` writes the **lyric block only** — no title, no style prompt, no tag list — so the file feeds `generate --lyrics-file` directly and nothing but lyrics reaches the model. Bare `suno write` at a terminal prints that same lyric block to stdout, so copy-paste is always safe too. The title, Style Prompt and Suno Tags go to stderr (human mode) and into the JSON envelope; `--project-out FILE` additionally saves the full composite document for humans (it must be a different path than `--out`). `suno generate` and `suno extend` refuse lyrics that still contain `<...>` scaffold placeholders — even spans split across lines (exit 3, naming the line numbers) — so an unfilled draft can never burn credits; `--allow-placeholders` sends literal placeholders when explicitly wanted. `--force` bypasses only the duplicate-run guard.

Note that shell redirection (`suno write > song.txt`) receives the JSON envelope, not lyrics: output is a JSON envelope whenever stdout is not a terminal. `--out` is the way to get an editable lyrics file.

Fuzzy genre matching covers ~24 subgenres; an unknown genre is passed through verbatim as a style tag, so `write` never fails on input. Piped or with `--json` you get a `{title, mode, genre, style_prompt, structure, suno_tags, structure_tags, bpm, vocal, theme, viral, instrumental, placeholders_remaining, ready_to_generate, missing_requirements, next_action, written}` envelope. `next_action.argv` is the authoritative handoff — run it as argv, never shell-parse `next_action.command`. It is `null` until `--out` names a real file, and the emitted command omits `--model` so your configured default applies (v6 out of the box).

### Priming / research songs

`--mode priming` swaps in a chill-lounge, low-arousal scaffold (72 BPM) and appends a Prime-Stack Map table plus a research-artefact block:

```bash
suno write --mode priming \
  --target "anonymised batch (n=40)" \
  --objective "increase recall of brand X" \
  --domain marketing --subtlety stealth --out song.txt
```

Priming is consent-based, so `--target`, `--objective` and `--domain` are required: an incomplete request exits 3 with the missing flags named, rather than emitting a scaffold and a ready-to-run command. The objective also seeds the song theme. The Prime-Stack Map and research artefact stay out of the lyrics file — they live in the JSON envelope and `--project-out`.

The deep references live in the built-in guides: `suno guide songwriting` for the full grammar, `suno guide priming` for the consent frame, evidence-graded prime library, and quality gates.

| Flag | What it does | Values |
|---|---|---|
| `--theme` | What the song is about | free text |
| `--genre` | Genre/subgenre | fuzzy match; unknown → verbatim style tag |
| `--mood` | Mood override | e.g. `"bittersweet and hopeful"` (else genre default) |
| `--vocal` | Vocal gender direction | male, female |
| `--bpm` | Tempo | number (else the genre's default) |
| `--viral` | Add earworm/hook meta-tags | flag |
| `--instrumental` | No vocals, no lyric placeholders; adds `--instrumental` to the emitted command | flag |
| `--title` | Song title | free text (else derived from theme) |
| `--mode` | Composition mode | songwriting (default), priming |
| `--target` / `--objective` / `--domain` | Priming research fields | required with `--mode priming` |
| `--subtlety` | Priming subtlety dial | stealth, medium (default), overt |
| `--out` | Write the lyric block to a file (the generation input) | path |
| `--project-out` | Write the composite human document to a file | path |
| `--download` | Download dir baked into the emitted generate command | path (default `./`) |

## Commands

### Create

```
suno prompt          Build an explicit musical brief and generation argv offline (free)
suno write           Compose a Suno-ready song scaffold from the built-in grammar (free)
suno generate        Custom mode — lyrics + tags + title + sliders + voice persona
suno describe        Description mode — Suno writes lyrics from your prompt
suno lyrics          Generate lyrics only (free, no credits)
suno extend          Continue a clip from a timestamp
suno concat          Stitch clips into a full song
suno cover           Create a cover with different style/model
suno remaster        Remaster with a different model version
suno stems           Extract vocals and instruments
```

### Browse & Inspect

```
suno list            List your songs (--cursor for the next page)
suno search <query>  Search songs by title or tags
suno info <id>       Detailed view of a single clip; without ID, discovery manifest
suno persona <id>    View a voice persona
suno status <ids>    Check or resume existing generation IDs (`--wait`, `--download`)
suno jobs            List durable generation receipts for recovery
suno credits         Show balance and plan info
suno models          List available models with limits
```

### Manage

```
suno download <ids>  Download MP3/WAV/M4A/MP4 through signed preparation APIs
suno delete <ids>    Move clips to trash (--confirm or -y; --restore undoes it)
suno set <id>        Update title, lyrics, caption, or remove cover
suno publish <ids>   Toggle public/private visibility
suno timed-lyrics    Get word-level timestamped lyrics (--lrc for LRC format)
```

### Config, Auth & Tooling

```
suno auth            Set up authentication (--login | --refresh | --cookie-stdin | --jwt-stdin | --logout)
suno config          show | set | path | check
suno doctor          Health checks: auth, JWT, Chrome, API reach, credits, captcha state
suno agent-info      Machine-readable capabilities; --command scopes to a command or group
suno guide           List built-in songwriting guides, or print one (guides <name>)
suno skill           install | status — agent skill for Claude Code / Codex / Gemini
suno update          Distribution-aware update (--check to peek first)
```

## Guides

The CLI ships its songwriting knowledge as built-in guides — a single source of truth compiled into the binary, so what agents read never drifts from the tool.

```bash
suno guide                  # list every guide (name, aliases, description)
suno guide prompting        # BPM, casting, performance, exclusions, and examples
suno guide songwriting      # raw Markdown at a terminal, JSON when piped
suno guide priming          # `prime` aliases priming; `grammar` aliases songwriting
```

| Guide | What it covers |
| --- | --- |
| `prompting` | Current sources, tempo and meter, casting, delivery, instrumentation, exclusions, sliders, and audition workflow |
| `songwriting` | Lyrics, structure, prosody, vocal vocabulary, and an executable render workflow |
| `priming` | Research/priming songs: evidence-graded psychological priming woven into lyrics, consent-first |

Write, then generate — the guide's output maps straight onto the flags:

```bash
suno guide songwriting | jq -r .data.content > song.md  # export Markdown, then draft song.txt
suno generate --title "Weekend Code" \
  --tags "indie rock, upbeat, male vocals" \
  --lyrics-file song.txt --wait --download ./songs/
```

Piped or `--json`, `suno guide <name>` returns a `{name, content}` envelope; the bare `suno guide` list returns an array of `{name, aliases, description}`.

## Features

### Authentication

```bash
suno auth --login    # Extracts session from your browser automatically
```

Reads the Clerk auth cookie from Chrome, Arc, Brave, Firefox, or Edge. Exchanges it for a JWT via Clerk token exchange, stores the refreshable session in a `0600` local auth file, and refreshes stale JWTs automatically when the underlying browser session is still valid.

Auth methods (in order of convenience):
1. `suno auth --login` — automatic browser extraction (recommended)
2. `printf '%s' "$COOKIE" | suno auth --cookie-stdin` — secret-safe input for a raw `__client` value or full Cookie header
3. `printf '%s' "$JWT" | suno auth --jwt-stdin` — direct short-lived JWT without putting it in process arguments
4. `suno auth --cookie <cookie>` or `suno auth --jwt <token>` — compatible argument forms
5. `suno auth --refresh` — force a fresh JWT from the stored Clerk session

If browser extraction does not work, `scripts/suno-auth-cookie.sh` prompts for the
`__client` cookie without echoing it or placing it in shell history or process
arguments. It passes the value to `suno --no-browser auth --cookie-stdin`.
Install it as a command available from any directory with:

```bash
mkdir -p "$HOME/.local/bin"
install -m 0755 scripts/suno-auth-cookie.sh "$HOME/.local/bin/suno-auth-cookie"
# Ensure ~/.local/bin is on PATH, then run:
suno-auth-cookie --check
suno-auth-cookie
```

The helper uses `suno` from `PATH` by default. Set `SUNO_BIN=/path/to/suno`
when you need to select a different build. The cookie is exchanged for a stored
session; `suno --no-browser credits` then verifies access without creating music.

`suno auth` with no flags checks the existing session, or starts browser login if no auth is configured. `suno auth --logout` removes stored credentials.

### Generation Parameters

| Flag | What it does | Values |
|---|---|---|
| `--title` | Song title | up to 100 UTF-16 units |
| `--tags` | Style direction | up to 1000 UTF-16 units |
| `--exclude` | Styles to avoid | up to 1000 UTF-16 units |
| `--lyrics` / `--lyrics-file` | Custom lyrics with `[Verse]` tags | up to 5000 UTF-16 units |
| `--prompt` (describe) | Free text description | up to 3000 UTF-16 units |
| `--model` | Model version | v6, v6-wild, v6-mini; legacy names remain parseable but are rejected if absent from the live catalogue |
| `--vocal` | Vocal gender | male, female |
| `--persona` | Voice persona ID | UUID from Suno voice creation |
| `--weirdness` | How experimental | 0-100 |
| `--style-influence` | How strictly to follow tags | 0-100 |
| `--audio-influence` | How strongly source audio shapes the output (generate/cover) | 0-100 |
| `--instrumental` | No vocals | flag |
| `--wait` | Block until done | flag |
| `--download <dir>` | Auto-download after generation | directory path |
| `--dry-run` | Validate and print the request without authentication, submission, or credits | generate/describe flag |
| `--max-mode` | Ask Suno to spend more compute and credits | generate/describe flag |
| `--request-id <uuid>` | Stable retry identity | generate/describe UUID |
| `--token` | Pre-solved captcha token (headless servers) | token string |
| `--no-captcha` | Never run the captcha auto-solver | flag |
| `--force` | Bypass the duplicate-run guard | flag |

`--wait` exits non-zero when Suno reports the generation failed (moderation rejections exit 3 — retrying the same prompt fails identically).

### Browser and captcha policy

Before generate/describe/extend/cover/remaster, the CLI asks `/api/c/check` whether the account is captcha-gated. With normal defaults, it may use browser automation only when required. Global `--headless` permits invisible Chrome for the challenge and never falls back to a visible window. Global `--no-browser` is HTTP-only: it never reads or launches a browser and returns `captcha_required` if a challenge needs browser work. `--token` supplies a solved token; `--token-provider 1|2` (alias `--captcha-provider`) selects hCaptcha or Turnstile; command-level `--no-captcha` disables the solver for deliberate API tests or use with `--token`.

Examples:

```bash
suno --headless generate --title "Night Drive" --tags "indie rock" --lyrics-file song.txt
suno --no-browser generate --title "Night Drive" --tags "indie rock" --lyrics-file song.txt
```

### Voice Personas

Generate songs using your own voice. Create a voice in Suno's web UI, then use the persona ID:

```bash
# View persona details
suno persona <persona_id>

# Generate with your voice
suno generate --persona <persona_id> --title "My Song" --tags "pop" --lyrics "[Verse]\nHello world"

# Works with describe mode too
suno describe --persona <persona_id> --prompt "a warm ballad about starlight"
```

### Covers & Remasters

Create covers with different styles or remaster clips with newer models:

```bash
# Cover with different style tags
suno cover <clip_id> --tags "jazz, smooth piano" --model v6 --wait

# Remaster an old clip with the latest model
suno remaster <clip_id> --model v6 --wait --download ./remastered/
```

Both route through Suno's unified web generation endpoint (`/api/generate/v2-web/`).

### Clip Info

```bash
# Full details for any clip
suno info <clip_id>

# JSON for scripting
suno info <clip_id> --json | jq '.data.audio_url'
```

### Edit & Manage

```bash
# Update title and lyrics on an existing clip
suno set <clip_id> --title "New Title" --lyrics-file updated.txt

# Make clips public
suno publish <clip_id_1> <clip_id_2>

# Get timed lyrics in LRC format
suno timed-lyrics <clip_id> --lrc > song.lrc
```

### Resumable generation

Use a UUID with `generate` or `describe` when an agent may retry after a timeout:

```bash
suno generate --request-id 9b2d06c7-3899-4471-a596-1b1df34b11f1 \
  --title "Night Drive" --tags "indie rock" --lyrics-file song.txt
suno jobs
suno status <clip_id_1> <clip_id_2> --wait --download ./songs/
```

The CLI writes a durable receipt before submission. Reusing a request ID with the same payload returns its saved clip IDs; a changed payload or an outcome-unknown receipt is rejected rather than submitted again. Receipts contain the payload hash, state, IDs, and recovery action. They omit credentials, tokens, and lyrics. `--download` implies `--wait`, and completed clips returned by `generate`, `describe`, `cover`, `remaster`, or `status` include `local_path` after download.

### Signed downloads

Downloads use Suno's signed preparation APIs:

```bash
suno download <id1> <id2> --format mp3 --source auto --output ./songs/
suno download <id> --format wav --source studio --output ./exports/
suno download <id> --format m4a --source library --output ./songs/
suno download <id> --format mp4 --source auto --output ./videos/
```

`--source auto` checks the account's `accessible_features` for `studio`. It uses Studio when available; otherwise it calls library authorization once, then prepares the signed URL. Suno's public bundle directly confirms Library MP3/M4A and the Studio preparation route; Library WAV uses Suno's conversion-and-polling flow. `--video` remains as a compatibility shortcut for MP4. MP3 downloads embed lyrics via ID3 tags:

- **USLT** (plain lyrics) — shown in most music players
- **SYLT** (synced word-by-word timestamps) — shown in Apple Music with timing

```bash
suno download <id1> <id2> --output ./songs/
```

Files use slug format: `title-slug-clipid8.mp3` — no overwrites when Suno generates 2 variations.

### Models

| Version | Codename | Default | Notes |
|---|---|---|---|
| **v6** | `chirp-hawk` | Yes | Current flagship; Pro and Premier |
| v6-wild | `chirp-hawk-wild` | | Experimental v6; Pro and Premier |
| v6-mini | `chirp-goose` | | Faster v6 variant; all plans |

Current remaster key: v6 = `chirp-halibut`.

Suno retired pre-v6 models on September 9, 2026. Their CLI flag names remain accepted for compatibility, but the CLI checks `/api/billing/info/` before submission and rejects a model that is absent or unavailable. `suno models` is the account-specific authority.

### Configuration

Config lives in a TOML file (`suno config path` shows where) and every key is overridable via `SUNO_*` env vars. Precedence: **flag > env > config file > default**.

| Key | Env var | Default | What it does |
|---|---|---|---|
| `default_model` | `SUNO_DEFAULT_MODEL` | `v6` | Default `--model` for generate/describe/extend/cover |
| `poll_interval_secs` | `SUNO_POLL_INTERVAL_SECS` | `5` | Initial `--wait` poll backoff (doubles up to 15s) |
| `poll_timeout_secs` | `SUNO_POLL_TIMEOUT_SECS` | `600` | Total `--wait` timeout |
| `output_dir` | `SUNO_OUTPUT_DIR` | `.` | Default directory for `download` |

`SUNO_CONFIG_DIR` and `SUNO_DATA_DIR` relocate the config/auth directory and
the state directory (guard locks, captcha Chrome profile) — useful for
sandboxing or running isolated instances. `suno config path` shows the
resolved location.

```bash
suno config show                        # effective merged config
suno config set default_model v6        # migrate a persisted pre-v6 override
suno config check                       # validate the file
```

### Agent-Friendly

Every command supports `--json` for structured output. When stdout is piped, JSON is auto-detected. Progress and errors go to stderr. Exit codes are semantic:

| Code | Meaning | Agent action |
|---|---|---|
| 0 | Success | Continue |
| 1 | Transient error (network, API, download) | Retry with backoff |
| 2 | Configuration or auth error | Run `suno doctor`; for auth `suno auth --login` |
| 3 | Bad input (arguments, unknown ID, moderation rejection, duplicate run) | Fix before retrying |
| 4 | Rate limited | Wait 30-60s, retry |

> **Breaking change in v0.6.0:** exit codes were remapped to the [agent-cli-framework](https://github.com/paperfoot/agent-cli-framework) contract. Auth errors moved 3 → 2, not-found moved 5 → 3, and code 5 no longer exists. `list --json` data changed from a bare clip array to `{clips, next_cursor, has_more}`, `list --page` was replaced by `--cursor`, and `generate --variation` was removed. Agents pinned to the 0.5.x contract must update their handling.

Error responses include actionable suggestions:

```json
{
  "version": "1",
  "status": "error",
  "error": {
    "code": "auth_expired",
    "message": "JWT expired or rejected by Suno",
    "suggestion": "Run `suno auth --refresh`; if that fails, run `suno auth --login`"
  }
}
```

```bash
# Pipe-friendly: auto-JSON when piped
suno list | jq '.data.clips[0].title'

# Paginate with the opaque cursor
suno list --cursor "$(suno list | jq -r '.data.next_cursor')"

# Agent capabilities discovery
suno agent-info

# Deterministic exit-code probe (hidden, for conformance tests)
suno contract 3; echo $?   # 3
```

The vendored framework conformance probe runs in CI: `./conformance/conformance.sh target/release/suno`.

### Install as a Coding Agent Skill

Teach Claude Code, Codex CLI, and Gemini CLI how to use `suno` with one command:

```bash
suno skill install   # writes SKILL.md to every detected platform:
                     #   ~/.claude/skills/suno/  ~/.codex/skills/suno/  ~/.gemini/skills/suno/
suno skill status    # which platforms have it, and whether it's current
```

Install is idempotent (`already_current` when nothing changed). The 0.5.x spelling `suno install-skill` still works as a hidden alias. After a CLI update, re-run `suno skill install` so agents see the new surface.

### API surfaces and evidence

| Surface | Route | Evidence state |
|---|---|---|
| Account catalogue | `GET /api/billing/info/` | Live verified for the v6 keys and limits documented above |
| Feed | `POST /api/feed/v3` | Previously live verified and covered by contract tests |
| Generate | `POST /api/generate/v2-web/` | Live verified for v6 custom and description generation, completion, download, and saved request replay |
| Downloads | `/api/studio/clip/{id}/download`, `/api/download/authorize`, `/api/download/clip/{id}` | Studio MP3/WAV/M4A and Library MP3/WAV/MP4 live verified and decoded |
| Aligned lyrics | `GET /api/gen/{id}/aligned_lyrics/v2/` | Previously live verified |

See [API_INTELLIGENCE.md](API_INTELLIGENCE.md) for exact evidence links and the distinction between live observations, public bundle evidence, and inferred request shapes.

## Known limitations

- **Update trust.** SHA256 verification relies on the GitHub release channel. Independent release signing and attestations are not implemented.

## Contributing

1. Fork the repo
2. Create a branch (`git checkout -b feature/your-idea`)
3. Make your changes and test with `cargo test`
4. Open a PR

We especially welcome:
- Audio upload implementation (S3 presigned flow documented in `API_INTELLIGENCE.md`)
- Voice persona creation workflow (endpoints captured, request bodies needed)
- OS keychain/Secret Service/CredMan storage for auth secrets

## License

MIT — see [LICENSE](LICENSE).

---

<div align="center">

Built by [Boris Djordjevic](https://github.com/longevityboris) at [199 Biotechnologies](https://github.com/199-biotechnologies)

<br />

**If this saves you time:**

[![Star this repo](https://img.shields.io/github/stars/paperfoot/suno-cli?style=for-the-badge&logo=github&label=%E2%AD%90%20Star%20this%20repo&color=yellow)](https://github.com/paperfoot/suno-cli/stargazers)
&nbsp;&nbsp;
[![Follow @longevityboris](https://img.shields.io/badge/Follow_%40longevityboris-000000?style=for-the-badge&logo=x&logoColor=white)](https://x.com/longevityboris)

</div>
