//! Where a stored claim's artifact is read from (D17): an own claim by its
//! CID in this store's own-claim directory, a pulled claim by its recorded
//! relative path in the pulled-claim directory — never at an absolute path
//! recorded when it was written, and never from a CID or recorded path that
//! could climb out of those directories.

use std::path::{Component, Path, PathBuf};

/// The longest CID token accepted (a CIDv1 base32 sha2-256 is 59 chars).
const MAX_CID_LEN: usize = 128;

/// Is `cid` a plain CID token: 1..=128 ASCII letters, digits, `-` or `_`?
/// That covers the multibase alphabets CIDs are written in (base32, base36,
/// base58) and rejects every path separator, `.`, `..` and NUL, so a CID
/// joined onto a directory names a file directly inside it.
pub(crate) fn is_plain_cid(cid: &str) -> bool {
    (1..=MAX_CID_LEN).contains(&cid.len())
        && cid
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// `<claims_dir>/<cid>.json`, or `None` when `cid` is not a plain CID token.
pub(crate) fn own_artifact(claims_dir: &Path, cid: &str) -> Option<PathBuf> {
    is_plain_cid(cid).then(|| claims_dir.join(format!("{cid}.json")))
}

/// A pulled claim's recorded `peer_claims/<encoded_did>/<cid>.json` resolved
/// under `peer_claims_root`, or `None` when the record is not a relative path
/// of plain components under `peer_claims/` ending in `<cid>.json` for a
/// plain CID token.
pub(crate) fn peer_artifact(peer_claims_root: &Path, recorded: &str, cid: &str) -> Option<PathBuf> {
    let relative = Path::new(recorded.strip_prefix("peer_claims/")?);
    let plain = relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    let names_cid = relative.file_name()? == format!("{cid}.json").as_str();
    (plain && names_cid && is_plain_cid(cid)).then(|| peer_claims_root.join(relative))
}

/// Where a stored claim's artifact is read from: an `Own` row by its CID
/// under `claims_dir`, a `Peer` row from its recorded relative path under
/// `peer_claims_root`. `None` for anything else, or a CID or recorded path
/// that is not plain.
pub(crate) fn stored_artifact(
    claims_dir: &Path,
    peer_claims_root: &Path,
    source_table: &str,
    cid: &str,
    recorded: &str,
) -> Option<PathBuf> {
    match source_table {
        "Own" => own_artifact(claims_dir, cid),
        "Peer" => peer_artifact(peer_claims_root, recorded, cid),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_peer_artifact_resolves_only_under_peer_claims_and_names_its_cid() {
        let root = Path::new("/store/peer_claims");
        assert_eq!(
            peer_artifact(root, "peer_claims/did_plc_rachel/bafyabc.json", "bafyabc"),
            Some(root.join("did_plc_rachel/bafyabc.json"))
        );
        for hostile in [
            "peer_claims/../outside/bafyabc.json",
            "/etc/bafyabc.json",
            "peer_claims//bafyabc.json/../bafyabc.json",
            "peer_claims/did/other.json",
            "elsewhere/bafyabc.json",
        ] {
            assert_eq!(peer_artifact(root, hostile, "bafyabc"), None, "{hostile}");
        }
    }

    #[test]
    fn an_own_artifact_is_its_cid_directly_inside_claims() {
        let claims = Path::new("/store/claims");
        assert_eq!(
            own_artifact(claims, "bafyabc"),
            Some(claims.join("bafyabc.json"))
        );
        assert_eq!(own_artifact(claims, "../bafyabc"), None);
    }
}
