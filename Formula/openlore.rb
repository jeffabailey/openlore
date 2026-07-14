# DO NOT EDIT Formula/openlore.rb BY HAND — it is generated from this template
# (Formula/openlore.rb.tmpl) by scripts/release/bump-formula.sh, which the
# `bump-formula` job in .github/workflows/release.yml runs on each GA release to
# refresh the version + the 4 sha256 and commit the result to main (trunk-based,
# no PR; OD-HB-2). Edit THIS template, not the rendered file. The placeholders
# (VERSION + the 4 per-triple sha256) are documented in scripts/release/bump-formula.sh.
#
# Homebrew formula for the openlore CLI — feature homebrew-binary-distribution (ADR-061).
#
# In-repo tap (D-1). Install flow — three steps, NO separate homebrew-openlore repo:
#     brew tap jeffabailey/openlore https://github.com/jeffabailey/openlore
#     brew trust jeffabailey/openlore
#     brew install jeffabailey/openlore/openlore
#   1. explicit-URL tap (OD-HB-1): Homebrew's `user/name` shorthand otherwise resolves
#      to a `homebrew-name` repo, so the URL is required to point at THIS repo.
#   2. brew trust (Homebrew 6.0.0+): third-party taps are ignored until trusted; a
#      custom-remote tap must be trusted whole-tap (`brew trust --formula …` is rejected).
#      Older Homebrew has no `trust` command — that step is a harmless no-op there.
#
# Prebuilt tarball, NOT build-from-source (D-2): the url/sha256 pairs point at the
# release.yml artifacts (ADR-011 matrix); brew verifies each download against the
# published sha256 (D-4). Ships the `openlore` CLI only (D-3); no auto-updater / no
# phone-home (D-5). Single multi-platform formula covering all 4 triples (OD-HB-3).
class Openlore < Formula
  desc "Local-first CLI for authoring and querying federated philosophical claims"
  homepage "https://github.com/jeffabailey/openlore"
  version "0.1.0"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/jeffabailey/openlore/releases/download/v0.1.0/openlore-0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "685a0427582b0a549a1e39c470670cfb81ddbd8cf7bc1052478924dd490dd919"
    end
    on_intel do
      url "https://github.com/jeffabailey/openlore/releases/download/v0.1.0/openlore-0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "df3bf0a1dad8a5e3cef2d2d1df5a8dc739c8bb3e69c7bcf67c9c346a8b2e0c12"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/jeffabailey/openlore/releases/download/v0.1.0/openlore-0.1.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "440955ab3c96a16eb87fcf93528780e467f1c0f322938568161f7cdca090a861"
    end
    on_intel do
      url "https://github.com/jeffabailey/openlore/releases/download/v0.1.0/openlore-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "36d62d3e198061f42479fee8e5cdaeddd60013b488a35ab535dc0e7ecfff9146"
    end
  end

  def install
    bin.install "openlore"
  end

  test do
    # The binary reports its Cargo version (e.g. "openlore 0.1.0"), which can differ
    # from a pre-release formula version (0.1.0-rc6) — assert the shape, not an exact
    # string, so the test holds for both RC and GA formulae.
    assert_match(/^openlore \d+\.\d+\.\d+/, shell_output("#{bin}/openlore --version"))
  end
end
