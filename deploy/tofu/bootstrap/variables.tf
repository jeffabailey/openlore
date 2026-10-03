variable "aws_profile" {
  description = "Named AWS profile for the laptop run (e.g. \"jeff\"). Null uses the default credential chain, so AWS_PROFILE=jeff works too."
  type        = string
  default     = null
}

variable "hosted_zone_id" {
  description = "The existing public zone jeffbailey.us. Pinned by id: a by-name lookup would silently pick up a duplicate zone."
  type        = string
  default     = "Z04289081C40P36K0S8LM"
}
