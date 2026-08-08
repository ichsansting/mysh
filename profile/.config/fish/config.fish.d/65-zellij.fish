if test "$TERM" != dumb
    eval (zellij setup --generate-auto-start fish | string collect)
end
