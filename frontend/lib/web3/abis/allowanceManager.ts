export const allowanceManagerAbi = [
  {
    type: "function",
    name: "batchGrantAllowance",
    inputs: [
      { name: "tokens", type: "address[]", internalType: "address[]" },
      { name: "spenders", type: "address[]", internalType: "address[]" },
      { name: "amounts", type: "uint256[]", internalType: "uint256[]" },
    ],
    outputs: [],
    stateMutability: "nonpayable",
  },
  {
    type: "function",
    name: "batchRevokeAllowance",
    inputs: [
      { name: "tokens", type: "address[]", internalType: "address[]" },
      { name: "spenders", type: "address[]", internalType: "address[]" },
    ],
    outputs: [],
    stateMutability: "nonpayable",
  },
] as const;
