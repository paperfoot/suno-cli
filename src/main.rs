mod api;
mod auth;
mod captcha;
mod cli;
mod commands;
mod config;
mod download;
mod errors;
mod guard;
mod output;

use clap::Parser;

use api::SunoClient;
use api::types::{ControlSliders, GenerateRequest, SetMetadataRequest};
use auth::AuthState;
use cli::*;
use errors::CliError;
use output::OutputFormat;

async fn client() -> Result<SunoClient, CliError> {
    let auth = AuthState::load()?;
    SunoClient::new_with_refresh(auth).await
}

/// Flag > config (`default_model`, itself env-overridable) > compiled default.
fn resolve_model(
    flag: Option<ModelVersion>,
    cfg: &config::AppConfig,
) -> Result<ModelVersion, CliError> {
    match flag {
        Some(m) => Ok(m),
        None => {
            <ModelVersion as clap::ValueEnum>::from_str(&cfg.default_model, true).map_err(|_| {
                CliError::Config(format!(
                    "config default_model '{}' is not a valid --model value — \
                     fix it with `suno config set default_model v5.5`",
                    cfg.default_model
                ))
            })
        }
    }
}

/// Read a caller-supplied input file (e.g. --lyrics-file). A missing or
/// unreadable path is bad input (exit 3), not a retryable io_error (exit 1):
/// retrying the same wrong path fails identically.
fn read_input_file(path: &str) -> Result<String, CliError> {
    std::fs::read_to_string(path)
        .map_err(|e| CliError::InvalidInput(format!("cannot read input file '{path}': {e}")))
}

/// Credit protection shared by every command that sends lyrics into the
/// v2-web `prompt` field (generate, extend): an unfilled `suno write`
/// scaffold would be sung verbatim at real credit cost. Refuse it unless
/// forced.
fn reject_unfilled_scaffold(lyrics: Option<&str>, force: bool) -> Result<(), CliError> {
    let Some(text) = lyrics else { return Ok(()) };
    let lines = commands::write::placeholder_lines(text);
    if lines.is_empty() || force {
        return Ok(());
    }
    let numbers: Vec<String> = lines.iter().map(|n| n.to_string()).collect();
    Err(CliError::InvalidInput(format!(
        "lyrics contain {} unresolved scaffold placeholder(s) at line(s) {} — fill the <...> spans before generating (or pass --force to send them as written)",
        lines.len(),
        numbers.join(", ")
    )))
}

fn build_tags(tags: Option<&str>, vocal: Option<&VocalGender>) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(t) = tags {
        parts.push(t);
    }
    match vocal {
        Some(VocalGender::Male) => parts.push("male vocals"),
        Some(VocalGender::Female) => parts.push("female vocals"),
        None => {}
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// Build a control_sliders block when any slider flag is set.
/// Returns None when none is provided so we don't pollute the request.
fn build_control_sliders(
    weirdness: Option<f64>,
    style_influence: Option<f64>,
    audio_influence: Option<f64>,
) -> Option<ControlSliders> {
    if weirdness.is_none() && style_influence.is_none() && audio_influence.is_none() {
        return None;
    }
    Some(ControlSliders {
        // Normalize 0-100 → 0.0-1.0
        weirdness_constraint: weirdness.map(|w| (w / 100.0).clamp(0.0, 1.0)),
        style_weight: style_influence.map(|s| (s / 100.0).clamp(0.0, 1.0)),
        audio_weight: audio_influence.map(|a| (a / 100.0).clamp(0.0, 1.0)),
    })
}

/// Resolve the captcha token for the five captcha-gated v2-web commands
/// (generate, describe, extend, cover, remaster). Returns the token to attach
/// to the request body, or `None` when no captcha is needed.
///
/// An explicit --token wins (assumed hCaptcha-solved); --no-captcha means
/// never solve. Otherwise preflight `/api/c/check` — most accounts are above
/// Suno's trust threshold and need no captcha, so we skip the Chrome solver
/// entirely. A preflight failure falls back to solving (never a hard gate).
async fn resolve_captcha(
    c: &SunoClient,
    token: Option<String>,
    no_captcha: bool,
    quiet: bool,
) -> Result<Option<String>, CliError> {
    if let Some(t) = token {
        return Ok(Some(t));
    }
    if no_captcha {
        return Ok(None);
    }

    // Test hook: SUNO_FORCE_CAPTCHA=1 skips the preflight and always runs the
    // solver so the browser path is exercisable on accounts that return
    // required=false. Attaching an unrequested token is harmless.
    let forced = std::env::var("SUNO_FORCE_CAPTCHA").is_ok_and(|v| v == "1");
    let check = if forced {
        None
    } else {
        Some(c.captcha_check("generation").await)
    };
    let required = match &check {
        Some(Ok(resp)) => resp.required,
        Some(Err(_)) => true,
        None => true,
    };
    if !required {
        if !quiet {
            eprintln!("Captcha not required — skipping solver");
        }
        return Ok(None);
    }

    if !quiet {
        if forced {
            eprintln!("SUNO_FORCE_CAPTCHA=1 — solving hCaptcha via piloted Chrome...");
        } else {
            match check.and_then(|r| r.ok()).and_then(|r| r.captcha_version) {
                Some(v) => {
                    eprintln!("Captcha required (version {v}) — solving via piloted Chrome...")
                }
                None => eprintln!("Solving hCaptcha via piloted Chrome..."),
            }
        }
    }
    let auth = AuthState::load()?;
    let solved = captcha::solve(&auth).await.map_err(|e| match e {
        // A Config error (no Chrome installed, non-loopback CDP endpoint) is a
        // setup problem the user must fix — surface it verbatim as exit 2, not
        // a retryable GenerationFailed. Only genuine solve failures get the
        // "clear the challenge in the UI" retry guidance.
        CliError::Config(_) => e,
        other => CliError::GenerationFailed(format!(
            "captcha solve failed: {other} — Suno is enforcing a captcha on this account; \
             generate one song in the suno.com UI to clear the challenge, then retry"
        )),
    })?;
    if forced && !quiet {
        eprintln!(
            "Solved token: {} chars, prefix {}",
            solved.len(),
            &solved[..solved.len().min(24)]
        );
    }
    Ok(Some(solved))
}

/// Generate, wait, optionally download with lyrics embedding.
/// Poll timing comes from config (`poll_timeout_secs`, `poll_interval_secs`).
async fn handle_generation(
    c: &SunoClient,
    clips: Vec<api::types::Clip>,
    wait: bool,
    download_dir: Option<&str>,
    fmt: OutputFormat,
    quiet: bool,
    cfg: &config::AppConfig,
) -> Result<(), CliError> {
    let ids: Vec<String> = clips.iter().map(|c| c.id.clone()).collect();

    if wait && !ids.is_empty() {
        if !quiet {
            eprintln!("Waiting for generation to complete...");
        }
        let final_clips = c
            .poll_clips(&ids, cfg.poll_timeout_secs, cfg.poll_interval_secs)
            .await?;

        if let Some(dir) = download_dir {
            for clip in &final_clips {
                if clip.status == "complete" {
                    let path = download::download_clip(clip, dir, false).await?;

                    // Embed lyrics into MP3
                    let plain_lyrics = clip.metadata.prompt.as_deref();
                    // Try to get timed lyrics for synced display
                    let aligned = c.aligned_lyrics(&clip.id).await.ok();
                    download::embed_lyrics_in_mp3(
                        &path,
                        &clip.title,
                        plain_lyrics,
                        aligned.as_deref(),
                    )?;

                    if !quiet {
                        eprintln!("Downloaded: {path} (lyrics embedded)");
                    }
                }
            }
        }

        // A clip that polled to "error" is a failed generation — exit
        // non-zero so agents don't treat moderation rejections and internal
        // failures as success. Completed siblings were already downloaded.
        let failed: Vec<String> = final_clips
            .iter()
            .filter(|c| c.status == "error")
            .map(|c| {
                let reason = c
                    .metadata
                    .error_message
                    .as_deref()
                    .or(c.metadata.error_type.as_deref())
                    .unwrap_or("unknown error");
                format!("{} ({reason})", c.id)
            })
            .collect();
        if !failed.is_empty() {
            return Err(CliError::GenerationFailed(format!(
                "clip(s) errored: {}",
                failed.join("; ")
            )));
        }

        match fmt {
            OutputFormat::Json => output::json::success(&final_clips),
            OutputFormat::Table => output::table::clips(&final_clips),
        }
    } else {
        match fmt {
            OutputFormat::Json => output::json::success(&clips),
            OutputFormat::Table => {
                output::table::clips(&clips);
                if !ids.is_empty() {
                    eprintln!("\nUse `suno status {}` to check progress", ids.join(" "));
                }
            }
        }
    }
    Ok(())
}

async fn run(cli: Cli, fmt: OutputFormat) -> Result<(), CliError> {
    match cli.command {
        Commands::Auth(args) => {
            if args.logout {
                AuthState::delete()?;
                match fmt {
                    OutputFormat::Json => {
                        output::json::success(serde_json::json!({ "authenticated": false }))
                    }
                    OutputFormat::Table => {
                        eprintln!("Logged out; removed stored Suno authentication")
                    }
                }
                return Ok(());
            }

            let mut state = match AuthState::load() {
                Ok(s) => s,
                Err(CliError::AuthMissing) => AuthState::default(),
                Err(e) => return Err(e),
            };

            let has_explicit_auth_input =
                args.login || args.refresh || args.jwt.is_some() || args.cookie.is_some();
            let should_login = args.login
                || (!has_explicit_auth_input
                    && state.jwt.is_none()
                    && state.clerk_client_cookie.is_none());

            if args.refresh {
                // Force-refresh the JWT via the stored Clerk session cookie.
                // Useful when the API rejects the current JWT mid-session
                // before the CLI's own staleness check fires.
                let cookie = state.clerk_client_cookie.clone().ok_or_else(|| {
                    CliError::Config(
                        "no Clerk session cookie stored — run `suno auth --login` first".into(),
                    )
                })?;
                let http = reqwest::Client::new();
                eprintln!("Refreshing JWT via Clerk session cookie...");
                let (session_id, jwt) = if let Some(session_id) = state.session_id.clone() {
                    (
                        session_id.clone(),
                        auth::clerk_refresh_jwt(&http, &cookie, &session_id).await?,
                    )
                } else {
                    auth::clerk_token_exchange(&http, &cookie).await?
                };
                state.session_id = Some(session_id);
                state.jwt = Some(jwt);
                state.save()?;
                eprintln!("JWT refreshed successfully");
            } else if should_login {
                // Automatic: extract cookies from browser
                eprintln!("Extracting Suno session from your browser...");
                let browser_auth = auth::extract_browser_auth()?;

                let http = reqwest::Client::new();
                eprintln!("Exchanging for access token via Clerk...");
                let (session_id, jwt) =
                    auth::clerk_token_exchange(&http, &browser_auth.clerk_client_cookie).await?;

                state.cookie = Some(browser_auth.cookie_header);
                state.clerk_client_cookie = Some(browser_auth.clerk_client_cookie);
                state.session_id = Some(session_id);
                state.jwt = Some(jwt);
                state.device_id = browser_auth
                    .device_id
                    .or(state.device_id)
                    .or_else(|| Some(uuid::Uuid::new_v4().to_string()));
            } else if let Some(cookie) = args.cookie.as_deref() {
                // Manual: user provides a full Cookie header or raw Clerk __client value.
                let browser_auth = auth::normalize_cookie_input(cookie)?;
                let http = reqwest::Client::new();
                eprintln!("Exchanging cookie for access token...");
                let (session_id, jwt) =
                    auth::clerk_token_exchange(&http, &browser_auth.clerk_client_cookie).await?;

                state.cookie = Some(browser_auth.cookie_header);
                state.clerk_client_cookie = Some(browser_auth.clerk_client_cookie);
                state.session_id = Some(session_id);
                state.jwt = Some(jwt);
                state.device_id = browser_auth
                    .device_id
                    .or(state.device_id)
                    .or_else(|| Some(uuid::Uuid::new_v4().to_string()));
            } else if let Some(jwt) = args.jwt.clone() {
                // Legacy: direct JWT paste (expires in ~1 hour)
                state.jwt = Some(jwt);
                if state.device_id.is_none() {
                    state.device_id = Some(uuid::Uuid::new_v4().to_string());
                }
            } else {
                eprintln!("Checking existing authentication...");
            }

            if let Some(device) = args.device.as_ref() {
                state.device_id = Some(device.clone());
            }

            // Verify
            let should_save_after_verify = args.refresh
                || should_login
                || args.cookie.is_some()
                || args.jwt.is_some()
                || args.device.is_some();
            let client = SunoClient::new_with_refresh(state.clone()).await?;
            let info = client.billing_info().await?;
            if should_save_after_verify {
                state.save()?;
            }
            match fmt {
                OutputFormat::Json => output::json::success(serde_json::json!({
                    "authenticated": true,
                    "plan": info.plan_name(),
                    "credits": info.total_credits_left,
                })),
                OutputFormat::Table => eprintln!(
                    "Authenticated! Plan: {}, Credits: {}",
                    info.plan_name(),
                    info.total_credits_left
                ),
            }
        }

        Commands::Credits => {
            let info = client().await?.billing_info().await?;
            match fmt {
                OutputFormat::Json => output::json::success(&info),
                OutputFormat::Table => output::table::billing(&info),
            }
        }

        Commands::Models => {
            let info = client().await?.billing_info().await?;
            match fmt {
                OutputFormat::Json => output::json::success(&info.models),
                OutputFormat::Table => output::table::models(&info.models),
            }
        }

        Commands::List(args) => {
            let feed = client().await?.feed(args.cursor).await?;
            match fmt {
                // Object shape (not a bare clip array) so agents can page:
                // feed/v3 only accepts the opaque next_cursor token.
                OutputFormat::Json => {
                    let status = if feed.clips.is_empty() {
                        "no_results"
                    } else {
                        "success"
                    };
                    output::json::with_status(
                        status,
                        serde_json::json!({
                            "clips": feed.clips,
                            "next_cursor": feed.next_cursor,
                            "has_more": feed.has_more,
                        }),
                    );
                }
                OutputFormat::Table => {
                    output::table::clips(&feed.clips);
                    if let Some(cursor) = feed.next_cursor.as_deref()
                        && feed.has_more
                    {
                        eprintln!("\nNext page: suno list --cursor {cursor}");
                    }
                }
            }
        }

        Commands::Search(args) => {
            let feed = client().await?.search(&args.query).await?;
            match fmt {
                OutputFormat::Json => {
                    if feed.clips.is_empty() {
                        output::json::with_status("no_results", &feed.clips);
                    } else {
                        output::json::success(&feed.clips);
                    }
                }
                OutputFormat::Table => {
                    if feed.clips.is_empty() {
                        eprintln!("No clips matching \"{}\"", args.query);
                    } else {
                        output::table::clips(&feed.clips);
                    }
                }
            }
        }

        Commands::Lyrics(args) => {
            if !cli.quiet {
                eprintln!("Generating lyrics...");
            }
            let result = client().await?.generate_lyrics(&args.prompt).await?;
            match fmt {
                OutputFormat::Json => output::json::success(&result),
                OutputFormat::Table => output::table::lyrics(&result),
            }
        }

        Commands::Generate(args) => {
            let cfg = config::AppConfig::load()?;
            let model = resolve_model(args.model, &cfg)?;
            let lyrics = match (&args.lyrics, &args.lyrics_file) {
                (Some(l), _) => Some(l.clone()),
                (_, Some(path)) => Some(read_input_file(path)?),
                _ => None,
            };
            reject_unfilled_scaffold(lyrics.as_deref(), args.force)?;
            let tags = build_tags(args.tags.as_deref(), args.vocal.as_ref());
            let control_sliders =
                build_control_sliders(args.weirdness, args.style_influence, args.audio_influence);

            // Guard before any credit is spent or Chrome piloted.
            let mut guard = guard::DuplicateGuard::new(&config::data_dir(), "generate");
            guard.acquire(args.force)?;

            let c = client().await?;

            // Build the new v2-web request shape. Persona generation routes
            // through the same endpoint with persona_id set; the legacy
            // task="vox" field no longer exists in the v2-web schema.
            let mut req = GenerateRequest::new(model.to_api_key(), "custom");
            req.prompt = lyrics.unwrap_or_default();
            req.title = args.title;
            req.tags = tags;
            req.negative_tags = args.exclude.unwrap_or_default();
            req.make_instrumental = args.instrumental;
            req.persona_id = args.persona.clone();
            req.metadata.control_sliders = control_sliders;

            req.token = resolve_captcha(&c, args.token, args.no_captcha, cli.quiet).await?;

            if !cli.quiet {
                let persona_note = if args.persona.is_some() {
                    " with voice persona"
                } else {
                    ""
                };
                eprintln!(
                    "Submitting generation ({}{persona_note})...",
                    model.display_name()
                );
            }
            let clips = c.generate(&req).await?;
            handle_generation(
                &c,
                clips,
                args.wait,
                args.download.as_deref(),
                fmt,
                cli.quiet,
                &cfg,
            )
            .await?;
        }

        Commands::Describe(args) => {
            let cfg = config::AppConfig::load()?;
            let model = resolve_model(args.model, &cfg)?;
            let tags = build_tags(args.tags.as_deref(), args.vocal.as_ref());
            let control_sliders = build_control_sliders(args.weirdness, args.style_influence, None);

            let mut guard = guard::DuplicateGuard::new(&config::data_dir(), "describe");
            guard.acquire(args.force)?;

            // The v2-web schema dropped `gpt_description_prompt` — inspiration
            // mode is now signalled by `create_mode: "inspiration"` and the
            // text is sent in the same `prompt` field as custom mode.
            let mut req = GenerateRequest::new(model.to_api_key(), "inspiration");
            req.prompt = args.prompt;
            req.tags = tags;
            req.make_instrumental = args.instrumental;
            req.persona_id = args.persona.clone();
            req.metadata.control_sliders = control_sliders;

            let c = client().await?;
            req.token = resolve_captcha(&c, args.token, args.no_captcha, cli.quiet).await?;

            if !cli.quiet {
                eprintln!("Submitting description ({})...", model.display_name());
            }
            let clips = c.generate(&req).await?;
            handle_generation(
                &c,
                clips,
                args.wait,
                args.download.as_deref(),
                fmt,
                cli.quiet,
                &cfg,
            )
            .await?;
        }

        Commands::Extend(args) => {
            let cfg = config::AppConfig::load()?;
            let model = resolve_model(args.model, &cfg)?;

            // extend spends credits through the same v2-web create as generate,
            // so it gets the same scaffold preflight and duplicate guard
            // before any credit is spent.
            reject_unfilled_scaffold(args.lyrics.as_deref(), args.force)?;
            let mut guard = guard::DuplicateGuard::new(&config::data_dir(), "extend");
            guard.acquire(args.force)?;

            let mut req = GenerateRequest::new(model.to_api_key(), "custom");
            req.prompt = args.lyrics.unwrap_or_default();
            req.tags = args.tags;
            req.continue_clip_id = Some(args.clip_id);
            req.continue_at = Some(args.at);

            let c = client().await?;
            req.token = resolve_captcha(&c, args.token, args.no_captcha, cli.quiet).await?;

            let clips = c.generate(&req).await?;
            handle_generation(&c, clips, args.wait, None, fmt, cli.quiet, &cfg).await?;
        }

        Commands::Concat(args) => {
            let clip = client().await?.concat(&args.clip_id).await?;
            match fmt {
                OutputFormat::Json => output::json::success(&clip),
                OutputFormat::Table => output::table::clips(&[clip]),
            }
        }

        Commands::Cover(args) => {
            let cfg = config::AppConfig::load()?;
            let model = resolve_model(args.model, &cfg)?;
            let mut guard = guard::DuplicateGuard::new(&config::data_dir(), "cover");
            guard.acquire(args.force)?;

            let c = client().await?;
            let token = resolve_captcha(&c, args.token, args.no_captcha, cli.quiet).await?;
            let control_sliders = build_control_sliders(None, None, args.audio_influence);

            if !cli.quiet {
                eprintln!("Creating cover ({})...", model.display_name());
            }
            let clips = c
                .cover(
                    &args.clip_id,
                    model.to_api_key(),
                    args.tags.as_deref(),
                    token,
                    control_sliders,
                )
                .await?;
            handle_generation(
                &c,
                clips,
                args.wait,
                args.download.as_deref(),
                fmt,
                cli.quiet,
                &cfg,
            )
            .await?;
        }

        Commands::Remaster(args) => {
            let cfg = config::AppConfig::load()?;
            let mut guard = guard::DuplicateGuard::new(&config::data_dir(), "remaster");
            guard.acquire(args.force)?;

            let c = client().await?;
            let token = resolve_captcha(&c, args.token, args.no_captcha, cli.quiet).await?;

            if !cli.quiet {
                eprintln!("Remastering with {}...", args.model.to_api_key());
            }
            let clips = c
                .remaster(&args.clip_id, args.model.to_api_key(), token)
                .await?;
            handle_generation(
                &c,
                clips,
                args.wait,
                args.download.as_deref(),
                fmt,
                cli.quiet,
                &cfg,
            )
            .await?;
        }

        Commands::Stems(args) => {
            let clip = client().await?.stems(&args.clip_id).await?;
            match fmt {
                OutputFormat::Json => output::json::success(&clip),
                OutputFormat::Table => output::table::clips(&[clip]),
            }
        }

        Commands::Info(args) => {
            let clips = client()
                .await?
                .get_clips(std::slice::from_ref(&args.id))
                .await?;
            if clips.is_empty() {
                return Err(CliError::NotFound(format!("clip: {}", args.id)));
            }
            match fmt {
                OutputFormat::Json => output::json::success(&clips[0]),
                OutputFormat::Table => output::table::clip_detail(&clips[0]),
            }
        }

        Commands::Persona(args) => {
            let persona = client().await?.get_persona(&args.id).await?;
            match fmt {
                OutputFormat::Json => output::json::success(&persona),
                OutputFormat::Table => output::table::persona(&persona),
            }
        }

        Commands::Status(args) => {
            let clips = client().await?.get_clips(&args.ids).await?;
            match fmt {
                OutputFormat::Json => output::json::success(&clips),
                OutputFormat::Table => output::table::clips(&clips),
            }
        }

        Commands::Download(args) => {
            let cfg = config::AppConfig::load()?;
            let out_dir = args.output.unwrap_or(cfg.output_dir);
            let c = client().await?;
            let clips = c.get_clips(&args.ids).await?;
            if clips.is_empty() {
                return Err(CliError::NotFound(format!("clip: {}", args.ids.join(", "))));
            }

            // IDs the feed didn't return don't exist (or aren't yours) —
            // report them per-item instead of failing the whole batch.
            let mut failed: Vec<serde_json::Value> = args
                .ids
                .iter()
                .filter(|id| !clips.iter().any(|c| &&c.id == id))
                .map(|id| serde_json::json!({ "id": id, "error": "not found" }))
                .collect();

            let mut paths = Vec::new();
            for clip in &clips {
                // Download + lyric-embed per clip; one bad clip (still
                // streaming, deleted mid-batch) must not sink the rest.
                let result: Result<String, CliError> = async {
                    let path = download::download_clip(clip, &out_dir, args.video).await?;
                    if !args.video {
                        let plain_lyrics = clip.metadata.prompt.as_deref();
                        let aligned = c.aligned_lyrics(&clip.id).await.ok();
                        download::embed_lyrics_in_mp3(
                            &path,
                            &clip.title,
                            plain_lyrics,
                            aligned.as_deref(),
                        )?;
                        if !cli.quiet {
                            eprintln!("Embedded lyrics into {path}");
                        }
                    }
                    Ok(path)
                }
                .await;

                match result {
                    Ok(path) => {
                        if !cli.quiet {
                            eprintln!("Downloaded: {path}");
                        }
                        paths.push(path);
                    }
                    Err(e) => {
                        eprintln!("Failed: {} — {e}", clip.id);
                        failed.push(serde_json::json!({
                            "id": clip.id,
                            "error": e.to_string(),
                        }));
                    }
                }
            }

            if paths.is_empty() && !failed.is_empty() {
                return Err(CliError::Download(format!(
                    "all {} download(s) failed",
                    failed.len()
                )));
            }
            match fmt {
                OutputFormat::Json => {
                    let data = serde_json::json!({ "downloaded": paths, "failed": failed });
                    if failed.is_empty() {
                        output::json::success(data);
                    } else {
                        output::json::with_status("partial_success", data);
                    }
                }
                OutputFormat::Table => {}
            }
        }

        Commands::Delete(args) => {
            if args.ids.is_empty() {
                return Err(CliError::InvalidInput("no clip IDs provided".into()));
            }
            // No interactive confirmation and no sleep-then-proceed: agents
            // can't answer prompts, and a timed auto-proceed deletes data the
            // caller never confirmed. Explicit -y or nothing. Restoring is
            // non-destructive (it undoes a trash), so it needs no -y.
            if !args.yes && !args.restore {
                return Err(CliError::InvalidInput(format!(
                    "delete requires -y to confirm — re-run: suno delete {} -y",
                    args.ids.join(" ")
                )));
            }
            client()
                .await?
                .set_trashed(&args.ids, !args.restore)
                .await?;
            let verb = if args.restore { "restored" } else { "deleted" };
            match fmt {
                OutputFormat::Json => output::json::success(serde_json::json!({
                    verb: args.ids.len(),
                    "ids": args.ids,
                })),
                OutputFormat::Table => {
                    if args.restore {
                        eprintln!("Restored {} clip(s) from trash", args.ids.len());
                    } else {
                        eprintln!("Deleted {} clip(s)", args.ids.len());
                    }
                }
            }
        }

        Commands::Set(args) => {
            let lyrics = match (&args.lyrics, &args.lyrics_file) {
                (Some(l), _) => Some(l.clone()),
                (_, Some(path)) => Some(read_input_file(path)?),
                _ => None,
            };
            let req = SetMetadataRequest {
                title: args.title.clone(),
                lyrics,
                caption: args.caption.clone(),
                remove_image_cover: if args.remove_cover { Some(true) } else { None },
                remove_video_cover: None,
            };
            client().await?.set_metadata(&args.id, &req).await?;
            let mut changes = Vec::new();
            if args.title.is_some() {
                changes.push("title");
            }
            if args.lyrics.is_some() || args.lyrics_file.is_some() {
                changes.push("lyrics");
            }
            if args.caption.is_some() {
                changes.push("caption");
            }
            if args.remove_cover {
                changes.push("cover");
            }
            match fmt {
                OutputFormat::Json => output::json::success(serde_json::json!({
                    "id": args.id,
                    "updated": changes,
                })),
                OutputFormat::Table => eprintln!("Updated: {}", changes.join(", ")),
            }
        }

        Commands::Publish(args) => {
            let c = client().await?;
            let is_public = !args.private;
            for id in &args.ids {
                c.set_visibility(id, is_public).await?;
            }
            let state = if is_public { "public" } else { "private" };
            match fmt {
                OutputFormat::Json => output::json::success(serde_json::json!({
                    "published": args.ids,
                    "visibility": state,
                })),
                OutputFormat::Table => eprintln!("Set {} clip(s) to {state}", args.ids.len()),
            }
        }

        Commands::TimedLyrics(args) => {
            let words = client().await?.aligned_lyrics(&args.id).await?;
            // Empty stdout with exit 0 is correct (instrumentals have no
            // alignment) but reads like a silent failure — say why on stderr.
            if words.iter().all(|w| !w.success) && !cli.quiet {
                eprintln!("No timed lyrics available (instrumental or not yet aligned)");
            }
            if args.lrc {
                // LRC format: [mm:ss.xx] word
                for w in &words {
                    if !w.success {
                        continue;
                    }
                    let mins = (w.start_s / 60.0) as u32;
                    let secs = w.start_s % 60.0;
                    println!("[{:02}:{:05.2}] {}", mins, secs, w.word);
                }
            } else {
                match fmt {
                    OutputFormat::Json => output::json::success(&words),
                    OutputFormat::Table => {
                        for w in &words {
                            if w.success {
                                println!("{:>6.2}s - {:>6.2}s  {}", w.start_s, w.end_s, w.word);
                            }
                        }
                    }
                }
            }
        }

        Commands::Config(args) => match args.action {
            ConfigAction::Show => {
                let cfg = config::AppConfig::load()?;
                match fmt {
                    OutputFormat::Json => output::json::success(&cfg),
                    OutputFormat::Table => {
                        println!("{}", serde_json::to_string_pretty(&cfg)?)
                    }
                }
            }
            ConfigAction::Set { key, value } => {
                let path = config::AppConfig::set_value(&key, &value)?;
                match fmt {
                    OutputFormat::Json => output::json::success(serde_json::json!({
                        "updated": { "key": key, "value": value },
                        "path": path.display().to_string(),
                    })),
                    OutputFormat::Table => {
                        eprintln!("Set {key} = {value} in {}", path.display())
                    }
                }
            }
            ConfigAction::Path => {
                let path = config::config_path();
                match fmt {
                    OutputFormat::Json => output::json::success(serde_json::json!({
                        "path": path.display().to_string(),
                    })),
                    OutputFormat::Table => println!("{}", path.display()),
                }
            }
            // Parse + semantic validation of the merged effective config; the
            // auth/Chrome/network health checks this used to half-do live in
            // `doctor` now.
            ConfigAction::Check => {
                let path = config::config_path();
                config::AppConfig::load()?.validate()?;
                match fmt {
                    OutputFormat::Json => output::json::success(serde_json::json!({
                        "valid": true,
                        "path": path.display().to_string(),
                        "exists": path.exists(),
                    })),
                    OutputFormat::Table => eprintln!(
                        "Config OK ({}{})",
                        path.display(),
                        if path.exists() {
                            ""
                        } else {
                            " — not present, defaults apply"
                        }
                    ),
                }
            }
        },

        Commands::Doctor => commands::doctor::run(fmt, cli.quiet).await?,

        Commands::Skill(args) => match args.action {
            SkillAction::Install => commands::skill::install(fmt, cli.quiet, false)?,
            SkillAction::Status => commands::skill::status(fmt)?,
        },

        // Hidden back-compat for pre-0.6 scripts; `skill install` is the
        // real command.
        Commands::InstallSkill(args) => {
            if args.print {
                print!("{}", commands::skill::SKILL_CONTENT);
            } else if let Some(path) = args.path.as_deref() {
                commands::skill::install_to_path(path, fmt, cli.quiet, args.force)?;
            } else {
                commands::skill::install(fmt, cli.quiet, args.force)?;
            }
        }

        Commands::Update(args) => {
            // self_update drives blocking reqwest, which refuses to run on the
            // async runtime thread (panics under debug_assertions) — hop to a
            // blocking thread.
            let (check, force, quiet) = (args.check, args.force, cli.quiet);
            tokio::task::spawn_blocking(move || commands::update::run(check, force, fmt, quiet))
                .await
                .map_err(|e| CliError::Update(format!("update task failed: {e}")))??
        }

        Commands::Contract { code } => commands::contract::run(fmt, code)?,

        Commands::Write(args) => commands::write::run(args, fmt, cli.quiet)?,

        Commands::Guide(args) => commands::guide::run(args.name, fmt, cli.quiet)?,

        Commands::AgentInfo => commands::agent_info::run(),
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    // Pre-scan argv for --json before clap runs so help, version, and parse
    // errors honor it too (clap hasn't populated the Cli struct on those
    // paths).
    let json_mode = std::env::args_os().any(|a| a == "--json")
        || !std::io::IsTerminal::is_terminal(&std::io::stdout());

    // try_parse so exit codes and envelopes stay ours, not clap's: help and
    // --version are data (exit 0, enveloped when piped), parse errors are bad
    // input (exit 3, JSON error envelope on stderr in JSON mode).
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                if json_mode {
                    output::json::help(e.to_string().trim_end());
                    std::process::exit(0);
                }
                e.exit();
            }
            if json_mode {
                output::json::error(
                    "invalid_input",
                    e.to_string().trim_end(),
                    "Check arguments with `suno --help`",
                );
            } else {
                eprint!("{e}");
            }
            std::process::exit(3);
        }
    };

    let fmt = OutputFormat::detect(cli.json);
    // Race the command against Ctrl-C so the solver Chrome is torn down on
    // every exit path — it must never outlive the invocation.
    let result = tokio::select! {
        r = run(cli, fmt) => r,
        _ = tokio::signal::ctrl_c() => {
            captcha::shutdown().await;
            eprintln!("Interrupted");
            std::process::exit(130);
        }
    };
    captcha::shutdown().await;
    if let Err(e) = result {
        if json_mode {
            output::json::error(e.error_code(), &e.to_string(), e.suggestion());
        } else {
            eprintln!("Error [{}]: {}", e.error_code(), e);
            eprintln!("Hint: {}", e.suggestion());
        }
        std::process::exit(e.exit_code());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_sliders_normalize_and_clamp() {
        let s = build_control_sliders(Some(50.0), Some(150.0), Some(0.0)).unwrap();
        assert_eq!(s.weirdness_constraint, Some(0.5));
        // Out-of-range input clamps rather than sending >1.0 to the API.
        assert_eq!(s.style_weight, Some(1.0));
        assert_eq!(s.audio_weight, Some(0.0));

        // No flags → no block at all, so the request stays clean.
        assert!(build_control_sliders(None, None, None).is_none());

        // A lone --audio-influence still produces a block.
        let s = build_control_sliders(None, None, Some(65.0)).unwrap();
        assert_eq!(s.audio_weight, Some(0.65));
        assert_eq!(s.weirdness_constraint, None);
    }

    #[test]
    fn tags_merge_vocal_gender() {
        assert_eq!(
            build_tags(Some("pop"), Some(&VocalGender::Female)).as_deref(),
            Some("pop, female vocals")
        );
        assert_eq!(build_tags(None, None), None);
    }

    #[test]
    fn scaffold_preflight_covers_every_lyrics_path() {
        // The guard generate AND extend share: unresolved spans are refused...
        let scaffold = "[Verse]\n<4-6 lines — set the scene>\n";
        assert!(matches!(
            reject_unfilled_scaffold(Some(scaffold), false),
            Err(CliError::InvalidInput(_))
        ));
        // ...including spans split across lines...
        let split = "[Verse]\n<4-6 lines — set the scene:\nroad trips>\n";
        assert!(reject_unfilled_scaffold(Some(split), false).is_err());
        // ...while --force, filled lyrics, and no lyrics pass through.
        assert!(reject_unfilled_scaffold(Some(scaffold), true).is_ok());
        assert!(reject_unfilled_scaffold(Some("[Verse]\nwe rise\n"), false).is_ok());
        assert!(reject_unfilled_scaffold(None, false).is_ok());
    }
}
