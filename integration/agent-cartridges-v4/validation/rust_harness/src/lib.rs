#[path="../../../runtime/rhai_agent_bridge.rs"]pub mod rhai_agent_bridge;
#[path="../../../runtime/agent_graph.rs"]pub mod agent_graph;
#[path="../../../runtime/snapshot_services.rs"]pub mod snapshot_services;
#[path="../../../runtime/proposal_contract.rs"]pub mod proposal_contract;
#[path="../../../runtime/context_router.rs"]pub mod context_router;

#[cfg(test)]mod conformance {
    use super::*;
    use rhai::{Engine,Dynamic,Map,Scope};
    use std::{sync::Arc,path::PathBuf};
    use serde_json::Value;
    // A test-only backend that deliberately provides NO market data. This
    // verifies compilation/bindings/explicit refusal, not economic validity.
    struct EmptyTestBackend;
    impl rhai_agent_bridge::AgentServices for EmptyTestBackend {
        fn discover(&self,_:&Value,_:&Value)->Result<Value,String>{Err("TEST_ONLY_NO_MARKET_SNAPSHOT".into())}
        fn quote(&self,_:&Value,_:&Value,_:&Value)->Result<rhai_agent_bridge::QuotedPlan,String>{Err("TEST_ONLY_NO_QUOTES".into())}
        fn operators(&self,_:&Value,_:&Value,_:&Value)->Result<Value,String>{Err("TEST_ONLY_NO_OPERATORS".into())}
        fn policy(&self,_:&Value,_:&Value)->Result<rhai_agent_bridge::PolicyView,String>{Err("TEST_ONLY_NO_POLICY".into())}
        fn verify_requirements(&self,_:&Value,_:&Value,_:&Value,_:&[String])->Result<Vec<rhai_agent_bridge::RequirementReceipt>,String>{Err("TEST_ONLY_NO_RECEIPTS".into())}
        fn build_payload(&self,_:&Value,_:&Value)->Result<Value,String>{Err("TEST_ONLY_NO_ENCODER".into())}
    }
    #[test]fn compile_and_call_every_cartridge_with_empty_scope(){
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../generated/cartridges/strategies");
        let mut paths:Vec<_>=std::fs::read_dir(root).unwrap().map(|p|p.unwrap().path()).filter(|p|p.extension().is_some_and(|x|x=="rhai")).collect();paths.sort();assert_eq!(paths.len(),264);
        let mut engine=Engine::new();
        // Mirrors existing runner constants for conformance only. This test
        // doesn't modify the live engine, repository files or operator config.
        engine.set_max_operations(1_000_000);engine.set_max_call_levels(64);
        engine.set_max_string_size(65_536);engine.set_max_array_size(4_096);
        engine.set_max_map_size(1_024);engine.set_max_expr_depths(64,32);engine.set_max_modules(0);
        rhai_agent_bridge::register(&mut engine,Arc::new(EmptyTestBackend));
        for path in paths {
            let source=std::fs::read_to_string(&path).unwrap();let ast=engine.compile(&source).unwrap_or_else(|e|panic!("{}: {e}",path.display()));
            let meta=engine.call_fn::<Map>(&mut Scope::new(),&ast,"init_strategy",()).unwrap();assert!(meta.contains_key("mev_id"));
            let mut ctx=Map::new();ctx.insert("context_id".into(),"TEST_ONLY".into());
            let result=engine.call_fn::<Map>(&mut Scope::new(),&ast,"evaluate_opportunity",(ctx,)).unwrap_or_else(|e|panic!("{}: {e}",path.display()));
            assert!(!result["is_opportunity"].as_bool().unwrap());assert!(!result["approved_for_execution"].as_bool().unwrap());
            assert!(result["net_profit_usd"].is_unit());assert!(result["estimated_profit"].is_unit());
            let payload=engine.call_fn::<Dynamic>(&mut Scope::new(),&ast,"build_payload",(result,)).unwrap();assert!(payload.is::<Map>());
        }
    }
}
