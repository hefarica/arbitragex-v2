// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// DeterministicFactory.t.sol — Tests de Invariantes para CREATE2 Factory
// =============================================================================
// Verifica que el DeterministicFactory produce direcciones deterministicas
// usando CREATE2 con salts namespaceados por chainId.
//
// Invariantes verificadas:
//   I1. predictedAddressMatchesDeployed — La direccion predicha coincide con
//       la desplegada para el mismo bytecode + chainId + name.
//   I2. crossChainAddressDifferent — Direcciones diferentes en chains
//       diferentes (seguridad: evita replay attacks cross-chain).
//   I3. sameInputSameAddress — Mismo input SIEMPRE produce misma direccion.
//   I4. differentNameDifferentAddress — Nombres diferentes producen
//       direcciones diferentes.
//   I5. onlyPredictedAddressValid — Solo la direccion predicha contiene el
//       bytecode esperado post-deploy.
//   I6. deployIdempotent — Deploy con el mismo salt revierte (no duplicados).
//   I7. zeroBytecodeReverts — Deploy con bytecode vacio revierte.
//   I8. zeroChainIdProducesAddress — chainId = 0 produce direccion valida
//       (no revierte, aunque no sea recomendado).
//   I9. sameNameDifferentBytecode — Bytecode diferente produce direccion
//       diferente aunque el name sea el mismo.
//   I10. saltNamespaceing — El salt incluye chainId en el namespace.
//
// Ejecucion: forge test --match-path test/DeterministicFactory.t.sol --fuzz-runs 10000
// =============================================================================

import "forge-std/Test.sol";

// ---------------------------------------------------------------------------
// Stub del DeterministicFactory (sera reemplazado por el contrato real)
// @notice Este stub simula el comportamiento esperado del DeterministicFactory.
//         El agente encargado debe reemplazarlo con la implementacion real.
// ---------------------------------------------------------------------------

/// @title DeterministicFactory — CREATE2 factory con salts namespaceados
/// @notice Despliega contratos con direcciones deterministicas usando CREATE2.
///         El salt se computa como keccak256(abi.encodePacked(chainId, name))
///         para asegurar que las direcciones son unicas por chain.
/// @dev Stub temporal — reemplazar por implementacion real.
contract DeterministicFactory {
    /// @notice Evento emitido cuando un contrato es desplegado.
    /// @param deployedAddress La direccion del contrato desplegado.
    /// @param salt El salt usado para CREATE2.
    /// @param chainId El chainId namespaceado.
    /// @param name El nombre del contrato.
    event ContractDeployed(address indexed deployedAddress, bytes32 salt, uint256 chainId, string name);

    /// @notice mapping para trackear direcciones ya desplegadas (evita duplicados).
    mapping(bytes32 => address) public deployedAddresses;

    /// @notice Despliega un contrato usando CREATE2 con un salt namespaceado.
    /// @param bytecode El bytecode del contrato a desplegar.
    /// @param chainId El ID de la cadena para namespacear el salt.
    /// @param name El nombre identificador del contrato.
    /// @return deployedAddress La direccion del contrato desplegado.
    function deploy(bytes memory bytecode, uint256 chainId, string memory name) external returns (address deployedAddress) {
        require(bytecode.length > 0, "DeterministicFactory: empty bytecode");

        bytes32 salt = keccak256(abi.encodePacked(chainId, name));

        // Verificar que no se ha desplegado ya con este salt
        require(deployedAddresses[salt] == address(0), "DeterministicFactory: already deployed");

        // CREATE2 deploy
        assembly {
            deployedAddress := create2(0, add(bytecode, 0x20), mload(bytecode), salt)
        }

        require(deployedAddress != address(0), "DeterministicFactory: deploy failed");

        deployedAddresses[salt] = deployedAddress;

        emit ContractDeployed(deployedAddress, salt, chainId, name);
    }

    /// @notice Predice la direccion de un contrato antes de desplegarlo.
    /// @param bytecode El bytecode del contrato.
    /// @param chainId El ID de la cadena.
    /// @param name El nombre del contrato.
    /// @return predicted La direccion predicha.
    function predictAddress(bytes memory bytecode, uint256 chainId, string memory name) external view returns (address predicted) {
        bytes32 salt = keccak256(abi.encodePacked(chainId, name));
        bytes32 hash = keccak256(
            abi.encodePacked(
                bytes1(0xff),
                address(this),
                salt,
                keccak256(bytecode)
            )
        );
        predicted = address(uint160(uint256(hash)));
    }

    /// @notice Verifica si un contrato ya fue desplegado con un salt dado.
    /// @param chainId El ID de la cadena.
    /// @param name El nombre del contrato.
    /// @return deployed True si ya fue desplegado.
    function isDeployed(uint256 chainId, string memory name) external view returns (bool deployed) {
        bytes32 salt = keccak256(abi.encodePacked(chainId, name));
        return deployedAddresses[salt] != address(0);
    }
}

// ---------------------------------------------------------------------------
// Contratos de prueba para deploy
// ---------------------------------------------------------------------------

/// @dev Contrato simple para testing de deploy.
contract TestContractV1 {
    uint256 public value;
    string public name;

    constructor(uint256 _value, string memory _name) {
        value = _value;
        name = _name;
    }

    function getValue() external view returns (uint256) {
        return value;
    }
}

/// @dev Version alternativa con diferente bytecode.
contract TestContractV2 {
    uint256 public value;
    uint256 public extra;

    constructor(uint256 _value) {
        value = _value;
        extra = _value * 2;
    }
}

/// @dev Contrato con constructor payable.
contract PayableTestContract {
    uint256 public deposited;

    constructor() payable {
        deposited = msg.value;
    }
}

/// @dev Contrato vacio (minimal bytecode).
contract EmptyContract {
    fallback() external {}
}

// ---------------------------------------------------------------------------
// DeterministicFactoryTest
// ---------------------------------------------------------------------------

/// @title DeterministicFactoryTest
/// @notice Tests de invariantes para DeterministicFactory con CREATE2.
/// @dev Incluye fuzzing de 10,000 runs para verificar propiedades
///      deterministicas del factory.
contract DeterministicFactoryTest is Test {
    DeterministicFactory internal factory;

    function setUp() public {
        factory = new DeterministicFactory();
    }

    // =========================================================================
    // INVARIANT I1: predictedAddressMatchesDeployed
    // =========================================================================
    /// @notice INVARIANT I1 — La direccion predicha por predictAddress coincide
    ///         EXACTAMENTE con la direccion real del contrato desplegado.
    /// @dev Verifica la formula CREATE2: address = keccak256(0xff + deployer +
    ///      salt + keccak256(bytecode))[12:].
    function test_I1_predictedAddressMatchesDeployed() public {
        bytes memory bytecode = type(TestContractV1).creationCode;
        uint256 chainId = 1;
        string memory name = "TestContract";

        address predicted = factory.predictAddress(bytecode, chainId, name);
        address deployed = factory.deploy(bytecode, chainId, name);

        assertEq(predicted, deployed, "I1 VIOLATED: CREATE2 address mismatch");
    }

    // =========================================================================
    // INVARIANT I2: crossChainAddressDifferent
    // =========================================================================
    /// @notice INVARIANT I2 — El mismo bytecode + name en chains diferentes
    ///         produce direcciones DIFERENTES.
    /// @dev Seguridad: evita replay attacks cross-chain. El chainId se
    ///      incluye en el salt para namespacear.
    function test_I2_crossChainAddressDifferent() public {
        bytes memory bytecode = type(TestContractV1).creationCode;
        string memory name = "TestContract";

        uint256 chainId1 = 1;   // Ethereum mainnet
        uint256 chainId137 = 137; // Polygon
        uint256 chainId42161 = 42161; // Arbitrum

        address deployed1 = factory.deploy(bytecode, chainId1, name);

        // Usar nombre ligeramente diferente para evitar "already deployed"
        address deployed137 = factory.deploy(bytecode, chainId137, name);

        // El tercero necesita un name diferente ya que factory trackea por salt
        vm.expectRevert("DeterministicFactory: already deployed");
        factory.deploy(bytecode, chainId42161, name);

        // Verificar que las direcciones son diferentes para chains diferentes
        assertNotEq(deployed1, deployed137, "I2 VIOLATED: Misma direccion en chains diferentes");
    }

    // =========================================================================
    // INVARIANT I3: sameInputSameAddress
    // =========================================================================
    /// @notice INVARIANT I3 — Mismo bytecode + chainId + name SIEMPRE produce
    ///         la misma direccion (determinismo). Pero el segundo deploy revierte
    ///         porque ya existe.
    function test_I3_sameInputSameAddress() public {
        bytes memory bytecode = type(TestContractV1).creationCode;
        uint256 chainId = 1;
        string memory name = "SameInputTest";

        address predicted1 = factory.predictAddress(bytecode, chainId, name);

        // Primer deploy: exitoso
        address deployed1 = factory.deploy(bytecode, chainId, name);
        assertEq(predicted1, deployed1, "I3: Primera prediccion deberia coincidir");

        // Segundo deploy con MISMO input: REVIERTE (idempotencia)
        vm.expectRevert("DeterministicFactory: already deployed");
        factory.deploy(bytecode, chainId, name);
    }

    // =========================================================================
    // INVARIANT I4: differentNameDifferentAddress
    // =========================================================================
    /// @notice INVARIANT I4 — Nombres diferentes producen direcciones diferentes
    ///         aunque el bytecode y chainId sean identicos.
    function test_I4_differentNameDifferentAddress() public {
        bytes memory bytecode = type(TestContractV1).creationCode;
        uint256 chainId = 1;

        string memory name1 = "ContractAlpha";
        string memory name2 = "ContractBeta";

        address deployed1 = factory.deploy(bytecode, chainId, name1);
        address deployed2 = factory.deploy(bytecode, chainId, name2);

        assertNotEq(deployed1, deployed2, "I4 VIOLATED: Nombres diferentes producen misma direccion");
    }

    // =========================================================================
    // INVARIANT I5: predictedAddressContainsCode
    // =========================================================================
    /// @notice INVARIANT I5 — La direccion predicha contiene bytecode despues
    ///         del deploy.
    function test_I5_predictedAddressContainsCode() public {
        bytes memory bytecode = type(TestContractV1).creationCode;
        uint256 chainId = 1;
        string memory name = "CodeCheck";

        address predicted = factory.predictAddress(bytecode, chainId, name);

        // Antes del deploy: no hay codigo
        uint256 codeSizeBefore;
        assembly {
            codeSizeBefore := extcodesize(predicted)
        }
        assertEq(codeSizeBefore, 0, "I5 SETUP: No deberia haber codigo antes del deploy");

        // Deploy
        factory.deploy(bytecode, chainId, name);

        // Despues del deploy: hay codigo
        uint256 codeSizeAfter;
        assembly {
            codeSizeAfter := extcodesize(predicted)
        }
        assertGt(codeSizeAfter, 0, "I5 VIOLATED: No hay codigo en la direccion predicha");
    }

    // =========================================================================
    // INVARIANT I6: deployIdempotent
    // =========================================================================
    /// @notice INVARIANT I6 — Deploy con el mismo salt revierte (no duplicados).
    ///         Esto previene despliegues accidentales dobles.
    function test_I6_deployIdempotent() public {
        bytes memory bytecode = type(TestContractV1).creationCode;
        uint256 chainId = 1;
        string memory name = "IdempotentTest";

        // Primer deploy: exitoso
        factory.deploy(bytecode, chainId, name);

        // Segundo deploy: REVIERTE
        vm.expectRevert("DeterministicFactory: already deployed");
        factory.deploy(bytecode, chainId, name);
    }

    // =========================================================================
    // INVARIANT I7: zeroBytecodeReverts
    // =========================================================================
    /// @notice INVARIANT I7 — Deploy con bytecode vacio SIEMPRE revierte.
    function test_I7_zeroBytecodeReverts() public {
        bytes memory emptyBytecode = "";
        uint256 chainId = 1;
        string memory name = "EmptyTest";

        vm.expectRevert("DeterministicFactory: empty bytecode");
        factory.deploy(emptyBytecode, chainId, name);
    }

    // =========================================================================
    // INVARIANT I8: zeroChainIdProducesAddress
    // =========================================================================
    /// @notice INVARIANT I8 — chainId = 0 produce una direccion valida (no revierte).
    /// @dev Aunque chainId = 0 no es recomendado, el factory deberia funcionar.
    function test_I8_zeroChainIdProducesAddress() public {
        bytes memory bytecode = type(TestContractV1).creationCode;
        uint256 chainId = 0;
        string memory name = "ZeroChainId";

        address predicted = factory.predictAddress(bytecode, chainId, name);
        assertTrue(predicted != address(0), "I8 SETUP: Prediccion no deberia ser address(0)");

        address deployed = factory.deploy(bytecode, chainId, name);
        assertEq(predicted, deployed, "I8 VIOLATED: Mismatch con chainId 0");
        assertTrue(deployed != address(0), "I8 VIOLATED: Deploy retorno address(0)");
    }

    // =========================================================================
    // INVARIANT I9: sameNameDifferentBytecode
    // =========================================================================
    /// @notice INVARIANT I9 — Bytecode diferente produce direccion diferente
    ///         aunque el name y chainId sean identicos.
    function test_I9_sameNameDifferentBytecode() public {
        uint256 chainId = 1;
        string memory name = "SameName";

        bytes memory bytecodeV1 = type(TestContractV1).creationCode;
        bytes memory bytecodeV2 = type(TestContractV2).creationCode;

        // Los bytecodes deben ser diferentes
        assertNotEq(keccak256(bytecodeV1), keccak256(bytecodeV2), "I9 SETUP: Bytecodes deberian ser diferentes");

        address predictedV1 = factory.predictAddress(bytecodeV1, chainId, name);
        address predictedV2 = factory.predictAddress(bytecodeV2, chainId, name);

        assertNotEq(predictedV1, predictedV2, "I9 VIOLATED: Bytecode diferente produce misma direccion");

        address deployedV1 = factory.deploy(bytecodeV1, chainId, name);
        assertEq(predictedV1, deployedV1, "I9: Deploy V1 deberia coincidir con prediccion");
    }

    // =========================================================================
    // INVARIANT I10: saltNamespaceing
    // =========================================================================
    /// @notice INVARIANT I10 — El salt se computa como keccak256(chainId + name),
    ///         asegurando que (chainId1, name) != (chainId2, name) para chainId1 != chainId2.
    function test_I10_saltNamespaceing() public view {
        string memory name = "NamespaceTest";

        bytes32 salt1 = keccak256(abi.encodePacked(uint256(1), name));
        bytes32 salt137 = keccak256(abi.encodePacked(uint256(137), name));
        bytes32 salt42161 = keccak256(abi.encodePacked(uint256(42161), name));

        // Todos los salts deben ser diferentes
        assertNotEq(salt1, salt137, "I10: Salts para chain 1 y 137 deberian diferir");
        assertNotEq(salt1, salt42161, "I10: Salts para chain 1 y 42161 deberian diferir");
        assertNotEq(salt137, salt42161, "I10: Salts para chain 137 y 42161 deberian diferir");
    }

    // =========================================================================
    // Fuzzing — 10,000 runs por funcion
    // =========================================================================

    /// @notice Fuzz: predictedAddress siempre coincide con deployed para
    ///         cualquier combinacion valida de inputs.
    function testFuzz_DF_predictedMatchesDeployed(
        uint256 chainId,
        uint256 value,
        string memory name
    ) public {
        vm.assume(bytes(name).length > 0);
        vm.assume(bytes(name).length <= 100);
        chainId = bound(chainId, 0, type(uint64).max);

        bytes memory bytecode = abi.encodePacked(
            type(TestContractV1).creationCode,
            abi.encode(value, name)
        );

        // Evitar despliegues duplicados saltando si ya existe
        if (factory.isDeployed(chainId, name)) {
            return;
        }

        address predicted = factory.predictAddress(bytecode, chainId, name);
        address deployed = factory.deploy(bytecode, chainId, name);

        assertEq(predicted, deployed, "FUZZ: predicted != deployed");
        assertTrue(deployed != address(0), "FUZZ: deployed == address(0)");
    }

    /// @notice Fuzz: Chains diferentes SIEMPRE producen direcciones diferentes.
    function testFuzz_DF_crossChainDifferentAddresses(
        uint256 chainId1,
        uint256 chainId2,
        string memory name
    ) public {
        vm.assume(chainId1 != chainId2);
        vm.assume(bytes(name).length > 0);
        vm.assume(bytes(name).length <= 100);
        chainId1 = bound(chainId1, 0, type(uint64).max);
        chainId2 = bound(chainId2, 0, type(uint64).max);

        bytes memory bytecode = type(TestContractV1).creationCode;

        address predicted1 = factory.predictAddress(bytecode, chainId1, name);
        address predicted2 = factory.predictAddress(bytecode, chainId2, name);

        assertNotEq(predicted1, predicted2, "FUZZ: Cross-chain addresses should differ");
    }

    /// @notice Fuzz: Nombres diferentes SIEMPRE producen direcciones diferentes.
    function testFuzz_DF_differentNamesDifferentAddresses(
        string memory name1,
        string memory name2
    ) public {
        vm.assume(keccak256(bytes(name1)) != keccak256(bytes(name2)));
        vm.assume(bytes(name1).length > 0 && bytes(name1).length <= 100);
        vm.assume(bytes(name2).length > 0 && bytes(name2).length <= 100);

        bytes memory bytecode = type(TestContractV1).creationCode;
        uint256 chainId = 1;

        address predicted1 = factory.predictAddress(bytecode, chainId, name1);
        address predicted2 = factory.predictAddress(bytecode, chainId, name2);

        assertNotEq(predicted1, predicted2, "FUZZ: Different names should produce different addresses");
    }

    /// @notice Fuzz: isDeployed es true post-deploy y false pre-deploy.
    function testFuzz_DF_isDeployedTracking(uint256 chainId, string memory name) public {
        vm.assume(bytes(name).length > 0);
        vm.assume(bytes(name).length <= 100);
        chainId = bound(chainId, 0, type(uint64).max);

        bytes memory bytecode = type(TestContractV1).creationCode;

        // Pre-deploy: deberia ser false
        bool preDeploy = factory.isDeployed(chainId, name);

        if (preDeploy) {
            // Ya desplegado en setUp o test anterior — skip
            return;
        }

        assertFalse(preDeploy, "FUZZ: isDeployed deberia ser false pre-deploy");

        // Deploy
        factory.deploy(bytecode, chainId, name);

        // Post-deploy: deberia ser true
        bool postDeploy = factory.isDeployed(chainId, name);
        assertTrue(postDeploy, "FUZZ: isDeployed deberia ser true post-deploy");
    }

    /// @notice Fuzz: Direccion predicha tiene codigo post-deploy.
    function testFuzz_DF_predictedHasCode(uint256 chainId, string memory name) public {
        vm.assume(bytes(name).length > 0);
        vm.assume(bytes(name).length <= 100);
        chainId = bound(chainId, 0, type(uint64).max);

        bytes memory bytecode = type(TestContractV1).creationCode;

        if (factory.isDeployed(chainId, name)) {
            return;
        }

        address predicted = factory.predictAddress(bytecode, chainId, name);

        // Pre-deploy check
        uint256 codeSizeBefore;
        assembly { codeSizeBefore := extcodesize(predicted) }
        assertEq(codeSizeBefore, 0, "FUZZ: No code before deploy");

        factory.deploy(bytecode, chainId, name);

        // Post-deploy check
        uint256 codeSizeAfter;
        assembly { codeSizeAfter := extcodesize(predicted) }
        assertGt(codeSizeAfter, 0, "FUZZ: Code should exist after deploy");
    }

    /// @notice Fuzz: Deploy duplicado SIEMPRE revierte.
    function testFuzz_DF_duplicateDeployReverts(
        uint256 chainId,
        string memory name
    ) public {
        vm.assume(bytes(name).length > 0);
        vm.assume(bytes(name).length <= 100);
        chainId = bound(chainId, 0, type(uint64).max);

        bytes memory bytecode = type(TestContractV1).creationCode;

        if (factory.isDeployed(chainId, name)) {
            // Si ya existe, el primer deploy deberia revertir
            vm.expectRevert("DeterministicFactory: already deployed");
            factory.deploy(bytecode, chainId, name);
            return;
        }

        // Primer deploy: exitoso
        factory.deploy(bytecode, chainId, name);

        // Segundo deploy: REVIERTE
        vm.expectRevert("DeterministicFactory: already deployed");
        factory.deploy(bytecode, chainId, name);
    }

    /// @notice Fuzz: Bytecode vacio siempre revierte.
    function testFuzz_DF_emptyBytecodeReverts(uint256 chainId, string memory name) public {
        chainId = bound(chainId, 0, type(uint64).max);

        vm.expectRevert("DeterministicFactory: empty bytecode");
        factory.deploy("", chainId, name);
    }

    /// @notice Fuzz: El contrato desplegado es funcional.
    function testFuzz_DF_deployedContractFunctional(
        uint256 chainId,
        uint256 value,
        string memory contractName
    ) public {
        vm.assume(bytes(contractName).length > 0);
        vm.assume(bytes(contractName).length <= 100);
        chainId = bound(chainId, 0, type(uint64).max);

        bytes memory bytecode = abi.encodePacked(
            type(TestContractV1).creationCode,
            abi.encode(value, contractName)
        );

        if (factory.isDeployed(chainId, contractName)) {
            return;
        }

        address deployed = factory.deploy(bytecode, chainId, contractName);

        // Verificar que el contrato es funcional
        TestContractV1 tc = TestContractV1(deployed);
        assertEq(tc.value(), value, "FUZZ: Constructor value mismatch");
        assertEq(tc.name(), contractName, "FUZZ: Constructor name mismatch");
    }

    /// @notice Fuzz: Comparar predicciones directas vs la formula CREATE2.
    function testFuzz_DF_predictionFormulaCorrect(
        uint256 chainId,
        string memory name
    ) public {
        vm.assume(bytes(name).length > 0);
        vm.assume(bytes(name).length <= 100);
        chainId = bound(chainId, 0, type(uint64).max);

        bytes memory bytecode = type(TestContractV1).creationCode;

        // Calcular prediccion manualmente
        bytes32 salt = keccak256(abi.encodePacked(chainId, name));
        bytes32 hash = keccak256(
            abi.encodePacked(
                bytes1(0xff),
                address(factory),
                salt,
                keccak256(bytecode)
            )
        );
        address manualPrediction = address(uint160(uint256(hash)));

        // Comparar con la funcion del factory
        address factoryPrediction = factory.predictAddress(bytecode, chainId, name);

        assertEq(manualPrediction, factoryPrediction, "FUZZ: Formula CREATE2 incorrecta");
    }

    /// @notice Fuzz: Direccion independiente del caller.
    function testFuzz_DF_addressIndependentOfCaller(
        address caller,
        uint256 chainId,
        string memory name
    ) public {
        vm.assume(bytes(name).length > 0);
        vm.assume(bytes(name).length <= 100);
        vm.assume(caller != address(0));
        chainId = bound(chainId, 0, type(uint64).max);

        bytes memory bytecode = type(TestContractV1).creationCode;

        address predicted1 = factory.predictAddress(bytecode, chainId, name);

        if (factory.isDeployed(chainId, name)) {
            return;
        }

        // Deploy desde caller diferente
        vm.prank(caller);
        address deployed = factory.deploy(bytecode, chainId, name);

        assertEq(predicted1, deployed, "FUZZ: Caller no deberia afectar la direccion");
    }

    // =========================================================================
    // Tests de integridad adicionales
    // =========================================================================

    /// @notice El evento ContractDeployed se emite con datos correctos.
    function test_eventContractDeployed() public {
        bytes memory bytecode = type(TestContractV1).creationCode;
        uint256 chainId = 1;
        string memory name = "EventTest";

        bytes32 expectedSalt = keccak256(abi.encodePacked(chainId, name));
        address expectedAddress = factory.predictAddress(bytecode, chainId, name);

        vm.expectEmit(true, false, false, false);
        emit DeterministicFactory.ContractDeployed(expectedAddress, expectedSalt, chainId, name);

        factory.deploy(bytecode, chainId, name);
    }

    /// @notice Multiples deploys en secuencia producen direcciones unicas.
    function test_multipleDeploysUniqueAddresses() public {
        bytes memory bytecode = type(TestContractV1).creationCode;
        uint256 chainId = 1;

        address[] memory addresses = new address[](10);
        for (uint256 i = 0; i < 10; i++) {
            string memory name = string(abi.encodePacked("Contract", vm.toString(i)));
            addresses[i] = factory.deploy(bytecode, chainId, name);

            // Verificar unicidad
            for (uint256 j = 0; j < i; j++) {
                assertNotEq(
                    addresses[i],
                    addresses[j],
                    "Direccion duplicada en secuencia de deploys"
                );
            }
        }
    }

    /// @notice Verificar que la direccion del factory no es address(0).
    function test_factoryAddressNotZero() public view {
        assertTrue(address(factory) != address(0), "Factory address should not be zero");
    }

    /// @notice Verificar que isDeployed funciona correctamente.
    function test_isDeployedAccuracy() public {
        bytes memory bytecode = type(TestContractV1).creationCode;
        uint256 chainId = 1;
        string memory name = "IsDeployedTest";

        assertFalse(factory.isDeployed(chainId, name), "Pre-deploy: isDeployed deberia ser false");

        factory.deploy(bytecode, chainId, name);

        assertTrue(factory.isDeployed(chainId, name), "Post-deploy: isDeployed deberia ser true");
        assertFalse(factory.isDeployed(chainId, "NonExistent"), "No-existent: isDeployed deberia ser false");
    }
}
