use super::SunoClient;
use super::types::{Clip, GenerateRequest};
use crate::errors::CliError;

impl SunoClient {
    /// Create a cover of an existing clip.
    /// Posts to `/api/generate/v2-web/` with `cover_clip_id` set. The legacy
    /// `task: "cover"` field is gone in v2-web; we still don't have a fresh
    /// web-app capture for the cover flow, so this is a best-guess port — if
    /// the API rejects, we'll need to capture a real cover request and add
    /// any missing required fields (e.g. cover_start_s/cover_end_s).
    pub async fn cover(
        &self,
        clip_id: &str,
        model_key: &str,
        tags: Option<&str>,
        title: Option<&str>,
    ) -> Result<Vec<Clip>, CliError> {
        let mut req = GenerateRequest::new(model_key, "cover");
        req.tags = tags.map(String::from);
        req.cover_clip_id = Some(clip_id.to_string());
        // v2-web rejects a null `params.title` with HTTP 422, so it must always
        // be a string. Use the caller's title, else inherit the source clip's
        // title (matching the web app), else a generic fallback.
        req.title = Some(self.resolve_title(clip_id, title, "Cover").await);
        self.generate(&req).await
    }
}
