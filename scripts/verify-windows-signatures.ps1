[CmdletBinding()]
param(
    # The release output folder: src-tauri\target\<triple>\release
    [Parameter(Mandatory = $true)][string]$Root
)

# Fails the release when the app executable or any installer is not
# Authenticode-signed with a valid, timestamped signature. Runs only when
# signing was configured (docs/CODE_SIGNING.md).

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-SignTool {
    $sdkRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $signTool = Get-ChildItem -LiteralPath $sdkRoot -Recurse -File -Filter 'signtool.exe' -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } |
        Sort-Object FullName -Descending |
        Select-Object -First 1 -ExpandProperty FullName
    if ([string]::IsNullOrWhiteSpace($signTool)) { throw 'Windows SDK SignTool was not found.' }
    return $signTool
}

function Get-FilesToVerify([string]$Root) {
    $bundle = Join-Path $Root 'bundle'
    $installers = @(Get-ChildItem -LiteralPath $bundle -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Extension -in '.msi', '.exe' })
    $apps = @(Get-ChildItem -LiteralPath $Root -File -Filter '*.exe' -ErrorAction SilentlyContinue)
    if ($installers.Count -eq 0) { throw "No installers found under $bundle." }
    if ($apps.Count -eq 0) { throw "No app executable found in $Root." }
    return @($apps + $installers)
}

$signTool = Get-SignTool
$failures = @()
foreach ($file in Get-FilesToVerify $Root) {
    # /pa: default Authenticode policy; /tw: warn (non-zero) when not timestamped.
    $output = & $signTool verify /pa /tw $file.FullName 2>&1
    if ($LASTEXITCODE -ne 0) {
        $failures += "$($file.Name): $($output -join ' ')"
    } else {
        $subject = (Get-AuthenticodeSignature -LiteralPath $file.FullName).SignerCertificate.Subject
        Write-Host "Signed: $($file.Name) by $subject"
    }
}
if ($failures.Count -gt 0) {
    throw "Unsigned or invalid binaries:`n$($failures -join "`n")"
}
