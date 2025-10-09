# Use Ubuntu as base image
FROM ubuntu:20.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    libssl-dev \
    libwebsockets-dev \
    libpq-dev \
    redis-tools \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy source files
COPY cpp-arbitrage-engine.cpp cpp-types-header.h ./

# Copy main file
COPY main.cpp ./

# Build the application
RUN g++ -std=c++17 -O3 -o arbitrage_scanner main.cpp cpp-arbitrage-engine.cpp

# Expose port
EXPOSE 8080 9090

# Run the application
CMD ["./arbitrage_scanner"]