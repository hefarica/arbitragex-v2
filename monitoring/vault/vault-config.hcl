# Vault configuration — Dev mode with TLS
# Generated 2026-05-17 as hotfix for P0-002

listener "tcp" {
  address     = "0.0.0.0:8200"
  tls_cert_file = "/vault/tls/vault-cert.pem"
  tls_key_file  = "/vault/tls/vault-key.pem"
  tls_disable   = false
}

storage "file" {
  path = "/vault/data"
}

api_addr = "https://vault:8200"
cluster_addr = "https://vault:8201"

ui = true

# Dev mode settings (no production!)
disable_mlock = true
