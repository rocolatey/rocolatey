
task Build {
  cargo build --release
}

task Pack -Depends Build {
  Copy-Item ./target/release/*.exe nuget/tools/.
  Copy-Item ./LICENSE.txt nuget/tools/.
  choco pack nuget/rocolatey.nuspec
}

task Clean {
  Remove-Item ./target/release/* -recurse -ErrorAction SilentlyContinue
  Remove-Item nuget/tools/*.exe -ErrorAction SilentlyContinue
  cargo clean
}
