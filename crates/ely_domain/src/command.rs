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

        match UrlText::from_address_text(trimmed) {
            Ok(url) => Ok(Self::Navigate(url)),
            Err(DomainError::InvalidUrl { .. }) => Ok(Self::Search(trimmed.to_string())),
            Err(error) => Err(error),
        }
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

#[cfg(test)]
mod tests {
    use super::CommandIntent;

    #[test]
    fn plain_text_parses_as_search() {
        assert_eq!(
            CommandIntent::parse("servo browser").unwrap(),
            CommandIntent::Search("servo browser".to_string())
        );
    }

    #[test]
    fn domain_like_text_parses_as_navigation() {
        let intent = CommandIntent::parse("example.com").unwrap();
        let CommandIntent::Navigate(url) = intent else {
            panic!("expected navigation intent");
        };
        assert_eq!(url.as_str(), "https://example.com");
    }
}
