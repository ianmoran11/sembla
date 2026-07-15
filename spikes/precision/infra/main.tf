locals {
  gpu_profiles = {
    commodity = {
      instance_type                = "g4dn.xlarge"
      gpu_model                    = "NVIDIA T4"
      fp64_class                   = "rate-limited"
      fp64_fp32_ratio              = "approximately 1:32"
      full_rate_extrapolation      = "refused"
      on_demand_us_east_1_usd_hour = 0.526
    }
    full_rate = {
      instance_type                = "p4d.24xlarge"
      gpu_model                    = "8x NVIDIA A100 (the spike uses one selected device)"
      fp64_class                   = "full-rate"
      fp64_fp32_ratio              = "approximately 1:2"
      full_rate_extrapolation      = "allowed"
      on_demand_us_east_1_usd_hour = 32.7726
    }
  }

  gpu_profile = local.gpu_profiles[var.gpu_class]

  common_tags = {
    Project         = "sembla-precision-spike"
    ManagedBy       = "terraform"
    Purpose         = "throwaway-native-f64-measurement"
    GPUClass        = var.gpu_class
    FP64Class       = local.gpu_profile.fp64_class
    AutoStopEnabled = tostring(var.auto_stop_enabled)
  }
}

resource "aws_vpc" "gpu" {
  cidr_block           = "10.44.0.0/16"
  enable_dns_support   = true
  enable_dns_hostnames = true

  tags = merge(local.common_tags, {
    Name = "${var.name_prefix}-vpc"
  })
}

resource "aws_internet_gateway" "gpu" {
  vpc_id = aws_vpc.gpu.id

  tags = merge(local.common_tags, {
    Name = "${var.name_prefix}-igw"
  })
}

resource "aws_subnet" "gpu" {
  vpc_id                  = aws_vpc.gpu.id
  cidr_block              = "10.44.1.0/24"
  availability_zone       = var.availability_zone
  map_public_ip_on_launch = true

  tags = merge(local.common_tags, {
    Name = "${var.name_prefix}-public"
  })
}

resource "aws_route_table" "gpu" {
  vpc_id = aws_vpc.gpu.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.gpu.id
  }

  tags = merge(local.common_tags, {
    Name = "${var.name_prefix}-public"
  })
}

resource "aws_route_table_association" "gpu" {
  subnet_id      = aws_subnet.gpu.id
  route_table_id = aws_route_table.gpu.id
}

resource "aws_security_group" "gpu" {
  name_prefix = "${var.name_prefix}-"
  description = "Restricted SSH access to the throwaway precision GPU VM"
  vpc_id      = aws_vpc.gpu.id

  ingress {
    description = "SSH from the operator-supplied CIDR only"
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = [var.ssh_cidr]
  }

  egress {
    description = "Package, Git, and Rust toolchain downloads"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = merge(local.common_tags, {
    Name = "${var.name_prefix}-ssh"
  })

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_instance" "gpu" {
  depends_on = [aws_route_table_association.gpu]

  ami                                  = var.ami_id
  instance_type                        = local.gpu_profile.instance_type
  key_name                             = var.key_name
  subnet_id                            = aws_subnet.gpu.id
  vpc_security_group_ids               = [aws_security_group.gpu.id]
  associate_public_ip_address          = true
  instance_initiated_shutdown_behavior = "stop"
  user_data_replace_on_change          = true

  user_data = templatefile("${path.module}/cloud-init.sh.tftpl", {
    ami_id                  = var.ami_id
    auto_stop_enabled       = tostring(var.auto_stop_enabled)
    auto_stop_hours         = var.auto_stop_hours
    aws_region              = var.aws_region
    fp64_class              = local.gpu_profile.fp64_class
    fp64_fp32_ratio         = local.gpu_profile.fp64_fp32_ratio
    full_rate_extrapolation = local.gpu_profile.full_rate_extrapolation
    gpu_class               = var.gpu_class
    gpu_model               = local.gpu_profile.gpu_model
    instance_type           = local.gpu_profile.instance_type
    repository_ref          = var.repository_ref
    repository_url          = var.repository_url
    run_spike_b64           = base64encode(file("${path.module}/run-spike.sh"))
  })

  dynamic "instance_market_options" {
    for_each = var.use_spot ? [1] : []

    content {
      market_type = "spot"

      spot_options {
        max_price = var.spot_max_price
      }
    }
  }

  root_block_device {
    delete_on_termination = true
    encrypted             = true
    volume_size           = var.root_volume_gb
    volume_type           = "gp3"
  }

  metadata_options {
    http_endpoint = "enabled"
    http_tokens   = "required"
  }

  lifecycle {
    precondition {
      # Keep this pinned ID in sync with variables.tf, example.tfvars, and the
      # documented rotation checklist in README.md.
      condition     = var.aws_region == "us-east-1" || var.ami_id != "ami-072e487908654a0d2"
      error_message = "ami-072e487908654a0d2 is pinned to us-east-1; override ami_id with an immutable AMI from the selected aws_region."
    }
  }

  tags = merge(local.common_tags, {
    Name      = "${var.name_prefix}-${var.gpu_class}"
    GPUModel  = local.gpu_profile.gpu_model
    FP64Ratio = local.gpu_profile.fp64_fp32_ratio
  })
}
