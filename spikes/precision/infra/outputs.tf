output "instance_id" {
  description = "EC2 instance ID."
  value       = aws_instance.gpu.id
}

output "public_ip" {
  description = "Public IPv4 address used for SSH and result retrieval."
  value       = aws_instance.gpu.public_ip
}

output "ssh_command" {
  description = "SSH command; add -i /path/to/private-key.pem when it is not in the SSH agent."
  value       = "ssh ubuntu@${aws_instance.gpu.public_ip}"
}

output "fetch_results_command" {
  description = "Copies the generated RESULTS.md into the current local directory."
  value       = "scp ubuntu@${aws_instance.gpu.public_ip}:/home/ubuntu/sembla/spikes/precision/RESULTS.md ./RESULTS.md"
}

output "gpu_profile" {
  description = "Selected instance and mandatory fp64 throughput metadata."
  value = {
    aws_region              = var.aws_region
    ami_request             = var.ami_id
    gpu_class               = var.gpu_class
    instance_type           = local.gpu_profile.instance_type
    gpu_model               = local.gpu_profile.gpu_model
    fp64_class              = local.gpu_profile.fp64_class
    fp64_fp32_ratio         = local.gpu_profile.fp64_fp32_ratio
    full_rate_extrapolation = local.gpu_profile.full_rate_extrapolation
  }
}

output "auto_stop" {
  description = "In-guest safety timer configuration."
  value = {
    enabled          = var.auto_stop_enabled
    hours_after_boot = var.auto_stop_hours
    shutdown_action  = "stop"
  }
}

output "estimated_on_demand_cost" {
  description = "Illustrative us-east-1 Linux On-Demand compute price; excludes EBS and transfer and must be rechecked before apply."
  value       = format("approximately $%.4f/hour plus storage and transfer", local.gpu_profile.on_demand_us_east_1_usd_hour)
}
