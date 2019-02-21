FROM ubuntu:18.04 AS build-deps

RUN \
  apt-get update && \
  apt-get -y install curl clang-7 zlib1g-dev llvm-7 llvm-7-dev && \
  apt-get clean

# Use Clang as it understands LLVM target triples
RUN update-alternatives --install /usr/bin/cc cc /usr/bin/clang-7 100

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain 1.32.0
ENV PATH "/root/.cargo/bin:${PATH}"

ADD . /opt/arret
WORKDIR /opt/arret

RUN cargo fetch

###

FROM build-deps as full-compiler
RUN cargo build --release

###

FROM ubuntu:18.04 AS repl

ARG vcs_ref

COPY --from=full-compiler /opt/arret/.arret-root /opt/arret/.arret-root
COPY --from=full-compiler /opt/arret/stdlib/arret /opt/arret/stdlib/arret
COPY --from=full-compiler /opt/arret/target/release/arret /opt/arret/target/release/arret
COPY --from=full-compiler /opt/arret/target/release/*.so /opt/arret/target/release/

RUN groupadd arret && useradd -r -g arret arret
USER arret:arret

WORKDIR /opt/arret
ENTRYPOINT ["/opt/arret/target/release/arret"]
CMD ["repl"]

# Label the commit that was used to build this
LABEL \
  org.label-schema.vcs-ref=$vcs_ref \
  org.label-schema.vcs-url="https://github.com/etaoins/arret"
