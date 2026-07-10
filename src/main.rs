mod api;
mod auth;
mod captcha;
mod cli;
mod config;
mod download;
mod errors;
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

/// Build a control_sliders block when --weirdness or --style-influence is set.
/// Returns None when neither is provided so we don't pollute the request.
fn build_control_sliders(
    weirdness: Option<f64>,
    style_influence: Option<f64>,
) -> Option<ControlSliders> {
    if weirdness.is_none() && style_influence.is_none() {
        return None;
    }
    Some(ControlSliders {
        // Normalize 0-100 → 0.0-1.0
        weirdness_constraint: weirdness.map(|w| (w / 100.0).clamp(0.0, 1.0)),
        style_weight: style_influence.map(|s| (s / 100.0).clamp(0.0, 1.0)),
    })
}

/// Generate, wait, optionally download with lyrics embedding.
async fn handle_generation(
    c: &SunoClient,
    clips: Vec<api::types::Clip>,
    wait: bool,
    download_dir: Option<&str>,
    fmt: &OutputFormat,
    quiet: bool,
) -> Result<(), CliError> {
    let ids: Vec<String> = clips.iter().map(|c| c.id.clone()).collect();

    if wait && !ids.is_empty() {
        if !quiet {
            eprintln!("Waiting for generation to complete...");
        }
        let final_clips = c.poll_clips(&ids, 600).await?;

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

async fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    let fmt = OutputFormat::detect(cli.json);

    match cli.command {
        Commands::Auth(args) => {
            if args.logout {
                AuthState::delete()?;
                eprintln!("Logged out; removed stored Suno authentication");
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
            eprintln!(
                "Authenticated! Plan: {}, Credits: {}",
                info.plan_name(),
                info.total_credits_left
            );
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
            let feed = client().await?.feed(args.page).await?;
            match fmt {
                OutputFormat::Json => output::json::success(&feed.clips),
                OutputFormat::Table => output::table::clips(&feed.clips),
            }
        }

        Commands::Search(args) => {
            let feed = client().await?.search(&args.query).await?;
            match fmt {
                OutputFormat::Json => output::json::success(&feed.clips),
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
            let lyrics = match (&args.lyrics, &args.lyrics_file) {
                (Some(l), _) => Some(l.clone()),
                (_, Some(path)) => Some(std::fs::read_to_string(path)?),
                _ => None,
            };
            let tags = build_tags(args.tags.as_deref(), args.vocal.as_ref());
            let control_sliders = build_control_sliders(args.weirdness, args.style_influence);

            let c = client().await?;

            // Build the new v2-web request shape. Persona generation routes
            // through the same endpoint with persona_id set; the legacy
            // task="vox" field no longer exists in the v2-web schema.
            let mut req = GenerateRequest::new(args.model.to_api_key(), "custom");
            req.prompt = lyrics.unwrap_or_default();
            req.title = args.title;
            req.tags = tags;
            req.negative_tags = args.exclude.unwrap_or_default();
            req.make_instrumental = args.instrumental;
            req.persona_id = args.persona.clone();
            req.metadata.control_sliders = control_sliders;

            // Solve hCaptcha. Suno gates v2-web with an invisible challenge
            // — only a real piloted Chrome can pass it. The user can override
            // by passing --token explicitly (e.g. from a 2Captcha solution).
            req.token = if let Some(t) = args.token {
                Some(t)
            } else if !args.no_captcha {
                if !cli.quiet {
                    eprintln!("Solving hCaptcha via piloted Chrome...");
                }
                let auth = AuthState::load()?;
                Some(captcha::solve(&auth).await?)
            } else {
                None
            };

            if !cli.quiet {
                let persona_note = if args.persona.is_some() {
                    " with voice persona"
                } else {
                    ""
                };
                eprintln!(
                    "Submitting generation ({}{persona_note})...",
                    args.model.display_name()
                );
            }
            let clips = c.generate(&req).await?;
            handle_generation(
                &c,
                clips,
                args.wait,
                args.download.as_deref(),
                &fmt,
                cli.quiet,
            )
            .await?;
        }

        Commands::Describe(args) => {
            let tags = build_tags(args.tags.as_deref(), args.vocal.as_ref());
            let control_sliders = build_control_sliders(args.weirdness, args.style_influence);

            // The v2-web schema dropped `gpt_description_prompt` — inspiration
            // mode is now signalled by `create_mode: "inspiration"` and the
            // text is sent in the same `prompt` field as custom mode.
            let mut req = GenerateRequest::new(args.model.to_api_key(), "inspiration");
            req.prompt = args.prompt;
            req.title = args.title;
            req.tags = tags;
            req.make_instrumental = args.instrumental;
            req.persona_id = args.persona.clone();
            req.metadata.control_sliders = control_sliders;

            // Same captcha auto-solve as generate.
            if !args.no_captcha {
                if !cli.quiet {
                    eprintln!("Solving hCaptcha via piloted Chrome...");
                }
                let auth = AuthState::load()?;
                req.token = Some(captcha::solve(&auth).await?);
            }

            if !cli.quiet {
                eprintln!("Submitting description ({})...", args.model.display_name());
            }
            let c = client().await?;
            let clips = c.generate(&req).await?;
            handle_generation(
                &c,
                clips,
                args.wait,
                args.download.as_deref(),
                &fmt,
                cli.quiet,
            )
            .await?;
        }

        Commands::Extend(args) => {
            let mut req = GenerateRequest::new("chirp-fenix", "custom");
            req.prompt = args.lyrics.unwrap_or_default();
            req.tags = args.tags;
            req.continue_clip_id = Some(args.clip_id);
            req.continue_at = Some(args.at);

            let c = client().await?;
            let clips = c.generate(&req).await?;
            handle_generation(&c, clips, args.wait, None, &fmt, cli.quiet).await?;
        }

        Commands::Concat(args) => {
            let clip = client().await?.concat(&args.clip_id).await?;
            match fmt {
                OutputFormat::Json => output::json::success(&clip),
                OutputFormat::Table => output::table::clips(&[clip]),
            }
        }

        Commands::Cover(args) => {
            if !cli.quiet {
                eprintln!("Creating cover ({})...", args.model.display_name());
            }
            let c = client().await?;
            let clips = c
                .cover(&args.clip_id, args.model.to_api_key(), args.tags.as_deref())
                .await?;
            handle_generation(
                &c,
                clips,
                args.wait,
                args.download.as_deref(),
                &fmt,
                cli.quiet,
            )
            .await?;
        }

        Commands::Remaster(args) => {
            if !cli.quiet {
                eprintln!("Remastering with {}...", args.model.to_api_key());
            }
            let c = client().await?;
            let clips = c.remaster(&args.clip_id, args.model.to_api_key()).await?;
            handle_generation(
                &c,
                clips,
                args.wait,
                args.download.as_deref(),
                &fmt,
                cli.quiet,
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
            let c = client().await?;
            let clips = c.get_clips(&args.ids).await?;
            if clips.is_empty() {
                return Err(CliError::NotFound(format!(
                    "clips: {}",
                    args.ids.join(", ")
                )));
            }
            let mut paths = Vec::new();
            for clip in &clips {
                let path = download::download_clip(clip, &args.output, args.video).await?;

                // Embed lyrics into MP3 downloads
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

                if !cli.quiet {
                    eprintln!("Downloaded: {path}");
                }
                paths.push(path);
            }
            match fmt {
                OutputFormat::Json => output::json::success(&paths),
                OutputFormat::Table => {}
            }
        }

        Commands::Delete(args) => {
            if args.ids.is_empty() {
                return Err(CliError::Config("no clip IDs provided".into()));
            }
            if !args.yes {
                eprintln!(
                    "Deleting {} clip(s): {}",
                    args.ids.len(),
                    args.ids.join(", ")
                );
                eprintln!("Use -y to skip confirmation, or press Ctrl+C to cancel");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            client().await?.delete_clips(&args.ids).await?;
            eprintln!("Deleted {} clip(s)", args.ids.len());
        }

        Commands::Set(args) => {
            let lyrics = match (&args.lyrics, &args.lyrics_file) {
                (Some(l), _) => Some(l.clone()),
                (_, Some(path)) => Some(std::fs::read_to_string(path)?),
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
                changes.push("cover removed");
            }
            eprintln!("Updated: {}", changes.join(", "));
        }

        Commands::Publish(args) => {
            let c = client().await?;
            let is_public = !args.private;
            for id in &args.ids {
                c.set_visibility(id, is_public).await?;
            }
            let state = if is_public { "public" } else { "private" };
            eprintln!("Set {} clip(s) to {state}", args.ids.len());
        }

        Commands::TimedLyrics(args) => {
            let words = client().await?.aligned_lyrics(&args.id).await?;
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
                let cfg = config::AppConfig::load();
                println!("{}", serde_json::to_string_pretty(&cfg)?);
            }
            ConfigAction::Set { key, value } => {
                return Err(CliError::Config(format!(
                    "`config set {key}={value}` is not yet implemented — use env vars (SUNO_{key})",
                    key = key.to_uppercase()
                )));
            }
            ConfigAction::Check => {
                let _ = config::AppConfig::load();
                match AuthState::load() {
                    Ok(auth) => match SunoClient::new_with_refresh(auth).await {
                        Ok(client) => {
                            let info = client.billing_info().await?;
                            eprintln!(
                                "Auth: OK — {}, {} credits",
                                info.plan_name(),
                                info.total_credits_left
                            );
                        }
                        Err(CliError::AuthExpired) => {
                            eprintln!("Auth: expired — run `suno auth --login`");
                        }
                        Err(e) => return Err(e),
                    },
                    Err(_) => eprintln!("Auth: not configured — run `suno auth`"),
                }
            }
        },

        Commands::InstallSkill(args) => {
            const SKILL_BODY: &str = include_str!("../assets/SKILL.md");

            if args.print {
                print!("{SKILL_BODY}");
                return Ok(());
            }

            let home = directories::UserDirs::new()
                .map(|d| d.home_dir().to_path_buf())
                .ok_or_else(|| CliError::Config("could not determine home directory".into()))?;

            let dest_path: std::path::PathBuf = if let Some(custom) = args.path {
                if let Some(stripped) = custom.strip_prefix("~/") {
                    home.join(stripped)
                } else if custom == "~" {
                    home.clone()
                } else {
                    std::path::PathBuf::from(custom)
                }
            } else {
                match args.target {
                    SkillTarget::Claude => home.join(".claude/skills/suno/SKILL.md"),
                    SkillTarget::Cursor => std::env::current_dir()?.join(".cursor/rules/suno.mdc"),
                }
            };

            if dest_path.exists() && !args.force {
                return Err(CliError::Config(format!(
                    "{} already exists — pass --force to overwrite",
                    dest_path.display()
                )));
            }

            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&dest_path, SKILL_BODY)?;

            match fmt {
                OutputFormat::Json => output::json::success(serde_json::json!({
                    "installed": true,
                    "path": dest_path.display().to_string(),
                    "target": match args.target {
                        SkillTarget::Claude => "claude",
                        SkillTarget::Cursor => "cursor",
                    },
                })),
                OutputFormat::Table => {
                    eprintln!("Installed suno skill to: {}", dest_path.display());
                    match args.target {
                        SkillTarget::Claude => eprintln!(
                            "Restart Claude Code to pick up the new skill (or it loads on next session)."
                        ),
                        SkillTarget::Cursor => {
                            eprintln!("Cursor will pick up the rule on next workspace reload.")
                        }
                    }
                }
            }
        }

        Commands::Update(args) => {
            let current = env!("CARGO_PKG_VERSION");
            let updater = self_update::backends::github::Update::configure()
                .repo_owner("paperfoot")
                .repo_name("suno-cli")
                .bin_name("suno")
                .show_download_progress(!cli.quiet)
                .current_version(current)
                .build()
                .map_err(|e| CliError::Update(e.to_string()))?;

            if args.check {
                let latest = updater
                    .get_latest_release()
                    .map_err(|e| CliError::Update(e.to_string()))?;
                let v = latest.version.trim_start_matches('v').to_string();
                let up_to_date = v == current;
                let status = if up_to_date {
                    "up_to_date"
                } else {
                    "update_available"
                };
                let result = serde_json::json!({
                    "current_version": current,
                    "latest_version": v,
                    "status": status,
                });
                match fmt {
                    OutputFormat::Json => output::json::success(&result),
                    OutputFormat::Table => {
                        if up_to_date {
                            eprintln!("Up to date (v{current})");
                        } else {
                            eprintln!("Update available: v{current} -> v{v}");
                            eprintln!("Run `suno update` to install");
                        }
                    }
                }
            } else {
                let release = updater
                    .update()
                    .map_err(|e| CliError::Update(e.to_string()))?;
                let v = release.version().trim_start_matches('v').to_string();
                let up_to_date = v == current;
                let status = if up_to_date { "up_to_date" } else { "updated" };
                let result = serde_json::json!({
                    "current_version": current,
                    "latest_version": v,
                    "status": status,
                });
                match fmt {
                    OutputFormat::Json => output::json::success(&result),
                    OutputFormat::Table => {
                        if up_to_date {
                            eprintln!("Already up to date (v{current})");
                        } else {
                            eprintln!("Updated: v{current} -> v{v}");
                            eprintln!(
                                "Run `suno install-skill --force` to refresh the agent skill"
                            );
                        }
                    }
                }
            }
        }

        Commands::AgentInfo => {
            let auth_path = directories::ProjectDirs::from("com", "suno-cli", "suno-cli")
                .map(|d| d.config_dir().join("auth.json").display().to_string())
                .unwrap_or_else(|| "~/.config/suno-cli/auth.json".into());

            let info = serde_json::json!({
                "name": "suno",
                "version": env!("CARGO_PKG_VERSION"),
                "description": "Suno AI music generation CLI — v5.5 with voice personas, covers, remasters",
                "commands": [
                    "generate", "describe", "lyrics", "extend", "concat",
                    "cover", "remaster", "stems", "info", "persona",
                    "list", "search", "status", "download", "delete",
                    "set", "publish", "timed-lyrics",
                    "credits", "models", "auth", "config", "agent-info",
                    "install-skill", "update"
                ],
                "models": {
                    "v5.5": "chirp-fenix",
                    "v5": "chirp-crow",
                    "v4.5+": "chirp-bluejay",
                    "v4.5": "chirp-auk",
                    "v4": "chirp-v4",
                },
                "remaster_models": {
                    "v5.5": "chirp-flounder",
                    "v5": "chirp-carp",
                    "v4.5+": "chirp-bass",
                },
                "features": [
                    "tags", "negative_tags", "vocal_gender",
                    "weirdness", "style_influence",
                    "instrumental", "extend", "concat", "cover", "remaster",
                    "stems", "lyrics", "timed_lyrics", "set_metadata",
                    "set_visibility", "search", "delete", "captcha_check",
                    "id3_lyrics_embedding", "voice_persona", "clip_info"
                ],
                "exit_codes": {
                    "0": "success",
                    "1": "transient error (network, API) — retry",
                    "2": "configuration error — check config",
                    "3": "auth error — run `suno auth --login`",
                    "4": "rate limited — wait and retry",
                    "5": "not found — verify resource ID"
                },
                "env_prefix": "SUNO_",
                "auth_path": auth_path,
                "auth": {
                    "recommended": "suno auth --login",
                    "methods": [
                        "browser_cookie_extract",
                        "full_cookie_header",
                        "raw_clerk_client_cookie",
                        "direct_jwt",
                        "stored_clerk_refresh",
                    ],
                    "logout": "suno auth --logout",
                    "generation_captcha": "Default generation uses the browser-backed hCaptcha solver. Use --no-captcha only with a valid --token or for deliberate API tests.",
                },
                "provider": "direct_suno_unofficial",
                "auth_required": true,
                "default_model": "chirp-fenix (v5.5)",
            });
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        let json_mode = std::env::args().any(|a| a == "--json")
            || !std::io::IsTerminal::is_terminal(&std::io::stdout());

        if json_mode {
            output::json::error(e.error_code(), &e.to_string(), e.suggestion());
        } else {
            eprintln!("Error [{}]: {}", e.error_code(), e);
            eprintln!("Hint: {}", e.suggestion());
        }
        std::process::exit(e.exit_code());
    }
}
