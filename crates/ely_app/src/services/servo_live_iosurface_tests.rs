use std::collections::BTreeSet;

use super::{
    ServoLiveError, apply_iosurface_import_results,
    iosurface_importer::{IOSurfaceImportFailure, IOSurfaceImportResult},
};
use crate::services::iosurface_metal::IOSurfaceCache;

#[test]
fn import_failure_clears_every_completed_pending_surface() -> Result<(), String> {
    let mut cache = IOSurfaceCache::new();
    let mut pending = BTreeSet::from([1, 2, 3]);
    let results = vec![failed_import(1), failed_import(2), failed_import(3)];

    let Err(error) = apply_iosurface_import_results(&mut cache, &mut pending, results) else {
        return Err("the failed import batch succeeded".to_string());
    };

    assert!(matches!(error, ServoLiveError::IOSurfaceImportFailed { surface_id: 1, .. }));
    assert!(pending.is_empty());
    Ok(())
}

fn failed_import(surface_id: u64) -> IOSurfaceImportResult {
    IOSurfaceImportResult::Failed(IOSurfaceImportFailure {
        surface_id,
        mach_port_name: surface_id as u32,
        message: format!("surface {surface_id} failed"),
    })
}
