use ely_domain::{BookmarkEntry, BookmarkId};
use gpui::{AppContext, Context, Entity, Window};
use gpui_component::input::InputState;

use super::{ElyShell, ShellState};

#[derive(Clone)]
pub(super) struct PendingBookmarkEdit {
    bookmark_id: BookmarkId,
    collection_input: Entity<InputState>,
    tags_input: Entity<InputState>,
    note_input: Entity<InputState>,
}

impl PendingBookmarkEdit {
    fn new(bookmark: &BookmarkEntry, window: &mut Window, cx: &mut Context<ElyShell>) -> Self {
        let collection_input =
            bookmark_input(bookmark.collection_name().to_string(), "Collection", window, cx);
        let tags_input = bookmark_input(bookmark.tags().join(", "), "Tags", window, cx);
        let note_input =
            bookmark_input(bookmark.note().unwrap_or_default().to_string(), "Note", window, cx);

        Self { bookmark_id: bookmark.id().clone(), collection_input, tags_input, note_input }
    }

    pub(super) fn bookmark_id(&self) -> &BookmarkId {
        &self.bookmark_id
    }

    pub(super) fn collection_input(&self) -> &Entity<InputState> {
        &self.collection_input
    }

    pub(super) fn tags_input(&self) -> &Entity<InputState> {
        &self.tags_input
    }

    pub(super) fn note_input(&self) -> &Entity<InputState> {
        &self.note_input
    }

    pub(super) fn matches(&self, bookmark_id: &BookmarkId) -> bool {
        &self.bookmark_id == bookmark_id
    }

    fn values(&self, cx: &mut Context<ElyShell>) -> (String, Vec<String>, Option<String>) {
        (
            self.collection_input.read(cx).value().to_string(),
            parse_bookmark_tags(&self.tags_input.read(cx).value()),
            optional_bookmark_note(&self.note_input.read(cx).value()),
        )
    }

    fn focus_collection(&self, window: &mut Window, cx: &mut Context<ElyShell>) {
        self.collection_input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
    }
}

impl ElyShell {
    pub(super) fn start_bookmark_edit(
        &mut self,
        bookmark: &BookmarkEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pending_edit = PendingBookmarkEdit::new(bookmark, window, cx);
        pending_edit.focus_collection(window, cx);
        self.pending_bookmark_edit = Some(pending_edit);
        self.bookmark_edit_error = None;
        cx.notify();
    }

    pub(super) fn cancel_bookmark_edit(&mut self, cx: &mut Context<Self>) {
        self.pending_bookmark_edit = None;
        self.bookmark_edit_error = None;
        cx.notify();
    }

    pub(super) fn save_bookmark_edit(&mut self, cx: &mut Context<Self>) {
        let Some(pending_edit) = self.pending_bookmark_edit.clone() else {
            return;
        };
        let (collection_name, tags, note) = pending_edit.values(cx);

        let result = match &mut self.state {
            ShellState::Ready(core) => core
                .update_bookmark_metadata(pending_edit.bookmark_id(), collection_name, tags, note)
                .map_err(|error| error.to_string()),
            ShellState::StartupError(message) => Err(message.clone()),
        };

        self.bookmark_edit_error = result.err();
        if self.bookmark_edit_error.is_none() {
            self.pending_bookmark_edit = None;
        }
        cx.notify();
    }
}

fn bookmark_input(
    value: String,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<ElyShell>,
) -> Entity<InputState> {
    cx.new(move |cx| InputState::new(window, cx).placeholder(placeholder).default_value(value))
}

fn parse_bookmark_tags(value: &str) -> Vec<String> {
    value.split(',').filter_map(normalized_text).collect()
}

fn optional_bookmark_note(value: &str) -> Option<String> {
    normalized_text(value)
}

fn normalized_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}
