# Homebrew formula for the openlore CLI — feature homebrew-binary-distribution (ADR-061).
#
# In-repo tap (D-1): this file lives in the openlore repo itself, so:
#     brew tap jeffabailey/openlore https://github.com/jeffabailey/openlore
#     brew install jeffabailey/openlore/openlore
# The explicit-URL tap (OD-HB-1) is needed once because Homebrew otherwise resolves
# `jeffabailey/openlore` to a `homebrew-openlore` repo.
#
# Prebuilt-tarball, NOT build-from-source (D-2): the `url`/`sha256` pairs point at the
# GitHub-Release artifacts produced by .github/workflows/release.yml (ADR-011 matrix).
# brew verifies each download against the sha256 the release pipeline published (D-4).
# Ships the `openlore` CLI only (D-3); no auto-updater / no phone-home (D-5) — upgrades
# stay an explicit `brew upgrade`.
#
# Single multi-platform formula (OD-HB-3): one `.rb` covers all 4 target triples via
# on_macos/on_linux x on_arm/on_intel. The `version` + the 4 `sha256` are regenerated
# and committed on each release by the `bump-formula` job in release.yml (OD-HB-2).
class Openlore < Formula
  desc "Local-first CLI for authoring and querying federated philosophical claims"
  homepage "https://github.com/jeffabailey/openlore"
  version "0.1.0-rc6"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/jeffabailey/openlore/releases/download/v0.1.0-rc6/openlore-0.1.0-rc6-aarch64-apple-darwin.tar.gz"
      sha256 "4c5f3c94b5e0df2110aed1f5aed8d302971bfb1bf814e7b34a590ddcb0e24277"
    end
    on_intel do
      url "https://github.com/jeffabailey/openlore/releases/download/v0.1.0-rc6/openlore-0.1.0-rc6-x86_64-apple-darwin.tar.gz"
      sha256 "1e455d4790d08f0303a5171bce9fda154d5678ac890db9f45eab392f477c830d"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/jeffabailey/openlore/releases/download/v0.1.0-rc6/openlore-0.1.0-rc6-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "7254eabbf4c2ba6cb91bf31e85e42b30c14fe40ceeb0d8754592ccb81caa09e3"
    end
    on_intel do
      url "https://github.com/jeffabailey/openlore/releases/download/v0.1.0-rc6/openlore-0.1.0-rc6-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "7cfb6c773a74c24684e615d910a23e7380225ef134036213cc55c29819753012"
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
