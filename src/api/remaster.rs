use super::SunoClient;
use super::types::{Clip, GenerateRequest};
use crate::errors::CliError;

impl SunoClient {
    /// Remaster a clip with a different model version.
    /// Posts to `/api/generate/v2-web/` with the remaster model key and
    /// `cover_clip_id` pointing to the original. As with `cover()`, this is
    /// a best-guess port pending a real captured remaster request.
    pub async fn remaster(
        &self,
        clip_id: &str,
        remaster_model_key: &str,
        title: Option<&str>,
    ) -> Result<Vec<Clip>, CliError> {
        let mut req = GenerateRequest::new(remaster_model_key, "remaster");
        req.cover_clip_id = Some(clip_id.to_string());
        // v2-web rejects a null `params.title` with HTTP 422; always send a
        // string (caller override, else the source clip's title, else fallback).
        req.title = Some(self.resolve_title(clip_id, title, "Remaster").await);
        self.generate(&req).await
    }
}
