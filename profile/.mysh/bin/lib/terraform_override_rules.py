"""Role routing used only when the Terraform wrapper receives --override."""

ENVIRONMENT = "prod"
RESOURCE_TYPES = {"aws_kms_key", "aws_ssm_parameter"}  # Match both resources and data sources.
PROFILE = "DBAccess@tvlk-fpr-prod"
ALLOWED_ACCOUNT_IDS = ["348767535692"]
