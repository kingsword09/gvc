class Gvc < Formula
  desc "CLI manager for Gradle version catalogs—check, list, update, and add dependencies"
  homepage "https://github.com/kingsword09/gvc"
  version "0.1.1"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/kingsword09/gvc/releases/download/v#{version}/gvc-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ARM64_SHA256"
    elsif Hardware::CPU.intel?
      url "https://github.com/kingsword09/gvc/releases/download/v#{version}/gvc-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_INTEL_SHA256"
    end
  elsif OS.linux?
    url "https://github.com/kingsword09/gvc/releases/download/v#{version}/gvc-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "REPLACE_WITH_LINUX_SHA256"
  end

  license "Apache-2.0"

  depends_on "rust" => :build

  def install
    bin.install "gvc"
    man1.install "gvc.1" if File.exist?("gvc.1")
    bash_completion.install "gvc.bash" if File.exist?("gvc.bash")
    zsh_completion.install "_gvc" if File.exist?("_gvc")
  end

  test do
    assert_match "gvc", shell_output("#{bin}/gvc --version")
  end
end
