param(
    [Parameter(Mandatory = $true)]
    [string]$RtkBinary,

    [Parameter(Mandatory = $true)]
    [string]$CommandLine
)

& $RtkBinary rewrite $CommandLine
exit $LASTEXITCODE
