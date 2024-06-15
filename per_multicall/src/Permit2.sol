// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Structs.sol";
import "./ExpressRelay.sol";
import "./WETH9.sol";
import "forge-std/console.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {IERC1271} from "openzeppelin-contracts/contracts/interfaces/IERC1271.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";

abstract contract Permit2 is ReentrancyGuard {
    using SafeERC20 for IERC20;

    // Cache the domain separator as an immutable value, but also store the chain id that it
    // corresponds to, in order to invalidate the cached domain separator if the chain id changes.
    bytes32 private _CACHED_DOMAIN_SEPARATOR;
    uint256 private _CACHED_CHAIN_ID;

    bytes32 private constant _HASHED_NAME = keccak256("Permit2");
    bytes32 private constant _TYPE_HASH =
        keccak256(
            "EIP712Domain(string name,uint256 chainId,address verifyingContract)"
        );

    bytes32 constant _UPPER_BIT_MASK = (
        0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
    );

    address _admin;
    address _opportunityAdapter;

    mapping(address => mapping(uint256 => uint256)) public nonceBitmap;

    bytes32 private constant _TOKEN_PERMISSIONS_TYPEHASH =
        keccak256("TokenPermissions(address token,uint256 amount)");

    string private constant _PERMIT_BATCH_WITNESS_TRANSFER_FROM_TYPEHASH_STUB =
        "PermitBatchWitnessTransferFrom(TokenPermissions[] permitted,address spender,uint256 nonce,uint256 deadline,";

    /// @notice Returns the domain separator for the current chain.
    /// @dev Uses cached version if chainid and address are unchanged from construction.
    function domainSeparator() public view returns (bytes32) {
        return
            block.chainid == _CACHED_CHAIN_ID
                ? _CACHED_DOMAIN_SEPARATOR
                : _buildDomainSeparator(_TYPE_HASH, _HASHED_NAME);
    }

    /// @notice Builds a domain separator using the current chainId and contract address.
    function _buildDomainSeparator(
        bytes32 typeHash,
        bytes32 nameHash
    ) private view returns (bytes32) {
        return
            keccak256(
                abi.encode(typeHash, nameHash, block.chainid, address(this))
            );
    }

    /// @notice Creates an EIP-712 typed data hash
    function _hashTypedData(bytes32 dataHash) private view returns (bytes32) {
        return
            keccak256(
                abi.encodePacked("\x19\x01", domainSeparator(), dataHash)
            );
    }

    function _verify(
        bytes calldata signature,
        bytes32 hash,
        address claimedSigner
    ) private view {
        bytes32 r;
        bytes32 s;
        uint8 v;

        if (claimedSigner.code.length == 0) {
            if (signature.length == 65) {
                (r, s) = abi.decode(signature, (bytes32, bytes32));
                v = uint8(signature[64]);
            } else if (signature.length == 64) {
                // EIP-2098
                bytes32 vs;
                (r, vs) = abi.decode(signature, (bytes32, bytes32));
                s = vs & _UPPER_BIT_MASK;
                v = uint8(uint256(vs >> 255)) + 27;
            } else {
                revert InvalidSignatureLength();
            }
            address signer = ecrecover(hash, v, r, s);
            if (signer == address(0)) revert InvalidSignature();
            if (signer != claimedSigner) revert InvalidSigner();
        } else {
            bytes4 magicValue = IERC1271(claimedSigner).isValidSignature(
                hash,
                signature
            );
            if (magicValue != IERC1271.isValidSignature.selector)
                revert InvalidContractSignature();
        }
    }

    /**
     * @notice Permit2 initializer - Initializes a new permit2 contract with given parameters
     *
     * @param admin: address of admin of permit2
     * @param opportunityAdapter: address of opportunity adapter
     */
    function _initialize(address admin, address opportunityAdapter) internal {
        _admin = admin;
        _opportunityAdapter = opportunityAdapter;
        _CACHED_CHAIN_ID = block.chainid;
        _CACHED_DOMAIN_SEPARATOR = _buildDomainSeparator(
            _TYPE_HASH,
            _HASHED_NAME
        );
    }

    /**
     * @notice setOpportunityAdapter function - sets the address of the opportunity adapter authenticated for calling this contract
     *
     * @param opportunityAdapter: address of opportunity adapter contract
     */
    function setOpportunityAdapter(address opportunityAdapter) public {
        if (msg.sender != _admin) {
            revert Unauthorized();
        }
        _opportunityAdapter = opportunityAdapter;
    }

    /**
     * @notice getOpportunityAdapter function - returns the address of the opportunity adapter authenticated for calling this contract
     */
    function getOpportunityAdapter() public view returns (address) {
        return _opportunityAdapter;
    }

    function _hashTokenPermissions(
        TokenPermissions memory permitted
    ) private pure returns (bytes32) {
        return keccak256(abi.encode(_TOKEN_PERMISSIONS_TYPEHASH, permitted));
    }

    function _hashWithWitness(
        PermitBatchTransferFrom memory permit,
        bytes32 witness,
        string calldata witnessTypeString
    ) private view returns (bytes32) {
        bytes32 typeHash = keccak256(
            abi.encodePacked(
                _PERMIT_BATCH_WITNESS_TRANSFER_FROM_TYPEHASH_STUB,
                witnessTypeString
            )
        );

        uint256 numPermitted = permit.permitted.length;
        bytes32[] memory tokenPermissionHashes = new bytes32[](numPermitted);

        for (uint256 i = 0; i < numPermitted; ++i) {
            tokenPermissionHashes[i] = _hashTokenPermissions(
                permit.permitted[i]
            );
        }

        return
            keccak256(
                abi.encode(
                    typeHash,
                    keccak256(abi.encodePacked(tokenPermissionHashes)),
                    msg.sender,
                    permit.nonce,
                    permit.deadline,
                    witness
                )
            );
    }

    /// @notice Transfers a token using a signed permit message
    /// @notice Includes extra data provided by the caller to verify signature over
    /// @dev The witness type string must follow EIP712 ordering of nested structs and must include the TokenPermissions type definition
    /// @dev Reverts if the requested amount is greater than the permitted signed amount
    /// @param permit The permit data signed over by the owner
    /// @param owner The owner of the tokens to transfer
    /// @param transferDetails The spender's requested transfer details for the permitted token
    /// @param witness Extra data to include when checking the user signature
    /// @param witnessTypeString The EIP-712 type definition for remaining string stub of the typehash
    /// @param signature The signature to verify
    function permitWitnessTransferFrom(
        PermitBatchTransferFrom memory permit,
        SignatureTransferDetails[] calldata transferDetails,
        address owner,
        bytes32 witness,
        string calldata witnessTypeString,
        bytes calldata signature
    ) external {
        if (msg.sender != _opportunityAdapter) {
            revert Unauthorized();
        }

        _permitTransferFrom(
            permit,
            transferDetails,
            owner,
            _hashWithWitness(permit, witness, witnessTypeString),
            signature
        );
    }

    /// @notice Transfers tokens using a signed permit messages
    /// @dev If to is the zero address, the tokens are sent to the spender
    /// @param permit The permit data signed over by the owner
    /// @param dataHash The EIP-712 hash of permit data to include when checking signature
    /// @param owner The owner of the tokens to transfer
    /// @param signature The signature to verify
    function _permitTransferFrom(
        PermitBatchTransferFrom memory permit,
        SignatureTransferDetails[] calldata transferDetails,
        address owner,
        bytes32 dataHash,
        bytes calldata signature
    ) private {
        uint256 numPermitted = permit.permitted.length;

        if (block.timestamp > permit.deadline)
            revert SignatureExpired(permit.deadline);
        if (numPermitted != transferDetails.length) revert LengthMismatch();

        _useUnorderedNonce(owner, permit.nonce);
        _verify(signature, _hashTypedData(dataHash), owner);

        unchecked {
            for (uint256 i = 0; i < numPermitted; ++i) {
                TokenPermissions memory permitted = permit.permitted[i];
                uint256 requestedAmount = transferDetails[i].requestedAmount;

                if (requestedAmount > permitted.amount)
                    revert InvalidAmount(permitted.amount);

                if (requestedAmount != 0) {
                    // allow spender to specify which of the permitted tokens should be transferred
                    IERC20(permitted.token).safeTransferFrom(
                        owner,
                        transferDetails[i].to,
                        requestedAmount
                    );
                }
            }
        }
    }

    /// @notice Returns the index of the bitmap and the bit position within the bitmap. Used for unordered nonces
    /// @param nonce The nonce to get the associated word and bit positions
    /// @return wordPos The word position or index into the nonceBitmap
    /// @return bitPos The bit position
    /// @dev The first 248 bits of the nonce value is the index of the desired bitmap
    /// @dev The last 8 bits of the nonce value is the position of the bit in the bitmap
    function _bitmapPositions(
        uint256 nonce
    ) private pure returns (uint256 wordPos, uint256 bitPos) {
        wordPos = uint248(nonce >> 8);
        bitPos = uint8(nonce);
    }

    /// @notice Checks whether a nonce is taken and sets the bit at the bit position in the bitmap at the word position
    /// @param from The address to use the nonce at
    /// @param nonce The nonce to spend
    function _useUnorderedNonce(address from, uint256 nonce) internal {
        (uint256 wordPos, uint256 bitPos) = _bitmapPositions(nonce);
        uint256 bit = 1 << bitPos;
        uint256 flipped = nonceBitmap[from][wordPos] ^= bit;

        if (flipped & bit == 0) revert InvalidNonce();
    }
}
