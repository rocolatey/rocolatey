
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
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -V 'Print version information'
            cand --version 'Print version information'
            cand list 'list local installed packages'
            cand bad 'list packages in lib-bad/'
            cand outdated 'Returns a list of outdated packages.'
            cand source 'list choco sources'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'roco;list'= {
            cand -r 'limit the output to essential information'
            cand --limitoutput 'limit the output to essential information'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Print help information'
            cand --help 'Print help information'
        }
        &'roco;bad'= {
            cand -r 'limit the output to essential information'
            cand --limitoutput 'limit the output to essential information'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Print help information'
            cand --help 'Print help information'
        }
        &'roco;outdated'= {
            cand --ignore-pinned 'ignore any pinned packages'
            cand --ignore-unfound 'ignore any unfound packages'
            cand -p 'include prerelease versions'
            cand --pre 'include prerelease versions'
            cand -r 'limit the output to essential information'
            cand --limitoutput 'limit the output to essential information'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Print help information'
            cand --help 'Print help information'
        }
        &'roco;source'= {
            cand -r 'limit the output to essential information'
            cand --limitoutput 'limit the output to essential information'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Print help information'
            cand --help 'Print help information'
        }
        &'roco;help'= {
        }
    ]
    $completions[$command]
}
