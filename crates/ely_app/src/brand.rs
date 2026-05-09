pub(crate) const PRODUCT_NAME: &str = "ELY Browser";
pub(crate) const FORMAL_PRODUCT_NAME: &str = "ELY Browser by Elydora";
pub(crate) const COMPANY_NAME: &str = "Elydora";
pub(crate) const SYNC_SERVICE_NAME: &str = "Elydora Cloud";
pub(crate) const DEEP_LINK_SCHEME: &str = "ely";
pub(crate) const DEEP_LINK_PREFIX: &str = "ely://";
pub(crate) const AUTH_CALLBACK_URL: &str = "ely://auth/callback";

#[cfg(test)]
mod tests {
    use super::{
        AUTH_CALLBACK_URL, COMPANY_NAME, DEEP_LINK_PREFIX, DEEP_LINK_SCHEME, FORMAL_PRODUCT_NAME,
        PRODUCT_NAME, SYNC_SERVICE_NAME,
    };

    #[test]
    fn brand_identity_matches_prd_names() {
        assert_eq!(PRODUCT_NAME, "ELY Browser");
        assert_eq!(FORMAL_PRODUCT_NAME, "ELY Browser by Elydora");
        assert_eq!(COMPANY_NAME, "Elydora");
        assert_eq!(SYNC_SERVICE_NAME, "Elydora Cloud");
        assert_eq!(DEEP_LINK_SCHEME, "ely");
        assert_eq!(DEEP_LINK_PREFIX, "ely://");
        assert_eq!(AUTH_CALLBACK_URL, "ely://auth/callback");
    }
}
