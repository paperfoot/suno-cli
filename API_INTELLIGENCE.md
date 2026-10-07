# Suno API Intelligence — refreshed September 28, 2026

Suno does not publish an API contract for the web application. This document separates direct observations from bundle evidence and inference so a route's existence is never confused with a successful end-to-end operation.

## Evidence labels

- **Live verified** — observed against the authenticated account on the stated date.
- **Public bundle** — present in Suno's publicly served web JavaScript. This supports route and response-shape compatibility but does not prove the current account can complete the operation.
- **Independent implementation** — corroborated by a pinned public repository source.
- **Historical capture** — observed previously; retain as a lead, then recapture before changing code.
- **Inferred** — request details remain incomplete or unexecuted.

Live validation on 2026-09-28 completed v6 custom and description generations, each producing two playable MP3s. The custom pair used 10 credits. Studio MP3, WAV, and M4A downloads and Library MP3, WAV, and MP4 downloads completed through HTTP; FFmpeg decoded every downloaded format. Reusing the saved request ID returned the original clip IDs without another submission. Strict headless challenges expired on this account; the existing offscreen hCaptcha fallback completed generation. Captcha requirements remain account-dependent.

## Primary sources

- Suno, [Current Models: v6](https://help.suno.com/en/articles/13924737) — current family and plan access.
- Suno, [v6 FAQ](https://help.suno.com/en/articles/13924481) — pre-v6 retirement, standard generation cost, and Max Mode.
- Suno public bundle, [`0lx_ptoxoz531.js`](https://suno.com/_next/static/immutable/chunks/0lx_ptoxoz531.js) — signed library/Studio preparation routes and preparation states, retrieved 2026-09-28.
- live-smith, pinned [`suno-download.ts`](https://github.com/SamKuler/live-smith/blob/2e5e65daeedc481da241295b465cc531df25f8b2/src/audio-services/suno-download.ts) and [`suno-http.ts`](https://github.com/SamKuler/live-smith/blob/2e5e65daeedc481da241295b465cc531df25f8b2/src/audio-services/suno-http.ts) — independent download authorization and preparation implementation.
- suno-toolkits, pinned [September 2026 download investigation](https://github.com/romanticamaj/suno-toolkits/blob/469a315532f4b107a94cde11c1e22d3d47f95263/docs/2026-09-suno-download-limits.md) and [`wav-source.mjs`](https://github.com/romanticamaj/suno-toolkits/blob/469a315532f4b107a94cde11c1e22d3d47f95263/extension/src/app/wav-source.mjs) — independent evidence for the post-change download flow.

### Captcha source snapshot

- [Provider selection and hCaptcha fallback](https://suno.com/_next/static/immutable/chunks/410gkexdr73e7.js)
- [Turnstile widget integration](https://suno.com/_next/static/immutable/chunks/3sk525-8t_hsm.js)
- [Public site keys](https://suno.com/_next/static/immutable/chunks/1fnqfl2zkfr3f.js)

## Auth and request headers

- **Base URL:** `https://studio-api-prod.suno.com`
- **Auth:** Clerk browser cookies can be exchanged for a JWT; API requests use `Authorization: Bearer <jwt>`.
- **Observed headers:**
  - `authorization: Bearer <jwt>`
  - `device-id: <uuid>`
  - `browser-token: {"token":"<base64 timestamp payload>"}`
  - `origin: https://suno.com`
  - `referer: https://suno.com/`
- JWTs are short-lived. A stored Clerk session supports refresh.
- For automation, the CLI accepts the Clerk cookie or JWT on stdin so the secret is not exposed in process arguments.

## Account catalogue — live verified 2026-09-28

`GET /api/billing/info/` returns plan information, credits, feature access, generation models, remaster models, and per-model limits. Do not commit account-specific balances because they drift.

Live verified on 2026-10-07: an authenticated account can omit `plan` while returning `total_credits_left`, `models`, and `accessible_features`. The CLI treats `plan` as optional display metadata. Authentication still requires a successful billing response with `total_credits_left`; an absent plan is reported as JSON `null` and omitted from text output.

JWT expiry follows the token's `exp` claim. A token with less than 30 minutes remaining can still pass the authenticated billing request. Server-side rejection remains authoritative and triggers the existing Clerk refresh path when refresh credentials are available.

### Current generation models

| Display name | External key | Plan access |
|---|---|---|
| v6 | `chirp-hawk` | Pro and Premier |
| v6-wild | `chirp-hawk-wild` | Pro and Premier |
| v6-mini | `chirp-goose` | All users |

Suno's official FAQ says every pre-v6 generation model was retired on September 9, 2026. The CLI retains old flag values so existing scripts still parse, then rejects a missing or unavailable catalogue entry before submission.

### Current remaster model

| Display name | External key |
|---|---|
| v6 | `chirp-halibut` |

### Current input limits

The web forms and CLI count UTF-16 code units, so an emoji can count as two units.

| Field | Limit |
|---|---:|
| Title | 100 |
| Custom prompt/lyrics | 5000 |
| Style tags | 1000 |
| Excluded styles | 1000 |
| Simple description | 3000 |

Suno documents standard v6 generation as 10 credits for two songs. Max Mode costs more. The authenticated account response is authoritative for the actual charge and access.

## Generation

### Catalogue validation

Before a paid request, the CLI fetches `/api/billing/info/` and verifies:

1. the external model key belongs to the relevant live catalogue;
2. the account can use the generation model;
3. every text field fits the returned limits.

This makes retained pre-v6 flags fail before submission rather than sending a retired key.

### `POST /api/generate/v2-web/`

**Evidence:** live v6 custom generation, completed downloads, and audio decode verification on 2026-09-28.

Representative custom request:

```json
{
  "token": null,
  "generation_type": "TEXT",
  "title": "Night Drive",
  "tags": "indie rock, warm male vocals",
  "negative_tags": "",
  "mv": "chirp-hawk",
  "prompt": "[Verse]\n...",
  "make_instrumental": false,
  "user_uploaded_images_b64": null,
  "metadata": {
    "web_client_pathname": "/create",
    "is_max_mode": false,
    "is_mumble": false,
    "create_mode": "custom",
    "user_tier": "",
    "create_session_token": "<uuid>",
    "disable_volume_normalization": false
  },
  "override_fields": [],
  "cover_clip_id": null,
  "cover_start_s": null,
  "cover_end_s": null,
  "persona_id": null,
  "artist_clip_id": null,
  "artist_start_s": null,
  "artist_end_s": null,
  "continue_clip_id": null,
  "continued_aligned_prompt": null,
  "continue_at": null,
  "transaction_uuid": "<request UUID>"
}
```

Missing titles and tags serialize as empty strings: a live v6 description submission rejected a null title with HTTP 422.

Simple/description mode sets `metadata.create_mode` to `inspiration` and puts the description in `prompt`. `--max-mode` sets `metadata.is_max_mode` to `true`.

The current client sends `token` with integer `token_provider`: 1 for hCaptcha, 2 for Turnstile. A string provider name is invalid. Omit the provider when no token is supplied.

### Captcha preflight: `POST /api/c/check`

Request: `{"ctype":"generation"}` with Bearer authentication. Live response on 2026-09-28: `{"required":true,"captcha_version":2}`. Current public client code maps 1 to hCaptcha, 2 to Cloudflare Turnstile, and latches an hCaptcha fallback after Turnstile failure.

- Default operation may use browser automation only when the preflight requires it, including the existing offscreen fallback.
- Global `--headless` allows invisible Chrome and forbids a visible fallback.
- Global `--no-browser` performs HTTP only and returns `captcha_required` if solving needs a browser.
- `--token` accepts an externally solved token; `--token-provider 1|2` selects its provider. Command-level `--no-captcha` disables the built-in solver.

Strict headless hCaptcha expired and strict headless Turnstile timed out before submission. Offscreen hCaptcha succeeded. CDP calls now use an absolute deadline so background events cannot extend a stuck solve indefinitely.

## Paid-request recovery and idempotency

The CLI reserves a `0600` receipt before sending the paid POST. A receipt contains:

```json
{
  "transaction_id": "<uuid>",
  "request_sha256": "<hash>",
  "updated_at": "<timestamp>",
  "state": "submitting | submitted | submission_unknown | rejected",
  "ids": ["<clip id>"],
  "next_action": {"argv": ["suno", "status", "<clip id>", "--wait"]}
}
```

Receipts omit credentials, captcha tokens, and lyric text. `--request-id UUID` supplies the transaction identity. The same ID plus the same submitted payload reuses saved clip IDs. A changed payload or `submission_unknown` receipt is rejected rather than replaying a possibly accepted paid request.

Recovery sequence:

1. `suno jobs`
2. if the receipt has IDs, `suno status <ids> --wait --download DIR`
3. if its outcome is unknown and it has no IDs, inspect `suno list` before any manual retry

`status` never submits generation. `--download` implies `--wait`, and downloaded clip objects include `local_path`.

## Signed downloads

Playback URLs are not treated as download contracts. The current flow prepares a short-lived signed URL, polls preparation states, and transfers bytes atomically through a `.part` file.

### Source selection

`--source auto` reads `accessible_features` from `/api/billing/info/`:

- feature `studio` present → Studio preparation;
- otherwise → Library authorization and preparation.

MP4 uses the Library route. `--source studio` and `--source library` override auto selection.

### Studio preparation

```text
GET /api/studio/clip/{clip_id}/download?format={format}
```

**Evidence:** Suno public bundle, independent implementations, and live Studio MP3/WAV downloads on 2026-09-28. The response progresses through `processing` to `ready` with a `download_url`; `rate_limited` is also handled. The CLI polls for at most three minutes.

### Library authorization and preparation

Authorize once when the clip is not already unlocked:

```text
POST /api/download/authorize
{"item_id":"<clip_id>","item_type":"clip"}
```

Then prepare:

```text
GET /api/download/clip/{clip_id}?format={format}
```

**Evidence:** The Suno bundle directly supports MP3 and M4A on the Library path. Its WAV flow still references `convert_wav` followed by `wav_file`; independent repositories corroborate the authorization/preparation family. The CLI follows that separate WAV conversion path for Library downloads; it is covered by a local server test. Studio MP3/WAV/M4A and Library MP3/WAV/MP4 were live verified and decoded on September 28. Library M4A is supported by source evidence but has not been separately live tested.

If byte transfer fails because a prepared URL expired, the CLI prepares a fresh URL once without repeating Library authorization.

### CLI formats

The CLI surface accepts `mp3`, `wav`, `m4a`, and `mp4`; `--video` remains a compatibility shortcut for MP4. MP3 downloads also receive plain USLT and timed SYLT lyric tags. Successful multi-file JSON is `{downloaded, failed}`. Any failed item causes a nonzero exit with completed paths, failed IDs, and a retry argv in `error.details`.

## Other endpoints

| Route | Evidence | Purpose |
|---|---|---|
| `POST /api/feed/v3` | Previously live verified | Opaque-cursor library feed |
| `POST /api/generate/lyrics/` | Previously live verified | Start lyrics-only generation |
| `GET /api/generate/lyrics/{id}` | Previously live verified | Poll lyrics-only result |
| `GET /api/gen/{id}/aligned_lyrics/v2/` | Previously live verified | Word-level timed lyrics |
| `POST /api/generate/concat/v2/` | Previously live verified | Concatenate a clip |
| `POST /api/edit/stems/{clip_id}` | Independent implementation | Stem separation |
| `POST /api/cover/` | Historical/inferred | Older cover route; current CLI uses the unified generation shape |
| `POST /api/remaster/` | Historical/inferred | Older remaster route; current CLI uses the unified generation shape |

`POST /api/feed/v3` accepts the opaque `next_cursor` from the preceding response. Page numbers are not part of this route.

## Historical voice/persona capture — April 6, 2026

Keep these as recapture leads. Their request bodies were not fully captured and they are not evidence of the September 2026 UI.

1. Upload finish: `POST /api/uploads/audio/{upload_id}/upload-finish/`
2. Poll upload: `GET /api/uploads/audio/{upload_id}/`
3. Extract vocals: `POST /api/processed_clip/voice-vox-stem`
4. Upload a verification phrase through the same upload flow
5. Verify voice: `POST /api/voice-verification/`
6. Create persona: `POST /api/persona/create/`

Missing evidence:

- the presigned upload request before `upload-finish`;
- exact JSON bodies for vocal extraction and voice verification;
- the exact persona creation body.

Recapture these bodies from the current web application before implementing persona creation.
