FROM rust:1.94-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY queries ./queries
RUN cargo build --locked --release --bins

FROM debian:bookworm-slim
RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    python3 python3-pip python3-setuptools python3-wheel \
    curl ca-certificates unzip ssh \
  && rm -rf /var/lib/apt/lists/*

# These remain external infrastructure tools; harness logic lives in Rust.
RUN python3 -m pip install --no-cache-dir --break-system-packages \
    ansible boto3 botocore awscli

ARG TERRAFORM_VERSION=1.9.8
ARG TARGETARCH
RUN curl -fsSL "https://releases.hashicorp.com/terraform/${TERRAFORM_VERSION}/terraform_${TERRAFORM_VERSION}_linux_${TARGETARCH}.zip" -o /tmp/terraform.zip \
  && unzip -o /tmp/terraform.zip -d /usr/local/bin/ \
  && rm /tmp/terraform.zip

COPY --from=build /build/target/release/vth /usr/local/bin/vth
COPY --from=build /build/target/release/vth-guest /usr/local/bin/vth-guest
WORKDIR /vector-test-harness
COPY . .
ENTRYPOINT ["/usr/local/bin/vth", "container"]
CMD ["--help"]
