"""Protocol-bound quote verification; no universal CPMM fallback.

Ports accept a quote provider supplied by the host application. The V2 exact
reference adapter is included; other protocols REQUIRE their real existing
provider. Registration alone is not proof of on-chain correctness.
"""
from __future__ import annotations
from dataclasses import dataclass, asdict
from typing import Callable, Any
from .integrity import digest, HASH, BLOCK_HASH
from .math_reference import uint, v2_amount_out

@dataclass(frozen=True)
class QuoteRequest:
    protocol: str
    chain_id: int
    block_hash: str
    pool_address: str
    token_in: str
    token_out: str
    amount_in_raw: str
    pool_state_hash: str

@dataclass(frozen=True)
class QuoteEvidence:
    request_hash: str
    amount_out_raw: str
    source_kind: str
    source_reference: str
    raw_evidence_hash: str
    adapter_version: str

class QuoteRouter:
    def __init__(self):
        self._adapters:dict[str,Callable[[QuoteRequest],QuoteEvidence]]={}
    def register(self,protocol:str,adapter:Callable[[QuoteRequest],QuoteEvidence])->None:
        if not protocol or protocol in self._adapters:raise ValueError('empty or duplicate protocol registration')
        self._adapters[protocol]=adapter
    def quote(self,request:QuoteRequest)->dict[str,Any]:
        uint(request.amount_in_raw)
        if request.chain_id<=0 or not BLOCK_HASH.fullmatch(request.block_hash) or not HASH.fullmatch(request.pool_state_hash):
            raise ValueError('verified snapshot identity required')
        request_hash=digest(asdict(request))
        adapter=self._adapters.get(request.protocol)
        absent={'amount_out_raw':None,'request_hash':request_hash,'execution_authorized':False}
        if adapter is None:return {**absent,'state':'not_computed','reason':'protocol_adapter_unavailable'}
        try:out=adapter(request)
        except Exception as exc:
            return {**absent,'state':'not_computed','reason':'adapter_failure','error_type':type(exc).__name__}
        if not isinstance(out,QuoteEvidence):
            return {**absent,'state':'invalid','reason':'quote_contract_mismatch'}
        if out.request_hash!=request_hash:
            return {**absent,'state':'invalid','reason':'stale_or_cross_request_quote'}
        if out.source_kind not in {'onchain_quote','exact_reference'} or not out.source_reference or not out.adapter_version or not HASH.fullmatch(out.raw_evidence_hash):
            return {**absent,'state':'invalid','reason':'quote_provenance_missing'}
        try:uint(out.amount_out_raw)
        except ValueError:return {**absent,'state':'invalid','reason':'invalid_output_amount'}
        return {'state':'computed','request_hash':request_hash,**asdict(out),
                'source_authenticity_verified':False,'execution_authorized':False}


def v2_reference_adapter(snapshot:dict[str,Any])->Callable[[QuoteRequest],QuoteEvidence]:
    """Register specifically as uniswap_v2_cpmm; not as StableSwap/weighted/V3."""
    required={'pool_address','chain_id','block_hash','token0','token1','reserve0','reserve1','fee_numerator','fee_denominator'}
    if not required<=snapshot.keys():raise ValueError('incomplete pool snapshot')
    frozen=dict(snapshot);snapshot_hash=digest(frozen)
    def quote(request:QuoteRequest)->QuoteEvidence:
        if request.protocol!='uniswap_v2_cpmm':raise ValueError('protocol-math mismatch')
        if (request.chain_id,request.block_hash,request.pool_address,request.pool_state_hash)!=(frozen['chain_id'],frozen['block_hash'],frozen['pool_address'],snapshot_hash):
            raise ValueError('pool snapshot mismatch')
        if {request.token_in,request.token_out}!={frozen['token0'],frozen['token1']}:raise ValueError('token pair mismatch')
        zero_for_one=request.token_in==frozen['token0']
        ri,ro=(frozen['reserve0'],frozen['reserve1']) if zero_for_one else (frozen['reserve1'],frozen['reserve0'])
        amount=v2_amount_out(request.amount_in_raw,ri,ro,frozen['fee_numerator'],frozen['fee_denominator'])
        return QuoteEvidence(digest(asdict(request)),str(amount),'exact_reference','math_reference.v2_amount_out',snapshot_hash,'v2-reference-1')
    return quote
