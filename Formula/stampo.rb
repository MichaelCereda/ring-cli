class Stampo < Formula
  desc "Turn any API or config into a real CLI — no code, no dependencies"
  homepage "https://github.com/MichaelCereda/stampo"
  version "3.0.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/MichaelCereda/stampo/releases/download/v#{version}/stampo-Darwin-aarch64.tar.gz"
      sha256 "a1a4bfc62675014d8c7f00a479d84d1b442938e4250686eab3982347d7d905ee"
    else
      url "https://github.com/MichaelCereda/stampo/releases/download/v#{version}/stampo-Darwin-x86_64.tar.gz"
      sha256 "9ece6f9b85344c6e75aff6c5d769fbc91c2eca0fc21e6b03b45d3c3ba9de229d"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/MichaelCereda/stampo/releases/download/v#{version}/stampo-Linux-aarch64-musl.tar.gz"
      sha256 "0cc34c6022764445fe1bb8e7664dc5088ce7e5ee680285736fe1796f020a579b"
    else
      url "https://github.com/MichaelCereda/stampo/releases/download/v#{version}/stampo-Linux-x86_64-musl.tar.gz"
      sha256 "0eb9cd4ec5b810e98d3cdc9d354926e61b2ab7a45d21a922740079906946c31c"
    end
  end

  def install
    bin.install "stampo"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/stampo --version")
  end
end
