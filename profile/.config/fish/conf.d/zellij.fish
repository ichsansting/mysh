if test "$TERM" != dumb
    # set -gx ZELLIJ_AUTO_ATTACH true
    # set -gx ZELLIJ_AUTO_EXIT true
    # eval (zellij setup --generate-auto-start fish | string collect)

    if status is-interactive; and not set -q ZELLIJ
        exec zellij attach -c main
    end
end
