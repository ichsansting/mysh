# Saved commands for Atuin; add one quoted command per line.
set -g atuin_snippets \
    'terraform init -backend-config var-stg-backend.tfvars' \
    'terraform init -backend-config var-prod-backend.tfvars' \
    'terraform plan -var-file var-stg.tfvars' \
    'terraform plan -var-file var-prod.tfvars' \
    'terraform apply -var-file var-stg.tfvars' \
    'terraform apply -var-file var-prod.tfvars'
