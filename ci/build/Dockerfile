FROM arm32v7/rust:buster
# see https://github.com/rust-lang/cargo/issues/8719
RUN --mount=type=tmpfs,target=/usr/local/cargo/registry/ true
RUN --mount=type=tmpfs,target=/usr/local/cargo/git/ true
# needed for nanotec_stepper_driver
RUN apt update && apt install libudev-dev
