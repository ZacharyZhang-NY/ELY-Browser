use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{
    CommandIntent, CommandScope, DomainError, ProfileKind, ReadingListId, ReadingProgress,
    ReadingProgressPercent, UrlText,
};

#[test]
fn save_active_tab_records_reading_list_context() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/long-read")?);
    let active_profile_id = core.active_tab()?.profile_id().clone();
    let active_space_id = core.active_tab()?.space_id().clone();

    let entry_id = core.save_active_tab_to_reading_list()?;
    let snapshot = core.snapshot()?;
    let [entry] = snapshot.reading_list.as_slice() else {
        return Err(
            format!("expected 1 reading list entry, got {}", snapshot.reading_list.len()).into()
        );
    };

    assert_eq!(snapshot.active_tab_id, tab_id);
    assert_eq!(entry.id(), &entry_id);
    assert_eq!(entry.profile_id(), &active_profile_id);
    assert_eq!(entry.space_id(), &active_space_id);
    assert_eq!(entry.title(), "example.com");
    assert_eq!(entry.source_url().as_str(), "https://example.com/long-read");
    assert_eq!(entry.progress(), &ReadingProgress::Unread);
    Ok(())
}

#[test]
fn save_active_tab_reuses_existing_reading_list_entry() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/long-read")?);

    let first_id = core.save_active_tab_to_reading_list()?;
    let second_id = core.save_active_tab_to_reading_list()?;

    assert_eq!(first_id, second_id);
    assert_eq!(core.snapshot()?.reading_list.len(), 1);
    Ok(())
}

#[test]
fn reading_list_progress_updates_entry() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/long-read")?);
    let entry_id = core.save_active_tab_to_reading_list()?;

    core.set_reading_list_progress(
        &entry_id,
        ReadingProgress::InProgress(ReadingProgressPercent::new(42)?),
    )?;
    let snapshot = core.snapshot()?;

    assert_eq!(snapshot.reading_list[0].id(), &entry_id);
    assert_eq!(
        snapshot.reading_list[0].progress(),
        &ReadingProgress::InProgress(ReadingProgressPercent::new(42)?)
    );

    core.set_reading_list_progress(&entry_id, ReadingProgress::Finished)?;
    let snapshot = core.snapshot()?;

    assert_eq!(snapshot.reading_list[0].id(), &entry_id);
    assert_eq!(snapshot.reading_list[0].progress(), &ReadingProgress::Finished);
    Ok(())
}

#[test]
fn active_tab_reading_progress_saves_partial_progress() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/long-read")?);

    let entry_id = core.set_active_tab_reading_progress(ReadingProgressPercent::new(47)?)?;
    let snapshot = core.snapshot()?;

    assert_eq!(snapshot.reading_list.len(), 1);
    assert_eq!(snapshot.reading_list[0].id(), &entry_id);
    assert_eq!(
        snapshot.reading_list[0].progress(),
        &ReadingProgress::InProgress(ReadingProgressPercent::new(47)?)
    );
    Ok(())
}

#[test]
fn reading_progress_command_updates_active_page_entry() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/long-read")?);

    core.set_command_query(">reading-progress 42%");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("reading-progress 42%".to_string())));
    assert_eq!(snapshot.command_query, "");
    assert_eq!(snapshot.reading_list.len(), 1);
    assert_eq!(
        snapshot.reading_list[0].progress(),
        &ReadingProgress::InProgress(ReadingProgressPercent::new(42)?)
    );
    Ok(())
}

#[test]
fn reading_progress_command_rejects_terminal_percent() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/long-read")?);

    core.set_command_query(">reading-progress 100");
    let Err(error) = core.submit_command() else {
        return Err("expected invalid reading progress error".into());
    };

    assert_eq!(
        error,
        CoreError::Domain(DomainError::InvalidReadingProgressPercent { value: "100".to_string() })
    );
    Ok(())
}

#[test]
fn missing_reading_list_progress_update_returns_error() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let missing_id = ReadingListId::new();

    let Err(error) = core.set_reading_list_progress(&missing_id, ReadingProgress::Finished) else {
        return Err("expected missing reading list entry error".into());
    };

    assert_eq!(error, ely_browser_core::CoreError::ReadingListEntryNotFound { id: missing_id });
    Ok(())
}

#[test]
fn remove_reading_list_entry_removes_saved_entry() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/long-read")?);
    let entry_id = core.save_active_tab_to_reading_list()?;

    core.remove_reading_list_entry(&entry_id)?;
    let snapshot = core.snapshot()?;

    assert!(snapshot.reading_list.is_empty());
    Ok(())
}

#[test]
fn missing_reading_list_remove_returns_error() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let missing_id = ReadingListId::new();

    let Err(error) = core.remove_reading_list_entry(&missing_id) else {
        return Err("expected missing reading list entry error".into());
    };

    assert_eq!(error, ely_browser_core::CoreError::ReadingListEntryNotFound { id: missing_id });
    Ok(())
}

#[test]
fn reading_list_scoped_search_opens_matching_entry() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/long-read")?);
    core.save_active_tab_to_reading_list()?;

    core.set_command_query("@reading-list long-read");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::ReadingList,
            query: "long-read".to_string()
        })
    );
    assert_eq!(active_tab.url().as_str(), "https://example.com/long-read");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn reading_list_stays_with_active_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.active_tab()?.profile_id().clone();
    let personal_profile_id = core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;

    core.open_tab(UrlText::parse("https://example.com/personal")?);
    core.save_active_tab_to_reading_list()?;
    core.select_profile(&default_profile_id)?;

    core.set_command_query("@reading-list personal");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::ReadingList,
            query: "personal".to_string()
        })
    );
    assert_eq!(core.active_tab()?.profile_id(), &default_profile_id);
    assert_ne!(core.active_tab()?.profile_id(), &personal_profile_id);
    assert!(snapshot.reading_list.is_empty());
    assert_eq!(snapshot.command_query, "@reading-list personal");
    Ok(())
}

#[test]
fn open_reading_list_command_opens_reading_list_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">open-reading-list");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("open-reading-list".to_string())));
    assert_eq!(active_tab.title(), "Reading List");
    assert_eq!(active_tab.url().as_str(), "ely://reading-list");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}
