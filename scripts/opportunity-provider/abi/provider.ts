export const abi = [
  {
    type: "constructor",
    inputs: [
      {
        name: "admin",
        type: "address",
        internalType: "address",
      },
      {
        name: "expressRelay",
        type: "address",
        internalType: "address",
      },
      {
        name: "permit2",
        type: "address",
        internalType: "address",
      },
    ],
    stateMutability: "nonpayable",
  },
  {
    type: "function",
    name: "OPPORTUNITY_PROVIDER_WITNESS_TYPE_STRING",
    inputs: [],
    outputs: [
      {
        name: "",
        type: "string",
        internalType: "string",
      },
    ],
    stateMutability: "view",
  },
  {
    type: "function",
    name: "_OPPORTUNITY_PROVIDER_WITNESS_TYPE",
    inputs: [],
    outputs: [
      {
        name: "",
        type: "string",
        internalType: "string",
      },
    ],
    stateMutability: "view",
  },
  {
    type: "function",
    name: "_TOKEN_AMOUNT_TYPE",
    inputs: [],
    outputs: [
      {
        name: "",
        type: "string",
        internalType: "string",
      },
    ],
    stateMutability: "view",
  },
  {
    type: "function",
    name: "execute",
    inputs: [
      {
        name: "params",
        type: "tuple",
        internalType: "struct ExecutionParams",
        components: [
          {
            name: "permit",
            type: "tuple",
            internalType: "struct ISignatureTransfer.PermitBatchTransferFrom",
            components: [
              {
                name: "permitted",
                type: "tuple[]",
                internalType: "struct ISignatureTransfer.TokenPermissions[]",
                components: [
                  {
                    name: "token",
                    type: "address",
                    internalType: "address",
                  },
                  {
                    name: "amount",
                    type: "uint256",
                    internalType: "uint256",
                  },
                ],
              },
              {
                name: "nonce",
                type: "uint256",
                internalType: "uint256",
              },
              {
                name: "deadline",
                type: "uint256",
                internalType: "uint256",
              },
            ],
          },
          {
            name: "witness",
            type: "tuple",
            internalType: "struct ExecutionWitness",
            components: [
              {
                name: "buyTokens",
                type: "tuple[]",
                internalType: "struct TokenAmount[]",
                components: [
                  {
                    name: "token",
                    type: "address",
                    internalType: "address",
                  },
                  {
                    name: "amount",
                    type: "uint256",
                    internalType: "uint256",
                  },
                ],
              },
              {
                name: "owner",
                type: "address",
                internalType: "address",
              },
            ],
          },
        ],
      },
      {
        name: "signature",
        type: "bytes",
        internalType: "bytes",
      },
    ],
    outputs: [],
    stateMutability: "nonpayable",
  },
  {
    type: "function",
    name: "hash",
    inputs: [
      {
        name: "params",
        type: "tuple",
        internalType: "struct ExecutionWitness",
        components: [
          {
            name: "buyTokens",
            type: "tuple[]",
            internalType: "struct TokenAmount[]",
            components: [
              {
                name: "token",
                type: "address",
                internalType: "address",
              },
              {
                name: "amount",
                type: "uint256",
                internalType: "uint256",
              },
            ],
          },
          {
            name: "owner",
            type: "address",
            internalType: "address",
          },
        ],
      },
    ],
    outputs: [
      {
        name: "",
        type: "bytes32",
        internalType: "bytes32",
      },
    ],
    stateMutability: "pure",
  },
  {
    type: "error",
    name: "AddressEmptyCode",
    inputs: [
      {
        name: "target",
        type: "address",
        internalType: "address",
      },
    ],
  },
  {
    type: "error",
    name: "AddressInsufficientBalance",
    inputs: [
      {
        name: "account",
        type: "address",
        internalType: "address",
      },
    ],
  },
  {
    type: "error",
    name: "DuplicateToken",
    inputs: [],
  },
  {
    type: "error",
    name: "FailedInnerCall",
    inputs: [],
  },
  {
    type: "error",
    name: "InvalidOpportunity",
    inputs: [],
  },
  {
    type: "error",
    name: "NotCalledByAdmin",
    inputs: [],
  },
  {
    type: "error",
    name: "ReentrancyGuardReentrantCall",
    inputs: [],
  },
  {
    type: "error",
    name: "SafeERC20FailedOperation",
    inputs: [
      {
        name: "token",
        type: "address",
        internalType: "address",
      },
    ],
  },
] as const;
