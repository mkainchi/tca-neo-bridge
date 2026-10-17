# Private daemon gates: clippy, 100% Rust lines/functions/regions (stable) and branches (nightly), 100% page JS.
$ErrorActionPreference = 'Stop'
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$daemon = Join-Path $PSScriptRoot '..\daemon'
Push-Location $daemon
try {
    cargo clippy --all-targets -- -D warnings; if ($LASTEXITCODE) { throw 'clippy failed' }
    # Stale instrumented binaries from the other toolchain skew reports; start each run clean.
    Remove-Item -Recurse -Force target/llvm-cov-target -ErrorAction SilentlyContinue
    cargo llvm-cov --summary-only --fail-under-lines 100 --fail-under-functions 100 --fail-under-regions 100; if ($LASTEXITCODE) { throw 'stable coverage gate failed' }
    Remove-Item -Recurse -Force target/llvm-cov-target -ErrorAction SilentlyContinue
    cargo +nightly llvm-cov --branch --summary-only --fail-under-lines 100 --fail-under-functions 100 --fail-under-regions 100
    if ($LASTEXITCODE) { throw 'nightly branch coverage gate failed' }
    $branches = cargo +nightly llvm-cov report --branch --summary-only | Select-String '^TOTAL'
    if ($branches -notmatch '100\.00%\s*$') { throw "branch coverage below 100%: $branches" }
} finally {
    Pop-Location
}
Push-Location (Resolve-Path (Join-Path $PSScriptRoot '..'))
try {
    npx.cmd vitest run --config vitest.config.js --coverage; if ($LASTEXITCODE) { throw 'page tests failed' }
} finally {
    Pop-Location
}
