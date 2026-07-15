# Safe credential-free planning example. Copy to terraform.tfvars and replace
# the key name and TEST-NET address before a real apply.
aws_region = "us-east-1"
key_name   = "replace-with-an-existing-ec2-key-pair"
ssh_cidr   = "203.0.113.10/32"
gpu_class  = "commodity"

# Keep true for local plan. Apply/destroy commands in README.md override it to
# false so credentials come from AWS_PROFILE/AWS_ACCESS_KEY_ID in the environment.
offline_plan = true

# Default-on forgotten-instance protection.
auto_stop_enabled = true
auto_stop_hours   = 4

# Amazon-owned Deep Learning Base OSS NVIDIA Driver GPU AMI (Ubuntu 22.04)
# 20260714, pinned and verified in us-east-1. Override with a different immutable
# AMI ID whenever aws_region changes.
ami_id = "ami-072e487908654a0d2"
