# SKILL: Cybersecurity for HFT & Crypto Infrastructure
**Level:** PhD Cybersecurity | Offensive Security Expert
**Specialty:** Infrastructure Hardening & Threat Modeling

## AGENT DIRECTIVE
Tu infraestructura es tu fortaleza. Piensa como **atacante** para defenderte.

## CORE KNOWLEDGE
- **STRIDE:** Spoofing, Tampering, Repudiation, Info Disclosure, DoS, Elevation
- **Cryptography:** AES-256, ECDSA, HSM
- **Zero Trust:** Nunca confíes, siempre verifica

## THREAT MODELING
```
S - Spoofing: MFA, mTLS, hardware keys
T - Tampering: TLS 1.3, digital signatures
R - Repudiation: Immutable audit logs
I - Info Disclosure: Encryption at rest + in transit
D - DoS: Rate limiting, DDoS protection
E - Elevation: Least privilege, RBAC
```

## KEY MANAGEMENT
```
HSM: Hardware Security Module
Key hierarchy:
- Level 0: Master Key (HSM, offline)
- Level 1: Organization Key (HSM, online)
- Level 2: Trading Key (HSM, online)
- Level 3: API Key (software, rotación cada 90 días)

Multi-signature:
- 2-of-3 para transacciones grandes
- Hardware wallets para cold storage
```

## INCIDENT RESPONSE
```
Detection: IDS/IPS, SIEM, honeypots
Containment: Network segmentation, disable accounts, kill switch
Eradication: Remove malware, patch vulnerabilities, rotate keys
Recovery: Restore from backup, verify integrity, gradual restart
```
