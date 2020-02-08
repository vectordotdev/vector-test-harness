FROM debian:buster-backports

# Prepare system.
RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    python3 \
    python3-pip \
    python3-setuptools \
    python3-wheel \
    curl \
    ca-certificates \
    p7zip-full \
    ssh \
    ruby \
  && rm -rf /var/lib/apt/lists/*

# Install Ansible, AWS CLI and boto.
RUN python3 -m pip install --no-cache-dir \
    ansible \
    boto \
    awscli \
  && ansible --version \
  && aws --version

# Install Terraform.
ARG TERRAFORM_VERSION=0.12.20
RUN curl -fsSL "https://releases.hashicorp.com/terraform/${TERRAFORM_VERSION}/terraform_${TERRAFORM_VERSION}_linux_amd64.zip" -o terraform.zip \
  && 7z x -o/usr/local/bin/ terraform.zip \
  && rm -rf terraform.zip \
  && terraform version

# Prepare terraform plugins.
ENV TF_PLUGIN_DIR=/usr/local/terraform-plugins/
# Install terraform-provider-aws.
ARG TERRAFORM_PROVIDER_AWS_VERSION=2.46.0
RUN curl -fsSL "https://releases.hashicorp.com/terraform-provider-aws/${TERRAFORM_PROVIDER_AWS_VERSION}/terraform-provider-aws_${TERRAFORM_PROVIDER_AWS_VERSION}_linux_amd64.zip" -o plugin.zip \
  && 7z x -o"$TF_PLUGIN_DIR" plugin.zip \
  && rm -rf plugin.zip

# Install jq.
RUN curl -fsSL https://github.com/stedolan/jq/releases/download/jq-1.6/jq-linux64 -o /usr/local/bin/jq \
  && chmod +x /usr/local/bin/jq \
  && jq --version

# Install ruby gems.
RUN gem install --no-rdoc --no-ri \
  table_print

# Expose Python 3 as `python`.
RUN ln -sT /usr/bin/python3 /usr/local/bin/python

# Print the state after the installation to simplify troubleshooting.
RUN set -x \
  && ls -la / \
  && ls -la /usr/local/bin \
  && ls -la "$TF_PLUGIN_DIR"

# Setup the workspace.
WORKDIR /vector-test-harness
COPY . .

# Set the entrypoint.
ENTRYPOINT [ "/vector-test-harness/docker/entrypoint" ]

# By default, print usage hint, and force users to specify command manually.
CMD [ "echo", "Usage: bin/test ..." ]
