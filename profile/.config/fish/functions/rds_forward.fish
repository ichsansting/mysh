# forward a local port to an RDS instance through an SSM bastion (bstn-tagged instance)
function rds_forward
    set -l target (aws ec2 describe-instances \
        --region ap-southeast-1 \
        --filters "Name=tag:Name,Values='*bstn*'" "Name=instance-state-name,Values=running" \
        --query 'Reservations[*].Instances[*].InstanceId' \
        --output text)

    set -l host (aws rds describe-db-instances \
        --query "DBInstances[?contains(DBInstanceIdentifier, '$argv[1]')].[Endpoint.Address]" \
        --output text)

    aws ssm start-session \
        --region ap-southeast-1 \
        --target "$target" \
        --document-name AWS-StartPortForwardingSessionToRemoteHost \
        --parameters \
        host="$host",portNumber="5432",localPortNumber="$argv[2]"
end
