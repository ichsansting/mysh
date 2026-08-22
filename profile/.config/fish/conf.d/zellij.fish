if test "$TERM" != dumb
    set -gx ZELLIJ_AUTO_ATTACH true
    eval (zellij setup --generate-auto-start fish | string collect)
end
