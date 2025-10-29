$packageName = 'gvc'
$version = '0.1.1'
$url32 = 'https://github.com/kingsword09/gvc/releases/download/v0.1.1/gvc-x86_64-pc-windows-msvc.zip'
$url64 = $url32
$checksum32 = 'REPLACE_WITH_WINDOWS_SHA256'
$checksum64 = $checksum32
$validExitCodes = @(0)

Write-Host "Installing $packageName version $version"
Install-ChocolateyZipPackage -PackageName $packageName `
                              -Url $url32 `
                              -UnzipLocation $env:TEMP `
                              -Checksum $checksum32 `
                              -ChecksumType 'sha256'

Move-Item "$env:TEMP\gvc.exe" "$env:ProgramData\chocolatey\bin\gvc.exe" -Force

# Add to PATH if not already there
$env:Path = [System.Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [System.Environment]::GetEnvironmentVariable('Path','User')
