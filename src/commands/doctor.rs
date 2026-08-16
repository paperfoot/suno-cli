//! Dependency diagnostics: "can this tool actually work right now?"
//! Agents run `doctor` before first use. Network checks are warn-only so a
//! flaky connection doesn't read as a broken install; exit 0 unless a check
//! fails, then exit 2 (config error).

use serde::Serialize;

use crate::api::SunoClient;
use crate::auth::AuthState;
use crate::config;
use crate::errors::CliError;
use crate::output::OutputFormat;

#[derive(Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Serialize)]
struct DoctorCheck {
    name: &'static str,
    status: CheckStatus,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<String>,
}

impl DoctorCheck {
    fn pass(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Pass,
            detail: detail.into(),
            suggestion: None,
        }
    }

    fn warn(name: &'static str, detail: impl Into<String>, suggestion: Option<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Warn,
            detail: detail.into(),
            suggestion,
        }
    }

    fn fail(name: &'static str, detail: impl Into<String>, suggestion: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Fail,
            detail: detail.into(),
            suggestion: Some(suggestion.into()),
        }
    }
}

#[derive(Serialize)]
struct DoctorSummary {
    pass: usize,
    warn: usize,
    fail: usize,
}

#[derive(Serialize)]
struct DoctorReport {
    checks: Vec<DoctorCheck>,
    summary: DoctorSummary,
}

fn auth_json_path() -> String {
    config::config_dir().join("auth.json").display().to_string()
}

fn check_config(checks: &mut Vec<DoctorCheck>) {
    let path = config::config_path();
    if path.exists() {
        checks.push(DoctorCheck::pass("config_file", path.display().to_string()));
    } else {
        checks.push(DoctorCheck::warn(
            "config_file",
            format!("{} not found (defaults apply)", path.display()),
            None,
        ));
    }
    match config::AppConfig::load() {
        Ok(_) => checks.push(DoctorCheck::pass(
            "config_parse",
            "defaults < file < SUNO_* env merge cleanly",
        )),
        Err(e) => checks.push(DoctorCheck::fail(
            "config_parse",
            e.to_string(),
            format!("Fix or delete {}", path.display()),
        )),
    }
}

/// Auth file + JWT freshness, no network. Returns the state when usable so
/// the network checks can build a client from it.
fn check_auth(checks: &mut Vec<DoctorCheck>) -> Option<AuthState> {
    let state = match AuthState::load() {
        Ok(state) => {
            checks.push(DoctorCheck::pass("auth_file", auth_json_path()));
            state
        }
        Err(CliError::AuthMissing) => {
            checks.push(DoctorCheck::fail(
                "auth_file",
                format!("{} not found", auth_json_path()),
                "Run `suno auth --login`",
            ));
            return None;
        }
        Err(e) => {
            checks.push(DoctorCheck::fail(
                "auth_file",
                e.to_string(),
                "Run `suno auth --login` to rewrite it",
            ));
            return None;
        }
    };

    if state.jwt.is_none() {
        if state.clerk_client_cookie.is_some() {
            checks.push(DoctorCheck::warn(
                "jwt",
                "no JWT stored — will be minted from the Clerk session on next call",
                None,
            ));
        } else {
            checks.push(DoctorCheck::fail(
                "jwt",
                "no JWT and no Clerk session cookie stored",
                "Run `suno auth --login`",
            ));
            return None;
        }
    } else if state.is_jwt_expired() {
        // is_jwt_expired() fires 30 min early on purpose (Suno's server-side
        // staleness window) — with a Clerk cookie this self-heals.
        if state.clerk_client_cookie.is_some() {
            checks.push(DoctorCheck::warn(
                "jwt",
                "stale (or within Suno's 30-min staleness window) — auto-refreshes via Clerk on next call",
                None,
            ));
        } else {
            checks.push(DoctorCheck::fail(
                "jwt",
                "expired, and no Clerk session cookie to refresh with",
                "Run `suno auth --login`",
            ));
            return None;
        }
    } else {
        checks.push(DoctorCheck::pass("jwt", "present and fresh"));
    }
    Some(state)
}

/// Returns whether a Chrome/Chromium binary is available, so the captcha
/// preflight check can escalate to a failure when the solver is actually
/// needed but can't run.
fn check_chrome(checks: &mut Vec<DoctorCheck>) -> bool {
    match crate::captcha::locate_chrome() {
        Ok(path) => {
            checks.push(DoctorCheck::pass("chrome", path));
            true
        }
        Err(e) => {
            checks.push(DoctorCheck::warn(
                "chrome",
                e.to_string(),
                Some("Only needed when Suno captcha-gates this account — install Chrome or set SUNO_CHROME_PATH".into()),
            ));
            false
        }
    }
}

async fn check_solver_chrome(checks: &mut Vec<DoctorCheck>) {
    match crate::captcha::detect_solver_chrome().await {
        Some(port) => checks.push(DoctorCheck::warn(
            "solver_chrome",
            format!(
                "something is still listening on solver port {port} — suno normally \
                 kills its Chrome on exit, so this is a crashed run's leftover or an \
                 unrelated service"
            ),
            Some(format!(
                "Solves never attach to it (they pick a fresh port), so it's inert; \
                 quit it via Activity Monitor (Chrome with --remote-debugging-port={port}) \
                 to reclaim memory"
            )),
        )),
        None => checks.push(DoctorCheck::pass("solver_chrome", "no leftover instance")),
    }
}

/// API reachability (warn-only), credits, and the captcha preflight —
/// everything that needs a working client.
async fn check_api(checks: &mut Vec<DoctorCheck>, state: AuthState, chrome_available: bool) {
    let client = match SunoClient::new_with_refresh(state).await {
        Ok(c) => c,
        // An auth rejection is a setup failure, not a flaky-network warning:
        // the fix is `suno auth`, so it must count toward exit 2.
        Err(e @ (CliError::AuthExpired | CliError::AuthMissing)) => {
            checks.push(DoctorCheck::fail(
                "studio_api",
                format!("authentication rejected: {e}"),
                "Run `suno auth --refresh`, then `suno auth --login` if that fails",
            ));
            return;
        }
        Err(e) => {
            checks.push(DoctorCheck::warn(
                "studio_api",
                format!("could not build an authenticated client: {e}"),
                Some("Run `suno auth --refresh`, then `suno auth --login` if that fails".into()),
            ));
            return;
        }
    };

    match client.billing_info().await {
        Ok(info) => {
            checks.push(DoctorCheck::pass(
                "studio_api",
                format!("reachable — plan {}", info.plan_name()),
            ));
            if info.total_credits_left > 0 {
                checks.push(DoctorCheck::pass(
                    "credits",
                    format!(
                        "{} left (v5.5 generation ≈ 70 per call; `lyrics` is free)",
                        info.total_credits_left
                    ),
                ));
            } else {
                checks.push(DoctorCheck::warn(
                    "credits",
                    "0 credits left — generation will fail",
                    Some("Top up or wait for the plan refill at suno.com".into()),
                ));
            }
        }
        Err(e) => checks.push(DoctorCheck::warn(
            "studio_api",
            format!("unreachable: {e}"),
            Some("Check your network; Suno itself may be down".into()),
        )),
    }

    match client.captcha_check("generation").await {
        Ok(resp) if resp.required && !chrome_available => {
            // Captcha is enforced on this account but the solver has no browser
            // to drive — generation will hard-fail. That's a setup failure.
            checks.push(DoctorCheck::fail(
                "captcha_preflight",
                "required: true, but no Chrome/Chromium is available to solve it",
                "Install Google Chrome or set SUNO_CHROME_PATH",
            ));
        }
        Ok(resp) => {
            let detail = if resp.required {
                "required: true — generation will pilot Chrome to solve hCaptcha".to_string()
            } else {
                "required: false — solver will be skipped".to_string()
            };
            checks.push(DoctorCheck::pass("captcha_preflight", detail));
        }
        Err(e) => checks.push(DoctorCheck::warn(
            "captcha_preflight",
            format!("preflight failed: {e} — generation falls back to solving"),
            None,
        )),
    }
}

pub async fn run(fmt: OutputFormat, quiet: bool) -> Result<(), CliError> {
    let mut checks = Vec::new();

    check_config(&mut checks);
    let auth = check_auth(&mut checks);
    let chrome_available = check_chrome(&mut checks);
    check_solver_chrome(&mut checks).await;
    if let Some(state) = auth {
        check_api(&mut checks, state, chrome_available).await;
    }

    let count = |s: CheckStatus| checks.iter().filter(|c| c.status == s).count();
    let summary = DoctorSummary {
        pass: count(CheckStatus::Pass),
        warn: count(CheckStatus::Warn),
        fail: count(CheckStatus::Fail),
    };
    let has_failures = summary.fail > 0;
    // The envelope status must reflect the checks: a failing run is not a
    // "success" envelope that happens to exit 2. Some checks still passing →
    // partial_success; nothing passed → all_failed.
    let envelope_status = if !has_failures {
        "success"
    } else if summary.pass > 0 {
        "partial_success"
    } else {
        "all_failed"
    };
    let report = DoctorReport { checks, summary };

    match fmt {
        OutputFormat::Json => crate::output::json::with_status(envelope_status, &report),
        OutputFormat::Table => {
            for check in &report.checks {
                let icon = match check.status {
                    CheckStatus::Pass => "ok  ",
                    CheckStatus::Warn => "warn",
                    CheckStatus::Fail => "FAIL",
                };
                println!("[{icon}] {}: {}", check.name, check.detail);
                if let Some(s) = &check.suggestion
                    && !quiet
                {
                    println!("       {s}");
                }
            }
            println!(
                "{} pass, {} warn, {} fail",
                report.summary.pass, report.summary.warn, report.summary.fail
            );
        }
    }

    if has_failures {
        return Err(CliError::Config("doctor found failing checks".into()));
    }
    Ok(())
}
