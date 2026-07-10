
use builtin;
use str;

set edit:completion:arg-completer[roco] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'roco'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'roco'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand list 'list local installed packages'
            cand bad 'list packages in lib-bad/'
            cand outdated 'Returns a list of outdated packages.'
            cand source 'list choco sources'
            cand search 'search for packages'
            cand license 'display license information'
            cand upgrade 'upgrade outdated choco packages (using choco.exe)'
            cand install 'install choco packages (using choco.exe)'
            cand uninstall 'uninstall choco packages (using choco.exe)'
            cand pin 'manage package pins'
            cand server 'manage roco server configuration and TLS setup'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'roco;list'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -r 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --limitoutput 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --json 'output results in JSON format'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand --dependency-tree 'list dependencies'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;bad'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -r 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --limitoutput 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --json 'output results in JSON format'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;outdated'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand --choco-mode 'enables ''ignore-pinned'' and ''ignore-unfound''  (otherwise they are true by default, even if not set)'
            cand --ignore-pinned 'ignore any pinned packages  (default, unless ''choco-mode'' is set)'
            cand --ignore-unfound 'ignore any unfound packages  (default, unless ''choco-mode'' is set)'
            cand -l 'output a whitespace-separated list of results'
            cand -p 'include prerelease versions'
            cand --pre 'include prerelease versions'
            cand -r 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --limitoutput 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --json 'output results in JSON format'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand --sslcheck 'require https/ssl-validation'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;source'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -r 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --limitoutput 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --json 'output results in JSON format'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;search'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -r 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --limitoutput 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --json 'output results in JSON format'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;license'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -f 'display full license information'
            cand --full 'display full license information'
            cand --json 'output results in JSON format'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;upgrade'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -p 'include prerelease versions'
            cand --pre 'include prerelease versions'
            cand -r 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --limitoutput 'limit output to essential information (automation-safe, no ANSI colors)'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand --sslcheck 'require https/ssl-validation'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;install'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -p 'include prerelease versions'
            cand --pre 'include prerelease versions'
            cand -r 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --limitoutput 'limit output to essential information (automation-safe, no ANSI colors)'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand --sslcheck 'require https/ssl-validation'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;uninstall'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -r 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --limitoutput 'limit output to essential information (automation-safe, no ANSI colors)'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;pin'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -h 'Print help'
            cand --help 'Print help'
            cand list 'list pinned packages'
            cand add 'pin a package to prevent upgrades'
            cand remove 'remove a package pin'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'roco;pin;list'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -r 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --limitoutput 'limit output to essential information (automation-safe, no ANSI colors)'
            cand --json 'output results in JSON format'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;pin;add'= {
            cand --version 'specific version to pin'
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;pin;remove'= {
            cand --version 'specific version to unpin'
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;pin;help'= {
            cand list 'list pinned packages'
            cand add 'pin a package to prevent upgrades'
            cand remove 'remove a package pin'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'roco;pin;help;list'= {
        }
        &'roco;pin;help;add'= {
        }
        &'roco;pin;help;remove'= {
        }
        &'roco;pin;help;help'= {
        }
        &'roco;server'= {
            cand --color 'Control color output: auto (default), always, or never. -r always stays uncolored'
            cand --setup-tls-help 'display TLS setup status and enrollment guidance'
            cand --gen-cert 'generate TLS certificates for client and server'
            cand --force 'regenerate certificates even if they exist (creates timestamped backups)'
            cand --bootstrap-local-trust 'bootstrap local key exchange and enroll current account client fingerprint on this host'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;help'= {
            cand list 'list local installed packages'
            cand bad 'list packages in lib-bad/'
            cand outdated 'Returns a list of outdated packages.'
            cand source 'list choco sources'
            cand search 'search for packages'
            cand license 'display license information'
            cand upgrade 'upgrade outdated choco packages (using choco.exe)'
            cand install 'install choco packages (using choco.exe)'
            cand uninstall 'uninstall choco packages (using choco.exe)'
            cand pin 'manage package pins'
            cand server 'manage roco server configuration and TLS setup'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'roco;help;list'= {
        }
        &'roco;help;bad'= {
        }
        &'roco;help;outdated'= {
        }
        &'roco;help;source'= {
        }
        &'roco;help;search'= {
        }
        &'roco;help;license'= {
        }
        &'roco;help;upgrade'= {
        }
        &'roco;help;install'= {
        }
        &'roco;help;uninstall'= {
        }
        &'roco;help;pin'= {
            cand list 'list pinned packages'
            cand add 'pin a package to prevent upgrades'
            cand remove 'remove a package pin'
        }
        &'roco;help;pin;list'= {
        }
        &'roco;help;pin;add'= {
        }
        &'roco;help;pin;remove'= {
        }
        &'roco;help;server'= {
        }
        &'roco;help;help'= {
        }
    ]
    $completions[$command]
}
