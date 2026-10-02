# Pick a cluster, service, task and container; optionally pass a command instead of /bin/sh.
function aws-ecs-exec
    set -l exec_command /bin/sh
    if set -q argv[1]
        set exec_command "$argv[1]"
    end

    set -l choices (aws ecs list-clusters \
        --region ap-southeast-1 --query 'clusterArns[]' --output text) || return
    if test -z "$choices"
        echo 'No ECS clusters found.' >&2
        return 1
    end
    set -l cluster (printf '%s\n' $choices | tr '\t' '\n' | string replace -r '.*/' '' | fzf --prompt='cluster> ') || return

    set choices (aws ecs list-services --cluster "$cluster" \
        --region ap-southeast-1 --query 'serviceArns[]' --output text) || return
    if test -z "$choices"
        echo 'No ECS services found in the selected cluster.' >&2
        return 1
    end
    set -l service (printf '%s\n' $choices | tr '\t' '\n' | string replace -r '.*/' '' | fzf --prompt='service> ') || return

    set choices (aws ecs list-tasks --cluster "$cluster" --service-name "$service" \
        --desired-status RUNNING --region ap-southeast-1 --query 'taskArns[]' --output text) || return
    if test -z "$choices"
        echo 'No running ECS tasks found for the selected service.' >&2
        return 1
    end
    set -l task_arns (string split -n \t -- $choices)
    set -l task_rows
    # DescribeTasks accepts at most 100 tasks per request.
    while test (count $task_arns) -gt 0
        set -l batch_rows (aws ecs describe-tasks --cluster "$cluster" --tasks $task_arns[1..100] \
            --region ap-southeast-1 \
            --query 'tasks[].[taskArn,lastStatus,healthStatus,startedAt,enableExecuteCommand]' \
            --output text) || return
        set -a task_rows $batch_rows
        set -e task_arns[1..100]
    end
    if test (count $task_rows) -eq 0
        echo 'No ECS task details found for the selected service.' >&2
        return 1
    end
    set -l task_row (printf '%s\n' $task_rows | string replace -r '^[^\t]*/' '' | \
        fzf --prompt='task> ' --header="TASK ID | STATUS | HEALTH | STARTED AT | ECS EXEC") || return
    set -l task_fields (string split \t -- "$task_row")
    set -l task $task_fields[1]
    if test "$task_fields[2]" != RUNNING
        echo "ECS task '$task' is not running (status: $task_fields[2])." >&2
        return 1
    end
    if test (string lower -- "$task_fields[5]") != true
        echo "ECS Exec is not enabled for task '$task'." >&2
        echo 'Enable ECS Exec on the service and launch new tasks before retrying.' >&2
        return 1
    end

    set choices (aws ecs describe-tasks --cluster "$cluster" --tasks "$task" \
        --region ap-southeast-1 \
        --query 'tasks[?lastStatus == `RUNNING`].containers[] | [?lastStatus == `RUNNING`].name' \
        --output text) || return
    if test -z "$choices"
        echo "No running containers found in ECS task '$task'; it may have stopped since selection." >&2
        return 1
    end
    set -l container (printf '%s\n' $choices | tr '\t' '\n' | fzf --prompt='container> ') || return

    aws ecs execute-command --region ap-southeast-1 \
        --cluster "$cluster" --task "$task" --container "$container" \
        --interactive --command "$exec_command"
end
