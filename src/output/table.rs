use comfy_table::{ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL};

use crate::api::types::{BillingInfo, Clip, LyricsResult, Model, PersonaInfo};

pub fn clips(clips: &[Clip]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["ID", "Title", "Status", "Model", "Duration", "Tags"]);

    for c in clips {
        let duration = c
            .metadata
            .duration
            .map(|d| format!("{:.0}s", d))
            .unwrap_or_default();
        let tags = c.metadata.tags.as_deref().unwrap_or("-");
        let short_id = if c.id.len() > 8 { &c.id[..8] } else { &c.id };
        table.add_row(vec![
            short_id,
            &c.title,
            &c.status,
            &c.model_name,
            &duration,
            tags,
        ]);
    }
    println!("{table}");
}

pub fn billing(info: &BillingInfo) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec!["Field", "Value"]);

    table.add_row(vec!["Plan", &info.plan_name()]);
    table.add_row(vec!["Credits Left", &info.total_credits_left.to_string()]);
    table.add_row(vec![
        "Monthly Usage",
        &format!("{} / {}", info.monthly_usage, info.monthly_limit),
    ]);
    table.add_row(vec!["Active", &info.is_active.to_string()]);
    println!("{table}");
}

pub fn models(models: &[Model]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            "Name",
            "Key",
            "Default",
            "Max Prompt",
            "Max Tags",
            "Description",
        ]);

    for m in models {
        if !m.can_use {
            continue;
        }
        table.add_row(vec![
            &m.name,
            &m.external_key,
            &if m.is_default_model {
                "yes".into()
            } else {
                String::new()
            },
            &m.max_lengths.prompt.to_string(),
            &m.max_lengths.tags.to_string(),
            &m.description,
        ]);
    }
    println!("{table}");
}

pub fn lyrics(result: &LyricsResult) {
    println!("Title: {}\n", result.title);
    println!("{}", result.text);
    if !result.tags.is_empty() {
        println!("\nSuggested style: {}", result.tags.join(", "));
    }
}

pub fn clip_detail(clip: &Clip) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec!["Field", "Value"]);

    table.add_row(vec!["ID", &clip.id]);
    table.add_row(vec!["Title", &clip.title]);
    table.add_row(vec!["Status", &clip.status]);
    table.add_row(vec!["Model", &clip.model_name]);
    table.add_row(vec!["Created", &clip.created_at]);
    table.add_row(vec![
        "Duration",
        &clip
            .metadata
            .duration
            .map(|d| format!("{:.1}s", d))
            .unwrap_or_else(|| "-".into()),
    ]);
    table.add_row(vec!["Tags", clip.metadata.tags.as_deref().unwrap_or("-")]);
    table.add_row(vec![
        "BPM",
        &clip
            .metadata
            .avg_bpm
            .map(|b| format!("{:.0}", b))
            .unwrap_or_else(|| "-".into()),
    ]);
    table.add_row(vec!["Plays", &clip.play_count.to_string()]);
    table.add_row(vec!["Upvotes", &clip.upvote_count.to_string()]);
    table.add_row(vec!["Has Stems", &clip.metadata.has_stem.to_string()]);
    table.add_row(vec![
        "Instrumental",
        &clip.metadata.make_instrumental.to_string(),
    ]);

    if let Some(ref url) = clip.audio_url {
        table.add_row(vec!["Audio URL", url]);
    }
    if let Some(ref url) = clip.video_url {
        table.add_row(vec!["Video URL", url]);
    }
    if let Some(ref prompt) = clip.metadata.prompt {
        let truncated = if prompt.len() > 200 {
            format!("{}...", &prompt[..200])
        } else {
            prompt.clone()
        };
        table.add_row(vec!["Lyrics", &truncated]);
    }

    println!("{table}");
}

pub fn persona(info: &PersonaInfo) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec!["Field", "Value"]);

    table.add_row(vec!["ID", &info.id]);
    table.add_row(vec!["Name", &info.name]);
    table.add_row(vec![
        "Description",
        info.description.as_deref().unwrap_or("-"),
    ]);
    if let Some(ref owner) = info.user_display_name {
        table.add_row(vec!["Owner", owner]);
    }
    if let Some(ref handle) = info.user_handle {
        table.add_row(vec!["Handle", handle]);
    }
    table.add_row(vec!["Clips", &info.persona_clips.len().to_string()]);

    println!("{table}");
}
