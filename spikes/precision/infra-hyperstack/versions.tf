terraform {
  required_version = ">= 1.5.0, < 2.0.0"

  required_providers {
    hyperstack = {
      source  = "NexGenCloud/hyperstack"
      version = "= 1.50.2-alpha"
    }
  }
}

provider "hyperstack" {
  # The alpha provider validates that a token exists when configured. A fixed,
  # non-secret placeholder keeps the zero-resource offline plan credential-free;
  # authenticated modes read HYPERSTACK_API_KEY from the environment.
  api_key = var.offline_plan ? "offline-placeholder-not-a-real-key" : null
}
