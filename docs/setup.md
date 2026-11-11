# Setup and benchmark guide

## Tools

For cloud runs, install Rust 1.94+, Terraform 1.0+, Ansible, the AWS CLI, and SSH.
Install Packer and its Amazon/Ansible plugins to build AMIs. Ansible needs the
`amazon.aws` collection and Python boto3/botocore packages. Choose compatible
Ansible, Python, and SDK versions using the installed collection's
[upstream requirements](https://github.com/ansible-collections/amazon.aws#requirements);
Ansible 2.16 is not a sufficient minimum for every collection release.
Local checks and saved-result analysis have fewer dependencies;
see the [workflow requirements](../README.md#setup).

```sh
cargo install --locked --path . --bin vth
vth doctor
```

Ruby, the `table_print` gem, jq, and GNU getopt are no longer harness-side
requirements. Some VM roles still install tools used by their workloads.

## Account and configuration

Configure AWS credentials using the normal AWS provider chain, such as an AWS
profile or temporary credentials supplied by your existing tooling. The harness
inherits them; it does not perform login or copy credentials into config files.

`bootstrap-aws` creates account-scoped state/results buckets, the Terraform lock
table, the Packer instance profile, an SSH identity if absent, and a plain `.env`.
It refuses to overwrite an existing `.env` unless `--force` is set.

```sh
vth bootstrap-aws --user-id <user>
```

Review an offline plan first if desired:

```sh
vth bootstrap-aws --user-id <user> --account-id <account-id> --dry-run
```

Alternatively copy `.env.example` to `.env` and fill in existing resources.
`vth` reads it automatically from the repository root, including when invoked
from a subdirectory. Existing exported settings override the file. The harness
never evaluates `.env` as shell code or sources `.envrc`.

For the legacy public Athena dataset and table setup, configure
`VECTOR_TEST_HARNESS_AWS_ACCOUNT_ID` and run `vth global-bootstrap`. That command
retains the global Terraform backend in `global/main.tf`; account-prerequisite
bootstrap and global dataset provisioning remain separate operations.

## Build the AMI

```sh
packer plugins install github.com/hashicorp/amazon
packer plugins install github.com/hashicorp/ansible
vth build-amis
```

Versions come from `config/subjects.json`. The AMI build also installs the Rust
`vth-guest` helper. Existing AMIs must be rebuilt for the migrated cases.

## Run and inspect a case

```sh
vth list
vth run -t tcp_to_tcp_performance -s vector -v 0.61.0 --dry-run
vth run -t tcp_to_tcp_performance -s vector -v 0.61.0
vth results -t tcp_to_tcp_performance
```

`-c` selects a configuration; the default is `default`. Available variants are
listed under each case's `configurations/` directory. `-u` selects the owning user.
Use `--ssh-cidr` when automatic public-IP discovery is inappropriate.

Use `--run-only` to reuse configured machines, `--skip-terraform` to repeat Ansible
bootstrap without reprovisioning, and `--bootstrap-only` to stop after setup.
Omitting `--subject` runs supported subjects sequentially.

Custom builds can be installed with
`--vector-deb /absolute/path/to/vector.deb --vector-version dev-name`.

## Debugging and results

```sh
vth ssh <instance-address>
vth compare -t tcp_to_tcp_performance -s vector -v <baseline> -v <candidate>
```

The SSH command selects the same identity as Ansible. Inspect VM services with
`journalctl`. `ANSIBLE_EXTRA_ARGS=-vvv` enables Ansible debug output.

`compare` and `export` use Athena. Set `VECTOR_TEST_ATHENA_OUTPUT` to an authorized
query-results S3 URI when the legacy default is unavailable. Raw dstat results
remain in the configured results bucket under the historical partitions.

## Teardown

```sh
vth teardown -t tcp_to_tcp_performance --yes
```

Use the same configuration and user ID as the run. The CLI initializes that
case's exact state key before destruction. Omitting `--yes` leaves Terraform's
interactive confirmation enabled. `--dry-run` prints the plan without contacting
AWS or changing files.

## Container

```sh
docker build -t vector-test-harness .
docker run --rm vector-test-harness --help
```

Mount an appropriate `.env`, SSH identity, and AWS configuration for cloud runs.
The container contains the compiled Rust binaries and runtime infrastructure
tools. Packer and Linux profiling tools are separate optional dependencies.
`GENERATE_SSH_KEY` and `INJECT_SSH_KEY` remain supported by the Rust entrypoint;
mounted identities can be configured directly instead.

The archived workloads and installers still need validation against your chosen
Vector version and environment. A successful harness unit/integration test does
not certify AWS provisioning, a Docker image, or a benchmark result.
