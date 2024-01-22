# Use a Rust-based image as the base image
FROM rust:1.75-buster as build

# Set the working directory inside the container
WORKDIR /app

# Copy the Cargo.toml and Cargo.lock files to the container
COPY Cargo.toml Cargo.lock ./

# Copy the rest of the application source code into the container
COPY . .

# Build the Rust application
RUN cargo build --release

# Create a new image that only contains the compiled application
FROM debian:bullseye-slim
WORKDIR /app

RUN apt-get update
RUN apt-get install ca-certificates -y 
RUN update-ca-certificate
RUN ldconfig
RUN apt-get install libssl-dev

RUN apt-get update && apt-get install -y libsqlite3-0

# Copy the compiled binary from the build stage into the final image
COPY --from=build /app/target/release/rust-rss-feed /app

# Expose port 8080 for the application
EXPOSE 8080

# Start the Rust application when the container starts
CMD ["./rust-rss-feed"]
