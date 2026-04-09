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

    # 삭제 대상 경로의 프로젝트 루트 내부 여부 검증
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
    # 이전 빌드 캐시와 배포 산출물 초기화
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
    # 임시 타깃 디렉터리로 빌드해 최종 산출물만 분리
    cargo build --release --target-dir $temporaryTargetDir
    if ($LASTEXITCODE -ne 0) {
        throw "릴리스 빌드 실패"
    }

    $builtExePath = Join-Path $temporaryTargetDir "release\$exeName"
    if (-not (Test-Path -LiteralPath $builtExePath)) {
        throw "생성된 exe 파일 미발견: $builtExePath"
    }

    $destinationPath = Join-Path $outputDir $exeName
    # 배포 폴더에는 최종 exe 파일만 복사
    Copy-Item -LiteralPath $builtExePath -Destination $destinationPath -Force

    Write-Host ""
    Write-Host "빌드 완료"
    Write-Host "출력 파일: $destinationPath"
}
finally {
    # 임시 빌드 디렉터리 정리
    Remove-DirectoryIfExists -Path $temporaryTargetDir
}
