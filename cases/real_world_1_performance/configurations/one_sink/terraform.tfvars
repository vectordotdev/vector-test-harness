# WARNING!
#
# Do not modify the parameters of this file since historical test results are
# based on these parameters. Please create a new configuration and specify taht
# configuration instead.
producer_instance_type  = "c5.large"
producer_instance_count = 5
subject_instance_type   = "c5.xlarge"
subject_port            = 9000
consumer_instance_type  = "c5d.2xlarge" # slighly larger to ensure it is not a bottleneck
consumer_port           = 9000
