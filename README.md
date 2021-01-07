# ROCOLATEY

> ***What is Rocolatey?***

R(ocket-fast) [Chocolatey](https://chocolatey.org/) queries.

* mimics output of Chocolatey commands (drop-in replacement)
* doesn't make use of Chocolatey or chocolatey.dll - just looks at the filesystem / does native ODATA calls.
* can be used to query Chocolatey status while `choco.exe` is running.
* it's supposed to be _much faster_ for each command that has a Chocolatey counterpart.

![roco logo](./roco.png)

> ***Why are Rocolatey queries so much faster than Chocolatey's counterparts?***

Rocolatey expoits some suboptimal algorithmic decisions in the original `choco.exe`, it uses SAX parsers instead of reading the whole DOM of nuspec and config files, does a lot less API-calls when talking to nuget repositories etc. etc.

> ***What can roco do for me?***

Take a look at the help (`roco -h`), basically roco allows you to [list installed packages](#roco-list), [list failed package installs](#roco-bad), [configured sources](#roco-source) and [check for updates](#roco-outdated).
In addition to that, there may be some hidden gems that are only available in roco, but not in vanilla Chocolatey.

Normally you should not run multiple Chocolatey instances at the same time, however this may be required in certain scenarios.
If you wanted to check if updates are available, or simply list all configured sources, roco is there to help you out without worring of breaking something because you accidentally called choco in parallel.

> ***seriously, why are you doing this?***

I've started roco as a pet-project in late 2019 because I wanted to take a stab at [Rust](https://www.rust-lang.org/) programming, and back then I spend a lot of time automating software deployments with Chocolatey.
The more packages and feeds I added to the setup, the more time was lost on doing basic choco commands such as `choco list -lo -r`, hence I figured it would be a good idea to speed up what I call "Chocolatey queries".

> ***How much faster is `roco.exe` compared to `choco.exe`?***

It very much depends on the number of installed packages and configured feeds, generally speaking roco should always be _much_ faster than choco, the difference becomes bigger when dealing with many packages and feeds.

Following table contains a couple of comparisons of `choco.exe` and `roco.exe`, mind that I've been using `Chocolatey v0.10.15` for all tests.

| Command                              | #Packages | #Feeds |    Choco |     Roco | Speedup |
| ------------------------------------ | :-------: | :----: | -------: | -------: | :-----: |
| `choco source list` vs `roco source` |    150    |   3    | 4.26 sec | 0.02 sec | > 200x  |
| `choco list -lo` vs `roco list`      |    150    |   2    |  8.2 sec | 0.15 sec |  > 50x  |
| `choco list -lo` vs `roco list`      |    231    |   5    |  6.6 sec | 0.21 sec |  > 30x  |
| `choco outdated` vs `roco outdated`  |    231    |   5    | 90.6 sec | 8.36 sec |  > 10x  |
| `choco outdated` vs `roco outdated`  |    150    |   2    | 97.6 sec | 12.2 sec |  > 8x   |

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
