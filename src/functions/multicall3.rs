use alloy_sol_types::sol;

sol! {
    struct Call3 {
        address target;
        bool allowFailure;
        bytes callData;
    }
    struct Result {
        bool success;
        bytes returnData;
    }
    function aggregate3(Call3[] calldata calls) external payable returns (Result[] memory returnData);
    function getBlockNumber() external view returns (uint256 blockNumber);
    function getCurrentBlockTimestamp() external view returns (uint256 timestamp);
    
}
pub type MultiResult = sol!(Result[]);
