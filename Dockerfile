FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    curl \
    wget \
    gnupg \
    software-properties-common \
    build-essential \
    pkg-config \
    libssl-dev \
    zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

# Installer LLVM 15 et forcer l'écrasement pour résoudre les conflits de paquets
RUN wget https://apt.llvm.org/llvm.sh && \
    chmod +x llvm.sh && \
    ./llvm.sh 15 && \
    apt-get install -o Dpkg::Options::="--force-overwrite" -y llvm-15-dev libpolly-15-dev && \
    rm llvm.sh

ENV LLVM_SYS_150_PREFIX=/usr/lib/llvm-15
ENV PATH="/usr/lib/llvm-15/bin:${PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /workspace
COPY . .

CMD ["cargo", "run"]
