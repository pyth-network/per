/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/express_relay.json`.
 */
export type ExpressRelay = {
  address: "PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou";
  metadata: {
    name: "expressRelay";
    version: "0.3.1";
    spec: "0.1.0";
    description: "Pyth Express Relay program for handling permissioning and bid distribution";
    repository: "https://github.com/pyth-network/per";
  };
  instructions: [
    {
      name: "checkPermission";
      docs: [
        "Checks if permissioning exists for a particular (permission, router) pair within the same transaction.",
        "Permissioning takes the form of a SubmitBid instruction with matching permission and router accounts.",
        "Returns the fees paid to the router in the matching instructions."
      ];
      discriminator: [154, 199, 232, 242, 96, 72, 197, 236];
      accounts: [
        {
          name: "sysvarInstructions";
          address: "Sysvar1nstructions1111111111111111111111111";
        },
        {
          name: "permission";
        },
        {
          name: "router";
        },
        {
          name: "configRouter";
          pda: {
            seeds: [
              {
                kind: "const";
                value: [
                  99,
                  111,
                  110,
                  102,
                  105,
                  103,
                  95,
                  114,
                  111,
                  117,
                  116,
                  101,
                  114
                ];
              },
              {
                kind: "account";
                path: "router";
              }
            ];
          };
        },
        {
          name: "expressRelayMetadata";
          pda: {
            seeds: [
              {
                kind: "const";
                value: [109, 101, 116, 97, 100, 97, 116, 97];
              }
            ];
          };
        }
      ];
      args: [];
      returns: "u64";
    },
    {
      name: "initialize";
      discriminator: [175, 175, 109, 31, 13, 152, 155, 237];
      accounts: [
        {
          name: "payer";
          writable: true;
          signer: true;
        },
        {
          name: "expressRelayMetadata";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [109, 101, 116, 97, 100, 97, 116, 97];
              }
            ];
          };
        },
        {
          name: "admin";
        },
        {
          name: "relayerSigner";
        },
        {
          name: "feeReceiverRelayer";
        },
        {
          name: "systemProgram";
          address: "11111111111111111111111111111111";
        }
      ];
      args: [
        {
          name: "data";
          type: {
            defined: {
              name: "initializeArgs";
            };
          };
        }
      ];
    },
    {
      name: "setAdmin";
      discriminator: [251, 163, 0, 52, 91, 194, 187, 92];
      accounts: [
        {
          name: "admin";
          writable: true;
          signer: true;
          relations: ["expressRelayMetadata"];
        },
        {
          name: "expressRelayMetadata";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [109, 101, 116, 97, 100, 97, 116, 97];
              }
            ];
          };
        },
        {
          name: "adminNew";
        }
      ];
      args: [];
    },
    {
      name: "setRelayer";
      discriminator: [23, 243, 33, 88, 110, 84, 196, 37];
      accounts: [
        {
          name: "admin";
          writable: true;
          signer: true;
          relations: ["expressRelayMetadata"];
        },
        {
          name: "expressRelayMetadata";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [109, 101, 116, 97, 100, 97, 116, 97];
              }
            ];
          };
        },
        {
          name: "relayerSigner";
        },
        {
          name: "feeReceiverRelayer";
        }
      ];
      args: [];
    },
    {
      name: "setRouterSplit";
      discriminator: [16, 150, 106, 13, 27, 191, 104, 8];
      accounts: [
        {
          name: "admin";
          writable: true;
          signer: true;
          relations: ["expressRelayMetadata"];
        },
        {
          name: "configRouter";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [
                  99,
                  111,
                  110,
                  102,
                  105,
                  103,
                  95,
                  114,
                  111,
                  117,
                  116,
                  101,
                  114
                ];
              },
              {
                kind: "account";
                path: "router";
              }
            ];
          };
        },
        {
          name: "expressRelayMetadata";
          pda: {
            seeds: [
              {
                kind: "const";
                value: [109, 101, 116, 97, 100, 97, 116, 97];
              }
            ];
          };
        },
        {
          name: "router";
        },
        {
          name: "systemProgram";
          address: "11111111111111111111111111111111";
        }
      ];
      args: [
        {
          name: "data";
          type: {
            defined: {
              name: "setRouterSplitArgs";
            };
          };
        }
      ];
    },
    {
      name: "setSplits";
      discriminator: [175, 2, 86, 49, 225, 202, 232, 189];
      accounts: [
        {
          name: "admin";
          writable: true;
          signer: true;
          relations: ["expressRelayMetadata"];
        },
        {
          name: "expressRelayMetadata";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [109, 101, 116, 97, 100, 97, 116, 97];
              }
            ];
          };
        }
      ];
      args: [
        {
          name: "data";
          type: {
            defined: {
              name: "setSplitsArgs";
            };
          };
        }
      ];
    },
    {
      name: "submitBid";
      docs: [
        "Submits a bid for a particular (permission, router) pair and distributes bids according to splits."
      ];
      discriminator: [19, 164, 237, 254, 64, 139, 237, 93];
      accounts: [
        {
          name: "searcher";
          writable: true;
          signer: true;
        },
        {
          name: "relayerSigner";
          signer: true;
          relations: ["expressRelayMetadata"];
        },
        {
          name: "permission";
        },
        {
          name: "router";
          writable: true;
        },
        {
          name: "configRouter";
          pda: {
            seeds: [
              {
                kind: "const";
                value: [
                  99,
                  111,
                  110,
                  102,
                  105,
                  103,
                  95,
                  114,
                  111,
                  117,
                  116,
                  101,
                  114
                ];
              },
              {
                kind: "account";
                path: "router";
              }
            ];
          };
        },
        {
          name: "expressRelayMetadata";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [109, 101, 116, 97, 100, 97, 116, 97];
              }
            ];
          };
        },
        {
          name: "feeReceiverRelayer";
          writable: true;
          relations: ["expressRelayMetadata"];
        },
        {
          name: "systemProgram";
          address: "11111111111111111111111111111111";
        },
        {
          name: "sysvarInstructions";
          address: "Sysvar1nstructions1111111111111111111111111";
        }
      ];
      args: [
        {
          name: "data";
          type: {
            defined: {
              name: "submitBidArgs";
            };
          };
        }
      ];
    },
    {
      name: "swap";
      discriminator: [248, 198, 158, 145, 225, 117, 135, 200];
      accounts: [
        {
          name: "searcher";
          docs: [
            "Searcher is the party that sends the input token and receives the output token"
          ];
          signer: true;
        },
        {
          name: "trader";
          docs: [
            "Trader is the party that sends the output token and receives the input token"
          ];
          signer: true;
        },
        {
          name: "searcherInputTa";
          writable: true;
        },
        {
          name: "searcherOutputTa";
          writable: true;
        },
        {
          name: "traderInputAta";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "account";
                path: "trader";
              },
              {
                kind: "account";
                path: "tokenProgramInput";
              },
              {
                kind: "account";
                path: "mintInput";
              }
            ];
            program: {
              kind: "const";
              value: [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ];
            };
          };
        },
        {
          name: "traderOutputAta";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "account";
                path: "trader";
              },
              {
                kind: "account";
                path: "tokenProgramOutput";
              },
              {
                kind: "account";
                path: "mintOutput";
              }
            ];
            program: {
              kind: "const";
              value: [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ];
            };
          };
        },
        {
          name: "routerFeeReceiverTa";
          docs: [
            "Router fee receiver token account: the referrer can provide an arbitrary receiver for the router fee"
          ];
          writable: true;
        },
        {
          name: "relayerFeeReceiverAta";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "account";
                path: "express_relay_metadata.fee_receiver_relayer";
                account: "expressRelayMetadata";
              },
              {
                kind: "account";
                path: "tokenProgramFee";
              },
              {
                kind: "account";
                path: "mintFee";
              }
            ];
            program: {
              kind: "const";
              value: [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ];
            };
          };
        },
        {
          name: "expressRelayFeeReceiverAta";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "account";
                path: "expressRelayMetadata";
              },
              {
                kind: "account";
                path: "tokenProgramFee";
              },
              {
                kind: "account";
                path: "mintFee";
              }
            ];
            program: {
              kind: "const";
              value: [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ];
            };
          };
        },
        {
          name: "mintInput";
        },
        {
          name: "mintOutput";
        },
        {
          name: "mintFee";
        },
        {
          name: "tokenProgramInput";
        },
        {
          name: "tokenProgramOutput";
        },
        {
          name: "tokenProgramFee";
        },
        {
          name: "expressRelayMetadata";
          docs: ["Express relay configuration"];
          pda: {
            seeds: [
              {
                kind: "const";
                value: [109, 101, 116, 97, 100, 97, 116, 97];
              }
            ];
          };
        }
      ];
      args: [
        {
          name: "data";
          type: {
            defined: {
              name: "swapArgs";
            };
          };
        }
      ];
    },
    {
      name: "withdrawFees";
      discriminator: [198, 212, 171, 109, 144, 215, 174, 89];
      accounts: [
        {
          name: "admin";
          writable: true;
          signer: true;
          relations: ["expressRelayMetadata"];
        },
        {
          name: "feeReceiverAdmin";
          writable: true;
        },
        {
          name: "expressRelayMetadata";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [109, 101, 116, 97, 100, 97, 116, 97];
              }
            ];
          };
        }
      ];
      args: [];
    }
  ];
  accounts: [
    {
      name: "configRouter";
      discriminator: [135, 66, 240, 166, 94, 198, 187, 36];
    },
    {
      name: "expressRelayMetadata";
      discriminator: [204, 75, 133, 7, 175, 241, 130, 11];
    }
  ];
  errors: [
    {
      code: 6000;
      name: "feeSplitLargerThanPrecision";
      msg: "Fee split(s) larger than fee precision";
    },
    {
      code: 6001;
      name: "feesHigherThanBid";
      msg: "Fees higher than bid";
    },
    {
      code: 6002;
      name: "deadlinePassed";
      msg: "Deadline passed";
    },
    {
      code: 6003;
      name: "invalidCpiSubmitBid";
      msg: "Invalid CPI into submit bid instruction";
    },
    {
      code: 6004;
      name: "missingPermission";
      msg: "Missing permission";
    },
    {
      code: 6005;
      name: "multiplePermissions";
      msg: "Multiple permissions";
    },
    {
      code: 6006;
      name: "insufficientSearcherFunds";
      msg: "Insufficient searcher funds";
    },
    {
      code: 6007;
      name: "insufficientRent";
      msg: "Insufficient funds for rent";
    },
    {
      code: 6008;
      name: "invalidAta";
      msg: "Invalid ATA provided";
    },
    {
      code: 6009;
      name: "invalidMint";
      msg: "A token account has the wrong mint";
    },
    {
      code: 6010;
      name: "invalidTokenProgram";
      msg: "A token account belongs to the wrong token program";
    }
  ];
  types: [
    {
      name: "configRouter";
      type: {
        kind: "struct";
        fields: [
          {
            name: "router";
            type: "pubkey";
          },
          {
            name: "split";
            type: "u64";
          }
        ];
      };
    },
    {
      name: "expressRelayMetadata";
      type: {
        kind: "struct";
        fields: [
          {
            name: "admin";
            type: "pubkey";
          },
          {
            name: "relayerSigner";
            type: "pubkey";
          },
          {
            name: "feeReceiverRelayer";
            type: "pubkey";
          },
          {
            name: "splitRouterDefault";
            type: "u64";
          },
          {
            name: "splitRelayer";
            type: "u64";
          },
          {
            name: "swapPlatformFeeBps";
            type: "u64";
          }
        ];
      };
    },
    {
      name: "feeToken";
      type: {
        kind: "enum";
        variants: [
          {
            name: "input";
          },
          {
            name: "output";
          }
        ];
      };
    },
    {
      name: "initializeArgs";
      type: {
        kind: "struct";
        fields: [
          {
            name: "splitRouterDefault";
            type: "u64";
          },
          {
            name: "splitRelayer";
            type: "u64";
          }
        ];
      };
    },
    {
      name: "setRouterSplitArgs";
      type: {
        kind: "struct";
        fields: [
          {
            name: "splitRouter";
            type: "u64";
          }
        ];
      };
    },
    {
      name: "setSplitsArgs";
      type: {
        kind: "struct";
        fields: [
          {
            name: "splitRouterDefault";
            type: "u64";
          },
          {
            name: "splitRelayer";
            type: "u64";
          }
        ];
      };
    },
    {
      name: "submitBidArgs";
      type: {
        kind: "struct";
        fields: [
          {
            name: "deadline";
            type: "i64";
          },
          {
            name: "bidAmount";
            type: "u64";
          }
        ];
      };
    },
    {
      name: "swapArgs";
      docs: [
        "For all swap instructions and contexts, input and output are defined with respect to the searcher",
        "So `mint_input` refers to the token that the searcher provides to the trader and",
        "`mint_output` refers to the token that the searcher receives from the trader",
        "This choice is made to minimize confusion for the searchers, who are more likely to parse the program"
      ];
      type: {
        kind: "struct";
        fields: [
          {
            name: "amountInput";
            type: "u64";
          },
          {
            name: "amountOutput";
            type: "u64";
          },
          {
            name: "referralFeeBps";
            type: "u64";
          },
          {
            name: "feeToken";
            type: {
              defined: {
                name: "feeToken";
              };
            };
          }
        ];
      };
    }
  ];
};
