pub const DEV_HOST: &str = "https://altered-dev.s3.eu-west-3.amazonaws.com/";
pub const PROD_HOST: &str = "https://altered-prod-eu.s3.amazonaws.com/";
pub const PROXY_BASE: &str = "https://www.altered.gg/_next/image";

/// Strip the dev host from a full URL, returning the relative `Art/...` path.
///
/// Falls back to returning the input unchanged when the URL doesn't start with the known dev host.
pub fn rel_path_from_dev_url(dev_url: &str) -> &str {
    dev_url.strip_prefix(DEV_HOST).unwrap_or(dev_url)
}

/// Build a full `altered-dev` URL from a relative `Art/...` path.
pub fn dev_url_for(rel_path: &str) -> String {
    let mut s = String::with_capacity(DEV_HOST.len() + rel_path.len());
    s.push_str(DEV_HOST);
    s.push_str(rel_path);
    s
}

/// Build a full `altered-prod-eu` URL from a relative `Art/...` path.
pub fn prod_url_for(rel_path: &str) -> String {
    let mut s = String::with_capacity(PROD_HOST.len() + rel_path.len());
    s.push_str(PROD_HOST);
    s.push_str(rel_path);
    s
}

/// Download URL for a relative `Art/...` path (proxy or raw prod S3).
pub fn fetch_url_for_rel_path(rel_path: &str, use_proxy: bool, width: u32, quality: u32) -> String {
    let prod = prod_url_for(rel_path);
    if use_proxy {
        proxy_url_for(&prod, width, quality)
    } else {
        prod
    }
}

/// Wrap an absolute URL in the altered.gg `/_next/image` proxy.
pub fn proxy_url_for(src_url: &str, width: u32, quality: u32) -> String {
    format!(
        "{base}?url={encoded}&w={w}&q={q}",
        base = PROXY_BASE,
        encoded = urlencoding::encode(src_url),
        w = width,
        q = quality
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_path_extraction() {
        let dev = "https://altered-dev.s3.eu-west-3.amazonaws.com/Art/EOLE/CARDS/ALT_EOLE_B_AX_106/UNIQUE/JPG/en_US/909e4d214e1910cc8eea15dc0e81ef2e.jpg";
        let rel = rel_path_from_dev_url(dev);
        assert_eq!(
            rel,
            "Art/EOLE/CARDS/ALT_EOLE_B_AX_106/UNIQUE/JPG/en_US/909e4d214e1910cc8eea15dc0e81ef2e.jpg"
        );
    }

    #[test]
    fn rel_path_passthrough_when_no_dev_host() {
        let rel = "Art/EOLE/CARDS/foo.jpg";
        assert_eq!(rel_path_from_dev_url(rel), rel);
    }

    #[test]
    fn prod_url_built_from_rel_path() {
        let rel = "Art/EOLE/CARDS/ALT_EOLE_B_AX_106/UNIQUE/JPG/en_US/909e4d214e1910cc8eea15dc0e81ef2e.jpg";
        let prod = prod_url_for(rel);
        assert_eq!(
            prod,
            "https://altered-prod-eu.s3.amazonaws.com/Art/EOLE/CARDS/ALT_EOLE_B_AX_106/UNIQUE/JPG/en_US/909e4d214e1910cc8eea15dc0e81ef2e.jpg"
        );
    }

    #[test]
    fn proxy_url_matches_example_format() {
        let prod = "https://altered-prod-eu.s3.amazonaws.com/Art/EOLE/CARDS/ALT_EOLE_B_AX_106/UNIQUE/JPG/en_US/332a320eec71103db2118458405b33ed.jpg";
        let proxied = proxy_url_for(prod, 1200, 75);
        assert!(proxied.starts_with("https://www.altered.gg/_next/image?url="));
        assert!(proxied.contains("altered-prod-eu.s3.amazonaws.com"));
        assert!(proxied.ends_with("&w=1200&q=75"));
    }
}
