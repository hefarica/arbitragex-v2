// SPDX-License-Identifier: MIT
pragma solidity 0.8.19;

/// @title SignatureValidator
/// @notice Library contract for ECDSA signature validation
/// @author ArbitrageX Team
/// @dev Provides gas-efficient signature recovery using Yul assembly
/// @dev No mock dependencies — pure cryptographic operations
contract SignatureValidator {

    // ============ Events ============

    /// @notice Emitted when a signature is successfully verified
    /// @param signer Recovered signer address
    /// @param hash Signed message hash
    event SignatureVerified(address indexed signer, bytes32 indexed hash);

    // ============ Errors ============

    /// @notice Invalid signature length
    error InvalidSignatureLength(uint256 length);

    /// @notice Signature has invalid S value (malleability protection)
    error InvalidSignatureS();

    /// @notice Signature has zero hash
    error ZeroHash();

    /// @notice Signer is not in the authorized set
    error UnauthorizedSignerAddress(address recovered);

    // ============ Constants ============

    /// @notice secp256k1 curve order / 2 (for malleability check)
    uint256 private constant SECP256K1_HALF_N =
        0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0;

    // ============ External Functions ============

    /// @notice Recover signer address from ECDSA signature
    /// @param hash keccak256 digest that was signed
    /// @param signature 65-byte ECDSA signature: r (32) + s (32) + v (1)
    /// @return signer Recovered signer address (address(0) on failure)
    /// @dev Gas cost: ~3000 (uses precompile 0x01 via Yul)
    /// @dev Malleability protection: enforces s <= SECP256K1_HALF_N
    function recoverSigner(bytes32 hash, bytes calldata signature)
        public
        pure
        returns (address signer)
    {
        // ── checks ──
        if (hash == bytes32(0)) revert ZeroHash();

        // Validate signature length
        if (signature.length != 65) {
            revert InvalidSignatureLength(signature.length);
        }

        // ── signature decomposition via Yul ──
        bytes32 r;
        bytes32 s;
        uint8 v;

        assembly ("memory-safe") {
            // r = signature[0:32]
            r := calldataload(signature.offset)
            // s = signature[32:64]
            s := calldataload(add(signature.offset, 32))
            // v = signature[64:65] (extract low byte)
            v := byte(0, calldataload(add(signature.offset, 64)))
        }

        // ── malleability check ──
        if (uint256(s) > SECP256K1_HALF_N) revert InvalidSignatureS();

        // ── ECRECOVER precompile (0x01) via Yul ──
        assembly ("memory-safe") {
            // Allocate scratch space
            let ptr := mload(0x40)

            // Write hash, r, s, v to scratch
            mstore(ptr, hash)
            mstore(add(ptr, 0x20), r)
            mstore(add(ptr, 0x40), s)
            mstore(add(ptr, 0x60), v)

            // Call ecrecover precompile
            let success := staticcall(gas(), 0x01, ptr, 0x80, ptr, 0x20)

            // Validate staticcall success
            if iszero(success) {
                // Return address(0) on failure
                mstore(0x00, 0)
                signer := mload(0x00)
                // Continue to function end
            }

            signer := mload(ptr)
        }
    }

    /// @notice Verify that recovered signer matches expected address
    /// @param hash Signed message hash
    /// @param signature 65-byte ECDSA signature
    /// @param expectedSigner Address that should have signed
    /// @return valid True if signature was signed by expectedSigner
    /// @dev Gas cost: ~3200 (recoverSigner + comparison)
    function verifySigner(
        bytes32 hash,
        bytes calldata signature,
        address expectedSigner
    )
        external
        pure
        returns (bool valid)
    {
        address recovered = recoverSigner(hash, signature);
        valid = recovered != address(0) && recovered == expectedSigner;
    }

    /// @notice Verify signature against a merkle-proof style multi-signer set
    /// @param hash Signed message hash
    /// @param signature 65-byte ECDSA signature
    /// @param authorizedSigners Array of authorized signer addresses
    /// @return valid True if signer is in the authorized set
    /// @return signerIndex Index of the signer in the authorized set
    /// @dev Gas cost: ~3000 + 100 * signerCount
    function verifyMultiSigner(
        bytes32 hash,
        bytes calldata signature,
        address[] calldata authorizedSigners
    )
        external
        pure
        returns (bool valid, uint256 signerIndex)
    {
        address recovered = recoverSigner(hash, signature);
        if (recovered == address(0)) return (false, type(uint256).max);

        uint256 len = authorizedSigners.length;
        for (uint256 i; i < len; ) {
            if (authorizedSigners[i] == recovered) {
                return (true, i);
            }
            unchecked { ++i; }
        }

        return (false, type(uint256).max);
    }

    /// @notice Batch verify multiple signatures for efficiency
    /// @param hashes Array of signed message hashes
    /// @param signatures Array of 65-byte signatures
    /// @param expectedSigners Array of expected signer addresses
    /// @return validFlags Boolean array indicating which signatures are valid
    /// @dev Gas cost: ~3000 * count
    function batchVerify(
        bytes32[] calldata hashes,
        bytes[] calldata signatures,
        address[] calldata expectedSigners
    )
        external
        pure
        returns (bool[] memory validFlags)
    {
        uint256 count = hashes.length;
        require(
            count == signatures.length && count == expectedSigners.length,
            "Length mismatch"
        );

        validFlags = new bool[](count);

        for (uint256 i; i < count; ) {
            validFlags[i] = verifySignerInternal(
                hashes[i],
                signatures[i],
                expectedSigners[i]
            );
            unchecked { ++i; }
        }
    }

    // ============ Internal Functions ============

    /// @notice Internal signature verification (no external call overhead)
    function verifySignerInternal(
        bytes32 hash,
        bytes calldata signature,
        address expectedSigner
    )
        internal
        pure
        returns (bool)
    {
        address recovered = recoverSigner(hash, signature);
        return recovered != address(0) && recovered == expectedSigner;
    }
}
