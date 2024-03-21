// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Script, console2} from "forge-std/Script.sol";
import "../src/SigVerify.sol";
import "forge-std/StdJson.sol";
import "forge-std/console.sol";
import "forge-std/StdMath.sol";

import {TokenVault} from "../src/TokenVault.sol";
import {SearcherVault} from "../src/SearcherVault.sol";
import {ExpressRelay} from "../src/ExpressRelay.sol";
import {OpportunityAdapter} from "../src/OpportunityAdapter.sol";
import {MyToken} from "../src/MyToken.sol";
import "../src/Structs.sol";
import "@pythnetwork/pyth-sdk-solidity/MockPyth.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import {WETH9} from "../src/WETH9.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";

import "../src/Errors.sol";
import {OpportunityAdapterUpgradable} from "../src/OpportunityAdapterUpgradable.sol";

contract VaultScript is Script {
    string public latestEnvironmentPath = "latestEnvironment.json";

    function getDeployer() public view returns (address, uint256) {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        if (deployerPrivateKey == 0) {
            revert("PRIVATE_KEY env variable is empty");
        }
        address deployerAddress = vm.addr(deployerPrivateKey);
        return (deployerAddress, deployerPrivateKey);
    }

    function deployWeth() public returns (address) {
        (, uint256 skDeployer) = getDeployer();
        vm.startBroadcast(skDeployer);
        WETH9 weth = new WETH9();
        vm.stopBroadcast();
        console.log("deployed weth contract at", address(weth));
        return address(weth);
    }

    function deployOpportunityAdapter(
        address owner,
        address admin,
        address expressRelay,
        address wethAddress
    ) public returns (address) {
        (, uint256 skDeployer) = getDeployer();
        vm.startBroadcast(skDeployer);
        OpportunityAdapterUpgradable _opportunityAdapter = new OpportunityAdapterUpgradable();
        // deploy proxy contract and point it to implementation
        ERC1967Proxy proxy = new ERC1967Proxy(address(_opportunityAdapter), "");
        // wrap in ABI to support easier calls
        OpportunityAdapterUpgradable opportunityAdapter = OpportunityAdapterUpgradable(
                payable(proxy)
            );
        opportunityAdapter.initialize(owner, admin, expressRelay, wethAddress);
        vm.stopBroadcast();
        return address(opportunityAdapter);
    }

    function upgradeOpportunityAdapter(address proxyAddress) public {
        (, uint256 skDeployer) = getDeployer();
        vm.startBroadcast(skDeployer);
        OpportunityAdapterUpgradable _newImplementation = new OpportunityAdapterUpgradable();
        // Proxy object is technically an OpportunityAdapterUpgradable because it points to an implementation
        // of such contract. Therefore we can call the upgradeTo function on it.
        OpportunityAdapterUpgradable proxy = OpportunityAdapterUpgradable(
            payable(proxyAddress)
        );
        proxy.upgradeTo(address(_newImplementation));
        vm.stopBroadcast();
    }

    function deployExpressRelay() public returns (address) {
        (, uint256 skDeployer) = getDeployer();
        (address operatorAddress, uint256 operatorSk) = makeAddrAndKey(
            "perOperator"
        );
        console.log("pk per operator", operatorAddress);
        console.log("sk per operator", operatorSk);
        uint256 feeSplitProtocolDefault = 50 * (10 ** 16);
        uint256 feeSplitRelayer = 10 * (10 ** 16);
        vm.startBroadcast(skDeployer);
        payable(operatorAddress).transfer(0.01 ether);
        // TODO: set admin to xc-admin
        ExpressRelay multicall = new ExpressRelay(
            operatorAddress,
            operatorAddress,
            feeSplitProtocolDefault,
            feeSplitRelayer
        );
        vm.stopBroadcast();
        console.log("deployed ExpressRelay contract at", address(multicall));
        return address(multicall);
    }

    function deployVault(
        address multicall,
        address oracle,
        bool allowUndercollateralized
    ) public returns (address) {
        // make token vault deployer wallet
        (, uint256 tokenVaultDeployerSk) = makeAddrAndKey("tokenVaultDeployer");
        console.log("sk token vault deployer", tokenVaultDeployerSk);
        vm.startBroadcast(tokenVaultDeployerSk);
        TokenVault vault = new TokenVault(
            multicall,
            oracle,
            allowUndercollateralized
        );
        vm.stopBroadcast();
        console.log("deployed vault contract at", address(vault));
        return address(vault);
    }

    function deployMockPyth() public returns (address) {
        (, uint256 skDeployer) = getDeployer();
        vm.startBroadcast(skDeployer);
        MockPyth mockPyth = new MockPyth(1_000_000_000_000, 0);
        vm.stopBroadcast();
        console.log("deployed mock pyth contract at", address(mockPyth));
        return address(mockPyth);
    }

    function deployAll()
        public
        returns (address, address, address, address, address)
    {
        (address deployer, ) = getDeployer();
        address weth = deployWeth();
        address expressRelay = deployExpressRelay();
        address opportunityAdapter = deployOpportunityAdapter(
            deployer,
            deployer,
            expressRelay,
            weth
        );
        address mockPyth = deployMockPyth();
        address vault = deployVault(expressRelay, mockPyth, false);
        return (expressRelay, opportunityAdapter, mockPyth, vault, weth);
    }

    /**
    @notice Sets up the testnet environment
    deploys WETH, ExpressRelay, OpportunityAdapter, TokenVault along with 5 ERC-20 tokens to use as collateral and debt tokens
    The erc-20 tokens have their actual name as symbol and pyth price feed id as their name. A huge amount of these tokens are minted to the token vault
    @param pyth The address of the already deployed pyth contract to use
    */
    function setupTestnet(
        address pyth,
        address weth,
        bool allowUndercollateralized
    ) public {
        (address deployer, uint256 skDeployer) = getDeployer();
        if (pyth == address(0)) pyth = deployMockPyth();
        if (weth == address(0)) weth = deployWeth();
        address expressRelay = deployExpressRelay();
        address opportunityAdapter = deployOpportunityAdapter(
            deployer,
            deployer,
            expressRelay,
            weth
        );
        address vault = deployVault(
            expressRelay,
            pyth,
            allowUndercollateralized
        );
        address[] memory tokens = new address[](5);
        uint256 lots_of_money = 10 ** 36;
        // Vault simulator assumes the token name is pyth pricefeed id in mainnet
        tokens[0] = address(
            new MyToken(
                "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43",
                "BTC"
            )
        );
        tokens[1] = address(
            new MyToken(
                "eaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a",
                "USDC"
            )
        );
        tokens[2] = address(
            new MyToken(
                "dcef50dd0a4cd2dcc17e45df1676dcb336a11a61c69df7a0299b0150c672d25c",
                "DOGE"
            )
        );
        tokens[3] = address(
            new MyToken(
                "ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d",
                "SOL"
            )
        );
        tokens[4] = address(
            new MyToken(
                "0bbf28e9a841a1cc788f6a361b17ca072d0ea3098a1e5df1c3922d06719579ff",
                "PYTH"
            )
        );

        vm.startBroadcast(skDeployer);
        for (uint i = 0; i < 5; i++) {
            MyToken(tokens[i]).mint(vault, lots_of_money);
        }
        vm.stopBroadcast();

        string memory obj = "";
        vm.serializeAddress(obj, "tokens", tokens);
        vm.serializeAddress(obj, "per", expressRelay);
        vm.serializeAddress(obj, "opportunityAdapter", opportunityAdapter);
        vm.serializeAddress(obj, "oracle", pyth);
        vm.serializeAddress(obj, "tokenVault", vault);
        string memory finalJSON = vm.serializeAddress(obj, "weth", weth);
        vm.writeJson(finalJSON, latestEnvironmentPath);
    }

    /**
    @notice Sets up the localnet environment for testing purposes
    deploys WETH, PER, OpportunityAdapter, MockPyth, TokenVault and 2 ERC-20 tokens to use as collateral and debt tokens
    Also creates and funds searcher wallets and contracts
    */
    function setUpLocalnet() public {
        SearcherVault searcherA;
        SearcherVault searcherB;

        MyToken token1;
        MyToken token2;

        bytes32 idToken1;
        bytes32 idToken2;

        address[] memory addressesScript = new address[](5);
        uint256[] memory sksScript = new uint256[](5);

        uint256[] memory qtys;

        // make searcherA and searcherB wallets
        (addressesScript[0], sksScript[0]) = makeAddrAndKey("searcherA");
        (addressesScript[1], sksScript[1]) = makeAddrAndKey("searcherB");
        console.log("sk searcherA", sksScript[0]);
        console.log("sk searcherB", sksScript[1]);

        // make depositor wallet
        (addressesScript[2], sksScript[2]) = makeAddrAndKey("depositor");
        console.log("sk depositor", sksScript[2]);

        // make perOperator wallet
        (addressesScript[3], sksScript[3]) = makeAddrAndKey("perOperator");

        // make tokenVaultDeployer wallet
        (addressesScript[4], sksScript[4]) = makeAddrAndKey(
            "tokenVaultDeployer"
        );

        // TODO: these are mnemonic wallets. figure out how to transfer ETH from them outside of explicitly hardcoding them here.
        (address pkDeployer, uint256 skDeployer) = getDeployer();

        // transfer ETH to relevant wallets
        vm.startBroadcast(skDeployer);
        console.log("balance of deployer", pkDeployer.balance);
        payable(addressesScript[3]).transfer(10 ether);
        console.log("balance of deployer", pkDeployer.balance);
        payable(addressesScript[0]).transfer(10 ether);
        console.log("balance of deployer", pkDeployer.balance);
        payable(addressesScript[1]).transfer(10 ether);
        console.log("balance of deployer", pkDeployer.balance);
        payable(addressesScript[2]).transfer(10 ether);
        console.log("balance of deployer", pkDeployer.balance);
        payable(addressesScript[4]).transfer(10 ether);
        vm.stopBroadcast();

        // deploy weth, multicall, opportunityAdapter, oracle, tokenVault
        address expressRelay;
        address opportunityAdapter;
        address oracleAddress;
        address tokenVaultAddress;
        address wethAddress;
        (
            expressRelay,
            opportunityAdapter,
            oracleAddress,
            tokenVaultAddress,
            wethAddress
        ) = deployAll();

        // instantiate searcher A's contract with searcher A as sender/origin
        vm.startBroadcast(sksScript[0]);
        console.log("balance of pk searcherA", addressesScript[0].balance);
        searcherA = new SearcherVault(expressRelay, tokenVaultAddress);
        vm.stopBroadcast();
        console.log("contract of searcher A is", address(searcherA));

        // instantiate searcher B's contract with searcher B as sender/origin
        vm.startBroadcast(sksScript[1]);
        console.log("balance of pk searcherB", addressesScript[1].balance);
        searcherB = new SearcherVault(expressRelay, tokenVaultAddress);
        vm.stopBroadcast();
        console.log("contract of searcher B is", address(searcherB));

        // fund the searcher contracts
        vm.startBroadcast(skDeployer);
        console.log("balance of deployer", pkDeployer.balance);
        payable(address(searcherA)).transfer(1 ether);
        payable(address(searcherB)).transfer(1 ether);
        vm.stopBroadcast();

        // instantiate ERC-20 tokens
        vm.startBroadcast(sksScript[3]);
        console.log("balance of pk perOperator", addressesScript[3].balance);

        // create token price feed IDs--see https://pyth.network/developers/price-feed-ids
        // TODO: automate converting bytes32 to string
        idToken1 = 0xff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace; // ETH/USD
        idToken2 = 0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43; // BTC/USD
        string
            memory idToken1Str = "ff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace";
        string
            memory idToken2Str = "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";
        console.log("ids of tokens");
        console.logBytes32(idToken1);
        console.logBytes32(idToken2);

        token1 = new MyToken(idToken1Str, "T_ETH");
        token2 = new MyToken(idToken2Str, "T_BTC");

        console.log("token 1 address", address(token1));
        console.log("token 2 address", address(token2));

        qtys = new uint256[](8); // q_1_dep, q_2_dep, q_1_A, q_2_A, q_1_B, q_2_B, q_1_tokenVault q_2_tokenVault
        qtys[0] = 1_000_000;
        qtys[1] = 1_000_000;
        qtys[2] = 2_000_000;
        qtys[3] = 2_000_000;
        qtys[4] = 3_000_000;
        qtys[5] = 3_000_000;
        qtys[6] = 0;
        qtys[7] = 500_000;

        // mint tokens to the depositor address
        token1.mint(addressesScript[2], qtys[0]);
        token2.mint(addressesScript[2], qtys[1]);

        // mint tokens to searcher A contract
        token1.mint(address(searcherA), qtys[2]);
        token2.mint(address(searcherA), qtys[3]);

        // mint tokens to searcher B contract
        token1.mint(address(searcherB), qtys[4]);
        token2.mint(address(searcherB), qtys[5]);

        // mint token 2 to the vault contract (to allow creation of initial vault with outstanding debt position)
        token2.mint(tokenVaultAddress, qtys[7]);

        // mint token to searchers A and B EOAs
        token1.mint(address(addressesScript[0]), 20_000_000);
        token2.mint(address(addressesScript[0]), 20_000_000);
        token1.mint(address(addressesScript[1]), 30_000_000);
        token2.mint(address(addressesScript[1]), 30_000_000);

        vm.stopBroadcast();

        // searchers A and B approve liquidation adapter to spend their tokens
        vm.startBroadcast(sksScript[0]);
        IERC20(address(token1)).approve(opportunityAdapter, 199_999_999);
        IERC20(address(token2)).approve(opportunityAdapter, 199_999_999);
        // deposit ETH to get WETH
        WETH9(payable(wethAddress)).deposit{value: 1 ether}();
        WETH9(payable(wethAddress)).approve(opportunityAdapter, 399_999_999);
        vm.stopBroadcast();
        vm.startBroadcast(sksScript[1]);
        IERC20(address(token1)).approve(opportunityAdapter, 199_999_999);
        IERC20(address(token2)).approve(opportunityAdapter, 199_999_999);
        WETH9(payable(wethAddress)).deposit{value: 1 ether}();
        WETH9(payable(wethAddress)).approve(opportunityAdapter, 399_999_999);
        vm.stopBroadcast();

        string memory obj = "latestEnvironment";
        vm.serializeAddress(obj, "tokenVault", tokenVaultAddress);
        vm.serializeAddress(obj, "searcherA", address(searcherA));
        vm.serializeAddress(obj, "searcherB", address(searcherB));
        vm.serializeAddress(obj, "expressRelay", expressRelay);
        vm.serializeAddress(obj, "opportunityAdapter", opportunityAdapter);
        vm.serializeAddress(obj, "oracle", oracleAddress);

        vm.serializeAddress(obj, "weth", wethAddress);

        vm.serializeAddress(obj, "token1", address(token1));
        vm.serializeAddress(obj, "token2", address(token2));

        vm.serializeBytes32(obj, "idToken1", idToken1);
        vm.serializeBytes32(obj, "idToken2", idToken2);

        vm.serializeBytes32(obj, "relayerPrivateKey", bytes32(sksScript[3]));
        vm.serializeAddress(obj, "searcherAOwnerAddress", addressesScript[0]);
        vm.serializeBytes32(obj, "searcherAOwnerSk", bytes32(sksScript[0]));
        vm.serializeAddress(obj, "searcherBOwnerAddress", addressesScript[1]);
        vm.serializeBytes32(obj, "searcherBOwnerSk", bytes32(sksScript[1]));
        vm.serializeAddress(obj, "depositor", addressesScript[2]);
        vm.serializeBytes32(obj, "depositorSk", bytes32(sksScript[2]));
        vm.serializeAddress(obj, "tokenVaultDeployer", addressesScript[4]);
        vm.serializeBytes32(obj, "tokenVaultDeployerSk", bytes32(sksScript[4]));
        string memory finalJSON = vm.serializeUint(obj, "numVaults", 0);
        vm.writeJson(finalJSON, latestEnvironmentPath);
    }

    function getNextPublishTime(bytes32 idToken) public view returns (uint64) {
        string memory json = vm.readFile(latestEnvironmentPath);
        address oracleLatest = vm.parseJsonAddress(json, ".oracle");
        MockPyth oracle = MockPyth(payable(oracleLatest));
        if (oracle.priceFeedExists(idToken) == false) {
            return 1;
        }
        return uint64(oracle.getPriceUnsafe(idToken).publishTime + 1);
    }

    function setOraclePrice(int64 priceT1, int64 priceT2) public {
        string memory json = vm.readFile(latestEnvironmentPath);
        address oracleLatest = vm.parseJsonAddress(json, ".oracle");
        bytes32 idToken1Latest = vm.parseJsonBytes32(json, ".idToken1");
        bytes32 idToken2Latest = vm.parseJsonBytes32(json, ".idToken2");

        console.log("oracle address:");
        console.log(oracleLatest);
        console.log("token 1 id:");
        console.logBytes32(idToken1Latest);
        console.log("token 2 id:");
        console.logBytes32(idToken2Latest);

        MockPyth oracle = MockPyth(payable(oracleLatest));

        // set initial oracle prices
        bytes memory token1UpdateData = oracle.createPriceFeedUpdateData(
            idToken1Latest,
            priceT1,
            1,
            0,
            priceT1,
            0,
            getNextPublishTime(idToken1Latest),
            0
        );
        bytes memory token2UpdateData = oracle.createPriceFeedUpdateData(
            idToken2Latest,
            priceT2,
            1,
            0,
            priceT2,
            0,
            getNextPublishTime(idToken2Latest),
            0
        );

        bytes[] memory updateData = new bytes[](2);
        updateData[0] = token1UpdateData;
        updateData[1] = token2UpdateData;
        vm.startBroadcast();
        oracle.updatePriceFeeds(updateData);
        vm.stopBroadcast();

        console.log("token 1, price after:");
        console.logInt(oracle.queryPriceFeed(idToken1Latest).price.price);
        console.log("token 2, price after:");
        console.logInt(oracle.queryPriceFeed(idToken2Latest).price.price);
    }

    function setUpVault(uint256 qT1, uint256 qT2, bool collatT1) public {
        string memory json = vm.readFile(latestEnvironmentPath);
        address depositorLatest = vm.parseJsonAddress(json, ".depositor");
        uint256 depositorSkLatest = vm.parseJsonUint(json, ".depositorSk");
        address tokenVaultLatest = vm.parseJsonAddress(json, ".tokenVault");
        address token1Latest = vm.parseJsonAddress(json, ".token1");
        address token2Latest = vm.parseJsonAddress(json, ".token2");
        bytes32 idToken1Latest = vm.parseJsonBytes32(json, ".idToken1");
        bytes32 idToken2Latest = vm.parseJsonBytes32(json, ".idToken2");
        uint256 numVaults;

        console.log(
            "depositor token balances, before:",
            IERC20(token1Latest).balanceOf(depositorLatest),
            IERC20(token2Latest).balanceOf(depositorLatest)
        );

        if (collatT1) {
            vm.startBroadcast(depositorSkLatest);
            IERC20(token1Latest).approve(address(tokenVaultLatest), qT1);
            numVaults = TokenVault(payable(tokenVaultLatest)).createVault(
                token1Latest,
                token2Latest,
                qT1,
                qT2,
                110 * (10 ** 16),
                1 * (10 ** 16),
                idToken1Latest,
                idToken2Latest,
                new bytes[](0)
            );
            vm.stopBroadcast();
        } else {
            vm.startBroadcast(depositorSkLatest);
            IERC20(token2Latest).approve(address(tokenVaultLatest), qT2);
            numVaults = TokenVault(payable(tokenVaultLatest)).createVault(
                token2Latest,
                token1Latest,
                qT2,
                qT1,
                110 * (10 ** 16),
                1 * (10 ** 16),
                idToken1Latest,
                idToken2Latest,
                new bytes[](0)
            );
            vm.stopBroadcast();
        }

        console.log(
            "depositor token balances, after:",
            IERC20(token1Latest).balanceOf(depositorLatest),
            IERC20(token2Latest).balanceOf(depositorLatest)
        );

        vm.writeJson(
            Strings.toString(numVaults),
            latestEnvironmentPath,
            ".numVaults"
        );
    }

    function getBalanceEth(address addy) public view returns (uint256) {
        uint256 balance = addy.balance;
        console.log(balance);
        return balance;
    }

    function getBalanceErc(
        address addy,
        address token
    ) public view returns (uint256) {
        uint256 balance = IERC20(token).balanceOf(addy);
        console.log(balance);
        return balance;
    }

    function getBalanceWeth(address addy) public view returns (uint256) {
        string memory json = vm.readFile(latestEnvironmentPath);
        address wethLatest = vm.parseJsonAddress(json, ".weth");

        uint256 balance = WETH9(payable(wethLatest)).balanceOf(addy);
        console.log(balance);
        return balance;
    }

    function getVault(uint256 vaultId) public view returns (Vault memory) {
        string memory json = vm.readFile(latestEnvironmentPath);
        address tokenVaultLatest = vm.parseJsonAddress(json, ".tokenVault");
        Vault memory vault = TokenVault(payable(tokenVaultLatest)).getVault(
            vaultId
        );
        console.log(
            "vault amounts are",
            vault.amountCollateral,
            vault.amountDebt
        );
        return vault;
    }

    function getAllowances(
        address from,
        address spender
    ) public view returns (uint256, uint256) {
        string memory json = vm.readFile(latestEnvironmentPath);
        address token1Latest = vm.parseJsonAddress(json, ".token1");
        address token2Latest = vm.parseJsonAddress(json, ".token2");
        console.log(
            "allowances are",
            IERC20(token1Latest).allowance(from, spender),
            IERC20(token2Latest).allowance(from, spender)
        );
        console.log(
            "balances are",
            IERC20(token1Latest).balanceOf(from),
            IERC20(token2Latest).balanceOf(from)
        );
        return (
            IERC20(token1Latest).allowance(from, spender),
            IERC20(token2Latest).allowance(from, spender)
        );
    }

    function tryOpportunityAdapterContract() public view returns (address) {
        string memory json = vm.readFile(latestEnvironmentPath);
        address opportunityAdapter = vm.parseJsonAddress(
            json,
            ".opportunityAdapter"
        );
        return OpportunityAdapter(payable(opportunityAdapter)).getWeth();
    }

    function createLiquidatableVault() public {
        setOraclePrice(110, 110);
        setUpVault(100, 80, true);
        setOraclePrice(110, 200);
    }
}
