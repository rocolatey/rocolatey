
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
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
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
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;bad'= {
            cand -r 'limit the output to essential information'
            cand --limitoutput 'limit the output to essential information'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;outdated'= {
            cand --choco-mode 'enables ''ignore-pinned'' and ''ignore-unfound'' 
(otherwise they are true by default, even if not set)'
            cand --ignore-pinned 'ignore any pinned packages 
(default, unless ''choco-mode'' is set)'
            cand --ignore-unfound 'ignore any unfound packages 
(default, unless ''choco-mode'' is set)'
            cand -p 'include prerelease versions'
            cand --pre 'include prerelease versions'
            cand -r 'limit the output to essential information'
            cand --limitoutput 'limit the output to essential information'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'roco;source'= {
            cand -r 'limit the output to essential information'
            cand --limitoutput 'limit the output to essential information'
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
        &'roco;help;help'= {
        }
    ]
    $completions[$command]
}
