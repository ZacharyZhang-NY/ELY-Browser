use ely_domain::ProfileKind;

use super::profile_allows_sync_controls;

#[test]
fn private_profile_hides_sync_controls() {
    assert!(!profile_allows_sync_controls(&ProfileKind::Private));
    assert!(profile_allows_sync_controls(&ProfileKind::Standard));
}
