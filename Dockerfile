FROM rust:1.90.0-slim as builder
# 安装必要的工具
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 创建工作目录
WORKDIR /usr/src/app

# 复制依赖文件
COPY Cargo.toml Cargo.lock ./

# 创建虚拟项目以缓存依赖
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# 复制源代码
COPY src ./src

# 构建真正的应用
RUN cargo build --release

# 运行时阶段
FROM debian:bookworm-slim

# 安装运行时依赖（根据你的RMCP需求调整）
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# 创建非root用户
RUN useradd -m -u 1000 appuser
USER appuser

# 复制可执行文件
COPY --from=builder /usr/src/app/target/release/weather /usr/local/bin/rmcp-service

# Vercel 需要监听 $PORT 环境变量
ENV PORT=3000

# 暴露端口
EXPOSE $PORT

# 健康检查（Vercel 需要）
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:$PORT/health || exit 1

# 启动命令
CMD ["rmcp-service"]