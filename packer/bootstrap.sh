#!/bin/bash

# This file is designed to bootstrap an Ubuntu instance before it is
# provisioned using Ansible.
#
# First, it updates the apt cache and upgrades all packages to the latest
# version. This is important because Ansible is incapable of doing this
# due to limitations with the apt module (actually, it's a problem with
# apt). See:
#
#  - https://github.com/ansible/ansible-modules-core/issues/2951
#  - https://github.com/ansible/ansible-modules-core/issues/3523
#  - https://github.com/ansible/ansible/issues/18987
#  - https://github.com/ansible/ansible/issues/30754
#
# Installing aptitude (an apt helper utility) as suggested by some of
# the comments did not help
#
# Second, it installs the python-minimal package. This is because Ansible
# expects a Python 2 interpreter to be available on the remote machine
# at /usr/bin/python; python-minimal installs Python 2.7 and links it
# to /usr/bin/python. Installing the python2.7 package will not provide
# the /usr/bin/python executable.

sudo apt-get -y update
sudo DEBIAN_FRONTEND=noninteractive apt-get -y upgrade
