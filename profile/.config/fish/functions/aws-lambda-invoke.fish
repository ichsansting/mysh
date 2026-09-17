# pick a lambda name via fzf
function __aws-lambda-pick-function
    aws lambda list-functions \
        --region ap-southeast-1 \
        --query 'Functions[*].FunctionName' \
        --output text | tr '\t' '\n' | fzf --prompt='lambda> '
end

# strip the "// ..." header lines added for the editor
function __aws-lambda-strip-header
    grep -v '^\s*//' $argv[1] | string collect
end

# write the editor draft: header instructions + last saved payload for this function
function __aws-lambda-write-draft
    set -l name $argv[1]
    set -l payload_file $argv[2]

    set -l previous '{}'
    if test -f "$payload_file"
        set previous (__aws-lambda-strip-header $payload_file)
    end
    begin
        printf '// payload for %s\n// :wq to invoke, :q! to cancel\n' $name
        echo $previous
    end >$payload_file
end

# true (0) if the file's mtime changed since $argv[2], meaning the editor
# saved (:wq); false if untouched, meaning the user cancelled (:q!)
function __aws-lambda-file-was-saved
    set -l payload_file $argv[1]
    set -l mtime_before $argv[2]
    test (stat -c '%Y.%N' $payload_file) != "$mtime_before"
end

function __aws-lambda-print-rerun-hint
    set -l name $argv[1]
    set -l payload_arg $argv[2]

    if test -n "$payload_arg"
        echo "rerun: aws-lambda-invoke $name $payload_arg" >&2
    else
        echo "rerun: aws-lambda-invoke $name" >&2
    end
end

function __aws-lambda-do-invoke
    set -l name $argv[1]
    set -l payload $argv[2]

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

# pick a lambda with fzf and invoke it; payload from $argv[2] (file or JSON string),
# else opens $EDITOR on a scratch .json file
# pass function name as $argv[1] to skip fzf (e.g. re-invoking the same function)
function aws-lambda-invoke
    # 1. get the function name: use $argv[1] if given, else pick via fzf
    set -l name $argv[1]
    if test -z "$name"
        set name (__aws-lambda-pick-function) || return
    end

    set -l payload
    set -l payload_file $argv[2]
    # 2. get the payload: from $argv[2] if it's a file or literal JSON string,
    # else fall through to the interactive editor flow below
    if test -f "$argv[2]"
        set payload (cat $argv[2] | string collect)
    else if test -n "$argv[2]"
        set payload $argv[2]
    else
        # persistent path per function, so re-running for the same function
        # reopens the same file with its last payload already in it
        set payload_file /tmp/aws-lambda-invoke-$name.json

        # 2a. write the editor draft: header instructions + last saved payload
        __aws-lambda-write-draft $name $payload_file

        # 2b. open $EDITOR, then tell :wq (saved) apart from :q! (cancelled)
        # by checking whether the file's mtime moved at all
        set -l mtime_before (stat -c '%Y.%N' $payload_file)
        $EDITOR $payload_file
        if not __aws-lambda-file-was-saved $payload_file $mtime_before
            echo "invoke cancelled" >&2
            return 1
        end

        # 2c. strip the header back out and validate what's left is real JSON
        set payload (__aws-lambda-strip-header $payload_file)
        if not echo $payload | jq -e . >/dev/null 2>&1
            echo "payload is not valid JSON, aborting" >&2
            return 1
        end
        # 2d. save the cleaned payload so the next edit reopens it, header-free
        echo $payload >$payload_file
    end

    # 3. print a copy-pasteable command to repeat this exact invocation
    __aws-lambda-print-rerun-hint $name $argv[2]
    # 4. call the lambda and print its decoded logs plus response
    __aws-lambda-do-invoke $name $payload
end
