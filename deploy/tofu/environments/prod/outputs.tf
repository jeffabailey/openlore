output "pds_url" { value = module.pds.pds_url }
output "handle" { value = module.pds.handle }
output "public_ip" { value = module.pds.public_ip }
output "atproto_namespace" { value = module.pds.atproto_namespace }
output "instance_id" { value = module.pds.instance_id }
output "data_volume_id" { value = module.pds.data_volume_id }
output "account_ssm_parameters" { value = module.pds.account_ssm_parameters }
output "backup_alarm_topic_arn" { value = module.pds.backup_alarm_topic_arn }
output "indexer_log_group" { value = aws_cloudwatch_log_group.indexer.name }
output "indexer_repo_dids_parameter" { value = aws_ssm_parameter.indexer_repo_dids.name }
output "indexer_alarm_names" {
  value = [
    aws_cloudwatch_metric_alarm.indexer_total_outage.alarm_name,
    aws_cloudwatch_metric_alarm.indexer_pass_failed.alarm_name,
    aws_cloudwatch_metric_alarm.indexer_not_live.alarm_name,
  ]
}
