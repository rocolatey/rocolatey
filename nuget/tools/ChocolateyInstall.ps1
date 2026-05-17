$ErrorActionPreference = 'Stop'
$toolsPath = Split-Path $MyInvocation.MyCommand.Definition

# 'Get-ChocolateyPath' is only available starting in choco 1.2+
# ... thus we cannot rely on it being available :-/
# $pkgBase = Get-ChocolateyPath -PathType 'PackagePath'
$rocoTabCompletion = Join-Path $env:ChocolateyInstall "lib\${env:ChocolateyPackageName}\tools\RocoTabCompletion.psm1"
if ($profile -and (Test-Path $profile)) {
	if (-not ((Get-Content $profile) -match '### RocolateyTabCompletion ###')) {
		@"
`n
### RocolateyTabCompletion ###
Import-Module "$rocoTabCompletion" -ErrorAction SilentlyContinue
`n
"@ | Out-File $profile -Append -Encoding utf8
	}
}

# If the server executable was packaged in tools, install it as a Windows Service using sc.exe
$serverExe = Join-Path $toolsPath 'rocolatey-server.exe'
$rocoExe = Join-Path $toolsPath 'roco.exe'
$serviceName = 'Rocolatey-Server'
if (Test-Path $serverExe) {
	Write-Host "Found rocolatey-server.exe at $serverExe. Installing Windows service '$serviceName'..."
	$serviceCreated = $false
	try {
		# prefer sc.exe for maximum compatibility
		$binPathQuoted = '"' + $serverExe + '"'
		# query service existence
		& sc.exe query $serviceName > $null 2>&1
		if ($LASTEXITCODE -ne 0) {
			Write-Host "Creating service '$serviceName' pointing to $serverExe"
			& sc.exe create $serviceName binPath= $binPathQuoted DisplayName= '"Rocolatey-Server"' start= auto
			$serviceCreated = $true
			Write-Host "Service '$serviceName' created."
		}
		else {
			Write-Host "Service '$serviceName' already exists; skipping installation."
		}

		if (Test-Path $rocoExe) {
			Write-Host 'Bootstrapping installer-account scoped local key exchange...'
			& $rocoExe server --bootstrap-local-trust
			if ($LASTEXITCODE -ne 0) {
				throw "Local key exchange bootstrap failed. Run 'roco server --bootstrap-local-trust' manually and retry."
			}
			Write-Host 'Local key exchange bootstrap completed.'
		}
		else {
			Write-Warning 'roco.exe not found in tools path; skipping local key exchange bootstrap.'
		}

		if ($serviceCreated) {
			& sc.exe start $serviceName > $null 2>&1
			Write-Host "Service '$serviceName' started."
		}
	}
 catch {
		Write-Error "Failed to configure service '$serviceName' and local trust bootstrap: $_"
		throw
	}
}
else {
	Write-Host 'rocolatey-server.exe not found in tools path; skipping service installation.'
}
