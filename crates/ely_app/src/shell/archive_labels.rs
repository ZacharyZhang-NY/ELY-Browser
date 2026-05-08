use std::time::SystemTime;

use ely_domain::{ArchiveSource, ArchivedTab, BrowserTab, Profile, Space};

pub(super) fn archive_detail_label(
    archived_tab: &ArchivedTab,
    spaces: &[Space],
    profiles: &[Profile],
) -> String {
    archive_detail_label_for(archived_tab, spaces, profiles, SystemTime::now())
}

fn archive_detail_label_for(
    archived_tab: &ArchivedTab,
    spaces: &[Space],
    profiles: &[Profile],
    now: SystemTime,
) -> String {
    let tab = archived_tab.tab();
    format!(
        "{} - {} - {} - {} - {}",
        tab.display_url(),
        archive_space_label(tab, spaces),
        archive_profile_label(tab, profiles),
        archive_source_label(archived_tab.source()),
        archived_at_label_for(archived_tab.archived_at(), now)
    )
}

fn archive_source_label(source: &ArchiveSource) -> &'static str {
    match source {
        ArchiveSource::ManualClose => "Closed",
        ArchiveSource::AutoArchive => "Auto archived",
    }
}

fn archive_space_label(tab: &BrowserTab, spaces: &[Space]) -> String {
    spaces
        .iter()
        .find(|space| space.id() == tab.space_id())
        .map(|space| format!("Space: {}", space.name()))
        .unwrap_or_else(|| format!("Space: {}", tab.space_id().as_str()))
}

fn archive_profile_label(tab: &BrowserTab, profiles: &[Profile]) -> String {
    profiles
        .iter()
        .find(|profile| profile.id() == tab.profile_id())
        .map(|profile| format!("Profile: {}", profile.name()))
        .unwrap_or_else(|| format!("Profile: {}", tab.profile_id().as_str()))
}

fn archived_at_label_for(archived_at: SystemTime, now: SystemTime) -> String {
    let Ok(elapsed) = now.duration_since(archived_at) else {
        return "Just now".to_string();
    };

    let seconds = elapsed.as_secs();
    if seconds < 60 {
        return "Just now".to_string();
    }
    if seconds < 3_600 {
        return archive_elapsed_label(seconds / 60, "min");
    }
    if seconds < 86_400 {
        return archive_elapsed_label(seconds / 3_600, "hr");
    }
    if seconds < 604_800 {
        return archive_elapsed_label(seconds / 86_400, "day");
    }
    "Earlier".to_string()
}

fn archive_elapsed_label(value: u64, unit: &str) -> String {
    match value {
        1 => format!("1 {unit} ago"),
        count => format!("{count} {unit}s ago"),
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, time::Duration};

    use ely_domain::{ProfileKind, TabId, UrlText};

    use super::*;

    #[test]
    fn archive_detail_label_includes_space_profile_source_and_time() -> Result<(), Box<dyn Error>> {
        let profile = Profile::new("Research", 0x9fc9a2, ProfileKind::Standard);
        let space = Space::new("Work", "W", 0xf54e00, profile.id().clone(), 0);
        let tab = BrowserTab::new(
            TabId::new(),
            space.id().clone(),
            profile.id().clone(),
            "Research",
            UrlText::parse("https://example.com/research")?,
        );
        let archived_tab = ArchivedTab::new(tab, ArchiveSource::ManualClose);
        let now = archived_tab.archived_at() + Duration::from_secs(7_200);

        assert_eq!(
            archive_detail_label_for(&archived_tab, &[space], &[profile], now),
            "example.com - Space: Work - Profile: Research - Closed - 2 hrs ago"
        );
        Ok(())
    }
}
