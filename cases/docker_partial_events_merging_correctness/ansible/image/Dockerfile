FROM rust:1.94-alpine AS build
COPY vth-guest.rs /build/vth-guest.rs
RUN rustc --edition=2021 -O /build/vth-guest.rs -o /vth-guest

FROM alpine
COPY --from=build /vth-guest /vth-guest
CMD ["/vth-guest", "longlog"]
