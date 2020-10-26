provider "aws" {
  version = "~> 3.11"
}

terraform {
  required_version = ">= 0.13"

  backend "s3" {
    bucket         = "vector-state"
    key            = "tests.tfstate"
    encrypt        = true
    dynamodb_table = "TerraformLocks"
  }
}
