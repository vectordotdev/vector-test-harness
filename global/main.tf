provider "aws" {
  region  = "us-east-1"
  version = "~> 2.53"
}

terraform {
  required_version = ">= 0.12"

  backend "s3" {
    bucket         = "vector-state"
    key            = "tests.tfstate"
    region         = "us-east-1"
    encrypt        = true
    dynamodb_table = "TerraformLocks"
  }
}
