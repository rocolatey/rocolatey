
edit:completion:arg-completer[roco] = [@words]{
    fn spaces [n]{
        repeat $n ' ' | joins ''
    }
    fn cand [text desc]{
        edit:complex-candidate $text &display-suffix=' '(spaces (- 14 (wcswidth $text)))$desc
    }
    command = 'roco'
    for word $words[1:-1] {
        if (has-prefix $word '-') {
            break
        }
        command = $command';'$word
    }
    completions = [
        &'roco'= {
            cand -h 'Prints help information'
            cand --help 'Prints help information'
            cand -V 'Prints version information'
            cand --version 'Prints version information'
            cand list 'list local installed packages'
            cand bad 'list packages in lib-bad/'
            cand outdated 'Returns a list of outdated packages.'
            cand source 'list choco sources'
            cand help 'Prints this message or the help of the given subcommand(s)'
        }
        &'roco;list'= {
            cand -r 'limit the output to essential information'
            cand --limitoutput 'limit the output to essential information'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Prints help information'
            cand --help 'Prints help information'
            cand -V 'Prints version information'
            cand --version 'Prints version information'
        }
        &'roco;bad'= {
            cand -r 'limit the output to essential information'
            cand --limitoutput 'limit the output to essential information'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Prints help information'
            cand --help 'Prints help information'
            cand -V 'Prints version information'
            cand --version 'Prints version information'
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
            cand -h 'Prints help information'
            cand --help 'Prints help information'
            cand -V 'Prints version information'
            cand --version 'Prints version information'
        }
        &'roco;source'= {
            cand -r 'limit the output to essential information'
            cand --limitoutput 'limit the output to essential information'
            cand -v 'be verbose'
            cand --verbose 'be verbose'
            cand -h 'Prints help information'
            cand --help 'Prints help information'
            cand -V 'Prints version information'
            cand --version 'Prints version information'
        }
        &'roco;help'= {
            cand -h 'Prints help information'
            cand --help 'Prints help information'
            cand -V 'Prints version information'
            cand --version 'Prints version information'
        }
    ]
    $completions[$command]
}
