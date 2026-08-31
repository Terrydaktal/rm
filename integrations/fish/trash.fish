# Map rm to the installed trash binary (do not use --save here).
alias rm '/home/lewis/.local/bin/trash'

# Route the exact "sudo rm ..." form through trash as root.
function sudo
    if test (count $argv) -ge 1; and test "$argv[1]" = "rm"
        command sudo /home/lewis/.local/bin/trash $argv[2..-1]
    else
        command sudo $argv
    end
end
