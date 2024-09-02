use std::cmp::Ordering;

/// Parse a version string into a vector of integers and a release candidate number.
///
/// # Arguments
///
/// * `v` - A version string (e.g., "v0.21.0", "22.0", or "v0.28.0rc1")
///
/// # Returns
///
/// A tuple containing:
/// - A vector of integers representing the version components
/// - An Option<u32> representing the release candidate number (if present)
fn parse_version(v: &str) -> (Vec<u32>, Option<u32>) {
    let v = v.trim_start_matches('v');
    let parts: Vec<&str> = v.split("rc").collect();

    let version_parts = parts[0]
        .split('.')
        .map(|s| s.parse().unwrap_or(0))
        .collect();

    let rc = parts.get(1).and_then(|&s| s.parse().ok());

    (version_parts, rc)
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
    let (va, rc_a) = parse_version(a);
    let (vb, rc_b) = parse_version(b);

    // Compare the first two components of the version numbers
    match va.iter().take(2).cmp(vb.iter().take(2)) {
        Ordering::Equal => {
            // If the first two components are equal, compare the third component
            match va.get(2).cmp(&vb.get(2)) {
                Ordering::Equal => {
                    // If all version components are equal, compare rc numbers
                    match (rc_a, rc_b) {
                        (None, None) => Ordering::Equal,
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (Some(a), Some(b)) => a.cmp(&b),
                    }
                }
                other => other,
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions() {
        assert_eq!(compare_versions("v0.21.0", "v0.28.0rc1"), Ordering::Less);
        assert_eq!(compare_versions("v0.28.0rc1", "v0.28.0"), Ordering::Less);
        assert_eq!(
            compare_versions("v0.28.0rc2", "v0.28.0rc1"),
            Ordering::Greater
        );
        assert_eq!(compare_versions("v0.28.0", "v0.28.0"), Ordering::Equal);
        assert_eq!(compare_versions("v1.0.0", "v0.28.0"), Ordering::Greater);
        assert_eq!(compare_versions("v0.28.1", "v0.28.0rc1"), Ordering::Greater);
    }
}
