//! Profile names, checked the same way by the daemon (which stores them) and the configuration app.

/// Letters, digits, spaces, `_` and `-`, 1 to 40 characters, not starting or ending with a space. This also keeps
/// names from reaching outside the profiles folder.
pub fn valid(name: &str) -> bool {
    (1..=40).contains(&name.chars().count())
        && name.trim() == name
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'))
}

pub fn checked(name: &str) -> Result<&str, String> {
    if valid(name) {
        Ok(name)
    } else {
        Err("Profile names use 1 to 40 letters, digits, spaces, _ or -.".into())
    }
}
