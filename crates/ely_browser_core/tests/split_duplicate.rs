use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{CommandIntent, MAX_SPLIT_PANES, UrlText};

#[test]
fn duplicate_split_pane_command_copies_active_pane_after_source() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_tab_id = core.open_tab(UrlText::parse("https://example.com/docs")?);
    let split_id = core.split_active_tab_right()?;
    let trailing_tab_id = core.active_tab()?.id().clone();
    core.select_tab(&source_tab_id)?;

    core.set_command_query(">duplicate-split-pane");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot
        .split_layouts
        .iter()
        .find(|layout| layout.id() == &split_id)
        .ok_or("missing split layout")?;
    let duplicated_tab = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &snapshot.active_tab_id)
        .ok_or("missing duplicated tab")?;
    let pane_ids = layout.panes().iter().map(|pane| pane.tab_id()).collect::<Vec<_>>();

    assert_eq!(intent, Some(CommandIntent::Command("duplicate-split-pane".to_string())));
    assert_eq!(snapshot.tabs.iter().filter(|tab| tab.split_id() == Some(&split_id)).count(), 3);
    assert_eq!(layout.pane_count(), 3);
    assert_eq!(pane_ids, vec![&source_tab_id, &snapshot.active_tab_id, &trailing_tab_id]);
    assert_eq!(duplicated_tab.url().as_str(), "https://example.com/docs");
    assert_eq!(duplicated_tab.parent_tab_id(), Some(&source_tab_id));
    assert_eq!(duplicated_tab.split_id(), Some(&split_id));
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn duplicate_split_pane_command_preserves_query_without_active_split() -> Result<(), Box<dyn Error>>
{
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">duplicate-split-pane");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("duplicate-split-pane".to_string())));
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.command_query, ">duplicate-split-pane");
    Ok(())
}

#[test]
fn duplicate_split_pane_rejects_full_layout() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    for _ in 1..MAX_SPLIT_PANES {
        core.split_active_tab_right()?;
    }
    let before = core.snapshot()?;

    let error = match core.duplicate_active_split_pane() {
        Err(error) => error,
        Ok(_) => return Err("full split layout accepted another pane".into()),
    };
    let after = core.snapshot()?;

    assert_eq!(error, CoreError::SplitPaneLimitReached { limit: MAX_SPLIT_PANES });
    assert_eq!(after.tabs.len(), before.tabs.len());
    assert_eq!(after.active_tab_id, before.active_tab_id);
    assert_eq!(after.split_layouts[0].pane_count(), MAX_SPLIT_PANES);
    Ok(())
}

#[test]
fn duplicate_split_pane_refreshes_saved_split_title() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_tab_id = core.open_tab(UrlText::parse("https://example.com/docs")?);
    let split_id = core.split_active_tab_right()?;
    core.save_active_split_view()?;
    core.select_tab(&source_tab_id)?;

    core.duplicate_active_split_pane()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot
        .split_layouts
        .iter()
        .find(|layout| layout.id() == &split_id)
        .ok_or("missing saved split layout")?;

    assert!(layout.saved());
    assert_eq!(layout.title(), "Split View: example.com + example.com");
    assert_eq!(layout.pane_count(), 3);
    Ok(())
}
