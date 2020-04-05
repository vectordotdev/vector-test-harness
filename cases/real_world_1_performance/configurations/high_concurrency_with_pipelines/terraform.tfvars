# WARNING!
#
# Do not modify the parameters of this file since historical test results are
# based on these parameters. Please create a new configuration and specify taht
# configuration instead.
producer_instance_type   = "c5.large"
producer_instance_count  = 5
subject_instance_type    = "c5d.9xlarge"
subject_api_port         = 9000
subject_admin_port       = 9001
subject_media_proxy_port = 9002
subject_unfurler_port    = 9003
subject_audit_port       = 9004
consumer_instance_type   = "c5d.9xlarge" # slighly larger to ensure it is not a bottleneck
consumer_port            = 9000
