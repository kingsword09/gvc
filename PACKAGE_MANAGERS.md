# Package Manager Integration Guide

This guide explains how to distribute GVC through popular package managers on macOS, Windows, and Linux.

## Overview

GVC can be installed through multiple channels:

- **crates.io** - Rust's official package registry (recommended for Rust users)
- **Homebrew** - macOS/Linux package manager (recommended for macOS/Linux users)
- **Winget** - Windows package manager (recommended for Windows users)
- **Chocolatey** - Windows package manager (alternative for Windows users)
- **Snap** - Cross-distribution Linux package manager
- **Flatpak** - Cross-distribution Linux package manager with sandbox
- **GitHub Releases** - Pre-built binaries (recommended for direct installation)

## Homebrew (macOS/Linux)

### Method 1: Submit to Homebrew Core (Official)

This is the most common method and gives users the simplest installation command: `brew install gvc`.

#### Prerequisites

1. You have a GitHub account
2. The formula is tested and working
3. You have a Homebrew/homebrew-crate tap in your repository

#### Steps

1. **Prepare your repository structure**:

   Create a separate repository for your Homebrew formula (e.g., `homebrew-gvc`) with this structure:

   ```
   homebrew-gvc/
   └── Formula/
       └── gvc.rb
   ```

2. **Test the formula locally** (optional but recommended):

   ```bash
   brew install --formula ./packages/homebrew/gvc.rb
   brew test ./packages/homebrew/gvc.rb
   ```

3. **Submit to Homebrew Core**:

   Homebrew Core is the official collection of formulas. To contribute:

   - Fork [Homebrew/homebrew-cask](https://github.com/Homebrew/homebrew-core) (for CLI tools, we use homebrew-core, not homebrew-cask)
   - Add your formula to `Formula/gvc.rb`
   - Create a pull request with:
     - Test results showing `brew test` passes
     - Full audit results
     - Clean style check (`brew audit --strict gvc`)

   For more details, see: [Homebrew Contributing Guidelines](https://docs.brew.sh/How-To-Open-a-Homebrew-Pull-Request)

#### Method 2: Create Your Own Tap

If you prefer to maintain your own tap:

1. Create a repository named `homebrew-<tap-name>` (e.g., `homebrew-gvc`)
2. Follow the [Tap creation guide](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap)
3. Users will install using: `brew install kingsword09/gvc/gvc`

#### Installation via Homebrew

Once published, users can install with:

```bash
# If published to Homebrew Core
brew install gvc

# If using a custom tap
brew install kingsword09/gvc/gvc
```

### Updating Homebrew Formula

When releasing a new version:

1. Update the `version` in `packages/homebrew/gvc.rb`
2. Update the `url` to point to the new release
3. Calculate and update SHA256 checksums:
   ```bash
   shasum -a 256 gvc-*.tar.gz
   ```
4. Submit the changes via PR or update your tap

### Automated Updates

We've set up GitHub Actions workflow `.github/workflows/update-package-managers.yml` that can:

- Download release artifacts
- Calculate checksums
- Create PRs to update the formula (requires proper repository setup)

To use this:

1. Set up a separate `homebrew-gvc` repository
2. Add repository URL to your formula's `bottle do` block if using bottles
3. Configure `HOMEBREW_REPO_TOKEN` secret in your GitHub repository
4. The workflow will trigger on each release

## Winget (Windows)

### Submit to Community Repository

Winget packages are distributed through the [winget-pkgs](https://github.com/microsoft/winget-pkgs) repository.

#### Prerequisites

1. You have a GitHub account
2. Your package is available on GitHub Releases
3. You have created the manifest following our template in `packages/winget/kingsword09.GVC.yaml`

#### Steps

1. **Fork the winget-pkgs repository**:
   ```bash
   git clone https://github.com/microsoft/winget-pkgs
   cd winget-pkgs
   git remote add upstream https://github.com/microsoft/winget-pkgs
   ```

2. **Create the directory structure**:
   ```
   winget-pkgs/
   └── manifests/
       └── k/
           └── kingsword09/
               └── GVC/
                   └── [version]/
                       └── kingsword09.GVC.yaml
   ```

3. **Calculate SHA256 for your installers**:
   ```powershell
   # Using PowerShell
   Get-FileHash -Path .\gvc-x86_64-pc-windows-msvc.zip -Algorithm SHA256

   # Using wingetutil (recommended)
   wingetcreate.exe hash .\gvc-x86_64-pc-windows-msvc.zip
   ```

4. **Update the manifest**:
   - Copy the manifest from `packages/winget/kingsword09.GVC.yaml` to `manifests/k/kingsword09/GVC/[version]/`
   - Replace `REPLACE_WITH_WINDOWS_SHA256` with the calculated hash
   - Update the `PackageVersion` field

5. **Test the manifest**:
   ```powershell
   # Validate the manifest
   Import-Module .\Tools\Yaml\ -Force
   Test-ManifestFile -ManifestFilePath .\manifests\k\kingsword09\GVC\[version]\kingsword09.GVC.yaml
   ```

6. **Submit via Pull Request**:
   - Push your changes to a fork
   - Create a PR with:
     - Clear title: `New package: GVC.GVC version 0.1.1`
     - Description explaining the package
     - Screenshots if applicable
     - Manifest validation results

7. **PR Review Process**:
   - The winget team reviews submissions
   - Automated validation checks run
   - Once approved, the package is merged and available within hours

#### Installation via Winget

After merging, users can install with:

```powershell
winget install GVC.GVC
```

### Updating Winget Manifest

When releasing a new version:

1. Create a new directory `manifests/k/kingsword09/GVC/[new-version]/`
2. Copy the previous manifest
3. Update:
   - `PackageVersion`
   - `InstallerUrl` to point to the new release
   - `InstallerSha256` with the new hash
4. Submit a PR with these changes

### Automated Updates

The GitHub Actions workflow `.github/workflows/update-package-managers.yml` can automate:

- Downloading Windows release assets
- Calculating SHA256 checksums
- Creating PRs to update manifests

To use this:

1. Fork and checkout `microsoft/winget-pkgs` to your account
2. Generate a GitHub token with `package` permissions
3. Add the token as `WINGET_PKGS_TOKEN` secret
4. The workflow will trigger on each release

## Chocolatey (Windows)

### Submit to Chocolatey Community Repository

Chocolatey packages are distributed through the [Chocolatey Community Repository](https://community.chocolatey.org/packages).

#### Prerequisites

1. You have a Chocolatey account (free registration)
2. Your package is available on GitHub Releases
3. You have created the package following our template in `packages/choco/`

#### Steps

1. **Test the package locally** (optional but recommended):

   ```powershell
   choco install gvc --source .
   choco uninstall gvc
   ```

2. **Calculate checksums**:

   ```powershell
   Get-FileHash -Path .\gvc-x86_64-pc-windows-msvc.zip -Algorithm SHA256
   ```

3. **Update package files**:
   - Update `packages/choco/gvc.nuspec` with the new version
   - Update `packages/choco/tools/chocolateyinstall.ps1` with the new URL and SHA256
   - Update `packages/choco/tools/VERIFICATION.txt` with the new SHA256

4. **Pack the package**:

   ```powershell
   choco pack packages\choco\gvc.nuspec
   ```

5. **Test the package again**:

   ```powershell
   choco install gvc --source .
   gvc --version
   choco uninstall gvc
   ```

6. **Submit to Chocolatey Community Repository**:

   Visit [https://community.chocolatey.org/packages/upload](https://community.chocolatey.org/packages/upload)
   - Sign in with your Chocolatey account
   - Upload the `.nupkg` file
   - Provide a clear package description
   - Submit for review

7. **PR Review Process**:
   - Chocolatey moderators review submissions
   - Automated validation checks run
   - Once approved, the package is published and available

#### Installation via Chocolatey

After publishing, users can install with:

```powershell
choco install gvc
```

### Updating Chocolatey Package

When releasing a new version:

1. Update the `version` in `packages/choco/gvc.nuspec`
2. Update URLs and checksums in `packages/choco/tools/chocolateyinstall.ps1`
3. Update verification checksum in `packages/choco/tools/VERIFICATION.txt`
4. Repack and resubmit via the web interface

### Automated Updates

The GitHub Actions workflow `.github/workflows/update-package-managers.yml` can automate:

- Downloading Windows release assets
- Calculating SHA256 checksums
- Updating version numbers
- Creating PRs to update your package repository

To use this:

1. Create a repository (e.g., `chocolatey-packages`)
2. Generate a GitHub token with `package` permissions
3. Add the token as `CHOCOLATEY_REPO_TOKEN` secret
4. The workflow will trigger on each release

## Snap (Linux)

### Submit to Snap Store

Snap packages are distributed through the [Snap Store](https://snapcraft.io/store).

#### Prerequisites

1. You have a Snapcraft account (free registration)
2. Your package is available on GitHub Releases
3. You have created the snapcraft.yaml following our template in `packages/snap/`

#### Steps

1. **Test the snap locally** (optional but recommended):

   ```bash
   # Install snapcraft
   sudo snap install snapcraft --classic

   # Build the snap
   snapcraft

   # Install locally
   sudo snap install --dangerous gvc_*.snap

   # Test installation
   gvc --version

   # Uninstall
   sudo snap remove gvc
   ```

2. **Register the snap name**:

   ```bash
   snapcraft register gvc
   ```

3. **Push the snap to the store**:

   ```bash
   snapcraft --use-experimental-pack
   snapcraft push gvc_*.snap
   ```

4. **Release to channels**:

   ```bash
   snapcraft release gvc <revision> stable
   ```

   Common channels:
   - `stable` - Production releases
   - `candidate` - Pre-release testing
   - `beta` - Early testing
   - `edge` - Development builds

#### Installation via Snap

After publishing, users can install with:

```bash
sudo snap install gvc
```

To enable automatic updates:

```bash
sudo snap set gvc autoupdate=true
```

### Updating Snap Package

When releasing a new version:

1. Update the `version` in `packages/snap/snapcraft.yaml`
2. Update the `source` URL to point to the new release
3. Rebuild and push to the store:
   ```bash
   snapcraft
   snapcraft push gvc_*.snap
   snapcraft release gvc <revision> stable
   ```

### Automated Updates

The GitHub Actions workflow `.github/workflows/update-package-managers.yml` can automate:

- Downloading Linux release assets
- Updating snapcraft.yaml with new version and URL
- Creating PRs to update your snap repository

To use this:

1. Create a repository (e.g., `snapcrafters/gvc`)
2. Generate a GitHub token with `package` permissions
3. Add the token as `SNAP_REPO_TOKEN` secret
4. The workflow will trigger on each release

## Flatpak (Linux)

### Submit to Flathub

Flatpak packages are distributed through [Flathub](https://flathub.org/), the official store for Flatpak applications.

#### Prerequisites

1. You have a Flathub account (free registration)
2. Your package is available on GitHub Releases
3. You have created the manifest following our template in `packages/flatpak/`

#### Steps

1. **Test the flatpak locally** (optional but recommended):

   ```bash
   # Install flatpak and build tools
   flatpak --user remote-add --no-gpg-verify flathub https://flathub.org/repo/flathub.flatpakrepo
   flatpak install flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08

   # Build and install locally
   flatpak-builder --user --install build-dir packages/flatpak/kingsword09.GVC.yml

   # Test installation
   flatpak run com.gvc.cli --version

   # Uninstall
   flatpak uninstall com.gvc.cli
   ```

2. **Calculate SHA256 for your package**:

   ```bash
   curl -L -O https://github.com/kingsword09/gvc/releases/download/v0.1.1/gvc-v0.1.1-linux-x86_64.tar.gz
   sha256sum gvc-v0.1.1-linux-x86_64.tar.gz
   ```

3. **Update the manifest**:
   - Update `packages/flatpak/kingsword09.GVC.yml` with the new version
   - Update the download URL
   - Update the SHA256 checksum

4. **Submit to Flathub**:

   Fork [flathub/org.freedesktop.Sdk](https://github.com/flathub/flathub) and submit a PR:
   - Add your manifest to `apps/com.gvc.cli/com.gvc.cli.yml`
   - Include verification that the package builds successfully
   - Provide clear description and metadata

5. **PR Review Process**:
   - Flathub reviewers check submissions
   - Automated CI builds and tests the application
   - Once approved, the app is published to Flathub

#### Installation via Flatpak

After publishing, users can install with:

```bash
flatpak install flathub com.gvc.cli
```

### Updating Flatpak Package

When releasing a new version:

1. Update the `version` in `packages/flatpak/kingsword09.GVC.yml`
2. Update the `url` to point to the new release
3. Update the `sha256` with the new hash
4. Submit a PR to the flathub repository

### Automated Updates

The GitHub Actions workflow `.github/workflows/update-package-managers.yml` can automate:

- Downloading Linux release assets
- Calculating SHA256 checksums
- Updating manifest version, URL, and checksum
- Creating PRs to update your flathub repository

To use this:

1. Fork the flathub repository
2. Generate a GitHub token with `package` permissions
3. Add the token as `FLATHUB_REPO_TOKEN` secret
4. The workflow will trigger on each release

## Release Workflow Integration

### Automated Release Process

When you create a new release on GitHub:

1. The `release.yml` workflow:
   - Builds binaries for all platforms (Linux, macOS, Windows)
   - Uploads artifacts to the release
   - Publishes to crates.io

2. The `update-package-managers.yml` workflow (optional):
   - Downloads the release artifacts
   - Calculates checksums
   - Creates PRs to update Homebrew and Winget packages

### Manual Release Process

If you prefer manual control:

1. Build and test your changes
2. Tag and push a new version:
   ```bash
   git tag v0.1.2
   git push origin v0.1.2
   ```
3. GitHub Actions creates the release and builds artifacts
4. Manually update Homebrew formula
5. Manually update Winget manifest

## File Locations

All package manager files are located in the `packages/` directory:

- **Homebrew Formula**: `packages/homebrew/gvc.rb`
- **Winget Manifest**: `packages/winget/kingsword09.GVC.yaml`
- **Chocolatey Package**: `packages/choco/gvc.nuspec` (and `tools/` scripts)
- **Snap Package**: `packages/snap/snapcraft.yaml`
- **Flatpak Manifest**: `packages/flatpak/kingsword09.GVC.yml`
- **GitHub Actions**: `.github/workflows/update-package-managers.yml`

## Testing Before Submission

### Homebrew

```bash
# Install locally
brew install --formula ./packages/homebrew/gvc.rb

# Test installation
brew test ./packages/homebrew/gvc.rb

# Audit for issues
brew audit --formula ./packages/homebrew/gvc.rb
brew audit --strict --online ./packages/homebrew/gvc.rb
```

### Winget

```powershell
# Install from local manifest
winget install --manifest .\packages\winget\

# Validate manifest
winget validate .\packages\winget\kingsword09.GVC.yaml
```

### Chocolatey

```powershell
# Pack the package
choco pack .\packages\choco\gvc.nuspec

# Install from local source
choco install gvc --source .

# Test the installation
gvc --version

# Uninstall
choco uninstall gvc

# Verify checksum
Get-FileHash -Path .\gvc-x86_64-pc-windows-msvc.zip -Algorithm SHA256
```

### Snap

```bash
# Install snapcraft
sudo snap install snapcraft --classic

# Build the snap
cd packages/snap
snapcraft

# Install locally (dangerous flag for local build)
sudo snap install --dangerous gvc_*.snap

# Test installation
gvc --version

# Run the snap
snap run gvc check

# Uninstall
sudo snap remove gvc

# Verify build
snapcraft lint
```

### Flatpak

```bash
# Install Flatpak and build tools
flatpak --user remote-add --no-gpg-verify flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08

# Build and install locally
flatpak-builder --user --install build-dir packages/flatpak/kingsword09.GVC.yml

# Test installation
flatpak run kingsword09.GVC --version

# Run the application
flatpak run kingsword09.GVC check

# Verify checksum
curl -L -O https://github.com/kingsword09/gvc/releases/download/v0.1.1/gvc-v0.1.1-linux-x86_64.tar.gz
sha256sum gvc-v0.1.1-linux-x86_64.tar.gz

# Uninstall
flatpak uninstall kingsword09.GVC

# Clean up build
flatpak-builder --user --clean build-dir packages/flatpak/kingsword09.GVC.yml
```

## Common Issues and Solutions

### Homebrew

**Issue**: SHA256 mismatch
```bash
# Solution: Recalculate checksum
shasum -a 256 path/to/downloaded/file.tar.gz
```

**Issue**: Formula audit fails
```bash
# Solution: Run with more details
brew audit --formula --verbose gvc
```

### Winget

**Issue**: Manifest validation fails
```powershell
# Solution: Use the wingetcreate tool
wingetcreate.exe validate --manifest .\packages\winget\kingsword09.GVC.yaml
```

**Issue**: Hash mismatch
```powershell
# Solution: Recalculate with wingetcreate
wingetcreate.exe hash .\gvc-windows.zip --output .\hash.txt
```

### Chocolatey

**Issue**: Package fails to install
```powershell
# Solution: Check execution policy
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser

# Solution: Verify checksum matches
Get-FileHash -Path .\gvc-x86_64-pc-windows-msvc.zip -Algorithm SHA256
```

**Issue**: Package validation fails
```powershell
# Solution: Repack with correct structure
choco pack .\packages\choco\gvc.nuspec

# Solution: Verify nuspec file
Test-ChocolateyPackage -Path .\gvc.0.1.1.nupkg
```

### Snap

**Issue**: Snap build fails
```bash
# Solution: Check for missing dependencies
snapcraft clean
snapcraft

# Solution: Verify snapcraft.yaml syntax
snapcraft lint
```

**Issue**: Snap won't install or run
```bash
# Solution: Check snap permissions
snap connections gvc
snap info gvc

# Solution: Enable classic confinement (if needed)
sudo snap install gvc --classic

# Solution: Check logs
snap logs gvc
```

**Issue**: Content interface not available
```bash
# Solution: Connect necessary interfaces
sudo snap connect gvc:home
sudo snap connect gvc:network
sudo snap connect gvc:network-bind
```

### Flatpak

**Issue**: Flatpak build fails
```bash
# Solution: Clean build directory
flatpak-builder --user --clean build-dir packages/flatpak/kingsword09.GVC.yml

# Solution: Update runtime
flatpak --user remote-modify --url=https://flathub.org/repo/flathub.flatpakrepo flathub
flatpak update
```

**Issue**: Runtime not found
```bash
# Solution: Install required runtime
flatpak install flathub org.freedesktop.Platform//24.08
flatpak install flathub org.freedesktop.Sdk//24.08
```

**Issue**: Flatpak app won't run
```bash
# Solution: Check app info
flatpak info com.gvc.cli

# Solution: Verify permissions
flatpak run --command=sh com.gvc.cli
# Inside the shell:
cat /app/bin/gvc
ldd /app/bin/gvc

# Solution: Run with debug
flatpak run --log-kinds=hostname,kernel,security,caller com.gvc.cli check --verbose
```

**Issue**: SHA256 checksum mismatch
```bash
# Solution: Recalculate and update
curl -L -O https://github.com/kingsword09/gvc/releases/download/v0.1.1/gvc-v0.1.1-linux-x86_64.tar.gz
sha256sum gvc-v0.1.1-linux-x86_64.tar.gz
# Update the hash in kingsword09.GVC.yml
```

## Resources

### Homebrew
- [Homebrew Documentation](https://docs.brew.sh/)
- [Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Contributing to Homebrew](https://github.com/Homebrew/brew/blob/master/CONTRIBUTING.md)

### Winget
- [Winget Documentation](https://docs.microsoft.com/en-us/windows/package-manager/winget/)
- [Manifest Specification](https://github.com/microsoft/winget-pkgs/blob/master/Packaging.md)
- [WingetCreate Tool](https://github.com/microsoft/winget-create)

### Chocolatey
- [Chocolatey Documentation](https://docs.chocolatey.org/)
- [Creating Packages](https://docs.chocolatey.org/en-us/create/create-packages)
- [Package Mantainer Handbook](https://docs.chocolatey.org/en-us/create/package-t mantleainer-handbook)

### Snap
- [Snapcraft Documentation](https://snapcraft.io/docs)
- [Building Snap Packages](https://snapcraft.io/docs/build-snaps)
- [Snap Store Publishing](https://snapcraft.io/docs/releasing-to-the-snap-store)
- [Snap Confinement](https://snapcraft.io/docs/snap-confinement)

### Flatpak
- [Flatpak Documentation](https://docs.flatpak.org/)
- [Building Flatpak Applications](https://docs.flatpak.org/en/latest/first-build.html)
- [Flathub Submission](https://github.com/flathub/flathub/wiki/App-Submission)
- [Flatpak Builder](https://docs.flatpak.org/en/latest/flatpak-builder.html)

### GitHub Actions
- [Uploading Release Assets](https://github.com/actions/upload-artifact)
- [Creating Pull Requests](https://github.com/peter-evans/create-pull-request)

## Support

If you encounter issues:

1. Check the [Troubleshooting](#common-issues-and-solutions) section
2. Review the respective package manager's documentation
3. Open an issue in this repository
