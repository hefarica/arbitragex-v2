# DIAGNOSIS: lint-and-test-contracts

- **Check:** `lint-and-test-contracts`
- **Run ID:** `26045625344`
- **Job ID:** `76568903091`
- **Step:** `Run forge test`
- **Comando Fallido:** `forge test`
- **Error Exacto:** `ParserError: Source "lib/openzeppelin-contracts/contracts/token/ERC20/ERC20.sol" not found`
- **Archivo Afectado:** `contracts/test/FlashLoanExecutor.t.sol` etc.
- **Línea:** Múltiples (6, 7, 24, 27, 28)
- **Causa Raíz:** Las dependencias de Forge no existen en el repositorio porque los submódulos Git no fueron definidos o añadidos (`.gitmodules` inexistente).
- **Fix Mínimo:** Añadir los submódulos de `openzeppelin-contracts`, `openzeppelin-contracts-upgradeable` y `forge-std` a `.gitmodules` para que `actions/checkout@v4` (con `submodules: recursive`) pueda descargar las librerías.
- **Validación Local:** `git submodule add` para verificar inicialización.
- **Riesgo:** Bajo. Solo afecta la suite de pruebas locales y CI de contratos.
- **Archivos Permitidos para Tocar:** `.gitmodules`, `contracts/lib/`
