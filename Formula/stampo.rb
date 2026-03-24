class Stampo < Formula
  desc "Turn any API or config into a real CLI — no code, no dependencies"
  homepage "https://github.com/MichaelCereda/stampo"
  version "2.3.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/MichaelCereda/stampo/releases/download/v#{version}/stampo-Darwin-aarch64.tar.gz"
      sha256 "dd3d7f6b8fabf1d8d9f69f54346b6e2c6fba2bbbecae38762f8e08a543f2ee44"
    else
      url "https://github.com/MichaelCereda/stampo/releases/download/v#{version}/stampo-Darwin-x86_64.tar.gz"
      sha256 "aaf0b0457c74507aa10500b0f3e49275c66d673ca363cb920854c586cb97349b"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/MichaelCereda/stampo/releases/download/v#{version}/stampo-Linux-aarch64-musl.tar.gz"
      sha256 "89e1c321ca279ba7b112557056f50c5596407e118bfc083fb1cc1dc18e89494e"
    else
      url "https://github.com/MichaelCereda/stampo/releases/download/v#{version}/stampo-Linux-x86_64-musl.tar.gz"
      sha256 "b41a7aab6c53315779ddab411e98c113a944a5ad007f201cc521d95a08045675"
    end
  end

  def install
    bin.install "stampo"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/stampo --version")
  end
end
