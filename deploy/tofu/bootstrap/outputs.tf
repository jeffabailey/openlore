# Nothing here is a secret. The prod root's variable defaults hold these same values; if they
# ever differ, the prod root is wrong.

output "account_id" { value = module.pds_bootstrap.account_id }
output "backup_bucket" { value = module.pds_bootstrap.backup_bucket }
output "host_instance_profile_names" { value = module.pds_bootstrap.host_instance_profile_names }
output "hosted_zone_id" { value = module.pds_bootstrap.hosted_zone_id }
output "default_vpc_id" { value = module.pds_bootstrap.default_vpc_id }
output "state_bucket" { value = module.pds_bootstrap.state_bucket }
