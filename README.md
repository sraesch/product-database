# Product Database
Simple service for managing a database of food products. The service is implemented in Rust and uses a postgres database.
See [Change Log](CHANGELOG.md) for the latest changes.

## Install Helm Chart
### Preqrequisites
- Kubernetes cluster
- ReadWriteOnce storage class
- Helm 3
- Helm 3 Secrets Plugin (See [here](https://github.com/jkroepke/helm-secrets) for installation instructions)

### Install with helm secrets plugin

#### Without a secrets manager
First you need to install the required tools:
```bash
# Ubuntu/Debian
sudo apt-get install gnupg
curl -LO https://github.com/getsops/sops/releases/download/v3.9.4/sops-v3.9.4.linux.amd64
sudo mv sops-v3.9.4.linux.amd64 /usr/local/bin/sops
sudo chmod +x /usr/local/bin/sops

# MacOS
brew install gnupg
brew install sops
```

Next, you'll need to generate a symmetric key on your system used to encrypt/decrypt secrets:
```bash
export KEY_NAME="John Doe"
export KEY_COMMENT="Key for encrypting secrets"

gpg --batch --full-generate-key <<EOF
%no-protection
Key-Type: 1
Key-Length: 4096
Subkey-Type: 1
Subkey-Length: 4096
Expire-Date: 0
Name-Comment: ${KEY_COMMENT}
Name-Real: ${KEY_NAME}
EOF
```
Let the public GPG fingerprint be printed by running:
```bash
gpg --list-secret-keys "${KEY_NAME}"
```
Extract the public key fingerprint and add it to the `.sops.yaml` file in the root of the repository:
```yaml
creation_rules:
- pgp: >-
        BF574406FD117762E9F4C8B11CB98A821DCBA1FC
```
Now SOPS will use the public key to encrypt the secrets in the repository.
Create a file called credentials.yaml.dec in the helm chart with the credentials as described in values.yaml. For example
```yaml
# The credentials for the database
credentials:
  db_password: postgres
...
```
Encrypt the file with SOPS:
```bash
helm secrets encrypt credentials.yaml.dec > credentials.yaml
```
Now you can install the helm chart with the following command:
```bash
helm secrets install product-database -f values.yaml -f credentials.yaml .
```

