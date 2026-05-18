terraform {
  required_version = ">= 1.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }

  backend "s3" {
    bucket         = "vector-state"
    key            = "tests.tfstate"
    encrypt        = true
    dynamodb_table = "TerraformLocks"
  }
}

provider "aws" {}
