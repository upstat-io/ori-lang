# Minimal LLVM 21 container for Ori compiler development
# Optimized for fast startup - all dependencies pre-baked

FROM debian:bookworm-slim

# Add LLVM apt repository and install LLVM 21
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    gnupg \
    && curl -fsSL https://apt.llvm.org/llvm-snapshot.gpg.key | gpg --dearmor -o /usr/share/keyrings/llvm-archive-keyring.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/llvm-archive-keyring.gpg] http://apt.llvm.org/bookworm/ llvm-toolchain-bookworm-21 main" > /etc/apt/sources.list.d/llvm.list \
    && apt-get update && apt-get install -y --no-install-recommends \
    llvm-21 \
    llvm-21-dev \
    libpolly-21-dev \
    clang-21 \
    lld-21 \
    build-essential \
    pkg-config \
    && rm -rf /var/lib/apt/lists/* \
    && ln -s /usr/bin/llvm-config-21 /usr/bin/llvm-config

# Install Rust (minimal, no docs) and cargo-tarpaulin for coverage
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/* \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --profile minimal --default-toolchain stable \
    && ~/.cargo/bin/rustup component add clippy \
    && ~/.cargo/bin/cargo install cargo-tarpaulin

# Put cargo in PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Set LLVM environment
ENV LLVM_SYS_211_PREFIX=/usr/lib/llvm-21

# Workspace mount point
WORKDIR /workspace

# Default to shell, but typically overridden
CMD ["/bin/bash"]
