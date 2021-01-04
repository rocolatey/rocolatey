# ROCOLATEY

> *What is Rocolatey?*

R(ocket-fast) Chocolatey queries.

* mimics output of Chocolatey commands (drop-in replacement)
* doesn't make use of chocolatey or chocolatey.dll - just looks at the filesystem / does native ODATA calls.
* it's supposed to be _much faster_ for each command that has a Chocolatey counterpart.

![roco logo](./roco.png)

> Why are Rocolatey queries so much faster than Chocolatey's counterparts?

Rocolatey expoits some suboptimal algorithmic decisions in the original `choco.exe`, it uses SAX parsers instead of reading the whole DOM of nuspec and config files, does a lot less API-calls when talking to nuget repositories etc. etc.

> How much faster is `roco.exe` compared to `choco.exe`?

It very much depends on the number of installed packages and configured feeds, generally speaking roco should be at least three times faster than choco, but it gets even better when dealing with many packages and repositories.

| Command                             | #Packages | #Feeds |    Choco |     Roco |
| ----------------------------------- | :-------: | :----: | -------: | -------: |
| `choco list -lo` vs `roco list`     |    150    |   2    |  8.2 sec | 0.15 sec |
| `choco outdated` vs `roco outdated` |    150    |   2    | 97.6 sec | 12.2 sec |

## rocolatey-cli ("roco")

call using `roco.exe`, see `roco -h` for help.

### roco list

mimics the output of `choco list -lo`, make sure to use `-r` switch in automated environments!

### roco bad

get a list of packages that failed to install.
(basically the same as `roco list`, but look in `lib-bad/` instead of list.)

### roco outdated

mimics the output of `choco outdated`, make sure to use `-r` switch in automated environments!

## rocolatey-server

exposes a REST api for fetching Chocolatey package info from a host.

currently implemented endpoints:

* `rocolatey/local`
* `rocolatey/local/r`
* `rocolatey/bad`
* `rocolatey/bad/r`

you can specify which address and port to listen to, use `-h` to display help text.

```
GET http://127.0.0.1:8081/rocolatey/local
```
