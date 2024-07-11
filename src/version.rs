use std::cmp::Ordering;

/// Parse a version string into a vector of integers.
///
/// # Arguments
///
/// * `v` - A version string (e.g., "v0.21.0" or "22.0")
///
/// # Returns
///
/// A vector of integers representing the version components.
fn parse_version(v: &str) -> Vec<u32> {
    v.trim_start_matches('v')
        .split('.')
        .map(|s| s.parse().unwrap_or(0))
        .collect()
}

/// Compares two version strings.
///
/// # Arguments
///
/// * `a` - First version string
/// * `b` - Second version string
///
/// # Returns
///
/// An Ordering indicating the relationship between the two versions.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let va = parse_version(a);
    let vb = parse_version(b);

    // If both versions start with 0, compare them as is
    if va.first() == Some(&0) && vb.first() == Some(&0) {
        va.cmp(&vb)
    } else {
        // Otherwise, compare only the first two components
        va.iter().take(2).cmp(vb.iter().take(2))
    }
}
