param(
    [ValidateSet('units','world','graphics')][string]$Stage = 'units',
    [string]$Image = 'nostalre-client-tests:work'
)
$ErrorActionPreference = 'Stop'
$editionRoot = Split-Path $PSScriptRoot -Parent
$sourceRoot = Join-Path $editionRoot 'source'
$buildRoot = Join-Path $editionRoot 'build'
foreach ($directory in @('evidence','outputs','build/host','build/tests','build/cargo-registry')) {
    New-Item -ItemType Directory -Path (Join-Path $editionRoot $directory) -Force | Out-Null
}
& docker image inspect $Image --format '{{.Id}}' | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Test image missing. Run scripts/setup.ps1 -Stage images.' }
$common = @('run','--rm','--memory','4g','-v',"${editionRoot}:/edition",'-v',"${sourceRoot}:/src",'-v',"${buildRoot}/host:/host-build",'-v',"${buildRoot}/tests:/test-build",'-v',"${buildRoot}/cargo-registry:/usr/local/cargo/registry",'-e','CARGO_TARGET_DIR=/test-build/client-host',$Image)
if ($Stage -eq 'units') { & docker @common bash /edition/scripts/prepare_tests.sh }
if ($Stage -eq 'world') { & docker @common python3 -u /edition/scripts/test_world.py }
if ($Stage -eq 'graphics') { & docker @common python3 -u /edition/scripts/test_graphics.py }
if ($LASTEXITCODE -ne 0) { throw "Classic test stage $Stage failed" }
