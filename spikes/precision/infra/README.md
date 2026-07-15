# Throwaway AWS GPU instance for the precision spike

> [!CAUTION]
> **GPU INSTANCES ARE EXPENSIVE.** The default `g4dn.xlarge` is approximately
> **$0.526/hour**, while `p4d.24xlarge` is approximately **$32.77/hour** in
> `us-east-1` for Linux On-Demand compute (illustrative prices checked for this
> spike; EBS and transfer are extra). Verify current AWS pricing and quota before
> every apply. Keep the default auto-stop timer enabled, retrieve `RESULTS.md`
> promptly, and always run `terraform destroy`.

This self-contained, AWS-specific Terraform root module creates exactly one GPU
EC2 instance plus a minimal VPC, public subnet, route, Internet gateway, and
restricted SSH security group. It deliberately uses local Terraform state. It is
throwaway measurement infrastructure, not a production module or remote-state
example.

## GPU classes and fp64 interpretation

| `gpu_class` | Instance | GPU | fp64 class recorded in `RESULTS.md` | Approx. fp64:fp32 | Illustrative us-east-1 On-Demand |
| --- | --- | --- | --- | --- | ---: |
| `commodity` (default) | `g4dn.xlarge` | NVIDIA T4 | `rate-limited` | ~1:32 | $0.526/hour |
| `full_rate` | `p4d.24xlarge` | 8x NVIDIA A100; the spike selects one device | `full-rate` | ~1:2 | $32.77/hour |

A commodity run is the cheapest honest signal, but it is pessimistic for native
`f64`; its output explicitly says `full-rate-extrapolation: refused`. The
`full_rate` result is the number the precision decision hinges on. **Run
`full_rate` at least once whenever the commodity result is not already clearly
decisive.** P4 capacity and service quota can require advance approval or a
specific `availability_zone`.

On-Demand is the reproducible default. Set `use_spot=true` only if interruption
and loss of an un-fetched result are acceptable. Spot has no fixed price delta;
AWS advertises discounts of up to 90%, but the real delta is the table's
On-Demand estimate minus the current Spot price in the chosen availability zone.
Check both classes immediately before apply:

```bash
aws ec2 describe-spot-price-history \
  --region us-east-1 \
  --instance-types g4dn.xlarge p4d.24xlarge \
  --product-descriptions 'Linux/UNIX' \
  --start-time "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --max-items 20 \
  --query 'SpotPriceHistory[].{type:InstanceType,az:AvailabilityZone,usd_per_hour:SpotPrice}'
```

Spot pricing and capacity change continuously, and a Spot P4d can still be
expensive. `spot_max_price` is an optional decimal hourly cap, not a cost
guarantee.

## AMI, credentials, state, and safety

`aws_region` defaults to `us-east-1`. `ami_id` is immutably pinned to
`ami-072e487908654a0d2`, Amazon's x86_64 **Deep Learning Base OSS NVIDIA Driver
GPU AMI (Ubuntu 22.04) 20260714** in that region. It was verified as
Amazon-owned, available, HVM/EBS-backed, and compatible with both G4dn and P4d.
That DLAMI provides the NVIDIA driver and CUDA stack; cloud-init verifies both
and installs CUDA Toolkit 12.9 if `nvcc` is absent.

AMI IDs are region-specific. Terraform rejects changing `aws_region` while
retaining this us-east-1 default. To rotate the image or use another region,
resolve and verify a new immutable ID, then update `ami_id` in
`terraform.tfvars`. When rotating the module default, update all four committed
locations together: the `ami_id` default in `variables.tf`, the value in
`example.tfvars`, the us-east-1 guard in `main.tf`, and the pinned ID/release
text in this README:

```bash
export AWS_REGION=us-east-1
AMI_ID=$(aws ssm get-parameter \
  --region "$AWS_REGION" \
  --name /aws/service/deeplearning/ami/x86_64/base-oss-nvidia-driver-gpu-ubuntu-22.04/latest/ami-id \
  --query Parameter.Value --output text)
aws ec2 describe-images \
  --region "$AWS_REGION" \
  --owners amazon \
  --image-ids "$AMI_ID" \
  --query 'Images[0].{id:ImageId,name:Name,state:State,arch:Architecture}'
printf 'Pin this verified regional DLAMI: %s\n' "$AMI_ID"
```

Never put the mutable `/latest/` selector in `ami_id`; it is only a discovery
mechanism for selecting a new fixed ID. No cloud credentials or secrets belong
in this module: use `AWS_PROFILE`, AWS SSO, or the standard `AWS_ACCESS_KEY_ID` /
`AWS_SECRET_ACCESS_KEY` environment variables.

State remains local and is ignored by Git (`*.tfstate*`, `.terraform/`, plans,
and operator `terraform.tfvars`). Losing local state makes cleanup harder, so do
not delete it until after destroy.

`offline_plan=true` is the safe default: the provider uses obvious mock
credentials and makes no AWS calls, allowing local plan without credentials.
Every real apply or destroy below overrides it to `false`. The instance also has
`instance_initiated_shutdown_behavior = "stop"`; by default cloud-init enables a
systemd timer that powers it off four hours after each boot. Change
`auto_stop_hours` if needed, but disabling `auto_stop_enabled` removes the last
forgotten-instance guard and is strongly discouraged. **A stopped instance still
incurs EBS charges and is not a substitute for destroy.**

## Exact plan → apply → run → fetch → destroy workflow

All Terraform commands run from this directory:

```bash
cd spikes/precision/infra
cp example.tfvars terraform.tfvars
```

Edit `terraform.tfvars` before a real apply:

- replace `key_name` with an existing EC2 key pair in `aws_region`;
- replace the TEST-NET `ssh_cidr` with the operator's public IP as `/32`;
- choose `gpu_class = "commodity"` or `"full_rate"`;
- keep the pinned default AMI in `us-east-1`, or set a verified immutable
  `ami_id` from the selected region.

Only a restricted IPv4 CIDR is accepted for SSH ingress; `0.0.0.0/0` is rejected.

### 1. Init, format, validate, and credential-free plan

```bash
terraform init
terraform fmt -check -recursive
terraform validate
terraform plan -refresh=false -var-file=example.tfvars
```

The example plan uses the documentation-only CIDR and key name, makes no AWS API
calls, and should show one `aws_instance.gpu` plus six minimal networking
resources (seven additions total). To verify the full-rate switch without credentials:

```bash
terraform plan -refresh=false -var-file=example.tfvars -var='gpu_class=full_rate'
```

### 2. Apply manually with credentials

Do not run apply in automation. Authenticate, confirm the selected class and
hourly cost, then apply with real provider credential checks enabled:

```bash
export AWS_PROFILE=your-profile
terraform plan -var-file=terraform.tfvars -var='offline_plan=false'
terraform apply -var-file=terraform.tfvars -var='offline_plan=false'
```

Do not apply a saved offline plan. Inspect the plan: it must contain exactly one
GPU instance and SSH ingress only from `ssh_cidr`.

### 3. Wait for bootstrap and SSH

```bash
terraform output -raw ssh_command
# Add: -i /path/to/private-key.pem
ssh -i /path/to/private-key.pem ubuntu@$(terraform output -raw public_ip) \
  'cloud-init status --wait && test -f /var/lib/sembla-bootstrap-complete'
ssh -i /path/to/private-key.pem ubuntu@$(terraform output -raw public_ip)
```

The bootstrap log is `/var/log/sembla-bootstrap.log`. It installs build tools,
ensures NVIDIA/CUDA and Rust stable are available, clones `repository_ref`, and
creates `/home/ubuntu/run-spike.sh`.

### 4. Run the spike

On the instance:

```bash
/home/ubuntu/run-spike.sh
```

The script sets Vulkan, runs `cargo build --release --features cuda` followed by
`cargo run --release --features cuda`, and writes
`/home/ubuntu/sembla/spikes/precision/RESULTS.md`. Its header records
`gpu_class`, `fp64-class`, ratio, extrapolation permission, AWS region,
requested and actual AMI IDs, instance type, repository commit, and actual
`nvidia-smi` device. PRD 0005's benchmark output is
captured by the same command when that benchmark is added to the spike binary.

### 5. Fetch `RESULTS.md`

From the local module directory:

```bash
scp -i /path/to/private-key.pem \
  ubuntu@$(terraform output -raw public_ip):/home/ubuntu/sembla/spikes/precision/RESULTS.md \
  ../RESULTS.md
```

Confirm the fetched file's `fp64-class` matches the chosen Terraform
`gpu_class`. A rate-limited result must not be presented or extrapolated as
full-rate.

### 6. Destroy immediately

```bash
terraform destroy -var-file=terraform.tfvars -var='offline_plan=false'
```

Verify AWS reports the instance and supporting resources destroyed. If Terraform
fails, stop/terminate the instance in the EC2 console immediately, then repair
state and rerun destroy. Do not leave the task until the paid GPU is gone.
