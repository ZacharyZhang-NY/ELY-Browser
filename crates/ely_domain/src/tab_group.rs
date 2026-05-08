use std::time::{Duration, SystemTime};

use crate::{DomainError, SpaceId, TabGroupId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabGroup {
    id: TabGroupId,
    space_id: SpaceId,
    name: String,
    color_hex: u32,
    collapsed: bool,
    sort_key: u64,
    created_at: SystemTime,
    updated_at: SystemTime,
}

impl TabGroup {
    pub fn new(
        space_id: SpaceId,
        name: impl Into<String>,
        color_hex: u32,
        sort_key: u64,
    ) -> Result<Self, DomainError> {
        let name = normalized_name(name)?;
        let created_at = SystemTime::now();
        Ok(Self {
            id: TabGroupId::new(),
            space_id,
            name,
            color_hex,
            collapsed: false,
            sort_key,
            created_at,
            updated_at: created_at,
        })
    }

    #[must_use]
    pub fn id(&self) -> &TabGroupId {
        &self.id
    }

    #[must_use]
    pub fn space_id(&self) -> &SpaceId {
        &self.space_id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn rename(&mut self, name: impl Into<String>) -> Result<(), DomainError> {
        self.name = normalized_name(name)?;
        self.record_update();
        Ok(())
    }

    #[must_use]
    pub fn color_hex(&self) -> u32 {
        self.color_hex
    }

    pub fn set_color_hex(&mut self, color_hex: u32) {
        self.color_hex = color_hex;
        self.record_update();
    }

    #[must_use]
    pub fn collapsed(&self) -> bool {
        self.collapsed
    }

    pub fn set_collapsed(&mut self, collapsed: bool) {
        self.collapsed = collapsed;
        self.record_update();
    }

    #[must_use]
    pub fn sort_key(&self) -> u64 {
        self.sort_key
    }

    pub fn set_sort_key(&mut self, sort_key: u64) {
        self.sort_key = sort_key;
        self.record_update();
    }

    #[must_use]
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    #[must_use]
    pub fn updated_at(&self) -> SystemTime {
        self.updated_at
    }

    fn record_update(&mut self) {
        let now = SystemTime::now();
        self.updated_at =
            if now > self.updated_at { now } else { self.updated_at + Duration::from_nanos(1) };
    }
}

fn normalized_name(name: impl Into<String>) -> Result<String, DomainError> {
    let name = name.into();
    let name = name.trim();
    if name.is_empty() {
        return Err(DomainError::EmptyField { field: "tab_group_name" });
    }

    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::TabGroup;
    use crate::{DomainError, SpaceId};

    #[test]
    fn creates_tab_group_with_trimmed_name() -> Result<(), DomainError> {
        let space_id = SpaceId::new();
        let group = TabGroup::new(space_id.clone(), " Research ", 0xf54e00, 7)?;

        assert_eq!(group.space_id(), &space_id);
        assert_eq!(group.name(), "Research");
        assert_eq!(group.color_hex(), 0xf54e00);
        assert_eq!(group.sort_key(), 7);
        assert!(!group.collapsed());
        Ok(())
    }

    #[test]
    fn rejects_empty_tab_group_name() -> Result<(), Box<dyn std::error::Error>> {
        let error = match TabGroup::new(SpaceId::new(), " ", 0xf54e00, 0) {
            Err(error) => error,
            Ok(_) => return Err("empty tab group name was accepted".into()),
        };

        assert_eq!(error, DomainError::EmptyField { field: "tab_group_name" });
        Ok(())
    }

    #[test]
    fn updates_mutable_tab_group_fields() -> Result<(), DomainError> {
        let mut group = TabGroup::new(SpaceId::new(), "Research", 0xf54e00, 0)?;

        group.rename("Docs")?;
        group.set_color_hex(0x2f6fed);
        group.set_collapsed(true);
        group.set_sort_key(9);

        assert_eq!(group.name(), "Docs");
        assert_eq!(group.color_hex(), 0x2f6fed);
        assert!(group.collapsed());
        assert_eq!(group.sort_key(), 9);
        Ok(())
    }
}
