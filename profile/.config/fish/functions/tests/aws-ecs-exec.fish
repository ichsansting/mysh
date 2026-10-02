# Run with: fish --no-config tests/aws-ecs-exec.fish
source (status dirname)/../aws-ecs-exec.fish

function date
    if test "$argv[1]" = +%s
        echo 1790942400
        return
    end
    command date $argv
end

function aws
    set -g aws_calls (math $aws_calls + 1)
    test "$failure_at" = "$aws_calls"; and return 42
    test "$empty_at" = "$aws_calls"; and return 0
    contains -- ap-southeast-1 $argv; or return 99
    switch $argv[2]
        case list-clusters
            printf 'arn:aws:ecs:region:account:cluster/cluster-other\tarn:aws:ecs:region:account:cluster/cluster-chosen\n'
        case list-services
            contains -- cluster-chosen $argv; or return 99
            printf 'arn:aws:ecs:region:account:service/cluster-chosen/service-other\tarn:aws:ecs:region:account:service/cluster-chosen/service-chosen\n'
        case list-tasks
            contains -- service-chosen $argv; or return 99
            contains -- RUNNING $argv; or return 99
            if test "$scenario" = batching
                printf 'arn:aws:ecs:region:account:task/cluster-chosen/task-other-%s\t' (seq 100)
            end
            printf 'arn:aws:ecs:region:account:task/cluster-chosen/task-other\tarn:aws:ecs:region:account:task/cluster-chosen/task-chosen\n'
        case describe-tasks
            if contains -- task-chosen $argv
                set -l query_index (contains -i -- --query $argv)
                set -g container_query $argv[(math $query_index + 1)]
                printf 'container-other\tcontainer-chosen\n'
                return
            end
            set -l tasks_index (contains -i -- --tasks $argv)
            set -l region_index (contains -i -- --region $argv)
            test (math $region_index - $tasks_index - 1) -le 100; or return 99
            printf 'arn:aws:ecs:region:account:task/cluster-chosen/task-other\tRUNNING\t2026-10-01T10:00:00+00:00\tFalse\n'
            set -l chosen_status RUNNING
            set -l chosen_exec True
            test "$scenario" = stopped; and set chosen_status STOPPED
            test "$scenario" = disabled; and set chosen_exec False
            if contains -- arn:aws:ecs:region:account:task/cluster-chosen/task-chosen $argv
                printf 'arn:aws:ecs:region:account:task/cluster-chosen/task-chosen\t%s\t2026-10-02T10:00:00+00:00\t%s\n' $chosen_status $chosen_exec
            end
        case execute-command
            set -g exec_args $argv
            return $exec_status
        case '*'
            return 99
    end
end

function fzf
    set -g picker_calls (math $picker_calls + 1)
    test "$cancel_at" = "$picker_calls"; and return 130
    read --local --null picker_input
    if test $picker_calls -eq 3
        string match -rq 'task-chosen\t(RUNNING|STOPPED)\t2h 0m\t(True|False)' -- "$picker_input"; or return 99
        string match -q '*arn:aws*' -- "$picker_input"; and return 99
    end
    printf '%s\n' "$picker_input" | command fzf --filter=chosen $argv
end

set -l error_log (mktemp)
for scenario_spec in default custom batching disabled:4 stopped:4 exec-failure aws-failure:1 aws-failure:2 aws-failure:3 aws-failure:4 aws-failure:5 empty:1 empty:2 empty:3 empty:4 empty:5 cancelled:1 cancelled:2 cancelled:3 cancelled:4
    set -l scenario_parts (string split : -- $scenario_spec)
    set -g scenario $scenario_parts[1]
    set -l stage $scenario_parts[2]
    set -g aws_calls 0
    set -g picker_calls 0
    set -g exec_args
    set -g failure_at 0
    set -g empty_at 0
    set -g cancel_at 0
    set -g exec_status 0
    set -l expected_status 0
    set -l exec_command /bin/sh
    switch $scenario
        case custom
            set exec_command 'ls -la /tmp'
        case exec-failure
            set -g exec_status 43
            set expected_status 43
        case aws-failure
            set -g failure_at $stage
            set expected_status 42
        case disabled stopped
            set expected_status 1
        case empty
            set -g empty_at $stage
            set expected_status 1
        case cancelled
            set -g cancel_at $stage
            set expected_status 130
    end
    if test "$scenario" = custom
        aws-ecs-exec "$exec_command" 2>$error_log
    else
        aws-ecs-exec 2>$error_log
    end
    set -l actual_status $status
    test $actual_status -eq $expected_status; or exit 1
    switch $scenario_spec
        case disabled:4
            string match -q '*ECS Exec is not enabled*' -- (cat $error_log); or exit 1
        case stopped:4
            string match -q '*is not running*' -- (cat $error_log); or exit 1
        case empty:5
            string match -q '*No running containers found*' -- (cat $error_log); or exit 1
    end
    set -l expected_calls $stage
    if test "$scenario" = cancelled; and test $stage -ge 3
        set expected_calls (math $stage + 1)
    end
    if contains -- $scenario default custom batching exec-failure
        test (string join '|' -- $exec_args) = "ecs|execute-command|--region|ap-southeast-1|--cluster|cluster-chosen|--task|task-chosen|--container|container-chosen|--interactive|--command|$exec_command"; or exit 1
    else
        test $aws_calls -eq $expected_calls; or exit 1
        test (count $exec_args) -eq 0; or exit 1
    end
end
rm -f $error_log

for age_case in '59:59s' '60:1m' '3599:59m' '3600:1h 0m' '86399:23h 59m' '86400:1d 0h' '266400:3d 2h' '-60:0s'
    set -l age_parts (string split : -- "$age_case")
    set -l now (math "1790935200 + $age_parts[1]")
    test (__aws-ecs-exec-task-age '2026-10-02T10:00:00+00:00' $now) = "$age_parts[2]"; or exit 1
end
test (__aws-ecs-exec-task-age None 1790942400) = unknown; or exit 1
test (__aws-ecs-exec-task-age '2026-10-02T18:00:00+08:00' 1790942400) = '2h 0m'; or exit 1

# Evaluate the actual container query locally with AWS's JMESPath engine.
# Skeleton generation and --no-sign-request avoid credentials and AWS requests.
set -l task_fixture '{"tasks":[{"enableExecuteCommand":true,"lastStatus":"RUNNING","containers":[{"name":"app","lastStatus":"RUNNING"},{"name":"stopped","lastStatus":"STOPPED"}]},{"enableExecuteCommand":true,"lastStatus":"STOPPED","containers":[{"name":"old","lastStatus":"RUNNING"}]}]}'
set -l queried_containers (command aws ecs describe-tasks --region ap-southeast-1 \
    --no-sign-request --tasks task-id --generate-cli-skeleton output --output text \
    --query "`$task_fixture` | $container_query") || exit 1
test "$queried_containers" = app; or exit 1
echo 'Passed: elapsed times, real container query, task metadata, batching, error diagnostics, selections, command arguments, AWS failures, empty lists, cancellation and session exit status.'
