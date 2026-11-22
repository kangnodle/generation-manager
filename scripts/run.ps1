$ErrorActionPreference = 'Stop'

if ($env:OS -eq 'Windows_NT') {
    $scriptPath = & wsl --exec wslpath -a -u (Join-Path $PSScriptRoot 'run.sh')
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    & wsl --exec bash -c 'export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"; exec bash "$@"' bash "$scriptPath" @args
} else {
    & bash (Join-Path $PSScriptRoot 'run.sh') @args
}

exit $LASTEXITCODE
