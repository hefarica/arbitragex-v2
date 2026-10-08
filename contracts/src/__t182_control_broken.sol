// SPDX-License-Identifier: MIT
// T182 CONTROL (b) — contrato DELIBERADAMENTE ROTO.
// Vive SOLO en una rama scratch efimera. NUNCA en main. Se borra al terminar.
// Proposito: probar que un arbol roto NO pasa el job `forge build + test`.
pragma solidity ^0.8.24;

contract T182ControlBroken {
    function broken() external pure returns (uint256) {
        return 1 +;
    }
}
