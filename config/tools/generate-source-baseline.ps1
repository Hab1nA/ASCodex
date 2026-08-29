param(
  [Parameter(Mandatory = $true)]
  [string]$Source,
  [Parameter(Mandatory = $true)]
  [string]$Output
)

$resolvedSource = (Resolve-Path -LiteralPath $Source).Path
$files = Get-ChildItem -LiteralPath $resolvedSource -File -Recurse |
  Where-Object { $_.FullName -notmatch '\\(?:\.git|target|node_modules|dist|build)\\' } |
  Sort-Object FullName

$records = foreach ($file in $files) {
  $hash = Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256
  [ordered]@{
    path = $file.FullName.Substring($resolvedSource.Length).TrimStart('\')
    bytes = $file.Length
    sha256 = $hash.Hash
  }
}

$document = [ordered]@{
  generated_at_utc = [DateTime]::UtcNow.ToString('o')
  source = $resolvedSource
  file_count = @($records).Count
  files = @($records)
}

$parent = Split-Path -Parent $Output
New-Item -ItemType Directory -Force -Path $parent | Out-Null
$document | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $Output -Encoding UTF8
