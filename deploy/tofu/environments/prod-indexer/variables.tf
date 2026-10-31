variable "aws_profile" {
  description = "Named AWS profile for the laptop run (e.g. \"jeff\"). Null uses the default credential chain."
  type        = string
  default     = null
}

variable "phase" {
  description = "trial or cutover: selects the Lambda log group and the prod log-group grant."
  type        = string
  default     = "trial"

  validation {
    condition     = contains(["trial", "cutover"], var.phase)
    error_message = "phase is trial or cutover."
  }
}

variable "schedules_enabled" {
  description = "Until deploy.sh's first flip, live is unchecked $LATEST code and a pass could publish from it (D-STZ-26)."
  type        = bool
  default     = false
}

variable "lambda_alarms_enabled" {
  description = "Alarm actions, off until the bootstrap gate: the first exit-0 pass, a green probe and the test-fires."
  type        = bool
  default     = false
}

variable "cost_allocation_tag_active" {
  description = "Set once the Component tag has appeared in billing data; AWS refuses the activation before that."
  type        = bool
  default     = false
}

variable "bootstrap_image_uri" {
  description = "Create-time only: the image deploy.sh lambda seed prints. deploy.sh owns the image afterwards."
  type        = string
  default     = null
}
