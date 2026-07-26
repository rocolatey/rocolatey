<#
  Stops the Rocolatey-Server Windows service if installed and running.
  This runs before package modify/upgrade so the updated binary can be replaced.
#>
$ErrorActionPreference = 'Stop'
$serviceName = 'Rocolatey-Server'

Write-Host "Checking for service '$serviceName'..."
& sc.exe query $serviceName > $null 2>&1
if ($LASTEXITCODE -ne 0) {
	Write-Host "Service '$serviceName' not installed; nothing to stop."
	return
}

Write-Host "Service '$serviceName' installed. Attempting to stop..."
try {
	& sc.exe stop $serviceName > $null 2>&1
}
catch {
	Write-Warning "Failed to send stop command to service '$serviceName': $_"
}

# Wait for service to reach STOPPED state (timeout after 15s)
for ($i = 0; $i -lt 15; $i++) {
	Start-Sleep -Seconds 1
	$out = & sc.exe query $serviceName 2>$null
	if ($out -match 'STATE\s*:\s*\d+\s+STOPPED') {
		Write-Host "Service '$serviceName' stopped."
		return
	}
}

Write-Warning "Service '$serviceName' did not stop within timeout; it may still be running."
