//! Release-tag signature checks for automatic installation.
//!
//! FakingView currently has no configured release-signing identity and installs no downloaded
//! update automatically. Its release pages remain available for manual downloads. The verifier
//! fails closed before networking until a separate reviewed signing policy establishes its own
//! trust anchors. Signature-rule tests use an explicitly supplied historical fixture identity;
//! no external project's maintainer identity is inherited by the application.

use crate::service::net::http::{self, Retry};
use crate::service::update::check::github_headers;
use serde::Deserialize;

/// Tagger emails accepted for installation, intentionally unconfigured for this release.
// No release-signing identity is inherited from another project. Manual downloads remain
// available while FakingView establishes its own reviewed signing policy.
const MAINTAINER_EMAILS: &[&str] = &[];
const API: &str = "https://api.github.com/repos/srg-sphynx/FakingView/git";

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum TagError {
    /// No tag object, a lightweight tag, or no signature on it.
    #[error("This release's tag is not signed.")]
    Unsigned,
    /// Signed, but GitHub does not accept the signature or the tagger is not the maintainer.
    #[error("This release's tag is not signed by the FakingView maintainer.")]
    NotTrusted,
    /// Signed, but for a different tag or a non-commit target.
    #[error("This release's tag signature belongs to another tag.")]
    WrongTag,
    #[error("Could not check the release signature: {0}")]
    Network(String),
}

#[derive(Deserialize)]
struct RefDto {
    object: RefObjectDto,
}

#[derive(Deserialize)]
struct RefObjectDto {
    sha: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct TagDto {
    tag: String,
    object: TargetDto,
    tagger: Option<TaggerDto>,
    verification: Option<VerificationDto>,
}

#[derive(Deserialize)]
struct TargetDto {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct TaggerDto {
    email: String,
}

#[derive(Deserialize)]
struct VerificationDto {
    verified: bool,
    reason: String,
}

/// Blocking: two GETs. `Ok` only when `tag` is an annotated tag GitHub verified, tagged by the
/// maintainer.
pub fn verify_release_tag(tag: &str) -> Result<(), TagError> {
    if MAINTAINER_EMAILS.is_empty() {
        return Err(TagError::NotTrusted);
    }
    verify_release_tag_at(API, tag)
}

fn verify_release_tag_at(api: &str, tag: &str) -> Result<(), TagError> {
    if tag.is_empty()
        || !tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(TagError::WrongTag);
    }
    let fetch = |url: String| {
        http::get_text(&url, &github_headers(), Retry::Transient).map_err(network_error)
    };
    let unreadable = |_| TagError::Network("unreadable reply from GitHub".to_owned());
    let reference: RefDto =
        serde_json::from_str(&fetch(format!("{api}/ref/tags/{tag}"))?).map_err(unreadable)?;
    if reference.object.kind != "tag" {
        // A lightweight tag points straight at a commit and cannot carry a signature.
        return Err(TagError::Unsigned);
    }
    let object: TagDto =
        serde_json::from_str(&fetch(format!("{api}/tags/{}", reference.object.sha))?)
            .map_err(unreadable)?;
    evaluate(tag, &object)
}

/// GitHub answers 403 (or 429) to an unauthenticated client that used its hourly quota, which is
/// shared by everything on the same address. Say so, as the update check does, instead of showing
/// "HTTP 403".
fn network_error(error: http::Error) -> TagError {
    match error {
        http::Error::Status(403 | 429) => {
            TagError::Network("GitHub is rate limiting requests. Try again later.".to_owned())
        }
        other => TagError::Network(other.to_string()),
    }
}

fn evaluate(tag: &str, object: &TagDto) -> Result<(), TagError> {
    evaluate_with_maintainers(tag, object, MAINTAINER_EMAILS)
}

fn evaluate_with_maintainers(
    tag: &str,
    object: &TagDto,
    maintainers: &[&str],
) -> Result<(), TagError> {
    let Some(verification) = &object.verification else {
        return Err(TagError::Unsigned);
    };
    if !(verification.verified && verification.reason == "valid") {
        return Err(if verification.reason == "unsigned" {
            TagError::Unsigned
        } else {
            TagError::NotTrusted
        });
    }
    // Verified, but make sure it is for this tag and not a replayed older one.
    if object.tag != tag || object.object.kind != "commit" {
        return Err(TagError::WrongTag);
    }
    let maintainer = object.tagger.as_ref().is_some_and(|tagger| {
        maintainers
            .iter()
            .any(|email| email.eq_ignore_ascii_case(&tagger.email))
    });
    if maintainer {
        Ok(())
    } else {
        Err(TagError::NotTrusted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GitHub's reply for the published v0.1.2 tag object (signature and payload elided).
    const REAL: &str = include_str!("testdata/tag_v0.1.2.json");

    fn tag(edit: impl FnOnce(&mut serde_json::Value)) -> TagDto {
        let mut json: serde_json::Value = serde_json::from_str(REAL).unwrap();
        edit(&mut json);
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn a_local_build_inherits_no_external_release_signing_identity() {
        assert!(MAINTAINER_EMAILS.is_empty());
        assert_eq!(
            super::evaluate("v0.1.2", &tag(|_| {})),
            Err(TagError::NotTrusted)
        );
        assert_eq!(verify_release_tag("v0.1.2"), Err(TagError::NotTrusted));
        assert_eq!(
            API,
            "https://api.github.com/repos/srg-sphynx/FakingView/git"
        );
    }

    // The recorded response tests the signature rules with an explicit fixture-only identity.
    // It is not a trust anchor for production FakingView releases.
    fn evaluate(tag: &str, object: &TagDto) -> Result<(), TagError> {
        evaluate_with_maintainers(tag, object, &["nicoswd@gmail.com"])
    }

    #[test]
    fn a_verified_historical_fixture_is_valid_with_its_explicit_test_identity() {
        assert_eq!(evaluate("v0.1.2", &tag(|_| {})), Ok(()));
    }

    #[test]
    fn the_maintainer_email_matches_case_insensitively() {
        let upper = tag(|json| json["tagger"]["email"] = "NicoSWD@Gmail.com".into());
        assert_eq!(evaluate("v0.1.2", &upper), Ok(()));
    }

    #[test]
    fn a_valid_signature_by_someone_else_is_not_trusted() {
        let other = tag(|json| json["tagger"]["email"] = "attacker@example.com".into());
        assert_eq!(evaluate("v0.1.2", &other), Err(TagError::NotTrusted));
        let none = tag(|json| json["tagger"] = serde_json::Value::Null);
        assert_eq!(evaluate("v0.1.2", &none), Err(TagError::NotTrusted));
    }

    #[test]
    fn unverified_tags_are_refused_by_reason() {
        for (reason, expected) in [
            ("unsigned", TagError::Unsigned),
            ("unknown_key", TagError::NotTrusted),
            ("bad_email", TagError::NotTrusted),
            ("expired_key", TagError::NotTrusted),
            ("invalid", TagError::NotTrusted),
        ] {
            let unverified = tag(|json| {
                json["verification"]["verified"] = false.into();
                json["verification"]["reason"] = reason.into();
            });
            assert_eq!(evaluate("v0.1.2", &unverified), Err(expected), "{reason}");
        }
    }

    #[test]
    fn verified_must_be_true_and_reason_valid() {
        let odd = tag(|json| json["verification"]["verified"] = false.into());
        assert_eq!(evaluate("v0.1.2", &odd), Err(TagError::NotTrusted));
        let odd = tag(|json| json["verification"]["reason"] = "gpgverify_error".into());
        assert_eq!(evaluate("v0.1.2", &odd), Err(TagError::NotTrusted));
    }

    #[test]
    fn a_missing_verification_is_unsigned() {
        let none = tag(|json| json["verification"] = serde_json::Value::Null);
        assert_eq!(evaluate("v0.1.2", &none), Err(TagError::Unsigned));
    }

    #[test]
    fn a_signature_replayed_for_another_tag_or_target_is_rejected() {
        assert_eq!(evaluate("v0.1.3", &tag(|_| {})), Err(TagError::WrongTag));
        let tree = tag(|json| json["object"]["type"] = "tree".into());
        assert_eq!(evaluate("v0.1.2", &tree), Err(TagError::WrongTag));
    }

    #[test]
    fn a_rate_limited_check_says_so_instead_of_showing_the_status_code() {
        for status in [403, 429] {
            assert_eq!(
                network_error(http::Error::Status(status)).to_string(),
                "Could not check the release signature: GitHub is rate limiting requests. Try again later."
            );
        }
        assert_eq!(
            network_error(http::Error::Status(500)).to_string(),
            "Could not check the release signature: HTTP 500"
        );
    }

    #[test]
    fn odd_tag_names_never_reach_the_network() {
        for name in ["", "../x", "v1/../../x", "v1 2"] {
            assert_eq!(
                verify_release_tag_at("http://127.0.0.1:1", name),
                Err(TagError::WrongTag)
            );
        }
    }
}
