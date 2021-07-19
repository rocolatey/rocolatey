
task Build {
  Exec {
    cargo build --release
  }
}

task Pack -Depends Build {
  Copy-Item .\target\release\*.exe nuget\tools\.
  Copy-Item .\rocolatey-cli\completions\_roco.ps1 nuget\tools\RocoTabCompletion.psm1
  Copy-Item .\LICENSE.txt nuget\tools\.
  choco pack nuget/rocolatey.nuspec
}

task Clean {
  Remove-Item .\target\release\* -recurse -ErrorAction SilentlyContinue
  Remove-Item nuget\tools\*.exe -ErrorAction SilentlyContinue
  cargo clean
}
