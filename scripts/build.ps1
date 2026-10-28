param(
    [ValidateSet('host','servers','client','all')][string]$Stage = 'all',
    [string]$Image = 'nostalre-switch-toolchain:work'
)
$ErrorActionPreference = 'Stop'
$editionRoot = Split-Path $PSScriptRoot -Parent
$sourceRoot = Join-Path $editionRoot 'source'
$buildRoot = Join-Path $editionRoot 'build'
foreach ($directory in @('host','servers','target','cargo-registry','tests')) {
    New-Item -ItemType Directory -Path (Join-Path $buildRoot $directory) -Force | Out-Null
}
foreach ($directory in @('evidence','outputs')) {
    New-Item -ItemType Directory -Path (Join-Path $editionRoot $directory) -Force | Out-Null
}
& docker image inspect $Image --format '{{.Id}}' | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Toolchain missing. Run scripts/setup.ps1 -Stage images.' }
if ($Stage -in @('client','all')) {
    foreach ($relative in @('source/bevy-horizon/nvk/lib/libvulkan_bh.a','source/nostalre-launcher/client/switch/romfs/server/ragnarok.db')) {
        if (-not (Test-Path -LiteralPath (Join-Path $editionRoot $relative))) { throw "Missing $relative; see docs/BUILDING.md" }
    }
}
$common = @('run','--rm','--memory','4g','-v',"${editionRoot}:/edition",'-v',"${sourceRoot}:/src")
function Invoke-ClassicBuild([string]$Name,[string[]]$Options,[string]$Script) {
    $logPath = Join-Path $editionRoot "evidence/build-$Name.log"
    & docker @common --name "ragswitch-pre-$Name" @Options $image bash "/edition/scripts/$Script" *> $logPath
    if ($LASTEXITCODE -ne 0) { throw "Build $Name failed; see $logPath" }
}
if ($Stage -in @('host','all')) { Invoke-ClassicBuild 'host' @('-v',"${buildRoot}/host:/host-build") 'build_host.sh' }
if ($Stage -in @('servers','all')) { Invoke-ClassicBuild 'servers' @('-v',"${buildRoot}/servers:/server-build") 'build_switch_servers.sh' }
if ($Stage -in @('client','all')) { Invoke-ClassicBuild 'client' @('-v',"${buildRoot}/target:/build",'-v',"${buildRoot}/cargo-registry:/usr/local/cargo/registry",'-e','CARGO_TARGET_DIR=/build') 'build_client.sh' }
