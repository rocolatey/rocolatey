
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'roco' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'roco'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-')) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'roco' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('-V', 'V', [CompletionResultType]::ParameterName, 'Prints version information')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Prints version information')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'list local installed packages')
            [CompletionResult]::new('bad', 'bad', [CompletionResultType]::ParameterValue, 'list packages in lib-bad/')
            [CompletionResult]::new('outdated', 'outdated', [CompletionResultType]::ParameterValue, 'Returns a list of outdated packages.')
            [CompletionResult]::new('source', 'source', [CompletionResultType]::ParameterValue, 'list choco sources')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Prints this message or the help of the given subcommand(s)')
            break
        }
        'roco;list' {
            [CompletionResult]::new('-r', 'r', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('--limitoutput', 'limitoutput', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('-V', 'V', [CompletionResultType]::ParameterName, 'Prints version information')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Prints version information')
            break
        }
        'roco;bad' {
            [CompletionResult]::new('-r', 'r', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('--limitoutput', 'limitoutput', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('-V', 'V', [CompletionResultType]::ParameterName, 'Prints version information')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Prints version information')
            break
        }
        'roco;outdated' {
            [CompletionResult]::new('--ignore-pinned', 'ignore-pinned', [CompletionResultType]::ParameterName, 'ignore any pinned packages')
            [CompletionResult]::new('--ignore-unfound', 'ignore-unfound', [CompletionResultType]::ParameterName, 'ignore any unfound packages')
            [CompletionResult]::new('-p', 'p', [CompletionResultType]::ParameterName, 'include prerelease versions')
            [CompletionResult]::new('--pre', 'pre', [CompletionResultType]::ParameterName, 'include prerelease versions')
            [CompletionResult]::new('-r', 'r', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('--limitoutput', 'limitoutput', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('-V', 'V', [CompletionResultType]::ParameterName, 'Prints version information')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Prints version information')
            break
        }
        'roco;source' {
            [CompletionResult]::new('-r', 'r', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('--limitoutput', 'limitoutput', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('-V', 'V', [CompletionResultType]::ParameterName, 'Prints version information')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Prints version information')
            break
        }
        'roco;help' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('-V', 'V', [CompletionResultType]::ParameterName, 'Prints version information')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Prints version information')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
