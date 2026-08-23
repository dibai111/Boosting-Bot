pub(crate) fn matches(expected: Option<&str>, observed: &str) -> bool {
    expected.is_some_and(|name| name.eq_ignore_ascii_case(observed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_names_without_case_sensitivity() {
        assert!(matches(Some("PlayerOne"), "playerone"));
        assert!(!matches(Some("PlayerOne"), "other"));
        assert!(!matches(None, "playerone"));
    }
}
