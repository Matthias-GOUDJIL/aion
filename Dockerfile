FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

# Pure-LLVM C/C++ toolchain — no build-essential / gcc / binutils. Cargo build
# scripts (cc/bindgen) pick up CC/CXX/AR; rustc links via clang-15 + lld. #100.
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    gnupg \
    software-properties-common \
    make \
    libc6-dev \
    pkg-config \
    libssl-dev \
    zlib1g-dev \
    libgc-dev \
    && rm -rf /var/lib/apt/lists/*

# Installer LLVM 15 et forcer l'écrasement pour résoudre les conflits de paquets
RUN wget https://apt.llvm.org/llvm.sh && \
    chmod +x llvm.sh && \
    ./llvm.sh 15 && \
    apt-get install -o Dpkg::Options::="--force-overwrite" -y llvm-15-dev libpolly-15-dev lld-15 && \
    rm llvm.sh

ENV LLVM_SYS_150_PREFIX=/usr/lib/llvm-15
ENV PATH="/usr/lib/llvm-15/bin:${PATH}"

# Use the LLVM toolchain for everything: C/C++ compilation (cc/bindgen),
# archiving (ar), and linking (lld via clang-15). No gcc/binutils. #100.
ENV CC=clang-15
ENV CXX=clang++-15
ENV AR=llvm-ar-15
ENV RUSTFLAGS="-Clinker=clang-15 -Clink-arg=-fuse-ld=lld"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /workspace
COPY . .

# Pre-compile the C runtime to LLVM bitcode at build time so the `aion run`
# link step can use lld-15 (via clang-15 -fuse-ld=lld) instead of gcc. #73.
RUN clang-15 -c -emit-llvm -O2 -I/usr/include src/runtime.c -o /opt/aion_runtime.bc
ENV AION_RUNTIME_BC=/opt/aion_runtime.bc

CMD ["cargo", "run"]
