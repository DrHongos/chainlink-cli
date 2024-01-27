use eyre::Result;
/* 
check available networks for chainlink!
adapt to it 
https://docs.chain.link/data-feeds/price-feeds/addresses?network=ethereum&page=1
*/
pub const MULTICALL3: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";

pub fn get_provider_rpc_url(chain: u64, rpc_url_id: &str) -> Result<String> {
    match chain {
        1 => Ok(format!("https://mainnet.infura.io/v3/{}", rpc_url_id)),
        11_155_111 => Ok(format!("https://sepolia.infura.io/v3/{}", rpc_url_id)),
        137 => Ok(format!("https://polygon-mainnet.infura.io/v3/{}", rpc_url_id)),
        80_001 => Ok(format!("https://polygon-mumbai.infura.io/v3/{}", rpc_url_id)),
        10 => Ok(format!("https://optimism-mainnet.infura.io/v3/{}", rpc_url_id)),
        420 => Ok(format!("https://optimism-goerli.infura.io/v3/{}", rpc_url_id)),
        42_161 => Ok(format!("https://arbitrum-mainnet.infura.io/v3/{}", rpc_url_id)),
        421_613 => Ok(format!("https://arbitrum-goerli.infura.io/v3/{}", rpc_url_id)),
        43_114 => Ok(format!("https://avalanche-mainnet.infura.io/v3/{}", rpc_url_id)),
        43_113 => Ok(format!("https://avalanche-fuji.infura.io/v3/{}", rpc_url_id)),
        97 => Ok(format!("https://data-seed-prebsc-1-s1.binance.org:8545/")),
        84_531 => Ok(String::from("https://base-goerli.blockpi.network/v1/rpc/public")),
        _ => Err(eyre::eyre!("Chain has no RPC URL")) 
    }
}