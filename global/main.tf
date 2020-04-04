provider "aws" {
  version = "~> 2.53"
}

terraform {
  required_version = ">= 0.12"

  backend "s3" {
    bucket         = "vector-state"
    key            = "tests.tfstate"
    encrypt        = true
    dynamodb_table = "TerraformLocks"
  }
}
