# open an SSM session to an EC2 instance found by its Name tag
function ssm
    set -l target (aws ec2 describe-instances \
        --region ap-southeast-1 \
        --filters "Name=tag:Name,Values=$argv[1]" "Name=instance-state-name,Values=running" \
        --query 'Reservations[*].Instances[*].InstanceId' \
        --output text)

    aws ssm start-session --region ap-southeast-1 --target $target
end
