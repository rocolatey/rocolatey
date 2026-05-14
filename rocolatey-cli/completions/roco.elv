
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
        &'roco;help;help'= {
        }
    ]
    $completions[$command]
}
