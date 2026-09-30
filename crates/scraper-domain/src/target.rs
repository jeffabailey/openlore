//! GitHub target normalization — the ONE place a pasted GitHub URL becomes the
//! `owner/repo` or `user` form every scrape surface (the `scrape github` verb
//! and the viewer's `POST /scrape`) resolves.
//!
//! PURE and total: a string that is not a GitHub URL is returned trimmed and
//! otherwise untouched, so `owner/repo` and bare `user` targets pass straight
//! through and a refusal (not-found, private) still names what the user typed.

/// The host prefixes (after the scheme is dropped) that mark a GitHub URL.
const GITHUB_HOSTS: [&str; 2] = ["github.com/", "www.github.com/"];

/// Normalize a scrape target: a GitHub URL (`https://github.com/owner/repo`,
/// `github.com/user`, `git@github.com:owner/repo.git`, a deep link such as
/// `.../tree/main/src`) becomes `owner/repo` or `user`; anything else is
/// returned trimmed. The host match is case-insensitive; a trailing `.git`
/// on the repo segment and any `?query` / `#fragment` are dropped.
pub fn normalize_target(raw: &str) -> String {
    let trimmed = raw.trim();
    match github_path(trimmed) {
        Some(path) => path_to_target(path).unwrap_or_else(|| trimmed.to_string()),
        None => trimmed.to_string(),
    }
}

/// The path after the GitHub host, or `None` when `target` is not a GitHub
/// URL (so `owner/repo` and `user` are never rewritten).
fn github_path(target: &str) -> Option<&str> {
    if let Some(rest) = strip_prefix_ignore_case(target, "git@github.com:") {
        return Some(rest);
    }
    let without_scheme = ["https://", "http://"]
        .iter()
        .find_map(|scheme| strip_prefix_ignore_case(target, scheme))
        .unwrap_or(target);
    GITHUB_HOSTS
        .iter()
        .find_map(|host| strip_prefix_ignore_case(without_scheme, host))
}

/// `owner/repo` from the first two path segments, or `user` from one; `None`
/// for an empty path (a bare `https://github.com/`).
fn path_to_target(path: &str) -> Option<String> {
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let mut segments = path.split('/').filter(|s| !s.is_empty());
    let owner = segments.next()?;
    Some(match segments.next() {
        Some(repo) => format!("{owner}/{}", repo.strip_suffix(".git").unwrap_or(repo)),
        None => owner.to_string(),
    })
}

fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    s.get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .map(|_| &s[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::normalize_target;

    #[test]
    fn repo_urls_normalize_to_owner_slash_repo() {
        for url in [
            "https://github.com/rust-lang/cargo",
            "https://github.com/rust-lang/cargo/",
            "http://github.com/rust-lang/cargo",
            "https://www.github.com/rust-lang/cargo",
            "HTTPS://GitHub.com/rust-lang/cargo",
            "github.com/rust-lang/cargo",
            "https://github.com/rust-lang/cargo.git",
            "git@github.com:rust-lang/cargo.git",
            "https://github.com/rust-lang/cargo/tree/master/src",
            "https://github.com/rust-lang/cargo?tab=readme-ov-file",
            "https://github.com/rust-lang/cargo#readme",
            "  https://github.com/rust-lang/cargo  ",
        ] {
            assert_eq!(normalize_target(url), "rust-lang/cargo", "input: {url:?}");
        }
    }

    #[test]
    fn user_urls_normalize_to_the_bare_user() {
        for url in [
            "https://github.com/torvalds",
            "https://github.com/torvalds/",
            "github.com/torvalds",
            "https://github.com/torvalds?tab=repositories",
        ] {
            assert_eq!(normalize_target(url), "torvalds", "input: {url:?}");
        }
    }

    #[test]
    fn non_url_targets_pass_through_trimmed() {
        assert_eq!(normalize_target("rust-lang/cargo"), "rust-lang/cargo");
        assert_eq!(normalize_target("torvalds"), "torvalds");
        assert_eq!(normalize_target("  torvalds\n"), "torvalds");
        assert_eq!(
            normalize_target("https://gitlab.com/o/r"),
            "https://gitlab.com/o/r"
        );
        assert_eq!(
            normalize_target("https://github.com/"),
            "https://github.com/"
        );
        assert_eq!(normalize_target(""), "");
    }

    #[test]
    fn normalizing_is_idempotent() {
        for input in [
            "https://github.com/rust-lang/cargo/tree/master",
            "github.com/torvalds",
            "rust-lang/cargo",
        ] {
            let once = normalize_target(input);
            assert_eq!(normalize_target(&once), once, "input: {input:?}");
        }
    }
}
