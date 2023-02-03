$ErrorActionPreference = "Stop"

# 'Get-ChocolateyPath' is only available starting in choco 1.2+
# ... thus we cannot rely on it being available :-/
# $pkgBase = Get-ChocolateyPath -PathType 'PackagePath'
$rocoTabCompletion = Join-Path $env:ChocolateyInstall "lib\${env:ChocolateyPackageName}\tools\RocoTabCompletion.psm1"
if ($profile -And (Test-Path $profile)) {
  if (-Not ((Get-Content $profile) -match "### RocolateyTabCompletion ###")) {
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
