$ErrorActionPreference = "Stop"

# roco.exe generate-shell-completions --powershell

$rocoExecutable = Join-Path $env:ChocolateyPackageFolder "tools\roco.exe"
$rocoTabCompletion = Join-Path $env:ChocolateyPackageFolder "tools\RocoTabCompletion.psm1"

(& $rocoExecutable generate-shell-completions --powershell) | Out-File $rocoTabCompletion -Encoding utf8

if ($profile -And (Test-Path $profile)) {
  if ((Get-Content $profile) -notmatch "RocolateyTabCompletion") {
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
