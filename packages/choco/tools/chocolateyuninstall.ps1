$packageName = 'gvc'

Write-Host "Uninstalling $packageName"

# Remove the executable
if (Test-Path "$env:ProgramData\chocolatey\bin\gvc.exe") {
    Remove-Item "$env:ProgramData\chocolatey\bin\gvc.exe" -Force -ErrorAction SilentlyContinue
}

# Remove temporary files
Get-ChildItem -Path $env:TEMP -Filter 'gvc*.zip' -Recurse -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue
Get-ChildItem -Path $env:TEMP -Filter 'gvc.exe' -Recurse -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue

Write-Host "$packageName has been uninstalled"
