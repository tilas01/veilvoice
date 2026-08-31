# SPDX-License-Identifier: GPL-3.0-or-later
#
# Homebrew formula for VeilVoice.
#
#   brew install --build-from-source packaging/homebrew/veilvoice.rb
#
# A formula rather than a cask, deliberately. A cask installs a prebuilt binary;
# a formula builds from source on the machine that will run it. For a tool whose
# whole argument is "you can check this yourself", compiling from the tagged
# source is the more honest default -- and it sidesteps macOS notarisation
# entirely, which this project cannot do under a pseudonym.
class Veilvoice < Formula
  desc "Irreversible voice de-identification, fully offline"
  homepage "https://tilas01.github.io/veilvoice/"
  url "https://github.com/tilas01/veilvoice/archive/refs/tags/v0.1.15.tar.gz"
  # Replace on each release with the sha256 from the published SHA256SUMS,
  # which is signed. `brew fetch --force veilvoice` then prints what it saw.
  sha256 "REPLACE_WITH_THE_SIGNED_SHA256_OF_THE_SOURCE_TARBALL"
  license "GPL-3.0-or-later"
  head "https://github.com/tilas01/veilvoice.git", branch: "main"

  depends_on "rust" => :build

  def install
    # --locked: exactly the dependency versions the project tested. A formula
    # that resolves fresh versions at install time is not installing the
    # software that was audited.
    #
    # The CLI and the verifier only. The desktop app needs a windowing stack
    # that Homebrew is a poor fit for; macOS users who want the GUI should take
    # the signed release archive, which is a real app bundle.
    system "cargo", "build", "--release", "--locked",
           "-p", "veilvoice-cli", "-p", "veilvoice-verify"
    bin.install "target/release/veilvoice"
    bin.install "target/release/veilvoice-verify"
    doc.install "README.md"
    doc.install Dir["docs/*"]
  end

  def caveats
    <<~EOS
      VeilVoice destroys the voiceprint, not the words. Intelligibility is
      preserved on purpose: the words remain in the output and can be
      transcribed. If the message itself is sensitive, encrypt it.

      This formula installs the command-line tool and the portable verifier.
      The desktop application ships in the signed release archives.

      Live microphone scrambling needs a virtual audio device (BlackHole or
      Loopback on macOS). Neither is bundled.
    EOS
  end

  test do
    # `info` reports what the build supports, so this asserts the binary runs
    # and that the offline claim survived packaging.
    assert_match "VeilVoice", shell_output("#{bin}/veilvoice info")
    assert_match "none, by construction", shell_output("#{bin}/veilvoice info")

    # The verifier must carry the right key. If a packaging step ever mangled
    # the embedded key, this is where it shows up.
    assert_match "8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A",
                 shell_output("#{bin}/veilvoice-verify key")
  end
end
