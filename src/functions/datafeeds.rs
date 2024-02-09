use alloy_sol_types::{sol, SolCall, SolType, SolValue};
use alloy_primitives::{utils::format_units, U256, Address};
use alloy_providers::provider::{Provider, TempProvider};
use alloy_rpc_types::{CallRequest, CallInput};
use alloy_chains::Chain;
use alloy_transport_http::Http;
use std::sync::Arc;
use reqwest::Client;
use datafeeds::{Oracle, OraclesIndex};
use crate::constants;
use crate::functions::multicall3;
use crate::functions::multicall3::Call3;
use eyre::{Result, eyre};

sol!(
    struct GetRoundDataReturn {
        uint80 roundId;
        int256 answer;
        uint256 startedAt;
        uint256 updatedAt;
        uint80 answeredInRound;
    }
);

impl std::fmt::Debug for GetRoundDataReturn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "roundId: {}\nanswer: {}\nstarted at: {}\nupdated at: {}\nanswered in round: {}", 
            self.roundId, 
            self.answer,
            self.startedAt,
            self.updatedAt,
            self.answeredInRound
        )
    }
}

/// Helper for multicalls
pub async fn handle_multicall(provider: Arc<Provider<Http<Client>>>, calls: Vec<Call3>) -> Result<Vec<multicall3::Result>> {
    let mc = multicall3::aggregate3Call { calls };
    let ci = CallInput::new(mc.abi_encode().into());
    let cr = CallRequest {
        to: Some(constants::MULTICALL3.parse::<Address>().unwrap()),
        input: ci,
        ..Default::default()
    };
    let r = provider.call(cr, None).await?;
    Ok(multicall3::MultiResult::abi_decode(&r, false).expect("Error on returned data"))           
}

pub async fn get_latest_answer(
    provider: Arc<Provider<Http<Client>>>,
    oracle: Address,
) -> Result<U256> {
    let tx = CallRequest {
        to: Some(oracle),
        input: CallInput::new(datafeeds::contracts::EACAggregatorProxy::EACAggregatorProxy::latestAnswerCall{}.abi_encode().into()),
        ..Default::default()
    };
    match provider.call(tx, None).await {
        Ok(r) => {
            let b: U256 = U256::abi_decode(&r, false).expect("cannot deserialize result");
            Ok(b)
        },
        Err(e) => Err(eyre!("Could not get latest answer {:?}", e))
    }        
}

pub async fn get_round_data(
    provider: Arc<Provider<Http<Client>>>, 
    oracle: Address, 
    round_id: u128,
) -> Result<GetRoundDataReturn> {
    let tx = CallRequest {
        to: Some(oracle),
        input: CallInput::new(
            datafeeds::contracts::
                EACAggregatorProxy::EACAggregatorProxy::
                    getRoundDataCall{
                        _roundId: round_id.to_owned()
                    }.abi_encode().into()
                ),
        ..Default::default()
    };
    match provider.call(tx, None).await {
        Ok(r) => {                      
            Ok(<GetRoundDataReturn as SolValue>::abi_decode(&r, false)?)
        },
        Err(e) => Err(eyre!("Error: {:#?}", e))
    }        
}

pub async fn get_latest_round_data(provider: Arc<Provider<Http<Client>>>, addr: Address) -> Result<GetRoundDataReturn> {
    let tx = CallRequest {
        to: Some(addr),
        input: CallInput::new(datafeeds::contracts::EACAggregatorProxy::EACAggregatorProxy::latestRoundDataCall{}.abi_encode().into()),
        ..Default::default()
    };
    match provider.call(tx, None).await {
        Ok(r) => {
            let resp = <GetRoundDataReturn as SolValue>::abi_decode(&r, false).unwrap();
            Ok(resp)
        },
        Err(e) => Err(eyre!("Error fetching latest round data for {addr}: {:?}", e)),
    }    
}

pub async fn get_description(provider: Arc<Provider<Http<Client>>>, oracle: &Oracle) {
    let tx = CallRequest {
        to: Some(oracle.proxy_address.unwrap()),
        input: CallInput::new(datafeeds::contracts::EACAggregatorProxy::EACAggregatorProxy::descriptionCall{}.abi_encode().into()),
        ..Default::default()
    };
    match provider.call(tx, None).await {
        Ok(r) => {
            let response: String = String::abi_decode(&r, false).expect("cannot deserialize result");
            println!("Description for {} is: {}", 
            oracle.proxy_address.unwrap(), 
            response 
        )
        },
        Err(e) => println!("Error: {:#?}", e)
    }        
}

pub async fn get_aggregators_version(provider: Arc<Provider<Http<Client>>>, addresses: Vec<Address>) -> Result<Vec<U256>> {
    let all_timestamps: Vec<Call3> = addresses
        .clone()
        .into_iter()
        .map(|a| 
            Call3 {
                target: a,
                allowFailure: true,
                callData: datafeeds::contracts::EACAggregatorProxy::EACAggregatorProxy::versionCall{}.abi_encode().into() 
            }
        )
        .collect();
    let mut response = Vec::new();
    match handle_multicall(provider, all_timestamps).await {
        Ok(r) => {
            for (result, aggr) in r.into_iter().zip(addresses) {
                if result.success {
                    let res_p = U256::try_from_be_slice(&result.returnData).unwrap_or(U256::ZERO);
                    //println!("Aggregator {} version is {}", aggr, res_p);
                    response.push(res_p);
                } else {
                    println!("Cannot retrieve version for {}", aggr);
                }
            }
        },
        Err(e) => panic!("{}", e)
    };
    Ok(response) 
    //every version needs to be handled differently (see  https://github.com/pappas999/historical-price-feed-data/blob/main/src/historical-price-ea/index.js 115-134)   
}

// pub fn find_round_id_timestamp() -> u128 {
//  needs to limit lower point (with a chain related hardcoded values)
//  what the ref does is an iteration of chunked round_ids for the logs of "newRound" in the Aggregator contract
//  gets a list of valid round ids where later does a binary search to find lower & upper bands
//}



pub async fn get_multiple_round_data(
    provider: Arc<Provider<Http<Client>>>, 
    oracle: Address,
    round_ids: Vec<u128>
) -> Result<Vec<GetRoundDataReturn>> {
    let mut all_queries: Vec<Call3> = Vec::new();
    for rid in round_ids.clone().into_iter() {
        all_queries.push(
            Call3 {
                target: oracle,
                allowFailure: true,
                callData: datafeeds::contracts::EACAggregatorProxy::EACAggregatorProxy::getRoundDataCall{_roundId: rid.clone()}.abi_encode().into()
            } 
        )
    }
    let mut all_responses: Vec<GetRoundDataReturn> = Vec::new();
    
    let all_results = handle_multicall(provider, all_queries).await?;
    for result in all_results.into_iter() {
        if result.success {
            let response = <GetRoundDataReturn as SolValue>::abi_decode(&result.returnData, false).unwrap();
            all_responses.push(response)          
        } else {
            //all_responses.push(None);
            println!("Error getting round data for {}", oracle)
        }
    }

    Ok(all_responses)                   
}

pub async fn get_multiple_latest_answer(
    provider: Arc<Provider<Http<Client>>>, 
    chain: Chain, 
    token: Vec<String>, 
    base: Vec<String>
) -> Result<Vec<U256>> {
    let mut tokens: Vec<String> = Vec::new();
    let mut bases: Vec<String> =  Vec::new();
    let datafeeds = OraclesIndex::load_reference_feeds(chain).await;
    let mut all_queries: Vec<Call3> = Vec::new();
    let mut all_oracles: Vec<&Oracle> = Vec::new();
    for (t,b) in token.into_iter().zip(base) {
        let tt = t.to_uppercase();
        let bb = b.to_uppercase();
        if let Some(oracle) = datafeeds.get_oracle(&tt, &bb) {
            all_oracles.push(oracle);
            tokens.push(tt);
            bases.push(bb);
            all_queries.push(
                Call3 {
                    target: oracle.proxy_address.unwrap(),
                    allowFailure: true,
                    callData: datafeeds::contracts::EACAggregatorProxy::EACAggregatorProxy::latestAnswerCall{}.abi_encode().into()
                } 
            )
        }
    }
    let mut vr = Vec::<U256>::new();
    match handle_multicall(provider, all_queries).await {
        Ok(res) => {
            for (((t,b) , o), r) in tokens.into_iter().zip(bases).zip(all_oracles).zip(res) {
                if r.success {
                    let val = U256::try_from_be_slice(&r.returnData).unwrap_or(U256::ZERO);
                    vr.push(val);
                    println!("Latest answer for {}: {} {}", t, format_units(val, o.decimals.unwrap()).unwrap(), b)
                } else {
                    println!("Error getting latest answer for {}/{}", t, b)
                }
            }
        },
        Err(e) => panic!("{}", e)
    }

    Ok(vr)
}

// WIP to collect historical data

/// Gets all the Aggregators used by this proxy, in order to get historical data
pub async fn get_aggregators(provider: Arc<Provider<Http<Client>>>, oracle: &Oracle) -> Vec<Address> {
    // get current phase
    let curr_phase_req = CallRequest {
        to: Some(oracle.proxy_address.unwrap()),
        input: CallInput::new(datafeeds::contracts::EACAggregatorProxy::EACAggregatorProxy::phaseIdCall{}.abi_encode().into()),
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
                        target: oracle.proxy_address.unwrap(),
                        allowFailure: true,
                        callData: datafeeds::contracts::EACAggregatorProxy::EACAggregatorProxy::phaseAggregatorsCall{ _0: i }.abi_encode().into(),
                    }
                )
            };
            match handle_multicall(provider, all_phases_calls).await {
                Ok(res) => {
                    res
                        .into_iter()
                        .map(|agg| 
                            // check if correct?
                            Address::abi_decode(&agg.returnData, false).unwrap()
                        )
                        .collect()
                },
                Err(e) => panic!("{}", e)
            }
        },
        Err(e) => panic!("{:#?}", e)   
    }
}

/// Returns addresses (Aggregator's) last round data
/// this method is not working properly
pub async fn get_aggregators_last_round_data(provider: Arc<Provider<Http<Client>>>, aggregators: Vec<Address>) -> Vec<Option<GetRoundDataReturn>> {
    let all_last_round_queries: Vec<Call3> = aggregators
        .clone()
        .into_iter()
        .map(|a| 
            Call3 {
                target: a,
                allowFailure: true,
                callData: datafeeds::contracts::EACAggregatorProxy::EACAggregatorProxy::latestRoundDataCall{}.abi_encode().into()
            }
        )
        .collect();
    let mut response = Vec::new();
    match handle_multicall(provider, all_last_round_queries).await {
        Ok(resp) => {
            for (result, aggr) in resp.into_iter().zip(aggregators) {
                if result.success {
                    if let Ok(res) = <GetRoundDataReturn as SolValue>::abi_decode(&result.returnData, false){
                        response.push(Some(res));
                    } else {
                        response.push(None);
                    }
                } else {
                    println!("Cannot retrieve timestamp for {}", aggr);
                    response.push(None);
                }
            }
        },
        Err(e) => panic!("{}", e)
    };
    response
}
