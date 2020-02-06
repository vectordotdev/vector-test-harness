output "cidr_block" {
  description = "The CIDR of the VPC"
  value       = aws_vpc.default.cidr_block
}

output "default_security_group_id" {
  description = "The ID of the default security group"
  value       = aws_security_group.default.id
}

output "default_subnet_id" {
  description = "The ID of the default subnet"
  value       = aws_subnet.default.id
}

output "vpc_id" {
  description = "The ID of the VPC"
  value       = aws_vpc.default.id
}
