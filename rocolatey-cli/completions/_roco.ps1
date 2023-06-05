
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
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'roco' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', 'V', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'list local installed packages')
            [CompletionResult]::new('bad', 'bad', [CompletionResultType]::ParameterValue, 'list packages in lib-bad/')
            [CompletionResult]::new('outdated', 'outdated', [CompletionResultType]::ParameterValue, 'Returns a list of outdated packages.')
            [CompletionResult]::new('source', 'source', [CompletionResultType]::ParameterValue, 'list choco sources')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'roco;list' {
            [CompletionResult]::new('-r', 'r', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('--limitoutput', 'limitoutput', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'roco;bad' {
            [CompletionResult]::new('-r', 'r', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('--limitoutput', 'limitoutput', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'roco;outdated' {
            [CompletionResult]::new('--choco-mode', 'choco-mode', [CompletionResultType]::ParameterName, 'enables ''ignore-pinned'' and ''ignore-unfound'' 
(otherwise they are true by default, even if not set)')
            [CompletionResult]::new('--ignore-pinned', 'ignore-pinned', [CompletionResultType]::ParameterName, 'ignore any pinned packages 
(default, unless ''choco-mode'' is set)')
            [CompletionResult]::new('--ignore-unfound', 'ignore-unfound', [CompletionResultType]::ParameterName, 'ignore any unfound packages 
(default, unless ''choco-mode'' is set)')
            [CompletionResult]::new('-p', 'p', [CompletionResultType]::ParameterName, 'include prerelease versions')
            [CompletionResult]::new('--pre', 'pre', [CompletionResultType]::ParameterName, 'include prerelease versions')
            [CompletionResult]::new('-r', 'r', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('--limitoutput', 'limitoutput', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'roco;source' {
            [CompletionResult]::new('-r', 'r', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('--limitoutput', 'limitoutput', [CompletionResultType]::ParameterName, 'limit the output to essential information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'be verbose')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'roco;help' {
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'list local installed packages')
            [CompletionResult]::new('bad', 'bad', [CompletionResultType]::ParameterValue, 'list packages in lib-bad/')
            [CompletionResult]::new('outdated', 'outdated', [CompletionResultType]::ParameterValue, 'Returns a list of outdated packages.')
            [CompletionResult]::new('source', 'source', [CompletionResultType]::ParameterValue, 'list choco sources')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'roco;help;list' {
            break
        }
        'roco;help;bad' {
            break
        }
        'roco;help;outdated' {
            break
        }
        'roco;help;source' {
            break
        }
        'roco;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
