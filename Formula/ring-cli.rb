class RingCli < Formula
  desc "Generate CLIs from YAML configs and OpenAPI specs"
  homepage "https://github.com/MichaelCereda/ring-cli"
  version "2.3.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/MichaelCereda/ring-cli/releases/download/v#{version}/ring-cli-Darwin-aarch64.tar.gz"
      sha256 "beee755fc962d4b1d11f7fc26f8a19f573dfc591597004d704bcc84bf2f690d4"
    else
      url "https://github.com/MichaelCereda/ring-cli/releases/download/v#{version}/ring-cli-Darwin-x86_64.tar.gz"
      sha256 "3b9eaa946d40b2d1bdf612ddd8e04f1069b21689116a8fba771efe0431fdec79"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/MichaelCereda/ring-cli/releases/download/v#{version}/ring-cli-Linux-aarch64-musl.tar.gz"
      sha256 "fe11d666c7bf1d65cce1bb8074c5df7c4e882fd4b9850b71112c9704734c4e99"
    else
      url "https://github.com/MichaelCereda/ring-cli/releases/download/v#{version}/ring-cli-Linux-x86_64-musl.tar.gz"
      sha256 "313d5c68e026d2a2b8302f0953c9dcb99da295eb975d03e10fc76411a180f728"
    end
  end

  def install
    bin.install "ring-cli"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/ring-cli --version")
  end
end
