

Task GenerateLicenseInfo {
	Remove-Item ./THIRDPARTY.json -ErrorAction SilentlyContinue
	Exec {
		cargo bundle-licenses --format json --output THIRDPARTY.json
	}
}

Task BuildWin -depends GenerateLicenseInfo {
	Exec {
		cargo build --target x86_64-pc-windows-gnu --release
	}
}

Task Build -depends GenerateLicenseInfo {
	Exec {
		cargo build --release
	}
}

Task Pack -depends Build, BuildWin {
	$binPath = '.\target\release'
	if ($PSVersionTable.Platform -ne 'Windows') {
		$binPath = '.\target\x86_64-pc-windows-gnu\release'
	}
	Copy-Item "$binPath\*.exe" nuget\tools\.
	Copy-Item .\rocolatey-cli\completions\_roco.ps1 nuget\tools\RocoTabCompletion.psm1
	Copy-Item .\LICENSE.txt nuget\tools\.

	Remove-Item .\target\rocolatey.*.nupkg -ErrorAction SilentlyContinue
	Exec {
		if ($PSVersionTable.Platform -ne 'Windows') {
			docker run -t --rm -v "${PWD}:/tmp" -w /tmp chocolatey/choco /bin/bash -c 'choco pack nuget/rocolatey.nuspec'
		}
		else {
			choco pack nuget/rocolatey.nuspec
		}
	}

	Remove-Item .\target\*.nupkg -ErrorAction SilentlyContinue
	Move-Item .\rocolatey.*.nupkg .\target\.
}

Task Clean {
	Remove-Item .\target\release\* -Recurse -ErrorAction SilentlyContinue
	Remove-Item nuget\tools\*.exe -ErrorAction SilentlyContinue
	Remove-Item ./THIRDPARTY.json -ErrorAction SilentlyContinue
	cargo clean
}
