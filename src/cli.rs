use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "suno",
    version,
    about = "Suno AI music generation CLI — v5.5 support"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output JSON (auto-detected when piped)
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress non-essential output
    #[arg(long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate music with custom lyrics, tags, and controls
    Generate(GenerateArgs),

    /// Generate music from a text description (Suno writes lyrics)
    Describe(DescribeArgs),

    /// Generate lyrics only (free, no credits used)
    Lyrics(LyricsArgs),

    /// Continue/extend a clip from a timestamp
    Extend(ExtendArgs),

    /// Concatenate clips into a full song
    Concat(ConcatArgs),

    /// Create a cover of an existing clip
    Cover(CoverArgs),

    /// Remaster a clip with a different model
    Remaster(RemasterArgs),

    /// Extract stems (vocals, instruments) from a clip
    Stems(StemsArgs),

    /// Show detailed info for a single clip
    Info(InfoArgs),

    /// View a voice persona
    Persona(PersonaArgs),

    /// List your songs
    List(ListArgs),

    /// Search your songs by title or tags
    Search(SearchArgs),

    /// Check generation status
    Status(StatusArgs),

    /// Download audio/video for clip(s)
    Download(DownloadArgs),

    /// Delete/trash a clip
    Delete(DeleteArgs),

    /// Update clip title, lyrics, or caption
    Set(SetArgs),

    /// Toggle clip public/private
    Publish(PublishArgs),

    /// Get word-level timestamped lyrics
    TimedLyrics(TimedLyricsArgs),

    /// Show credit balance and plan info
    Credits,

    /// List available models
    Models,

    /// Set up authentication
    Auth(AuthArgs),

    /// Manage configuration
    Config(ConfigArgs),

    /// Machine-readable capabilities (for AI agents)
    AgentInfo,

    /// Install the agent skill (teaches Claude Code / coding agents how to use this CLI)
    InstallSkill(InstallSkillArgs),

    /// Self-update from GitHub Releases
    Update(UpdateArgs),
}

#[derive(clap::Args)]
pub struct UpdateArgs {
    /// Check for a new version without installing
    #[arg(long)]
    pub check: bool,
}

#[derive(clap::Args)]
pub struct GenerateArgs {
    /// Song title
    #[arg(short, long)]
    pub title: Option<String>,

    /// Style tags (comma-separated): "pop, synths, upbeat"
    #[arg(long)]
    pub tags: Option<String>,

    /// Exclude styles (comma-separated): "metal, heavy"
    #[arg(long)]
    pub exclude: Option<String>,

    /// Lyrics text (with [Verse], [Chorus] tags)
    #[arg(short, long, conflicts_with = "lyrics_file")]
    pub lyrics: Option<String>,

    /// Read lyrics from file
    #[arg(long)]
    pub lyrics_file: Option<String>,

    /// Model version
    #[arg(short, long, default_value = "v5.5")]
    pub model: ModelVersion,

    /// Vocal gender
    #[arg(long)]
    pub vocal: Option<VocalGender>,

    /// Weirdness level (0-100)
    #[arg(long)]
    pub weirdness: Option<f64>,

    /// Style influence strength (0-100)
    #[arg(long)]
    pub style_influence: Option<f64>,

    /// Variation level
    #[arg(long)]
    pub variation: Option<VariationCategory>,

    /// Generate instrumental only (no vocals)
    #[arg(long)]
    pub instrumental: bool,

    /// Wait for generation to complete
    #[arg(short, long)]
    pub wait: bool,

    /// Download output to directory after generation
    #[arg(long)]
    pub download: Option<String>,

    /// hCaptcha token (overrides the auto-solver)
    #[arg(long)]
    pub token: Option<String>,

    /// Skip the built-in hCaptcha auto-solver. Useful for headless servers
    /// where you supply --token directly (e.g. from a 2Captcha solution).
    #[arg(long)]
    pub no_captcha: bool,

    /// Voice persona ID (generates with your custom voice)
    #[arg(long)]
    pub persona: Option<String>,
}

#[derive(clap::Args)]
pub struct DescribeArgs {
    /// Description of the song you want
    #[arg(short, long)]
    pub prompt: String,

    /// Song title
    #[arg(short, long)]
    pub title: Option<String>,

    /// Style tags (optional, guides the generation)
    #[arg(long)]
    pub tags: Option<String>,

    /// Model version
    #[arg(short, long, default_value = "v5.5")]
    pub model: ModelVersion,

    /// Vocal gender
    #[arg(long)]
    pub vocal: Option<VocalGender>,

    /// Weirdness level (0-100)
    #[arg(long)]
    pub weirdness: Option<f64>,

    /// Style influence strength (0-100)
    #[arg(long)]
    pub style_influence: Option<f64>,

    /// Generate instrumental only
    #[arg(long)]
    pub instrumental: bool,

    /// Wait for generation to complete
    #[arg(short, long)]
    pub wait: bool,

    /// Download output to directory
    #[arg(long)]
    pub download: Option<String>,

    /// Skip the built-in hCaptcha auto-solver
    #[arg(long)]
    pub no_captcha: bool,

    /// Voice persona ID (generates with your custom voice)
    #[arg(long)]
    pub persona: Option<String>,
}

#[derive(clap::Args)]
pub struct LyricsArgs {
    /// What the song should be about
    #[arg(short, long)]
    pub prompt: String,
}

#[derive(clap::Args)]
pub struct ExtendArgs {
    /// Clip ID to extend
    pub clip_id: String,

    /// Timestamp in seconds to continue from
    #[arg(long)]
    pub at: f64,

    /// New lyrics for the extension
    #[arg(long)]
    pub lyrics: Option<String>,

    /// Style tags
    #[arg(long)]
    pub tags: Option<String>,

    /// Wait for completion
    #[arg(short, long)]
    pub wait: bool,
}

#[derive(clap::Args)]
pub struct ConcatArgs {
    /// Clip ID to concatenate into a full song
    pub clip_id: String,

    /// Wait for completion
    #[arg(short, long)]
    pub wait: bool,
}

#[derive(clap::Args)]
pub struct CoverArgs {
    /// Clip ID to create a cover of
    pub clip_id: String,

    /// Style tags for the cover
    #[arg(long)]
    pub tags: Option<String>,

    /// Model version for the cover
    #[arg(short, long, default_value = "v5.5")]
    pub model: ModelVersion,

    /// Wait for completion
    #[arg(short, long)]
    pub wait: bool,

    /// Download output to directory
    #[arg(long)]
    pub download: Option<String>,
}

#[derive(clap::Args)]
pub struct RemasterArgs {
    /// Clip ID to remaster
    pub clip_id: String,

    /// Remaster model version
    #[arg(long, default_value = "v5.5")]
    pub model: RemasterModel,

    /// Wait for completion
    #[arg(short, long)]
    pub wait: bool,

    /// Download output to directory
    #[arg(long)]
    pub download: Option<String>,
}

#[derive(clap::Args)]
pub struct InfoArgs {
    /// Clip ID to inspect
    pub id: String,
}

#[derive(clap::Args)]
pub struct PersonaArgs {
    /// Persona ID to view
    pub id: String,
}

#[derive(clap::Args)]
pub struct StemsArgs {
    /// Clip ID to extract stems from
    pub clip_id: String,

    /// Wait for completion
    #[arg(short, long)]
    pub wait: bool,
}

#[derive(clap::Args)]
pub struct ListArgs {
    /// Page number (0-indexed)
    #[arg(short, long, default_value = "0")]
    pub page: u32,
}

#[derive(clap::Args)]
pub struct SearchArgs {
    /// Search query (matches title and tags)
    pub query: String,
}

#[derive(clap::Args)]
pub struct DeleteArgs {
    /// Clip ID(s) to delete
    pub ids: Vec<String>,

    /// Skip confirmation
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(clap::Args)]
pub struct StatusArgs {
    /// Clip ID(s) to check
    pub ids: Vec<String>,
}

#[derive(clap::Args)]
pub struct DownloadArgs {
    /// Clip ID(s) to download
    pub ids: Vec<String>,

    /// Output directory
    #[arg(short, long, default_value = ".")]
    pub output: String,

    /// Download video instead of audio
    #[arg(long)]
    pub video: bool,
}

#[derive(clap::Args)]
pub struct SetArgs {
    /// Clip ID to update
    pub id: String,

    /// New title
    #[arg(long)]
    pub title: Option<String>,

    /// New lyrics text
    #[arg(long)]
    pub lyrics: Option<String>,

    /// Read lyrics from file
    #[arg(long)]
    pub lyrics_file: Option<String>,

    /// New caption
    #[arg(long)]
    pub caption: Option<String>,

    /// Remove custom cover image
    #[arg(long)]
    pub remove_cover: bool,
}

#[derive(clap::Args)]
pub struct PublishArgs {
    /// Clip ID(s)
    pub ids: Vec<String>,

    /// Make public (default) or --private
    #[arg(long)]
    pub private: bool,
}

#[derive(clap::Args)]
pub struct TimedLyricsArgs {
    /// Clip ID
    pub id: String,

    /// Output as LRC format
    #[arg(long)]
    pub lrc: bool,
}

#[derive(clap::Args)]
pub struct AuthArgs {
    /// Auto-extract from browser (recommended)
    #[arg(long)]
    pub login: bool,

    /// Force-refresh the JWT via the stored Clerk session cookie. Use this
    /// when the CLI returns `auth_expired` or `Token validation failed`
    /// without requiring a full re-login from the browser.
    #[arg(long)]
    pub refresh: bool,

    /// JWT token (manual fallback)
    #[arg(long)]
    pub jwt: Option<String>,

    /// Clerk __client cookie (manual fallback for headless servers)
    ///
    /// Accepts either the raw __client value or a full browser Cookie header.
    #[arg(long)]
    pub cookie: Option<String>,

    /// Device ID
    #[arg(long)]
    pub device: Option<String>,

    /// Remove stored authentication
    #[arg(long)]
    pub logout: bool,
}

#[derive(clap::Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(clap::Args)]
pub struct InstallSkillArgs {
    /// Target coding agent
    #[arg(long, default_value = "claude")]
    pub target: SkillTarget,

    /// Custom output path (overrides --target default)
    #[arg(long)]
    pub path: Option<String>,

    /// Overwrite existing skill file
    #[arg(short, long)]
    pub force: bool,

    /// Print the skill content to stdout instead of writing
    #[arg(long)]
    pub print: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum SkillTarget {
    /// Claude Code (~/.claude/skills/suno/SKILL.md)
    Claude,
    /// Cursor (.cursor/rules/suno.mdc in current dir)
    Cursor,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set { key: String, value: String },
    /// Validate configuration
    Check,
}

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum ModelVersion {
    #[value(name = "v5.5")]
    #[default]
    V55,
    #[value(name = "v5")]
    V5,
    #[value(name = "v4.5+")]
    V45Plus,
    #[value(name = "v4.5")]
    V45,
    #[value(name = "v4.5-all")]
    V45All,
    #[value(name = "v4")]
    V4,
    #[value(name = "v3.5")]
    V35,
    #[value(name = "v3")]
    V3,
    #[value(name = "v2")]
    V2,
}

impl ModelVersion {
    pub fn to_api_key(&self) -> &'static str {
        match self {
            Self::V55 => "chirp-fenix",
            Self::V5 => "chirp-crow",
            Self::V45Plus => "chirp-bluejay",
            Self::V45 => "chirp-auk",
            Self::V45All => "chirp-auk-turbo",
            Self::V4 => "chirp-v4",
            Self::V35 => "chirp-v3-5",
            Self::V3 => "chirp-v3-0",
            Self::V2 => "chirp-v2-xxl-alpha",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::V55 => "v5.5",
            Self::V5 => "v5",
            Self::V45Plus => "v4.5+",
            Self::V45 => "v4.5",
            Self::V45All => "v4.5-all",
            Self::V4 => "v4",
            Self::V35 => "v3.5",
            Self::V3 => "v3",
            Self::V2 => "v2",
        }
    }
}

#[derive(ValueEnum, Clone, Debug)]
pub enum VocalGender {
    Male,
    Female,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum VariationCategory {
    High,
    Normal,
    Subtle,
}

impl VariationCategory {
    #[allow(dead_code)]
    pub fn to_api_value(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Normal => "normal",
            Self::Subtle => "subtle",
        }
    }
}

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum RemasterModel {
    #[value(name = "v5.5")]
    #[default]
    V55,
    #[value(name = "v5")]
    V5,
    #[value(name = "v4.5+")]
    V45Plus,
}

impl RemasterModel {
    pub fn to_api_key(&self) -> &'static str {
        match self {
            Self::V55 => "chirp-flounder",
            Self::V5 => "chirp-carp",
            Self::V45Plus => "chirp-bass",
        }
    }
}
