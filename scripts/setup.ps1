param(
    [ValidateSet('check','images','nvk','seed','all')][string]$Stage = 'check',
    [string]$Image = 'nostalre-switch-toolchain:work',
    [string]$TestImage = 'nostalre-client-tests:work',
    [string]$NvkImage = 'ragswitch-pre-nvk:local'
)
$ErrorActionPreference = 'Stop'
$editionRoot = Split-Path $PSScriptRoot -Parent
foreach ($directory in @('build','outputs','evidence')) {
    New-Item -ItemType Directory -Path (Join-Path $editionRoot $directory) -Force | Out-Null
}
if ($Stage -in @('images','all')) {
    & docker build --file (Join-Path $editionRoot 'source/bevy-horizon/Containerfile') --tag $Image (Join-Path $editionRoot 'toolchain')
    if ($LASTEXITCODE -ne 0) { throw 'Toolchain image build failed' }
    foreach ($entry in @(@('Tests.Containerfile',$TestImage),@('Nvk.Containerfile',$NvkImage))) {
        & docker build --build-arg "TOOLCHAIN_IMAGE=$Image" --file (Join-Path $editionRoot "toolchain/$($entry[0])") --tag $entry[1] (Join-Path $editionRoot 'toolchain')
        if ($LASTEXITCODE -ne 0) { throw "Image build failed: $($entry[1])" }
    }
}
if ($Stage -in @('nvk','all')) {
    $editionMount = '{0}:/edition' -f $editionRoot
    $sourceMount = '{0}/source:/src' -f $editionRoot
    & docker run --rm --memory 4g -v $editionMount -v $sourceMount $NvkImage bash /edition/scripts/build_nvk.sh
    if ($LASTEXITCODE -ne 0) { throw 'NVK build failed; existing libraries are preserved until staging' }
}
if ($Stage -in @('seed','all')) {
    $seed = Join-Path $editionRoot 'source/nostalre-launcher/client/switch/romfs/server/ragnarok.db'
    if (Test-Path -LiteralPath $seed) { Write-Output 'Existing seed preserved.' }
    else {
        & python (Join-Path $PSScriptRoot 'setup_seed.py')
        if ($LASTEXITCODE -ne 0) { throw 'Seed creation failed' }
    }
}
if ($Stage -eq 'check') {
    & docker image inspect $Image $TestImage --format '{{.Id}}'
    if ($LASTEXITCODE -ne 0) { throw 'Missing images. Run scripts/setup.ps1 -Stage images.' }
    foreach ($relative in @('source/bevy-horizon/nvk/lib/libvulkan_bh.a','source/nostalre-launcher/client/switch/romfs/server/ragnarok.db')) {
        if (-not (Test-Path -LiteralPath (Join-Path $editionRoot $relative))) { throw "Missing prerequisite: $relative" }
    }
    Write-Output 'Local prerequisites found; clean rebuild and performance are not certified.'
}
