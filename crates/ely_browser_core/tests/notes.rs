use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, CommandScope, NoteId, NoteTarget, ProfileKind, UrlText};

#[test]
fn url_note_records_active_page_context() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/notes-url")?);
    let active_profile_id = core.active_tab()?.profile_id().clone();
    let active_space_id = core.active_tab()?.space_id().clone();

    let note_id = core.save_active_url_note("# Research\n- fact")?;
    let snapshot = core.snapshot()?;
    let [note] = snapshot.notes.as_slice() else {
        return Err(format!("expected 1 note, got {}", snapshot.notes.len()).into());
    };

    assert_eq!(snapshot.active_tab_id, tab_id);
    assert_eq!(note.id(), &note_id);
    assert_eq!(note.profile_id(), &active_profile_id);
    assert_eq!(note.space_id(), &active_space_id);
    assert_eq!(note.target(), &NoteTarget::Url(UrlText::parse("https://example.com/notes-url")?));
    assert_eq!(note.title(), "example.com");
    assert_eq!(note.body(), "# Research\n- fact");
    Ok(())
}

#[test]
fn tab_note_records_tab_target() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/tab-note")?);

    core.save_active_tab_note("- tab detail")?;
    let snapshot = core.snapshot()?;
    let [note] = snapshot.notes.as_slice() else {
        return Err(format!("expected 1 note, got {}", snapshot.notes.len()).into());
    };

    assert_eq!(note.target(), &NoteTarget::Tab(tab_id));
    assert_eq!(note.target_label(), "Tab note");
    assert_eq!(note.source_url().as_str(), "https://example.com/tab-note");
    Ok(())
}

#[test]
fn url_note_command_updates_existing_note() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);

    core.set_command_query(">note first");
    core.submit_command()?;
    core.set_command_query(">note second");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("note second".to_string())));
    assert_eq!(snapshot.command_query, "");
    assert_eq!(snapshot.notes.len(), 1);
    assert_eq!(snapshot.notes[0].body(), "second");
    Ok(())
}

#[test]
fn tab_note_command_records_tab_target() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.open_tab(UrlText::parse("https://example.com/pinned-detail")?);

    core.set_command_query(">tab-note - pinned detail");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("tab-note - pinned detail".to_string())));
    assert_eq!(snapshot.command_query, "");
    assert_eq!(snapshot.notes.len(), 1);
    assert_eq!(snapshot.notes[0].target(), &NoteTarget::Tab(tab_id));
    assert_eq!(snapshot.notes[0].body(), "- pinned detail");
    Ok(())
}

#[test]
fn notes_scoped_search_opens_matching_note_url() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/matching-note")?);
    core.save_active_url_note("# Citation\nunique cue")?;
    core.open_tab(UrlText::parse("https://example.com/other")?);

    core.set_command_query("@notes unique cue");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Notes,
            query: "unique cue".to_string()
        })
    );
    assert_eq!(core.active_tab()?.url().as_str(), "https://example.com/matching-note");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn remove_note_entry_removes_saved_note() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/remove-note")?);
    let note_id = core.save_active_url_note("remove me")?;

    core.remove_note_entry(&note_id)?;
    let snapshot = core.snapshot()?;

    assert!(snapshot.notes.is_empty());
    Ok(())
}

#[test]
fn missing_note_remove_returns_error() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let missing_id = NoteId::new();

    let Err(error) = core.remove_note_entry(&missing_id) else {
        return Err("expected missing note error".into());
    };

    assert_eq!(error, ely_browser_core::CoreError::NoteNotFound { id: missing_id });
    Ok(())
}

#[test]
fn notes_stay_with_active_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.active_tab()?.profile_id().clone();
    let personal_profile_id = core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;

    core.open_tab(UrlText::parse("https://example.com/personal-note")?);
    core.save_active_url_note("private cue")?;
    core.select_profile(&default_profile_id)?;

    core.set_command_query("@notes private cue");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Notes,
            query: "private cue".to_string()
        })
    );
    assert_eq!(core.active_tab()?.profile_id(), &default_profile_id);
    assert_ne!(core.active_tab()?.profile_id(), &personal_profile_id);
    assert!(snapshot.notes.is_empty());
    assert_eq!(snapshot.command_query, "@notes private cue");
    Ok(())
}

#[test]
fn open_notes_command_opens_notes_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">open-notes");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("open-notes".to_string())));
    assert_eq!(active_tab.title(), "Notes");
    assert_eq!(active_tab.url().as_str(), "ely://notes");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}
