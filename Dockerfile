FROM debian:bookworm-slim

# Prepare system.
RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    python3 \
    python3-pip \
    python3-setuptools \
    python3-wheel \
    curl \
    ca-certificates \
    unzip \
    ssh \
    ruby \
  && rm -rf /var/lib/apt/lists/*

# Install Ansible, AWS CLI and boto3.
RUN python3 -m pip install --no-cache-dir --break-system-packages \
    ansible \
    boto3 \
    botocore \
    awscli \
  && ansible --version \
  && aws --version

# Install Terraform.
ARG TERRAFORM_VERSION=1.9.8
RUN curl -fsSL "https://releases.hashicorp.com/terraform/${TERRAFORM_VERSION}/terraform_${TERRAFORM_VERSION}_linux_amd64.zip" -o terraform.zip \
  && unzip -o terraform.zip -d /usr/local/bin/ \
  && rm -rf terraform.zip \
  && terraform version

# Install jq.
ARG JQ_VERSION=1.7.1
RUN curl -fsSL "https://github.com/jqlang/jq/releases/download/jq-${JQ_VERSION}/jq-linux-amd64" -o /usr/local/bin/jq \
  && chmod +x /usr/local/bin/jq \
  && jq --version

# Install ruby gems.
RUN gem install --no-document \
  table_print

# Expose Python 3 as `python`.
RUN ln -sT /usr/bin/python3 /usr/local/bin/python

# Print the state after the installation to simplify troubleshooting.
RUN set -x \
  && ls -la / \
  && ls -la /usr/local/bin

# Setup the workspace.
WORKDIR /vector-test-harness
COPY . .

# Set the entrypoint.
ENTRYPOINT [ "/vector-test-harness/docker/entrypoint" ]

# By default, print usage hint, and force users to specify command manually.
CMD [ "echo", "Usage: bin/test ..." ]
