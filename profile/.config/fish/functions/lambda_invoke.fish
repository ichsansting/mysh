# pick a lambda with fzf and invoke it; payload from $argv[1] (file or JSON string),
# else opens $EDITOR on a scratch .json file
function lambda_invoke
    set -l name (aws lambda list-functions \
        --region ap-southeast-1 \
        --query 'Functions[*].FunctionName' \
        --output text | tr '\t' '\n' | fzf --prompt='lambda> ') || return

    set -l payload
    if test -f "$argv[1]"
        set payload (cat $argv[1] | string collect)
    else if test -n "$argv[1]"
        set payload $argv[1]
    else
        # the header lines are the only instructions visible once the editor takes
        # over the screen; they are stripped again before the payload is validated
        set -l draft (mktemp --suffix=.json)
        printf '// payload for %s\n// save and quit to invoke; clear the file (or :q!) to cancel\n' $name >$draft
        $EDITOR $draft
        set payload (cat $draft | string match -v -r '^\s*//' | string collect)
        rm -f $draft
        if test -z (string trim -- "$payload")
            echo "empty payload, invoke cancelled" >&2
            return 1
        end
        if not echo $payload | jq -e . >/dev/null 2>&1
            echo "payload is not valid JSON, aborting" >&2
            return 1
        end
    end

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
