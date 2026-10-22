#!/usr/bin/env python3
"""REA-trace (REVERT-TRACE-01) — vector DERIVADO del dispatch de decode de revert data.

Determinista, stdlib pura, sin red. NO es un vector upstream: los candidatos
evaluados no llevan pc/opcode (parity traces) o no tienen el segmento (artemis).
Los BYTES son hechos citables: selectores y codigos Panic provienen de la
documentacion canonica de Solidity (docs.soliditylang.org/en/latest/control-structures.html,
seccion "Panic via assert and Error via require") y del ABI estandar.

Salida: una linea "OK <item>" por verificacion; exit != 0 si alguna falla.
"""

ERROR_SELECTOR = bytes.fromhex("08c379a0")   # Error(string)
PANIC_SELECTOR = bytes.fromhex("4e487b71")   # Panic(uint256)

PANIC_NAMES = {
    0x00: "generic compiler panic",
    0x01: "assert failed",
    0x11: "arithmetic underflow/overflow",
    0x12: "division or modulo by zero",
    0x21: "enum conversion out of range",
    0x22: "storage byte array incorrectly encoded",
    0x31: "pop on empty array",
    0x32: "array index out of bounds",
    0x41: "too much memory allocated",
    0x51: "zero-initialized internal function called",
}


def abi_encode_error_string(msg: str) -> bytes:
    raw = msg.encode()
    return (
        ERROR_SELECTOR
        + (32).to_bytes(32, "big")          # offset
        + len(raw).to_bytes(32, "big")      # length
        + raw                               # data (sin padding para el vector)
    )


def abi_encode_panic(code: int) -> bytes:
    return PANIC_SELECTOR + code.to_bytes(32, "big")


def decode_revert(output: bytes) -> str:
    """Display string — CONTRATO INVARIANTE: identico al fallback del destino.

    La ambiguedad 4-bytes-genericos vs selector-de-custom-error NO es
    resoluble sin ABI: `deadbeef` es un selector valido. Por eso el display
    no cambia (el test del destino sequence_runner.rs:1507-1512 espera
    "0xdeadbeef"); la clasificacion viaja en `kind` aparte.
    """
    if len(output) == 0:
        return "empty"
    if len(output) >= 68 and output[0:4] == ERROR_SELECTOR:
        msg_len = int.from_bytes(output[64:68], "big")
        if len(output) >= 68 + msg_len:
            try:
                return "Error(string): " + output[68 : 68 + msg_len].decode("utf-8")
            except UnicodeDecodeError:
                pass
    if len(output) == 36 and output[0:4] == PANIC_SELECTOR:
        code = output[35]
        name = PANIC_NAMES.get(code)
        if name:
            return f"Panic(0x{code:02x}): {name}"
        return f"Panic(0x{code:02x}): unknown code"
    return "0x" + output[:36].hex()


def classify_revert(output: bytes) -> str:
    """Kind estructurado — lo nuevo que aporta el port (RevertFrame.kind)."""
    if len(output) == 0:
        return "Empty"
    if len(output) >= 68 and output[0:4] == ERROR_SELECTOR:
        return "ErrorString"
    if len(output) == 36 and output[0:4] == PANIC_SELECTOR:
        code = output[35]
        return f"Panic(0x{code:02x})" + ("[named]" if code in PANIC_NAMES else "[unknown]")
    if len(output) >= 4:
        return f"Selector(0x{output[0:4].hex()})"
    return "Raw"


def main() -> int:
    failures = []

    def check(name: str, cond: bool) -> None:
        print(("OK " if cond else "FAIL ") + name)
        if not cond:
            failures.append(name)

    # T1: Error(string) — replica el test existente del destino
    # (sequence_runner.rs:1488-1503, mensaje "insufficient_balance").
    v1 = abi_encode_error_string("insufficient_balance")
    check(
        "T1 error_string_bytes",
        v1.hex()
        == "08c379a0"
        + "00" * 31 + "20"
        + "00" * 31 + hex(len(b"insufficient_balance"))[2:]
        + b"insufficient_balance".hex(),
    )
    check(
        "T1 decode",
        decode_revert(v1) == "Error(string): insufficient_balance",
    )

    # T2: Panic(0x11) — replica el payload del test PANIC-CODE-01 del destino
    # (sequence_runner.rs:1520-1531): selector + word con 0x11 al final.
    v2 = abi_encode_panic(0x11)
    check("T2 panic11_len", len(v2) == 36)
    check("T2 panic11_bytes", v2.hex() == "4e487b71" + "00" * 31 + "11")
    check(
        "T2 decode",
        decode_revert(v2) == "Panic(0x11): arithmetic underflow/overflow",
    )

    # T3: todos los codigos canonicos decodifican a nombre.
    for code in sorted(PANIC_NAMES):
        d = decode_revert(abi_encode_panic(code))
        check(f"T3 panic_0x{code:02x}_named", d.startswith(f"Panic(0x{code:02x}): "))
    # Codigo no listado: 0x99
    check(
        "T3b unknown_code",
        decode_revert(abi_encode_panic(0x99)) == "Panic(0x99): unknown code",
    )

    # T4: custom error — sin ABI no es nombrable; kind=Selector(0x...) y el
    # display queda como fallback hex (compat con el test del destino).
    v4 = bytes.fromhex("2ac11621")  # ejemplo de selector custom (hecho citable: 4 bytes)
    check("T4 display_fallback_hex", decode_revert(v4) == "0x2ac11621")
    check("T4 kind_selector", classify_revert(v4) == "Selector(0x2ac11621)")

    # T5: vacio, raw y el caso-limite del destino (deadbeef).
    check("T5a empty", decode_revert(b"") == "empty" and classify_revert(b"") == "Empty")
    check(
        "T5b raw_deadosdeadbeef",
        decode_revert(bytes.fromhex("deadbeef")) == "0xdeadbeef"
        and classify_revert(bytes.fromhex("deadbeef")) == "Selector(0xdeadbeef)",
    )
    check(
        "T5c short",
        decode_revert(bytes.fromhex("deadbe")) == "0xdeadbe"
        and classify_revert(bytes.fromhex("deadbe")) == "Raw",
    )

    # T6: opcode fact — revert = 0xfd (Solidity docs, "instruction 0xfd").
    check("T6 revert_opcode", 0xFD == 253)

    print("INFO tabla completa:")
    print("  08c379a0 -> Error(string)")
    print("  4e487b71 -> Panic(uint256), codes:",
          " ".join(f"0x{c:02x}" for c in sorted(PANIC_NAMES)))
    print("  REVERT=0xfd  INVALID=0xfe")

    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
