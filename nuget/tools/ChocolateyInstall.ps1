$ErrorActionPreference = "Stop"

$pkgBase = Get-ChocolateyPath -PathType 'PackagePath'
$rocoTabCompletion = Join-Path $pkgBase "tools\RocoTabCompletion.psm1"

if ($profile -And (Test-Path $profile)) {
  if ((Get-Content $profile) -notmatch "### RocolateyTabCompletion ###") {
    @"
`n
### RocolateyTabCompletion ###
`$RocolateyProfile = "$rocoTabCompletion"
if (Test-Path(`$RocolateyProfile)) {
  Import-Module "`$RocolateyProfile"
}`n
"@ | Out-File $profile -Append -Encoding utf8
  }
}
