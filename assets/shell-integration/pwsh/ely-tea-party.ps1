# Ely Tea Party: wrap the prompt to report folder and title.
$global:ElyPrompt = $function:prompt
function global:prompt {
    $text = & $global:ElyPrompt
    $folder = $executionContext.SessionState.Path.CurrentLocation
    if ($folder.Provider.Name -eq 'FileSystem') {
        $parts = $folder.ProviderPath.Replace('\', '/') -split '/'
        $path = ($parts | ForEach-Object { [Uri]::EscapeDataString($_) }) -join '/'
        if (-not $path.StartsWith('/')) { $path = "/$path" }
        $leaf = (Split-Path -Leaf $folder.ProviderPath) -replace '\p{Cc}', ''
        $esc, $bel = [char]27, [char]7
        [Console]::Write("$esc]7;file://$path$bel$esc]2;$leaf$bel")
    }
    $text
}
