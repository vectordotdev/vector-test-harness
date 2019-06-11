provider "aws" {
  region  = "us-east-1"
  version = "~> 2.12"
}

# Comment this out if you'd like
terraform {
  backend "s3" {
    bucket         = "vector-state"
    key            = "tests.tfstate"
    region         = "us-east-1"
    encrypt        = true
    dynamodb_table = "TerraformLocks"
  }
}