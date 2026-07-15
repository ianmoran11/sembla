terraform {
  required_version = ">= 1.5.0, < 2.0.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = var.aws_region

  # The default mode permits credential-free, API-free planning. Paid apply and
  # destroy operations must explicitly set offline_plan=false and use credentials
  # from the operator's environment.
  access_key = var.offline_plan ? "offline-plan-access-key" : null
  secret_key = var.offline_plan ? "offline-plan-secret-key" : null

  skip_credentials_validation = var.offline_plan
  skip_metadata_api_check     = var.offline_plan
  skip_region_validation      = var.offline_plan
  skip_requesting_account_id  = var.offline_plan
}
