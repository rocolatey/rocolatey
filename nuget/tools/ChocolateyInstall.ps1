$ErrorActionPreference = "Stop"

$rocoTabCompletion = Join-Path $env:ChocolateyPackageFolder "tools\RocoTabCompletion.psm1"

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
