from beacon.utils.types_liquidation_adapter import LiquidationOpportunity, LiquidationAdapterTransaction
from beacon.searcher.searcher_utils import UserLiquidationParams


def assess_liquidation_opportunity(
    opp: LiquidationOpportunity
) -> UserLiquidationParams | None:
    """
    Assesses whether a LiquidationOpportunity is worth liquidating; if so, returns a tuple of (bid, valid_until)

    This function can handle assessing the LiquidationOpportunity to determine whether it deals with repay and receipt tokens that the searcher wishes to transact in and whether it is profitable to conduct the liquidation.
    There are many ways to evaluate this, but the most common way is to check that the value of the amount the searcher will receive from the liquidation exceeds the value of the amount repaid.
    Individual searchers will have their own methods to determine market impact and the profitability of conducting a liquidation. This function can be expanded to include external prices to perform this evaluation.
    If the LiquidationOpportunity is deemed worthwhile, this function can return a bid amount representing the amount of native token to bid on this opportunity, and a timestamp representing the time at which the transaction will expire.
    Otherwise, this function can return None.
    """
    raise NotImplementedError


def create_liquidation_tx(
    opp: LiquidationOpportunity,
    sk_liquidator: str,
    valid_until: int,
    bid: int
) -> LiquidationAdapterTransaction:
    """
    Processes a LiquidationOpportunity into a LiquidationAdapterTransaction

    This function can handle constructing the LiquidationAdapterTransaction to submit. The calldata for the LiquidationAdapter contract should be constructed according to the LiquidationAdapterCalldata type; you can leverage the construct_signature_liquidator function to construct the signature field.
    """
    raise NotImplementedError
