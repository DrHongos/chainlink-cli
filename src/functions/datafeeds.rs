use alloy_sol_types::{sol, SolCall, SolType, SolValue};
use alloy_primitives::{utils::format_units, U256, Address};
use alloy_providers::provider::{Provider, TempProvider};
use alloy_rpc_types::{CallRequest, CallInput};
use alloy_chains::Chain;
use alloy_transport_http::Http;
use reqwest::Client;
use datafeeds::{DataFeeds, DataFeedIndexPrices};
use crate::constants;
use crate::functions::multicall3;
use crate::functions::multicall3::Call3;

use super::multicall3::MultiResult;

sol!(
    struct GetRoundDataReturn {
        uint80 roundId;
        int256 answer;
        uint256 startedAt;
        uint256 updatedAt;
        uint80 answeredInRound;
    }
);

pub async fn get_latest_answer(
    provider: Provider<Http<Client>>,
    chain: Chain,
    token: String,
    base: String
) {
    let price_feeds = DataFeeds::load_chain_feeds_prices(chain).await;
    let oracle = price_feeds.get_data_feed_prices(&token, &base).unwrap();
    let tx = CallRequest {
        to: Some(oracle.contract_address.unwrap()),
        input: datafeeds::latest_answer_call(),
        ..Default::default()
    };
    match provider.call(tx, None).await {
        Ok(r) => {
            let b: U256 = U256::abi_decode(&r, false).expect("cannot deserialize result");
            println!("In {} {}/{} is: {}", 
            chain, 
            token, 
            base, 
            format_units(b, oracle.decimals.unwrap()).unwrap()
        )
        },
        Err(e) => println!("Error: {:#?}", e)
    }        
}

pub async fn get_round_data(
    provider: Provider<Http<Client>>, 
    round_id: u128,
    chain: Chain, 
    token: String, 
    base: String
) {
    let price_feeds = DataFeeds::load_chain_feeds_prices(chain).await;
    let oracle = price_feeds.get_data_feed_prices(&token, &base).unwrap();
    let tx = CallRequest {
        to: Some(oracle.contract_address.unwrap()),
        input: datafeeds::get_round_data_call(round_id.to_owned()),
        ..Default::default()
    };
    match provider.call(tx, None).await {
        Ok(r) => {                      
//                        let response = <getRoundDataReturn>::abi_decode(&r, false).expect("Cannot deserialize");
//                        let response = getRoundDataReturn::abi_decode(&r, false).expect("cannot deserialize result");
            let response = <GetRoundDataReturn as SolValue>::abi_decode(&r, false).unwrap();
            println!("In {} {}/{} is: \nroundId:{:?} \nanswer: {} [{}]\nstarted at: {}\nupdated at: {}\nanswered id round: {}", 
            chain.named().unwrap(), 
            token, 
            base,
            response.roundId, 
            response.answer, 
            format_units(response.answer, oracle.decimals.unwrap()).unwrap(),
            response.startedAt, response.updatedAt, response.answeredInRound
        )
        },
        Err(e) => println!("Error: {:#?}", e)
    }        
}

pub async fn get_description(provider: Provider<Http<Client>>, chain: Chain, token: String, base: String) {
    let price_feeds = DataFeeds::load_chain_feeds_prices(chain).await;
    let oracle = price_feeds.get_data_feed_prices(&token, &base).unwrap();
    let tx = CallRequest {
        to: Some(oracle.contract_address.unwrap()),
        input: datafeeds::description_call(),
        ..Default::default()
    };
    match provider.call(tx, None).await {
        Ok(r) => {
            let response: String = String::abi_decode(&r, false).expect("cannot deserialize result");
            println!("In {} {}/{} description is: {}", 
            chain.named().unwrap(), 
            token, 
            base,
            response 
        )
        },
        Err(e) => println!("Error: {:#?}", e)
    }        
}
/// Gets all the Aggregators used by this proxy, in order to get historical data    --> Improve this!
pub async fn get_all_phases_addresses(provider: Provider<Http<Client>>, oracle: &DataFeedIndexPrices) -> Vec<Address> {
    // get current phase
    let curr_phase_req = CallRequest {
        to: Some(oracle.contract_address.unwrap()),
        input: datafeeds::phase_id(),
        ..Default::default()
    };
    match provider.call(curr_phase_req, None).await {
        Ok(phase) => {
            // collects phaseAggregators() from 1 to current (Multicall)
            let phase: u16 = u16::abi_decode(&phase, false).unwrap();
            let mut all_phases_calls: Vec<Call3> = Vec::new();
            for i in 1..phase {
                all_phases_calls.push(
                    Call3 {
                        target: oracle.contract_address.unwrap(),
                        allowFailure: true,
                        callData: datafeeds::phase_aggregators(i).into_input().unwrap().into(),
                    }
                )
            };
            let mc = multicall3::aggregate3Call {
                calls: all_phases_calls
            };
            let c = CallInput::new(mc.abi_encode().into());                    
            let tx = CallRequest {
                to: Some(constants::MULTICALL3.parse::<Address>().unwrap()),
                input: c,
                ..Default::default()
            };
            match provider.call(tx, None).await {
                Ok(res) => {
                    let response = MultiResult::abi_decode(&res, false).unwrap();
                    response
                        .into_iter()
                        .map(|agg| 
                            // check if correct?
                            Address::abi_decode(&agg.returnData, false).unwrap()
                        )
                        .collect()
                },
                Err(e) => panic!("{:#?}", e)
            }
        },
        Err(e) => panic!("{:#?}", e)   
    }
}
// pub fn find_phase_timestamp() -> Address {
//  get AggregatorContract version (for each aggregator)
//  every version needs to be handled differently (see  https://github.com/pappas999/historical-price-feed-data/blob/main/src/historical-price-ea/index.js 115-134)   
//  getTimestamp(1) on each to find where is the timestamp required
//};
// pub fn find_round_id_timestamp() -> u128 {
//  needs to limit lower point (with a chain related hardcoded values)
//  what the ref does is an iteration of chunked round_ids for the logs of "newRound" in the Aggregator contract
//  gets a list of valid round ids where later does a binary search to find lower & upper bands
//}