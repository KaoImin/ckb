use crate::helper::wait_for_exit;
use crate::Setup;
use ckb_chain::chain::{ChainBuilder, ChainController};
use ckb_core::script::Script;
use ckb_db::diskdb::RocksDB;
use ckb_miner::BlockAssembler;
use ckb_network::futures::sync::mpsc::channel;
use ckb_network::CKBProtocol;
use ckb_network::NetworkService;
use ckb_network::ProtocolId;
use ckb_network::{network::FEELER_PROTOCOL_ID, protocol::feeler::Feeler};
use ckb_notify::{NotifyController, NotifyService};
use ckb_rpc::RpcServer;
use ckb_shared::cachedb::CacheDB;
use ckb_shared::index::ChainIndex;
use ckb_shared::shared::{Shared, SharedBuilder};
use ckb_sync::{NetTimeProtocol, NetworkProtocol, Relayer, Synchronizer};
use ckb_traits::chain_provider::ChainProvider;
use crypto::secp::Generator;
use log::info;
use numext_fixed_hash::H256;
use std::sync::Arc;

pub fn run(setup: Setup) {
    let consensus = setup.chain_spec.to_consensus().unwrap();

    let shared = SharedBuilder::<CacheDB<RocksDB>>::default()
        .consensus(consensus)
        .db(&setup.configs.db)
        .tx_pool_config(setup.configs.tx_pool.clone())
        .txs_verify_cache_size(setup.configs.tx_pool.txs_verify_cache_size)
        .build();

    let notify = NotifyService::default().start(Some("notify"));

    let chain_controller = setup_chain(shared.clone(), notify.clone());
    info!(target: "main", "chain genesis hash: {:#x}", shared.genesis_hash());

    let block_assembler =
        BlockAssembler::new(shared.clone(), setup.configs.block_assembler.type_hash);
    let block_assembler_controller = block_assembler.start(Some("MinerAgent"), &notify);

    let synchronizer = Arc::new(Synchronizer::new(
        chain_controller.clone(),
        shared.clone(),
        setup.configs.sync,
    ));

    let relayer = Arc::new(Relayer::new(
        chain_controller.clone(),
        shared.clone(),
        synchronizer.peers(),
    ));

    let net_time_checker = Arc::new(NetTimeProtocol::default());

    let (sender, receiver) = channel(std::u8::MAX as usize);
    let protocols = vec![
        (
            CKBProtocol::new(
                "syn".to_string(),
                NetworkProtocol::SYNC as ProtocolId,
                &[1][..],
                sender.clone(),
            ),
            synchronizer as Arc<_>,
        ),
        (
            CKBProtocol::new(
                "rel".to_string(),
                NetworkProtocol::RELAY as ProtocolId,
                &[1][..],
                sender.clone(),
            ),
            relayer as Arc<_>,
        ),
        (
            CKBProtocol::new(
                "tim".to_string(),
                NetworkProtocol::TIME as ProtocolId,
                &[1][..],
                sender.clone(),
            ),
            net_time_checker as Arc<_>,
        ),
        // TODO Work around, should move to network after refactor
        (
            CKBProtocol::new(
                "flr".to_string(),
                FEELER_PROTOCOL_ID,
                &[1][..],
                sender.clone(),
            ),
            Arc::new(Feeler {}) as Arc<_>,
        ),
    ];
    let network = Arc::new(
        NetworkService::run_in_thread(&setup.configs.network, protocols, receiver)
            .expect("Create and start network"),
    );

    let rpc_server = RpcServer::new(
        setup.configs.rpc,
        Arc::clone(&network),
        shared,
        chain_controller,
        block_assembler_controller,
    );

    wait_for_exit();

    info!(target: "main", "Finishing work, please wait...");

    rpc_server.close();
    info!(target: "main", "Jsonrpc shutdown");

    // FIXME: should gracefully shutdown network
    network.close();
    info!(target: "main", "Network shutdown");
}

fn setup_chain<CI: ChainIndex + 'static>(
    shared: Shared<CI>,
    notify: NotifyController,
) -> ChainController {
    let chain_service = ChainBuilder::new(shared, notify).build();
    chain_service.start(Some("ChainService"))
}

pub fn type_hash(setup: &Setup) {
    let consensus = setup.chain_spec.to_consensus().unwrap();
    let system_cell_tx = &consensus.genesis_block().commit_transactions()[0];
    let system_cell_data_hash = system_cell_tx.outputs()[0].data_hash();

    let script = Script::new(0, vec![], Some(system_cell_data_hash), None, vec![]);
    println!("{:#x}", script.type_hash());
}

pub fn keygen() {
    let result: H256 = Generator::new().random_privkey().into();
    println!("{:#x}", result);
}
