<#
  Uninstalls the Rocolatey-Server Windows service if present.
  Called during Chocolatey package uninstall.
#>
$ErrorActionPreference = 'Stop'
$serviceName = 'Rocolatey-Server'

Write-Host "Checking for service '$serviceName'..."
& sc.exe query $serviceName > $null 2>&1
if ($LASTEXITCODE -ne 0) {
	Write-Host "Service '$serviceName' not installed; nothing to remove."
	return
}

Write-Host "Service '$serviceName' installed. Attempting to stop..."
try {
	& sc.exe stop $serviceName > $null 2>&1
}
catch {
	Write-Warning "Failed to send stop command to service '$serviceName': $_"
}

# Wait a short while for the service to stop
for ($i = 0; $i -lt 10; $i++) {
	Start-Sleep -Seconds 1
	$out = & sc.exe query $serviceName 2>$null
	if ($out -match 'STATE\s*:\s*\d+\s+STOPPED') {
		break
	}
}

Write-Host "Deleting service '$serviceName'..."
try {
	& sc.exe delete $serviceName > $null 2>&1
	Write-Host "Service '$serviceName' deletion requested."
}
catch {
	Write-Warning "Failed to delete service '$serviceName': $_"
}
