terraform {
  required_version = ">= 1.5.0"

  required_providers {
    vultr = {
      source  = "vultr/vultr"
      version = "~> 2.31"
    }
  }
}

provider "vultr" {
  # Never place the API key in Terraform files. The provider reads
  # VULTR_API_KEY from the environment.
  rate_limit  = 500
  retry_limit = 3
}
