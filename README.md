# Lazy-ACME

Lazy-ACME is a daemon that automates TLS certificate acquisition and renewal using Let's Encrypt and the `lego` client. It simplifies certificate management for your domains with support for various DNS providers.

## Features

- **Automated Certificate Management**: Acquires and renews TLS certificates via Let's Encrypt or other ACME-compatible providers.
- **DNS Provider Support**: Integrates with DNS providers (e.g., Cloudflare) through configuration files.
- **REST API**: Offers endpoints to manage certificates, check task status, and retrieve certificate data.
- **Periodic Renewal**: Automatically renews certificates nearing expiration.
- **Docker Support**: Easily deployable as a Docker container.

## Project Structure

- **`src/`**: Source code directory.
  - `acme.rs`: Handles certificate acquisition and renewal logic.
  - `config.rs`: Manages configuration loading and updates.
  - `handlers.rs`: Defines REST API endpoints.
  - `init.rs`: Initializes configuration files and directories.
  - `main.rs`: Application entry point.
  - `response.rs`: Formats API responses.
  - `server.rs`: Sets up the Axum web server.
  - `state.rs`: Manages shared application state.
  - `tasks.rs`: Handles background tasks for certificate checks and renewals.
- **`.env.example`**: Template for environment variables.
- **`build.sh`**: Script to download the `lego` binary.
- **`docker-compose.yml`**: Docker Compose configuration for deployment.
- **`Dockerfile`**: Instructions for building the Docker image.
- **`Makefile`**: Automates building and pushing Docker images.
- **`config.toml`**: Maps domains to DNS providers (created on first run).
- **`[provider].dns.toml`**: DNS provider configuration (e.g., `cloudflare.dns.toml`).

## Usage (Docker Compose)

1. **Pull the Image**:
   ```bash
   docker pull canmi/lazy-acme:latest
   ```

2. **Configure Environment**:
   Copy `.env.example` to `.env` and set variables:
   ```bash
   cp .env.example .env
   ```
   Edit `.env`:
   ```bash
   LOG_LEVEL=info
   UPDATE_INTERVAL_HOURS=24
   DIR_PATH=/opt/lazy-acme
   BIND_PORT=33301
   ```

3. **Set Up Configuration**:
   On first run, Lazy-ACME creates `config.toml` and `cloudflare.dns.toml` in `DIR_PATH`. Edit these files:
   - `config.toml`:
     ```toml
     [[domains]]
     name = "example.com"
     dns_provider = "cloudflare"
     ```
   - `cloudflare.dns.toml`:
     ```toml
     api_key = "YOUR_CLOUDFLARE_API_TOKEN"
     email = "your-email@example.com"
     ca = "https://acme-v02.api.letsencrypt.org/directory"
     ```

4. **Run with Docker Compose**:
   Use the provided `docker-compose.yml`:
   ```yaml
   services:
     lazy-acme:
       image: canmi/lazy-acme:latest
       container_name: lazy-acme
       networks:
         - internal
       ports:
         - "33301:33301/tcp"
       env_file:
         - ./.env
       volumes:
         - /opt/lazy-acme:/root/lazy-acme
       restart: unless-stopped
   networks:
     internal:
       driver: bridge
   ```
   Start the service:
   ```bash
   docker-compose up -d
   ```

5. **Access the API**:
   The service runs on `http://127.0.0.1:33301`. Use endpoints like:
   - `POST /v1/certificate`: Request a certificate.
   - `GET /v1/certificate/{domain}`: Retrieve a certificate.
   - `GET /v1/certificate/{domain}/key`: Retrieve a certificate key.

## Building and Compiling

To build and push a multi-architecture Docker image:

```bash
make push
```

This command uses `docker buildx` to create and push images for `linux/amd64` and `linux/arm64` to the Docker registry.