# ROCOLATEY

Main :rocket: [![Main Branch](https://github.com/mwallner/rocolatey/actions/workflows/Rust-Build-Pipeline.yml/badge.svg)](https://github.com/mwallner/rocolatey/actions/workflows/Rust-Build-Pipeline.yml)

Dev :rocket: [![Develop Branch](https://github.com/mwallner/rocolatey/actions/workflows/Rust-Build-Pipeline.yml/badge.svg?branch=dev)](https://github.com/mwallner/rocolatey/actions/workflows/Rust-Build-Pipeline.yml)

> ***What is Rocolatey?***

R(ocket-fast) [Chocolatey](https://chocolatey.org/) queries.

* mimics output of Chocolatey commands (drop-in replacement)
* doesn't make use of Chocolatey or chocolatey.dll - just looks at the filesystem / does native API call on source feeds.
* can be used to query Chocolatey status while `choco.exe` is running.
* it's supposed to be _much faster_ for each command that has a Chocolatey counterpart.

![roco logo](./roco.png)

> ***Important Notice, March 2024***

Due to changes in the OData endpoint of the [Chocolatey Community Repository](https://community.chocolatey.org/packages), the main performance benefit of roco when dealing with this feed is gone. - Bulk queries to this repository are not possible anymore.
Rocolatey will still be faster than choco with CCR, although only in the `--ignore-http-cache` mode of `choco.exe`.

All other NuGetV2 feeds/artifact repositories that I know of still support this feature - `roco.exe` will outperform `choco.exe` by factors on any of them, - especially interesting when you're dealing with internal feeds/ caching connectors to the community gallery.

Due to the nature of how Chocolatey handles it's internal cache and the huge amount of wrong results Chocolatey gives when searching for outdated packages, the development of Rocolatey will be continued for the forseeable future.

> ***Installing...***

Use Chocolatey!

```PowerShell
choco install rocolatey
```

... or grab the latest binary from [here](https://github.com/mwallner/rocolatey/releases).

> ***Why are Rocolatey queries so much faster than Chocolatey's counterparts?***

Rocolatey exploits some suboptimal algorithmic decisions in the original `choco.exe` (well, NuGet client library actually), it uses SAX parsers instead of reading the whole DOM of nuspec and config files, does a lot less API-calls when talking to Package repositories etc. etc.

> ***What can roco do for me?***

Take a look at the help (`roco -h`), basically roco allows you to [list installed packages](#roco-list), [list failed package installs](#roco-bad), [configured sources](#roco-source) and [check for updates](#roco-outdated).
In addition to that, there may be some hidden gems that are only available in roco, but not in vanilla Chocolatey.

Normally you should not run multiple Chocolatey instances at the same time, however this may be required in certain scenarios.
If you wanted to check if updates are available, or simply list all configured sources, roco is there to help you out without worrying of breaking something because you accidentally called choco in parallel.

Another scenario would be when traveling or on commute: checking for outdated packages on a slow network connection with `roco.exe` works for most users where `choco.exe` would time out or simply won't work.

> ***seriously, why are you doing this?***

I've started roco as a pet-project in late 2019 because I wanted to take a stab at [Rust](https://www.rust-lang.org/) programming, and back then I spend a lot of time automating software deployments with Chocolatey.
The more packages and feeds I added to the setup, the more time was lost on doing basic choco commands such as `choco list -lo -r`, hence I figured it would be a good idea to speed up what I call "Chocolatey queries".

> ***CAN I use Rocolatey in productive environments?***

Yes.

> ***SHOULD I use Rocolatey in productive environments?***

It depends. (probably yes if do a lot of outdated checks and require up-to-date (non-cached results))
For most use-cases, I would recommend sticking to `choco.exe`.
(note though `roco.exe` will give up-to-date results always, whereas `choco.exe` requires `--ignore-http-cache` for the same results)

> ***How much faster is `roco.exe` compared to `choco.exe`?***

It very much depends on the number of installed packages and configured feeds, generally speaking roco should always be faster than choco, except for the Chocolatey Community Gallery (starting March 2024).
Roco becomes faster when dealing with many packages and feeds.

## rocolatey-cli ("roco")

call using `roco.exe`, see `roco -h` for help.

### roco list

mimics the output of `choco list -lo`, make sure to use `-r` switch in automated environments!

### roco bad

get a list of packages that failed to install.
(basically the same as `roco list`, but look in `lib-bad/` instead of list.)

### roco source

mimics the output of `choco source list`, make sure to use `-r` switch in automated environments!

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
