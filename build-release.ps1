Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$utf8Encoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8Encoding
$OutputEncoding = $utf8Encoding

$projectRoot = [System.IO.Path]::GetFullPath((Split-Path -Parent $MyInvocation.MyCommand.Path))
$outputDir = Join-Path $projectRoot "dist"
$temporaryTargetDir = Join-Path $projectRoot ".build-target"
$exeName = "rust-caffeine.exe"

function Resolve-ProjectPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    if (-not $resolvedPath.StartsWith($projectRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "프로젝트 루트 바깥 경로 접근 차단: $resolvedPath"
    }

    return $resolvedPath
}

function Remove-DirectoryIfExists {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $resolvedPath = Resolve-ProjectPath -Path $Path
    if (Test-Path -LiteralPath $resolvedPath) {
        Remove-Item -LiteralPath $resolvedPath -Recurse -Force
    }
}

function Initialize-OutputDirectory {
    Remove-DirectoryIfExists -Path $outputDir
    $null = New-Item -ItemType Directory -Path $outputDir
}

function Clear-LegacyArtifacts {
    $artifactDirectories = Get-ChildItem -LiteralPath $projectRoot -Force -Directory | Where-Object {
        $_.Name -eq "target" -or
        $_.Name -eq ".build-target" -or
        $_.Name -eq "dist" -or
        $_.Name -like ".codex-target*"
    }

    foreach ($directory in $artifactDirectories) {
        Remove-DirectoryIfExists -Path $directory.FullName
    }
}

Clear-LegacyArtifacts
Initialize-OutputDirectory

try {
    cargo build --release --target-dir $temporaryTargetDir
    if ($LASTEXITCODE -ne 0) {
        throw "릴리스 빌드 실패"
    }

    $builtExePath = Join-Path $temporaryTargetDir "release\$exeName"
    if (-not (Test-Path -LiteralPath $builtExePath)) {
        throw "생성된 exe 파일 미발견: $builtExePath"
    }

    $destinationPath = Join-Path $outputDir $exeName
    Copy-Item -LiteralPath $builtExePath -Destination $destinationPath -Force

    Write-Host ""
    Write-Host "빌드 완료"
    Write-Host "출력 파일: $destinationPath"
}
finally {
    Remove-DirectoryIfExists -Path $temporaryTargetDir
}
