# Homebrew Tap formula template for Portkey
# Usage: place this file in your tap repo at Formula/portkey.rb

class Portkey < Formula
  desc "Secure SSH credential manager with TUI"
  homepage "https://github.com/your-org/portkey"
  url "https://github.com/your-org/portkey/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "<REPLACE_WITH_TARBALL_SHA256>"
  license "MIT"

  depends_on "rust" => :build
  depends_on "libsodium"

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/portkey --version")
  end
end

