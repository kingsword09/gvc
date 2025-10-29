# Packages Directory

This directory contains all package manager integrations for distributing GVC across different platforms.

## Directory Structure

```
packages/
├── README.md                     # This file
├── homebrew/
│   └── gvc.rb                    # Homebrew formula for macOS/Linux
├── winget/
│   └── kingsword09.GVC.yaml      # Winget manifest for Windows
├── choco/
│   ├── gvc.nuspec                # Chocolatey package specification
│   └── tools/
│       ├── chocolateyinstall.ps1 # Installation script
│       ├── chocolateyuninstall.ps1 # Uninstall script
│       └── VERIFICATION.txt      # Verification checklist
├── snap/
│   └── snapcraft.yaml           # Snap package definition
└── flatpak/
    └── kingsword09.GVC.yml      # Flatpak manifest
```

## Package Managers

### Homebrew (macOS/Linux)
- **Formula**: `homebrew/gvc.rb`
- **Installation**: `brew install gvc`
- **Platforms**: macOS (Intel & Apple Silicon), Linux
- **Repository**: [Homebrew/homebrew-core](https://github.com/Homebrew/homebrew-core)

### Winget (Windows)
- **Manifest**: `winget/kingsword09.GVC.yaml`
- **Installation**: `winget install GVC.GVC`
- **Platforms**: Windows (x64, ARM64)
- **Repository**: [Microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs)

### Chocolatey (Windows)
- **Package**: `choco/gvc.nuspec`
- **Installation**: `choco install gvc`
- **Platforms**: Windows (x64, ARM64)
- **Repository**: [Chocolatey Community Repository](https://community.chocolatey.org/)

### Snap (Linux)
- **Definition**: `snap/snapcraft.yaml`
- **Installation**: `sudo snap install gvc`
- **Platforms**: Linux (all distributions)
- **Repository**: [Snap Store](https://snapcraft.io/store)

### Flatpak (Linux)
- **Manifest**: `flatpak/kingsword09.GVC.yml`
- **Installation**: `flatpak install flathub com.gvc.cli`
- **Platforms**: Linux (all distributions)
- **Repository**: [Flathub](https://flathub.org/)

## Quick Start

### Testing Packages Locally

#### Homebrew
```bash
# Install locally
brew install --formula ./packages/homebrew/gvc.rb

# Test installation
brew test ./packages/homebrew/gvc.rb
```

#### Winget
```powershell
# Validate manifest
winget validate .\packages\winget\kingsword09.GVC.yaml
```

#### Chocolatey
```powershell
# Pack and install locally
choco pack .\packages\choco\gvc.nuspec
choco install gvc --source .
```

#### Snap
```bash
# Install snapcraft
sudo snap install snapcraft --classic

# Build the snap
cd packages/snap
snapcraft

# Install locally
sudo snap install --dangerous gvc_*.snap
```

#### Flatpak
```bash
# Build and install locally
flatpak-builder --user --install build-dir packages/flatpak/kingsword09.GVC.yml
```

### Updating Packages

When releasing a new version, update:

1. **Version number** in the respective package files
2. **Download URLs** to point to the new release
3. **SHA256 checksums** for the new binaries

### Automated Updates

GitHub Actions workflow `.github/workflows/update-package-managers.yml` can automate the update process:

1. Downloads release artifacts
2. Calculates checksums
3. Updates package versions
4. Creates PRs to update package repositories

To enable:

1. Create separate repositories for your packages:
   - `homebrew-gvc` for Homebrew
   - Fork `microsoft/winget-pkgs` for Winget
   - `chocolatey-packages` for Chocolatey
   - `snapcrafters/gvc` for Snap
   - Fork `flathub/kingsword09.GVC` for Flatpak

2. Add GitHub tokens as secrets:
   - `HOMEBREW_REPO_TOKEN`
   - `WINGET_PKGS_TOKEN`
   - `CHOCOLATEY_REPO_TOKEN`
   - `SNAP_REPO_TOKEN`
   - `FLATHUB_REPO_TOKEN`

## Documentation

See [PACKAGE_MANAGERS.md](../PACKAGE_MANAGERS.md) for complete integration guide including:

- Submission process for each package manager
- Automated workflow configuration
- Testing and validation procedures
- Troubleshooting common issues

## License

All package manager files are licensed under the same terms as the main project (Apache-2.0).
