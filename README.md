# ROCOLATEY

Main :rocket: [![Main Branch](https://github.com/mwallner/rocolatey/actions/workflows/Rust-Build-Pipeline.yml/badge.svg)](https://github.com/mwallner/rocolatey/actions/workflows/Rust-Build-Pipeline.yml)

Dev :rocket: [![Develop Branch](https://github.com/mwallner/rocolatey/actions/workflows/Rust-Build-Pipeline.yml/badge.svg?branch=dev)](https://github.com/mwallner/rocolatey/actions/workflows/Rust-Build-Pipeline.yml)

> What is Rocolatey?

R(ocket-fast) [Chocolatey](https://chocolatey.org/).

- Mimics the output of Chocolatey commands.
- Does not use Chocolatey or `chocolatey.dll` where possible; it inspects the filesystem and uses native API calls against source feeds. (`un`/`install` and `upgrade` commands are routed to `choco.exe`.)
- Can be used to query Chocolatey status while `choco.exe` is running.
- It is expected to be much faster for commands that have a Chocolatey counterpart.
- Is able to run "remotely" using `rocolatey-server.exe` (if `env:ROCO_SERVER_IP` is set when running `roco.exe`).

![roco logo](./roco.png)

> Important Notice — March 2024

Due to changes in the OData endpoint of the [Chocolatey Community Repository](https://community.chocolatey.org/packages), the main performance benefit of Rocolatey when dealing with this feed is gone. Bulk queries to this repository are no longer possible. Rocolatey will still be faster than `choco` with the CCR, although only when `choco.exe` is used with `--ignore-http-cache`.

Most other NuGetV2 feeds and artifact repositories still support this feature — `roco.exe` will outperform `choco.exe` on those, especially for internal feeds or caching connectors to the community gallery.

Because of how Chocolatey manages its internal cache and the incorrect results Chocolatey sometimes returns when searching for outdated packages, development of Rocolatey will continue for the foreseeable future.

> Installing

Use Chocolatey:

```powershell
choco install rocolatey
```

...or grab the latest binary from the releases page: https://github.com/mwallner/rocolatey/releases

> Why are Rocolatey queries so much faster than Chocolatey's counterparts?

Rocolatey avoids some suboptimal algorithmic choices in `choco.exe` (or rather, the NuGet client library). It uses SAX-style parsing instead of loading entire XML DOMs for nuspec and config files, and it makes far fewer API calls to package repositories.

> What can `roco` do for me?

See the help (`roco -h`). In short, `roco` can:

- List installed packages (`roco list`).
- List failed package installs (`roco bad`).
- Show configured sources (`roco source`).
- Check for updates (`roco outdated`).

There may also be additional features available in `roco` that are not present in vanilla Chocolatey.

Normally you should not run multiple Chocolatey instances at the same time, though in some scenarios this may be required. If you want to check for updates or list configured sources without invoking `choco.exe`, `roco` can help without risking conflicts from running `choco` in parallel.

When traveling or while on a slow network connection, checking for outdated packages with `roco.exe` often succeeds where `choco.exe` may time out or fail.

> Why was this created?

I started `roco` as a pet project in late 2019 to learn Rust and to speed up common Chocolatey queries. As I added more packages and feeds, basic `choco` commands became slower, so I implemented faster alternatives for the common query operations.

> Can I use Rocolatey in production?

Yes — but it depends on your use case. For many situations, I recommend sticking with `choco.exe`. Note, however, that `roco.exe` provides up-to-date results by default, while `choco.exe` requires `--ignore-http-cache` to achieve the same.

> How much faster is `roco.exe` compared to `choco.exe`?

Performance depends on the number of installed packages and configured feeds. Generally, `roco` should be faster than `choco`, especially when dealing with many packages and feeds. The exception is the Chocolatey Community Gallery (as noted above).

## rocolatey-cli ("roco")

Call using `roco.exe`; see `roco -h` for help.

### `roco list`

Mimics the output of `choco list -lo`. Use the `-r` switch in automated environments.

### `roco bad`

Lists packages that failed to install (similar to `roco list`, but reads `lib-bad/`).

### `roco source`

Mimics the output of `choco source list`. Use the `-r` switch in automated environments.

### `roco outdated`

Mimics the output of `choco outdated`. Use the `-r` switch in automated environments.

## rocolatey-server

**Unstable — use at your own risk**

Exposes a REST API for fetching Chocolatey package information from a host.

There is currently no authentication or encryption in place — set up a reverse proxy if you plan to use it outside your homelab.

Rocolatey-server listens on port `29295` by default (which is "ro" in hex). You can specify the address and port to listen on; use `-h` to display help.

It is possible to query the server directly:

```
GET http://roco-server-a:29295/rocolatey/local
```

Or configure the `roco` client via environment variables to point at a server instance:

```pwsh
$env:ROCO_SERVER_IP="172.42.10.101"
$env:ROCO_SERVER_PORT="29295"

roco outdated # checks for outdated packages on 172.42.10.101
```
