use crate::{DomainError, UrlText};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandScope {
    Archive,
    Spaces,
    Tabs,
    Bookmarks,
    Notes,
    ReadingList,
    History,
    Settings,
    Plugins,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandIntent {
    Navigate(UrlText),
    Search(String),
    Command(String),
    ScopedSearch { scope: CommandScope, query: String },
}

impl CommandIntent {
    pub fn parse(input: &str) -> Result<Self, DomainError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(DomainError::InvalidCommand);
        }

        if let Some(command) = trimmed.strip_prefix('>') {
            return non_empty_text(command).map(Self::Command);
        }

        if let Some(query) = trimmed.strip_prefix('?') {
            return non_empty_text(query).map(Self::Search);
        }

        if let Some((scope, query)) = parse_scope(trimmed) {
            return non_empty_text(query).map(|query| Self::ScopedSearch { scope, query });
        }

        UrlText::from_address_text(trimmed).map(Self::Navigate)
    }
}

fn parse_scope(value: &str) -> Option<(CommandScope, &str)> {
    let (scope, query) = value.split_once(' ')?;
    let scope = match scope {
        "@archive" => CommandScope::Archive,
        "@spaces" => CommandScope::Spaces,
        "@tabs" => CommandScope::Tabs,
        "@bookmarks" => CommandScope::Bookmarks,
        "@notes" => CommandScope::Notes,
        "@reading-list" | "@reading" => CommandScope::ReadingList,
        "@history" => CommandScope::History,
        "@settings" => CommandScope::Settings,
        "@plugins" => CommandScope::Plugins,
        _ => return None,
    };
    Some((scope, query))
}

fn non_empty_text(value: &str) -> Result<String, DomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainError::InvalidCommand);
    }
    Ok(trimmed.to_string())
}
