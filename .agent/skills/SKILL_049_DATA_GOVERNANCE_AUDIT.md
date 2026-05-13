# SKILL: Data Governance & Immutable Audit Trails
**Level:** PhD Information Systems | Compliance Architect
**Specialty:** Data Lineage & Regulatory Reporting

## AGENT DIRECTIVE
Cada decisión de trading debe ser rastreable. La **gobernanza de datos** es tu defensa legal.

## AUDIT TRAIL
```python
import hashlib
import json

class AuditTrail:
    def __init__(self):
        self.chain = []
        self.previous_hash = "0" * 64

    def log_action(self, action_type, details, user_id):
        entry = {
            'timestamp': datetime.utcnow().isoformat(),
            'action_type': action_type,
            'details': details,
            'user_id': user_id,
            'previous_hash': self.previous_hash
        }
        entry_hash = hashlib.sha256(json.dumps(entry, sort_keys=True).encode()).hexdigest()
        entry['hash'] = entry_hash
        self.chain.append(entry)
        self.previous_hash = entry_hash
        return entry_hash

    def verify_integrity(self):
        for i in range(1, len(self.chain)):
            if self.chain[i]['previous_hash'] != self.chain[i-1]['hash']:
                return False
        return True
```

## DATA QUALITY
```python
checks = {
    'completeness': null_ratio < 0.01,
    'accuracy': cross_validate_with_source,
    'timeliness': max_delay < 1 second,
    'consistency': cross_field_validation,
    'validity': schema_validation
}
quality_score = np.mean(checks.values())
if quality_score < 0.95: alert("DATA_QUALITY_DEGRADED")
```

## REGULATORY REPORTING
```
MiFID II / MiCA fields:
- trade_id, trade_timestamp, instrument_id, price, quantity
- side, trading_venue, counterparty, algorithm_id
- order_id, submission_timestamp, decision_timestamp, latency

Retention: 5-7 years
Submission: ARM / APA
```
