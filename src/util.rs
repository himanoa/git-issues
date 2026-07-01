//! Small helpers: UTC timestamps, slugs, and random issue IDs.

use chrono::Utc;
use rand::Rng;

/// The current time as an ISO-8601 UTC timestamp, e.g. `2026-07-02T12:34:56Z`.
pub fn utc_now() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Turn an arbitrary title into a lowercase, hyphen-separated slug.
pub fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = true; // avoids a leading dash
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    let trimmed = slug.trim_matches('-');
    let mut slug: String = trimmed.chars().take(40).collect();
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug.push_str("issue");
    }
    slug
}

/// A 4-character lowercase alphanumeric suffix that keeps IDs unique across
/// machines without any shared counter.
fn random_suffix() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..4)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Build an issue ID from a title: `<slug>-<random4>`.
pub fn make_id(title: &str) -> String {
    format!("{}-{}", slugify(title), random_suffix())
}
