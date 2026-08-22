# unset every AWS_* variable (handy when switching between AWS profiles/sessions)
function clear_aws_env
    for var in (set -n)
        if string match -q 'AWS_*' $var
            echo "Unsetting $var"
            set -e $var
        end
    end
end
