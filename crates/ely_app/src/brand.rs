pub(crate) const PRODUCT_NAME: &str = "ELY Browser";
pub(crate) const FORMAL_PRODUCT_NAME: &str = "ELY Browser by Elydora";
pub(crate) const COMPANY_NAME: &str = "Elydora";
pub(crate) const SYNC_SERVICE_NAME: &str = "Elydora Cloud";

#[cfg(test)]
mod tests {
    use super::{COMPANY_NAME, FORMAL_PRODUCT_NAME, PRODUCT_NAME, SYNC_SERVICE_NAME};

    #[test]
    fn brand_identity_matches_prd_names() {
        assert_eq!(PRODUCT_NAME, "ELY Browser");
        assert_eq!(FORMAL_PRODUCT_NAME, "ELY Browser by Elydora");
        assert_eq!(COMPANY_NAME, "Elydora");
        assert_eq!(SYNC_SERVICE_NAME, "Elydora Cloud");
    }
}
