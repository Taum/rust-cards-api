pub fn validate_collection_id(id: &str) -> Result<(), String> {
    let trimmed = id.trim();
    if trimmed.len() < 4 || trimmed.len() > 36 {
        return Err(format!(
            "collection id must be 4-36 characters, got {}",
            trimmed.len()
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(
            "collection id may only contain ASCII letters, digits, underscores, and hyphens"
                .to_string(),
        );
    }
    Ok(())
}

/// Parse a newline-separated reference list (blank lines and `#` comments ignored).
pub fn parse_refs_body(body: &str) -> Vec<String> {
    let mut refs = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        refs.push(trimmed.to_string());
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_collection_id_rejects_short_and_long() {
        assert!(validate_collection_id("abc").is_err());
        assert!(validate_collection_id(&"a".repeat(37)).is_err());
        assert!(validate_collection_id("abcd").is_ok());
        assert!(validate_collection_id(&"a".repeat(36)).is_ok());
    }

    #[test]
    fn validate_collection_id_rejects_invalid_chars() {
        assert!(validate_collection_id("ab cd").is_err());
        assert!(validate_collection_id("deck/list").is_err());
        assert!(validate_collection_id("my-deck_v2").is_ok());
    }

    #[test]
    fn parse_refs_body_skips_blanks_and_comments() {
        let body = "# header\n\nALT_TEST_B_AX_04_U_1\n  \n# tail\nALT_TEST_B_AX_04_U_1\n";
        let refs = parse_refs_body(body);
        assert_eq!(
            refs,
            vec![
                "ALT_TEST_B_AX_04_U_1".to_string(),
                "ALT_TEST_B_AX_04_U_1".to_string(),
            ]
        );
    }

    #[test]
    fn parse_refs_body_empty_returns_empty() {
        assert!(parse_refs_body("  \n# only comments\n").is_empty());
    }
}
