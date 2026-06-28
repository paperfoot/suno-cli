use super::SunoClient;
use super::types::{Clip, GenerateRequest, GenerateResponse};
use crate::errors::CliError;

impl SunoClient {
    /// Submit a music generation request (custom mode or inspiration mode).
    /// Posts to `/api/generate/v2-web/` — the legacy `/api/generate/v2/`
    /// returns `Token validation failed` since Suno migrated creates to
    /// `v2-web` server-side (verified 2026-04-07).
    /// Wrapped in `with_auth_retry` so a single stale-JWT failure recovers
    /// transparently via Clerk refresh.
    pub async fn generate(&self, req: &GenerateRequest) -> Result<Vec<Clip>, CliError> {
        self.with_auth_retry(|| async {
            let resp = self.post("/api/generate/v2-web/").json(req).send().await?;
            let resp = self.check_response(resp).await?;
            let result: GenerateResponse = resp.json().await?;
            Ok(result.clips)
        })
        .await
    }

    /// Resolve the title for a clip-derived generation (cover/remaster/extend).
    /// Prefers an explicit override, then the source clip's own title, then a
    /// generic fallback. `/api/generate/v2-web/` requires `params.title` to be a
    /// non-null string, so this never returns an empty value.
    pub async fn resolve_title(
        &self,
        clip_id: &str,
        override_title: Option<&str>,
        fallback: &str,
    ) -> String {
        if let Some(t) = override_title {
            let t = t.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
        if let Ok(clips) = self.get_clips(&[clip_id.to_string()]).await
            && let Some(c) = clips.into_iter().next()
        {
            let t = c.title.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
        fallback.to_string()
    }

    /// Poll clip status by IDs until all are complete or errored.
    /// "streaming" means still generating — we wait for "complete".
    pub async fn poll_clips(
        &self,
        ids: &[String],
        timeout_secs: u64,
    ) -> Result<Vec<Clip>, CliError> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_secs);
        let mut delay = std::time::Duration::from_secs(3);

        loop {
            let clips = self.get_clips(ids).await?;
            let all_done = clips
                .iter()
                .all(|c| matches!(c.status.as_str(), "complete" | "error"));

            if all_done {
                return Ok(clips);
            }
            if start.elapsed() >= timeout {
                return Err(CliError::GenerationFailed(format!(
                    "generation timed out after {timeout_secs}s for {}",
                    ids.join(", ")
                )));
            }
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(std::time::Duration::from_secs(15));
        }
    }

    /// Fetch clips by IDs. Batches in pairs to avoid Suno's limit
    /// (SunoAI-API #49: 4+ IDs from different batches only returns first 2).
    /// Each chunk is wrapped in `with_auth_retry` so long polling waits
    /// survive Suno's JWT staleness window mid-generation.
    pub async fn get_clips(&self, ids: &[String]) -> Result<Vec<Clip>, CliError> {
        let mut all_clips = Vec::new();
        for chunk in ids.chunks(2) {
            let ids_param = chunk.join(",");
            let path = format!("/api/feed/?ids={ids_param}");
            let clips: Vec<Clip> = self
                .with_auth_retry(|| async {
                    let resp = self.get(&path).send().await?;
                    let resp = self.check_response(resp).await?;
                    let clips: Vec<Clip> = resp.json().await?;
                    Ok(clips)
                })
                .await?;
            all_clips.extend(clips);
        }
        Ok(all_clips)
    }
}
