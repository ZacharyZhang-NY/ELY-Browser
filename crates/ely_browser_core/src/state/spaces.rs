use ely_domain::SpaceId;

use super::BrowserCore;
use crate::CoreError;

impl BrowserCore {
    pub fn move_space_up(&mut self, space_id: &SpaceId) -> Result<bool, CoreError> {
        let mut ordered_ids = self.sorted_space_ids();
        let Some(index) = ordered_ids.iter().position(|id| id == space_id) else {
            return Err(CoreError::SpaceNotFound { id: space_id.clone() });
        };

        if index == 0 {
            return Ok(false);
        }

        ordered_ids.swap(index, index - 1);
        self.apply_space_order(&ordered_ids)?;
        Ok(true)
    }

    pub fn move_space_down(&mut self, space_id: &SpaceId) -> Result<bool, CoreError> {
        let mut ordered_ids = self.sorted_space_ids();
        let Some(index) = ordered_ids.iter().position(|id| id == space_id) else {
            return Err(CoreError::SpaceNotFound { id: space_id.clone() });
        };

        if index + 1 == ordered_ids.len() {
            return Ok(false);
        }

        ordered_ids.swap(index, index + 1);
        self.apply_space_order(&ordered_ids)?;
        Ok(true)
    }

    fn sorted_space_ids(&self) -> Vec<SpaceId> {
        self.sorted_spaces().iter().map(|space| space.id().clone()).collect()
    }

    fn apply_space_order(&mut self, ordered_ids: &[SpaceId]) -> Result<(), CoreError> {
        for (sort_key, space_id) in ordered_ids.iter().enumerate() {
            let Some(space) = self.spaces.iter_mut().find(|space| space.id() == space_id) else {
                return Err(CoreError::SpaceNotFound { id: space_id.clone() });
            };

            let sort_key = sort_key as u64;
            if space.sort_key() != sort_key {
                space.set_sort_key(sort_key);
            }
        }

        Ok(())
    }
}
