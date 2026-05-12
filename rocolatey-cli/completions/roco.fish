# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_roco_global_optspecs
	string join \n h/help V/version
end

function __fish_roco_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_roco_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_roco_using_subcommand
	set -l cmd (__fish_roco_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c roco -n "__fish_roco_needs_command" -s h -l help -d 'Print help'
complete -c roco -n "__fish_roco_needs_command" -s V -l version -d 'Print version'
complete -c roco -n "__fish_roco_needs_command" -f -a "list" -d 'list local installed packages'
complete -c roco -n "__fish_roco_needs_command" -f -a "bad" -d 'list packages in lib-bad/'
complete -c roco -n "__fish_roco_needs_command" -f -a "outdated" -d 'Returns a list of outdated packages.'
complete -c roco -n "__fish_roco_needs_command" -f -a "source" -d 'list choco sources'
complete -c roco -n "__fish_roco_needs_command" -f -a "search" -d 'search for packages'
complete -c roco -n "__fish_roco_needs_command" -f -a "license" -d 'display license information'
complete -c roco -n "__fish_roco_needs_command" -f -a "upgrade" -d 'upgrade outdated choco packages (using choco.exe)'
complete -c roco -n "__fish_roco_needs_command" -f -a "install" -d 'install choco packages (using choco.exe)'
complete -c roco -n "__fish_roco_needs_command" -f -a "uninstall" -d 'uninstall choco packages (using choco.exe)'
complete -c roco -n "__fish_roco_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c roco -n "__fish_roco_using_subcommand list" -s r -l limitoutput -d 'limit the output to essential information'
complete -c roco -n "__fish_roco_using_subcommand list" -l json -d 'output results in JSON format'
complete -c roco -n "__fish_roco_using_subcommand list" -s v -l verbose -d 'be verbose'
complete -c roco -n "__fish_roco_using_subcommand list" -l dependency-tree -d 'list dependencies'
complete -c roco -n "__fish_roco_using_subcommand list" -s h -l help -d 'Print help'
complete -c roco -n "__fish_roco_using_subcommand bad" -s r -l limitoutput -d 'limit the output to essential information'
complete -c roco -n "__fish_roco_using_subcommand bad" -l json -d 'output results in JSON format'
complete -c roco -n "__fish_roco_using_subcommand bad" -s v -l verbose -d 'be verbose'
complete -c roco -n "__fish_roco_using_subcommand bad" -s h -l help -d 'Print help'
complete -c roco -n "__fish_roco_using_subcommand outdated" -l choco-mode -d 'enables \'ignore-pinned\' and \'ignore-unfound\'  (otherwise they are true by default, even if not set)'
complete -c roco -n "__fish_roco_using_subcommand outdated" -l ignore-pinned -d 'ignore any pinned packages  (default, unless \'choco-mode\' is set)'
complete -c roco -n "__fish_roco_using_subcommand outdated" -l ignore-unfound -d 'ignore any unfound packages  (default, unless \'choco-mode\' is set)'
complete -c roco -n "__fish_roco_using_subcommand outdated" -s l -d 'output a whitespace-separated list of results'
complete -c roco -n "__fish_roco_using_subcommand outdated" -s p -l pre -d 'include prerelease versions'
complete -c roco -n "__fish_roco_using_subcommand outdated" -s r -l limitoutput -d 'limit the output to essential information'
complete -c roco -n "__fish_roco_using_subcommand outdated" -l json -d 'output results in JSON format'
complete -c roco -n "__fish_roco_using_subcommand outdated" -s v -l verbose -d 'be verbose'
complete -c roco -n "__fish_roco_using_subcommand outdated" -l sslcheck -d 'require https/ssl-validation'
complete -c roco -n "__fish_roco_using_subcommand outdated" -s h -l help -d 'Print help'
complete -c roco -n "__fish_roco_using_subcommand source" -s r -l limitoutput -d 'limit the output to essential information'
complete -c roco -n "__fish_roco_using_subcommand source" -l json -d 'output results in JSON format'
complete -c roco -n "__fish_roco_using_subcommand source" -s v -l verbose -d 'be verbose'
complete -c roco -n "__fish_roco_using_subcommand source" -s h -l help -d 'Print help'
complete -c roco -n "__fish_roco_using_subcommand search" -s r -l limitoutput -d 'limit the output to essential information'
complete -c roco -n "__fish_roco_using_subcommand search" -l json -d 'output results in JSON format'
complete -c roco -n "__fish_roco_using_subcommand search" -s v -l verbose -d 'be verbose'
complete -c roco -n "__fish_roco_using_subcommand search" -s h -l help -d 'Print help'
complete -c roco -n "__fish_roco_using_subcommand license" -s f -l full -d 'display full license information'
complete -c roco -n "__fish_roco_using_subcommand license" -l json -d 'output results in JSON format'
complete -c roco -n "__fish_roco_using_subcommand license" -s h -l help -d 'Print help'
complete -c roco -n "__fish_roco_using_subcommand upgrade" -s p -l pre -d 'include prerelease versions'
complete -c roco -n "__fish_roco_using_subcommand upgrade" -s r -l limitoutput -d 'limit the output to essential information'
complete -c roco -n "__fish_roco_using_subcommand upgrade" -s v -l verbose -d 'be verbose'
complete -c roco -n "__fish_roco_using_subcommand upgrade" -l sslcheck -d 'require https/ssl-validation'
complete -c roco -n "__fish_roco_using_subcommand upgrade" -s h -l help -d 'Print help'
complete -c roco -n "__fish_roco_using_subcommand install" -s p -l pre -d 'include prerelease versions'
complete -c roco -n "__fish_roco_using_subcommand install" -s r -l limitoutput -d 'limit the output to essential information'
complete -c roco -n "__fish_roco_using_subcommand install" -s v -l verbose -d 'be verbose'
complete -c roco -n "__fish_roco_using_subcommand install" -l sslcheck -d 'require https/ssl-validation'
complete -c roco -n "__fish_roco_using_subcommand install" -s h -l help -d 'Print help'
complete -c roco -n "__fish_roco_using_subcommand uninstall" -s r -l limitoutput -d 'limit the output to essential information'
complete -c roco -n "__fish_roco_using_subcommand uninstall" -s v -l verbose -d 'be verbose'
complete -c roco -n "__fish_roco_using_subcommand uninstall" -s h -l help -d 'Print help'
complete -c roco -n "__fish_roco_using_subcommand help; and not __fish_seen_subcommand_from list bad outdated source search license upgrade install uninstall help" -f -a "list" -d 'list local installed packages'
complete -c roco -n "__fish_roco_using_subcommand help; and not __fish_seen_subcommand_from list bad outdated source search license upgrade install uninstall help" -f -a "bad" -d 'list packages in lib-bad/'
complete -c roco -n "__fish_roco_using_subcommand help; and not __fish_seen_subcommand_from list bad outdated source search license upgrade install uninstall help" -f -a "outdated" -d 'Returns a list of outdated packages.'
complete -c roco -n "__fish_roco_using_subcommand help; and not __fish_seen_subcommand_from list bad outdated source search license upgrade install uninstall help" -f -a "source" -d 'list choco sources'
complete -c roco -n "__fish_roco_using_subcommand help; and not __fish_seen_subcommand_from list bad outdated source search license upgrade install uninstall help" -f -a "search" -d 'search for packages'
complete -c roco -n "__fish_roco_using_subcommand help; and not __fish_seen_subcommand_from list bad outdated source search license upgrade install uninstall help" -f -a "license" -d 'display license information'
complete -c roco -n "__fish_roco_using_subcommand help; and not __fish_seen_subcommand_from list bad outdated source search license upgrade install uninstall help" -f -a "upgrade" -d 'upgrade outdated choco packages (using choco.exe)'
complete -c roco -n "__fish_roco_using_subcommand help; and not __fish_seen_subcommand_from list bad outdated source search license upgrade install uninstall help" -f -a "install" -d 'install choco packages (using choco.exe)'
complete -c roco -n "__fish_roco_using_subcommand help; and not __fish_seen_subcommand_from list bad outdated source search license upgrade install uninstall help" -f -a "uninstall" -d 'uninstall choco packages (using choco.exe)'
complete -c roco -n "__fish_roco_using_subcommand help; and not __fish_seen_subcommand_from list bad outdated source search license upgrade install uninstall help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
