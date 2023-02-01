complete -c roco -n "__fish_use_subcommand" -s h -l help -d 'Print help'
complete -c roco -n "__fish_use_subcommand" -s V -l version -d 'Print version'
complete -c roco -n "__fish_use_subcommand" -f -a "list" -d 'list local installed packages'
complete -c roco -n "__fish_use_subcommand" -f -a "bad" -d 'list packages in lib-bad/'
complete -c roco -n "__fish_use_subcommand" -f -a "outdated" -d 'Returns a list of outdated packages.'
complete -c roco -n "__fish_use_subcommand" -f -a "source" -d 'list choco sources'
complete -c roco -n "__fish_use_subcommand" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c roco -n "__fish_seen_subcommand_from list" -s r -l limitoutput -d 'limit the output to essential information' -r
complete -c roco -n "__fish_seen_subcommand_from list" -s v -l verbose -d 'be verbose' -r
complete -c roco -n "__fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c roco -n "__fish_seen_subcommand_from bad" -s r -l limitoutput -d 'limit the output to essential information' -r
complete -c roco -n "__fish_seen_subcommand_from bad" -s v -l verbose -d 'be verbose' -r
complete -c roco -n "__fish_seen_subcommand_from bad" -s h -l help -d 'Print help'
complete -c roco -n "__fish_seen_subcommand_from outdated" -l ignore-pinned -d 'ignore any pinned packages' -r
complete -c roco -n "__fish_seen_subcommand_from outdated" -l ignore-unfound -d 'ignore any unfound packages' -r
complete -c roco -n "__fish_seen_subcommand_from outdated" -s p -l pre -d 'include prerelease versions' -r
complete -c roco -n "__fish_seen_subcommand_from outdated" -s r -l limitoutput -d 'limit the output to essential information' -r
complete -c roco -n "__fish_seen_subcommand_from outdated" -s v -l verbose -d 'be verbose' -r
complete -c roco -n "__fish_seen_subcommand_from outdated" -s h -l help -d 'Print help'
complete -c roco -n "__fish_seen_subcommand_from source" -s r -l limitoutput -d 'limit the output to essential information' -r
complete -c roco -n "__fish_seen_subcommand_from source" -s v -l verbose -d 'be verbose' -r
complete -c roco -n "__fish_seen_subcommand_from source" -s h -l help -d 'Print help'
complete -c roco -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from list; and not __fish_seen_subcommand_from bad; and not __fish_seen_subcommand_from outdated; and not __fish_seen_subcommand_from source; and not __fish_seen_subcommand_from help" -f -a "list" -d 'list local installed packages'
complete -c roco -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from list; and not __fish_seen_subcommand_from bad; and not __fish_seen_subcommand_from outdated; and not __fish_seen_subcommand_from source; and not __fish_seen_subcommand_from help" -f -a "bad" -d 'list packages in lib-bad/'
complete -c roco -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from list; and not __fish_seen_subcommand_from bad; and not __fish_seen_subcommand_from outdated; and not __fish_seen_subcommand_from source; and not __fish_seen_subcommand_from help" -f -a "outdated" -d 'Returns a list of outdated packages.'
complete -c roco -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from list; and not __fish_seen_subcommand_from bad; and not __fish_seen_subcommand_from outdated; and not __fish_seen_subcommand_from source; and not __fish_seen_subcommand_from help" -f -a "source" -d 'list choco sources'
complete -c roco -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from list; and not __fish_seen_subcommand_from bad; and not __fish_seen_subcommand_from outdated; and not __fish_seen_subcommand_from source; and not __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
