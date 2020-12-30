# ROCOLATEY

R(ocket-fast) Chocolatey queries.

* mimics output of Chocolatey commands (drop-in replacement)
* doesn't make use of chocolatey or chocolatey.dll - just looks at the filesystem
* it's written in rust

## rocolatey-cli ("roco")

call using `roco.exe`, see `roco -h` for help.

## rocolatey-server

* exposes a REST api for fetching Chocolatey package info from a host

```
GET http://127.0.0.1:8081/rocolatey/local
```
