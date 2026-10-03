# The bootstrap's outputs, as variables with their known values as defaults.
#
# Why not terraform_remote_state: every value is deterministic -- "<name_prefix>-pds-host-<env>"
# and "<name_prefix>-identity-backup-<account>" -- so reading the bootstrap state would add a
# second state read (and its permission) to learn strings already known. If a default here ever
# disagrees with `tofu -chdir=deploy/tofu/bootstrap output`, the plan fails on a missing
# instance profile rather than doing anything destructive.

variable "hosted_zone_id" {
  description = "The existing public zone jeffbailey.us (bootstrap output hosted_zone_id)."
  type        = string
  default     = "Z04289081C40P36K0S8LM"
}

variable "instance_profile_name" {
  description = "Host instance profile (bootstrap output host_instance_profile_names[\"prod\"])."
  type        = string
  default     = "openlore-pds-host-prod"
}

variable "backup_bucket" {
  description = "Identity backup bucket (bootstrap output backup_bucket)."
  type        = string
  default     = "openlore-identity-backup-091153021562"
}

variable "ami_id" {
  description = "Pin the AMI. Null resolves the current AL2023 arm64 image at create time."
  type        = string
  default     = null
}

variable "ssh_key_name" {
  description = "Break-glass EC2 key pair. Null (the default) means none; use SSM Session Manager."
  type        = string
  default     = null
}

variable "ssh_ingress_cidr" {
  description = "One address, as CIDR. Null closes SSH, which is the default."
  type        = string
  default     = null
}

variable "aws_profile" {
  description = "Named AWS profile for the laptop run (e.g. \"jeff\"). Null uses the default credential chain."
  type        = string
  default     = null
}
