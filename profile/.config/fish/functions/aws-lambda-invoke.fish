# pick a lambda with fzf and invoke it; payload from $argv[2] (file or JSON string),
# else opens $EDITOR on a scratch .json file
# pass function name as $argv[1] to skip fzf (e.g. re-invoking the same function)
function aws-lambda-invoke
    set -l name $argv[1]
    if test -z "$name"
        set name (aws lambda list-functions \
            --region ap-southeast-1 \
            --query 'Functions[*].FunctionName' \
            --output text | tr '\t' '\n' | fzf --prompt='lambda> ') || return
    end

    set -l payload
    set -l payload_file $argv[2]
    if test -f "$argv[2]"
        set payload (cat $argv[2] | string collect)
    else if test -n "$argv[2]"
        set payload $argv[2]
    else
        # stable per-function path: re-editing the same function reopens its last payload
        set payload_file /tmp/aws-lambda-invoke-$name.json
        set -l previous
        if test -f "$payload_file"
            set previous (cat $payload_file | string collect)
        else
            set previous '{}'
        end
        # the header lines are the only instructions visible once the editor takes
        # over the screen; they are stripped again before the payload is validated
        begin
            printf '// payload for %s\n// :wq to invoke, :q! to cancel\n' $name
            echo $previous
        end >$payload_file
        set -l mtime_before (stat -c '%Y.%N' $payload_file)
        $EDITOR $payload_file
        # :q! never touches mtime; :wq always does, even with unchanged content
        if test (stat -c '%Y.%N' $payload_file) = "$mtime_before"
            echo "invoke cancelled" >&2
            return 1
        end
        set payload (cat $payload_file | string match -v -r '^\s*//' | string collect)
        if not echo $payload | jq -e . >/dev/null 2>&1
            echo "payload is not valid JSON, aborting" >&2
            return 1
        end
        # strip the header comments so the saved file replays as plain JSON
        echo $payload >$payload_file
    end

    echo "rerun: aws-lambda-invoke $name $payload_file" >&2
    set -l out (mktemp)
    aws lambda invoke \
        --region ap-southeast-1 \
        --function-name $name \
        --cli-binary-format raw-in-base64-out \
        --payload "$payload" \
        --log-type Tail \
        --query LogResult --output text $out | base64 -d

    jq . $out 2>/dev/null || cat $out
    rm -f $out
end
