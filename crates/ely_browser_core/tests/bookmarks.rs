use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, ELYBOOKMARKS_SCHEMA_VERSION, InitialBrowserConfig};
use ely_domain::{BookmarkId, CommandIntent, CommandScope, DomainError, ProfileKind, UrlText};
use serde_json::Value;

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
    assert_eq!(bookmark.thumbnail_key(), None);
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
fn bookmark_thumbnail_key_can_be_set_and_cleared() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);
    let bookmark_id = core.bookmark_active_tab()?;

    core.set_bookmark_thumbnail_key(&bookmark_id, " screenshots/example.avif ")?;
    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.bookmarks[0].thumbnail_key(), Some("screenshots/example.avif"));

    core.clear_bookmark_thumbnail_key(&bookmark_id)?;
    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.bookmarks[0].thumbnail_key(), None);
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
    let Err(thumbnail_error) = core.set_bookmark_thumbnail_key(&bookmark_id, " ") else {
        return Err("expected empty bookmark thumbnail key error".into());
    };
    assert_eq!(
        thumbnail_error,
        CoreError::Domain(DomainError::EmptyField { field: "bookmark thumbnail key" })
    );
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

#[test]
fn export_bookmarks_package_json_contains_active_profile_bookmarks() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.snapshot()?.active_profile_id;
    core.open_tab(UrlText::parse("https://example.com/research")?);
    let bookmark_id = core.bookmark_active_tab()?;
    core.set_bookmark_collection_name(&bookmark_id, "Research")?;
    core.set_bookmark_tags(&bookmark_id, vec!["rust".to_string(), "gpui".to_string()])?;
    core.set_bookmark_note(&bookmark_id, "Servo reference")?;
    core.set_bookmark_thumbnail_key(&bookmark_id, "screenshots/example.avif")?;

    core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;
    core.open_tab(UrlText::parse("https://example.com/personal")?);
    core.bookmark_active_tab()?;
    core.select_profile(&default_profile_id)?;

    let package = core.export_bookmarks_package()?;
    let package_json = core.export_bookmarks_package_json()?;
    let value: Value = serde_json::from_str(&package_json)?;
    let bookmarks = value["bookmarks"].as_array().ok_or("missing bookmarks array")?;

    assert_eq!(package.version(), ELYBOOKMARKS_SCHEMA_VERSION);
    assert_eq!(package.bookmark_count(), 1);
    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0]["title"], "example.com");
    assert_eq!(bookmarks[0]["url"], "https://example.com/research");
    assert_eq!(bookmarks[0]["collection_name"], "Research");
    assert_eq!(bookmarks[0]["space_name"], "Work");
    assert_eq!(bookmarks[0]["tags"], serde_json::json!(["rust", "gpui"]));
    assert_eq!(bookmarks[0]["note"], "Servo reference");
    assert_eq!(bookmarks[0]["thumbnail_key"], "screenshots/example.avif");
    Ok(())
}

#[test]
fn import_bookmarks_package_json_creates_active_profile_bookmarks() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    source.open_tab(UrlText::parse("https://example.com/research")?);
    let bookmark_id = source.bookmark_active_tab()?;
    source.set_bookmark_collection_name(&bookmark_id, "Research")?;
    source.set_bookmark_tags(&bookmark_id, vec!["rust".to_string()])?;
    source.set_bookmark_note(&bookmark_id, "Read later")?;
    let package_json = source.export_bookmarks_package_json()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_profile_id = target.snapshot()?.active_profile_id;
    let active_space_id = target.snapshot()?.active_space_id;
    let summary = target.import_bookmarks_package_json(&package_json)?;
    let snapshot = target.snapshot()?;

    assert_eq!(summary.imported(), 1);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(summary.label(), "Imported 1 bookmarks, skipped 0");
    assert_eq!(snapshot.bookmarks.len(), 1);
    assert_eq!(snapshot.bookmarks[0].profile_id(), &active_profile_id);
    assert_eq!(snapshot.bookmarks[0].space_id(), &active_space_id);
    assert_eq!(snapshot.bookmarks[0].collection_name(), "Research");
    assert_eq!(snapshot.bookmarks[0].tags(), &["rust".to_string()]);
    assert_eq!(snapshot.bookmarks[0].note(), Some("Read later"));
    Ok(())
}

#[test]
fn import_bookmarks_package_skips_duplicate_active_profile_urls() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    source.open_tab(UrlText::parse("https://example.com/research")?);
    source.bookmark_active_tab()?;
    let package_json = source.export_bookmarks_package_json()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    target.open_tab(UrlText::parse("https://example.com/research")?);
    target.bookmark_active_tab()?;
    let summary = target.import_bookmarks_package_json(&package_json)?;

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.skipped(), 1);
    assert_eq!(target.snapshot()?.bookmarks.len(), 1);
    Ok(())
}

#[test]
fn import_bookmarks_package_rejects_unknown_fields() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let package_json = r#"{
        "version": 1,
        "unexpected": true,
        "bookmarks": []
    }"#;

    let Err(error) = core.import_bookmarks_package_json(package_json) else {
        return Err("expected invalid bookmark package".into());
    };

    assert!(matches!(error, CoreError::InvalidBookmarkPackage { .. }));
    Ok(())
}

#[test]
fn bookmark_file_commands_open_bookmarks_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">export-bookmarks");
    let export_intent = core.submit_command()?;
    assert_eq!(export_intent, Some(CommandIntent::Command("export-bookmarks".to_string())));
    assert_eq!(core.active_tab()?.url().as_str(), "ely://bookmarks");

    core.set_command_query(">import-bookmarks");
    let import_intent = core.submit_command()?;
    assert_eq!(import_intent, Some(CommandIntent::Command("import-bookmarks".to_string())));
    assert_eq!(core.active_tab()?.title(), "Bookmarks");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}
