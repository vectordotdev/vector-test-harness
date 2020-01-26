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
  && rm -rf /var/lib/apt/lists/*

# Install Ansible.
RUN python3 -m pip install --no-cache-dir ansible \
  && ansible --version

# Install Terraform.
ARG TERRAFORM_VERSION=0.12.20
RUN curl -fsSL "https://releases.hashicorp.com/terraform/${TERRAFORM_VERSION}/terraform_${TERRAFORM_VERSION}_linux_amd64.zip" -o terraform.zip \
  && 7z x -o/usr/local/bin/ terraform.zip \
  && rm -rf terraform.zip \
  && terraform version

# Install AWS CLI.
RUN python3 -m pip install --no-cache-dir awscli \
  && aws --version

# Install jq.
RUN curl -fsSL https://github.com/stedolan/jq/releases/download/jq-1.6/jq-linux64 -o /usr/local/bin/jq \
  && chmod +x /usr/local/bin/jq \
  && jq --version

# Print the state after the installation to simplify troubleshooting.
RUN ls -la / \
  && ls -la /usr/local/bin

# Setup the workspace.
WORKDIR /vector-test-harness
COPY . .

CMD ["bin/test"]
