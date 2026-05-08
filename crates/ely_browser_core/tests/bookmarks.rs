use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{BookmarkId, CommandIntent, CommandScope, DomainError, ProfileKind, UrlText};

#[test]
fn bookmark_active_tab_records_current_context() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/research")?);
    let active_profile_id = core.active_tab()?.profile_id().clone();
    let active_space_id = core.active_tab()?.space_id().clone();

    let bookmark_id = core.bookmark_active_tab()?;
    let snapshot = core.snapshot()?;
    let [bookmark] = snapshot.bookmarks.as_slice() else {
        return Err(format!("expected 1 bookmark, got {}", snapshot.bookmarks.len()).into());
    };

    assert_eq!(snapshot.active_tab_id, tab_id);
    assert_eq!(bookmark.id(), &bookmark_id);
    assert_eq!(bookmark.profile_id(), &active_profile_id);
    assert_eq!(bookmark.space_id(), &active_space_id);
    assert_eq!(bookmark.collection_name(), "Work");
    assert_eq!(bookmark.title(), "example.com");
    assert_eq!(bookmark.url().as_str(), "https://example.com/research");
    assert!(bookmark.tags().is_empty());
    assert_eq!(bookmark.note(), None);
    Ok(())
}

#[test]
fn bookmark_active_tab_reuses_existing_bookmark() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);

    let first_id = core.bookmark_active_tab()?;
    let second_id = core.bookmark_active_tab()?;

    assert_eq!(first_id, second_id);
    assert_eq!(core.snapshot()?.bookmarks.len(), 1);
    Ok(())
}

#[test]
fn bookmark_metadata_updates_collection_tags_and_note() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);
    let bookmark_id = core.bookmark_active_tab()?;

    core.set_bookmark_collection_name(&bookmark_id, "Research")?;
    core.set_bookmark_tags(&bookmark_id, vec![" rust ".to_string(), "gpui".to_string()])?;
    core.set_bookmark_note(&bookmark_id, " Read with Servo notes ")?;
    let snapshot = core.snapshot()?;
    let [bookmark] = snapshot.bookmarks.as_slice() else {
        return Err(format!("expected 1 bookmark, got {}", snapshot.bookmarks.len()).into());
    };

    assert_eq!(bookmark.collection_name(), "Research");
    assert_eq!(bookmark.tags(), &["rust".to_string(), "gpui".to_string()]);
    assert_eq!(bookmark.note(), Some("Read with Servo notes"));

    core.clear_bookmark_note(&bookmark_id)?;
    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.bookmarks[0].note(), None);
    Ok(())
}

#[test]
fn bookmark_metadata_batch_update_is_atomic() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);
    let bookmark_id = core.bookmark_active_tab()?;

    let Err(error) = core.update_bookmark_metadata(
        &bookmark_id,
        "Research",
        vec!["rust".to_string(), " ".to_string()],
        Some("Read later".to_string()),
    ) else {
        return Err("expected invalid bookmark metadata error".into());
    };
    let snapshot = core.snapshot()?;

    assert_eq!(error, CoreError::Domain(DomainError::EmptyField { field: "bookmark tag" }));
    assert_eq!(snapshot.bookmarks[0].collection_name(), "Work");
    assert!(snapshot.bookmarks[0].tags().is_empty());
    assert_eq!(snapshot.bookmarks[0].note(), None);
    Ok(())
}

#[test]
fn bookmark_metadata_rejects_empty_fields() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);
    let bookmark_id = core.bookmark_active_tab()?;

    let Err(collection_error) = core.set_bookmark_collection_name(&bookmark_id, " ") else {
        return Err("expected empty bookmark collection error".into());
    };
    assert_eq!(
        collection_error,
        CoreError::Domain(DomainError::EmptyField { field: "bookmark collection" })
    );
    let Err(tag_error) =
        core.set_bookmark_tags(&bookmark_id, vec!["rust".to_string(), " ".to_string()])
    else {
        return Err("expected empty bookmark tag error".into());
    };
    assert_eq!(tag_error, CoreError::Domain(DomainError::EmptyField { field: "bookmark tag" }));
    let Err(note_error) = core.set_bookmark_note(&bookmark_id, " ") else {
        return Err("expected empty bookmark note error".into());
    };
    assert_eq!(note_error, CoreError::Domain(DomainError::EmptyField { field: "bookmark note" }));
    Ok(())
}

#[test]
fn bookmark_metadata_requires_known_bookmark() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let unknown_bookmark_id = BookmarkId::new();

    let Err(bookmark_error) = core.set_bookmark_note(&unknown_bookmark_id, "Read later") else {
        return Err("expected unknown bookmark error".into());
    };
    assert_eq!(bookmark_error, CoreError::BookmarkNotFound { id: unknown_bookmark_id });
    Ok(())
}

#[test]
fn bookmarks_scoped_search_opens_matching_bookmark() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);
    core.bookmark_active_tab()?;

    core.set_command_query("@bookmarks research");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Bookmarks,
            query: "research".to_string()
        })
    );
    assert_eq!(active_tab.url().as_str(), "https://example.com/research");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn bookmarks_scoped_search_matches_metadata() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);
    let bookmark_id = core.bookmark_active_tab()?;

    core.set_bookmark_collection_name(&bookmark_id, "Research")?;
    core.set_bookmark_tags(&bookmark_id, vec!["gpui".to_string()])?;
    core.set_bookmark_note(&bookmark_id, "Servo embed reference")?;

    core.set_command_query("@bookmarks gpui");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Bookmarks,
            query: "gpui".to_string()
        })
    );
    assert_eq!(active_tab.url().as_str(), "https://example.com/research");
    Ok(())
}

#[test]
fn bookmarks_stay_with_active_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.active_tab()?.profile_id().clone();
    let personal_profile_id = core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;

    core.open_tab(UrlText::parse("https://example.com/personal")?);
    core.bookmark_active_tab()?;
    core.select_profile(&default_profile_id)?;

    core.set_command_query("@bookmarks personal");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Bookmarks,
            query: "personal".to_string()
        })
    );
    assert_eq!(core.active_tab()?.profile_id(), &default_profile_id);
    assert_ne!(core.active_tab()?.profile_id(), &personal_profile_id);
    assert!(snapshot.bookmarks.is_empty());
    assert_eq!(snapshot.command_query, "@bookmarks personal");
    Ok(())
}

#[test]
fn open_bookmarks_command_opens_bookmarks_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">open-bookmarks");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("open-bookmarks".to_string())));
    assert_eq!(active_tab.title(), "Bookmarks");
    assert_eq!(active_tab.url().as_str(), "ely://bookmarks");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}
