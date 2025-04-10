

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

Task Pack -depends Build {
	Copy-Item .\target\release\*.exe nuget\tools\.
	Copy-Item .\rocolatey-cli\completions\_roco.ps1 nuget\tools\RocoTabCompletion.psm1
	Copy-Item .\LICENSE.txt nuget\tools\.
	choco pack nuget/rocolatey.nuspec
}

Task Clean {
	Remove-Item .\target\release\* -Recurse -ErrorAction SilentlyContinue
	Remove-Item nuget\tools\*.exe -ErrorAction SilentlyContinue
	Remove-Item ./THIRDPARTY.json -ErrorAction SilentlyContinue
	cargo clean
}
