use serde::{Deserialize, Serialize};

// --- Billing / Account ---

#[derive(Debug, Deserialize, Serialize)]
pub struct BillingInfo {
    pub credits: u64,
    pub total_credits_left: u64,
    pub monthly_usage: u64,
    pub monthly_limit: u64,
    pub is_active: bool,
    #[serde(default)]
    pub is_past_due: bool,
    /// Suno no longer nests the active plan under a `plan` key. Instead
    /// `subscription_type` is `false` on the free tier or the plan's
    /// `plan_key` string (e.g. `"pro"`) when subscribed, and the full catalog
    /// of plans (with pricing/features) is listed separately in `plans`.
    #[serde(default)]
    pub subscription_type: SubscriptionType,
    pub models: Vec<Model>,
    #[serde(default)]
    pub plans: Vec<PlanOption>,
    #[serde(default)]
    pub accessible_features: Vec<Feature>,
    #[serde(default)]
    pub remaster_model_types: Vec<RemasterModelInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SubscriptionType {
    /// No active paid subscription (free tier).
    None(bool),
    /// Active plan, identified by its `plan_key` (e.g. "pro").
    Plan(String),
}

impl Default for SubscriptionType {
    fn default() -> Self {
        SubscriptionType::None(false)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PlanOption {
    pub plan_key: String,
    pub name: String,
    #[serde(default)]
    pub usage_plan_features: Vec<Feature>,
}

impl BillingInfo {
    /// Human-readable name of the account's current plan, resolved from
    /// `subscription_type` against the `plans` catalog (falls back to the
    /// raw plan key, or "Free" when there's no active subscription).
    pub fn plan_name(&self) -> String {
        match &self.subscription_type {
            SubscriptionType::Plan(key) => self
                .plans
                .iter()
                .find(|p| &p.plan_key == key)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| key.clone()),
            SubscriptionType::None(_) => self
                .plans
                .iter()
                .find(|p| p.plan_key == "free")
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Free".to_string()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Feature {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Model {
    pub name: String,
    pub external_key: String,
    pub can_use: bool,
    pub is_default_model: bool,
    pub description: String,
    #[serde(default)]
    pub max_lengths: MaxLengths,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct MaxLengths {
    #[serde(default)]
    pub title: u32,
    #[serde(default)]
    pub prompt: u32,
    #[serde(default)]
    pub tags: u32,
    #[serde(default)]
    pub negative_tags: u32,
    #[serde(default)]
    pub gpt_description_prompt: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RemasterModelInfo {
    pub name: String,
    pub external_key: String,
    pub is_default_model: bool,
    /// Suno's billing/info response for remaster models does NOT include this
    /// field — keep it optional so deserialization succeeds.
    #[serde(default)]
    pub can_use: bool,
}

// --- Clips / Feed ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Clip {
    pub id: String,
    pub title: String,
    pub status: String,
    pub model_name: String,
    pub audio_url: Option<String>,
    pub video_url: Option<String>,
    pub image_url: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub play_count: u64,
    #[serde(default)]
    pub upvote_count: u64,
    #[serde(default)]
    pub metadata: ClipMetadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ClipMetadata {
    pub tags: Option<String>,
    pub prompt: Option<String>,
    pub duration: Option<f64>,
    pub avg_bpm: Option<f64>,
    #[serde(default)]
    pub has_stem: bool,
    #[serde(default)]
    pub is_remix: bool,
    #[serde(default)]
    pub make_instrumental: bool,
    #[serde(rename = "type")]
    pub clip_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FeedResponse {
    #[serde(default)]
    pub clips: Vec<Clip>,
    #[allow(dead_code)]
    pub next_cursor: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub has_more: bool,
}

// --- Feed V3 Request ---

#[derive(Debug, Serialize)]
pub struct FeedV3Request {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<FeedFilters>,
}

#[derive(Debug, Serialize)]
pub struct FeedFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "searchText")]
    pub search_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "fullSong")]
    pub full_song: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stem: Option<FilterPresence>,
}

#[derive(Debug, Serialize)]
pub struct FilterPresence {
    pub presence: String,
}

// --- Generation ---
//
// Schema captured from a real Suno web-app POST to `/api/generate/v2-web/`
// on 2026-04-07 (see API_INTELLIGENCE.md). The old `/api/generate/v2/` path
// returns `Token validation failed` since Suno started routing creates
// through `v2-web` exclusively. Most of the new `null` fields are pure
// placeholders the web app sends regardless of mode — they MUST be present
// or pydantic returns `missing field`.

#[derive(Debug, Serialize)]
pub struct GenerateRequest {
    /// Captcha/anti-bot token. Generation normally needs a fresh hCaptcha
    /// response from the browser-backed solver unless the caller supplies one.
    pub token: Option<String>,
    pub generation_type: String,
    pub title: Option<String>,
    pub tags: Option<String>,
    /// Always present, defaults to "" (empty string, NOT null).
    pub negative_tags: String,
    pub mv: String,
    pub prompt: String,
    pub make_instrumental: bool,
    pub user_uploaded_images_b64: Option<String>,
    pub metadata: GenerateMetadata,
    /// Always present, empty array unless overriding model fields.
    pub override_fields: Vec<serde_json::Value>,
    pub cover_clip_id: Option<String>,
    pub cover_start_s: Option<f64>,
    pub cover_end_s: Option<f64>,
    pub persona_id: Option<String>,
    pub artist_clip_id: Option<String>,
    pub artist_start_s: Option<f64>,
    pub artist_end_s: Option<f64>,
    pub continue_clip_id: Option<String>,
    pub continued_aligned_prompt: Option<String>,
    pub continue_at: Option<f64>,
    /// Random UUID generated per request — required.
    pub transaction_uuid: String,
}

impl GenerateRequest {
    /// Build a `GenerateRequest` with all the new-schema placeholder fields
    /// pre-populated (nulls, empty arrays, fresh UUIDs). Callers only need to
    /// override the fields that matter for their command.
    pub fn new(mv: &str, create_mode: &str) -> Self {
        Self {
            token: None,
            generation_type: "TEXT".to_string(),
            title: None,
            tags: None,
            negative_tags: String::new(),
            mv: mv.to_string(),
            prompt: String::new(),
            make_instrumental: false,
            user_uploaded_images_b64: None,
            metadata: GenerateMetadata::new(create_mode),
            override_fields: Vec::new(),
            cover_clip_id: None,
            cover_start_s: None,
            cover_end_s: None,
            persona_id: None,
            artist_clip_id: None,
            artist_start_s: None,
            artist_end_s: None,
            continue_clip_id: None,
            continued_aligned_prompt: None,
            continue_at: None,
            transaction_uuid: uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// Web-app metadata block. All fields are required by the new schema even if
/// they're decorative. `user_tier` is NOT validated server-side (verified with
/// empty string and arbitrary text — both succeed).
#[derive(Debug, Serialize)]
pub struct GenerateMetadata {
    pub web_client_pathname: String,
    pub is_max_mode: bool,
    pub is_mumble: bool,
    pub create_mode: String,
    pub user_tier: String,
    /// Random UUID generated per request — looks decorative but must be present.
    pub create_session_token: String,
    pub disable_volume_normalization: bool,
    /// Control sliders (weirdness / style influence). Optional — only sent
    /// when --weirdness or --style-influence is passed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_sliders: Option<ControlSliders>,
}

impl GenerateMetadata {
    /// Build a metadata block with default web-app values + a fresh session
    /// token. This matches what the real Suno UI sends per generation.
    pub fn new(create_mode: &str) -> Self {
        Self {
            web_client_pathname: "/create".to_string(),
            is_max_mode: false,
            is_mumble: false,
            create_mode: create_mode.to_string(),
            user_tier: String::new(),
            create_session_token: uuid::Uuid::new_v4().to_string(),
            disable_volume_normalization: false,
            control_sliders: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ControlSliders {
    /// Weirdness: 0.0-1.0 (maps from 0-100 in UI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weirdness_constraint: Option<f64>,
    /// Style weight: 0.0-1.0 (maps from 0-90 in UI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_weight: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateResponse {
    #[serde(default)]
    pub clips: Vec<Clip>,
    #[allow(dead_code)]
    pub status: Option<String>,
}

// --- Lyrics ---

#[derive(Debug, Deserialize)]
pub struct LyricsSubmitResponse {
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LyricsResult {
    pub text: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub error_message: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

// --- Aligned / Timed Lyrics ---

#[derive(Debug, Deserialize, Serialize)]
pub struct AlignedWord {
    pub word: String,
    pub start_s: f64,
    pub end_s: f64,
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub p_align: Option<f64>,
}

// --- Captcha Check ---

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CaptchaCheckResponse {
    #[serde(default)]
    pub captcha_required: bool,
    #[serde(default)]
    pub captcha_url: Option<String>,
}

// --- Set Metadata ---

#[derive(Debug, Serialize)]
pub struct SetMetadataRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lyrics: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_image_cover: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_video_cover: Option<bool>,
}

// --- Set Visibility ---

#[derive(Debug, Serialize)]
pub struct SetVisibilityRequest {
    pub is_public: bool,
}

// --- Concat ---

#[derive(Debug, Serialize)]
pub struct ConcatRequest {
    pub clip_id: String,
}

// --- Persona ---

#[derive(Debug, Deserialize, Serialize)]
pub struct PersonaResponse {
    pub persona: PersonaInfo,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PersonaInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub image_s3_id: Option<String>,
    #[serde(default)]
    pub user_display_name: Option<String>,
    #[serde(default)]
    pub user_handle: Option<String>,
    #[serde(default)]
    pub persona_clips: Vec<serde_json::Value>,
}
