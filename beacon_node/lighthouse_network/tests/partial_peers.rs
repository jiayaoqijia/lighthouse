use async_channel::Sender;
use libp2p::{PeerId, identity::secp256k1, multiaddr::Protocol};
use lighthouse_network::types::{GossipKind, NetworkGlobals, PubsubPartialMessage};
use lighthouse_network::{NetworkConfig, NetworkEvent, service::Network};
use serde::Deserialize;
use ssz::{Decode, Encode};
use std::{
    collections::BTreeMap,
    net::Ipv4Addr,
    sync::{Arc, Weak},
    time::Duration,
};
use tempfile::TempDir;
use tokio::{runtime::Runtime, time::timeout};
use types::{
    ChainSpec, DataColumnSidecarGloas, DataColumnSubnetId, EnrForkId, Epoch, ForkContext, Hash256,
    MainnetEthSpec, PartialDataColumn, PartialDataColumnGloas, PartialDataColumnSidecarGloas, Slot,
};

type E = MainnetEthSpec;
#[derive(Deserialize)]
struct Vectors {
    slot: u64,
    beacon_root: Hash256,
    column_index: u64,
    full_column_ssz: String,
    group_id: String,
    partials: BTreeMap<String, String>,
    metadata_all_requested: BTreeMap<String, String>,
}
fn bytes(value: &str) -> Vec<u8> {
    hex::decode(value.strip_prefix("0x").unwrap()).unwrap()
}
fn vectors() -> Vectors {
    serde_json::from_slice(include_bytes!("testdata/gloas-multi-row-partials.json")).unwrap()
}
fn spec() -> Arc<ChainSpec> {
    let config: types::Config =
        yaml_serde::from_slice(include_bytes!("testdata/gloas-private-network.yaml")).unwrap();
    Arc::new(ChainSpec::from_config::<E>(&config).unwrap())
}
fn column(v: &Vectors, name: &str) -> PartialDataColumnGloas<E> {
    PartialDataColumnGloas {
        block_root: v.beacon_root,
        slot: Slot::new(v.slot),
        index: v.column_index,
        sidecar: PartialDataColumnSidecarGloas::from_ssz_bytes(&bytes(&v.partials[name])).unwrap(),
    }
}
struct Peer {
    node: Network<E>,
    globals: Arc<NetworkGlobals<E>>,
    _signal: Sender<()>,
    _directory: TempDir,
}
async fn peer(runtime: Weak<Runtime>, spec: Arc<ChainSpec>, custody: u64) -> Peer {
    let directory = TempDir::new().unwrap();
    let mut config = NetworkConfig::default();
    config.set_ipv4_listening_address(Ipv4Addr::LOCALHOST, 0, 0, 0);
    config.enr_address = (Some(Ipv4Addr::LOCALHOST), None);
    config.network_dir = directory.path().to_path_buf();
    config.enable_partial_columns = true;
    let config = Arc::new(config);
    let genesis = Hash256::from_slice(&bytes(
        "0x0888cde6ee0c51578046793b377d6f6474908f7812b8db4d87a736d73e539d2c",
    ));
    let context = Arc::new(ForkContext::new::<E>(Slot::new(5747), genesis, &spec));
    assert_eq!(hex::encode(context.current_fork_digest()), "d65ba395");
    let fork = EnrForkId {
        fork_digest: context.current_fork_digest(),
        next_fork_version: spec.gloas_fork_version,
        next_fork_epoch: Epoch::new(u64::MAX),
    };
    let (signal, exit) = async_channel::bounded(1);
    let (shutdown, _) = futures::channel::mpsc::channel(1);
    let executor = task_executor::TaskExecutor::new(runtime, exit, shutdown);
    let (node, globals) = Network::new(
        executor,
        lighthouse_network::Context {
            config,
            enr_fork_id: fork,
            fork_context: context,
            chain_spec: spec,
            libp2p_registry: None,
        },
        custody,
        secp256k1::Keypair::generate().into(),
    )
    .await
    .unwrap();
    Peer {
        node,
        globals,
        _signal: signal,
        _directory: directory,
    }
}
async fn listen(peer: &mut Peer) -> libp2p::Multiaddr {
    timeout(Duration::from_secs(10), async {
        loop {
            if let NetworkEvent::NewListenAddr(addr) = peer.node.next_event().await {
                if addr.iter().any(|p| matches!(p, Protocol::Tcp(_))) {
                    return addr;
                }
            }
        }
    })
    .await
    .unwrap()
}
fn subscribed(a: &mut Peer, b: PeerId) -> bool {
    a.node.gossipsub_mut().all_peers().any(|(id, topics)| {
        *id == b
            && topics
                .iter()
                .any(|t| t.as_str().contains("data_column_sidecar_0"))
    })
}
async fn connect(a: &mut Peer, b: &mut Peer, address: libp2p::Multiaddr) {
    a.node.testing_dial(address).unwrap();
    timeout(Duration::from_secs(20), async {
        loop {
            if subscribed(a, b.node.local_peer_id) && subscribed(b, a.node.local_peer_id) {
                break;
            }
            tokio::select! {_=a.node.next_event()=>{},_=b.node.next_event()=>{}}
        }
    })
    .await
    .expect("real TCP peers did not negotiate the actual partial column topic");
}
fn publish(peer: &mut Peer, v: &Vectors, name: &str) {
    let mut requested = column(v, "full").sidecar.cells_present_bitmap;
    requested.set(0, true).unwrap();
    requested.set(1, true).unwrap();
    peer.node
        .publish_partial(vec![PubsubPartialMessage::DataColumnGloas {
            column: Arc::new(column(v, name)),
            request_cells: requested,
        }]);
}
fn observe(v: &Vectors, column: PartialDataColumn<E>, label: &str) -> PartialDataColumnGloas<E> {
    let PartialDataColumn::Gloas(column) = column else {
        panic!("wrong fork on actual Gloas wire")
    };
    assert_eq!(column.block_root, v.beacon_root);
    assert_eq!(column.slot, Slot::new(v.slot));
    assert_eq!(column.index, v.column_index);
    let unified = types::PartialDataColumnSidecar::<E>::Gloas(column.sidecar.clone());
    unified.verify_len().unwrap();
    let full = DataColumnSidecarGloas::<E>::from_ssz_bytes(&bytes(&v.full_column_ssz)).unwrap();
    for (row, cell, proof) in unified.to_ref().present_cells() {
        assert_eq!(cell, &full.column[row]);
        assert_eq!(proof, &full.kzg_proofs[row]);
    }
    println!(
        "PARTIAL_WIRE {}",
        serde_json::json!({"case":label,"beacon_root":v.beacon_root,"slot":v.slot,"column_index":column.index,"decoded_canonical_ssz":"0x".to_string()+&hex::encode(column.sidecar.as_ssz_bytes())})
    );
    column
}
#[test]
fn actual_gloas_tcp_partial_rows_recover_after_dropped_delivery_and_reconnect() {
    let rt = Arc::new(Runtime::new().unwrap());
    let weak = Arc::downgrade(&rt);
    rt.block_on(async move {
  let v=vectors();let config=spec();
  let mut a=peer(weak.clone(),config.clone(),8).await;let mut b=peer(weak,config,16).await;
  let _a_addr=listen(&mut a).await;let b_addr=listen(&mut b).await;
  assert!(a.node.subscribe_kind(GossipKind::DataColumnSidecar(DataColumnSubnetId::new(v.column_index))));
  assert!(b.node.subscribe_kind(GossipKind::DataColumnSidecar(DataColumnSubnetId::new(v.column_index))));
  connect(&mut a,&mut b,b_addr.clone()).await;
  publish(&mut a,&v,"row_0");publish(&mut b,&v,"row_1");
  let mut dropped=false;let mut source_learned=false;
  timeout(Duration::from_secs(20),async {
   while !(dropped&&source_learned) {
    tokio::select! {
     event=a.node.next_event()=>if let NetworkEvent::PartialDataColumnSidecar {column,..}=event {let c=observe(&v,*column,"sender_received_row_1");assert_eq!(c.sidecar.as_ssz_bytes(),bytes(&v.partials["row_1"]));source_learned=true;},
     event=b.node.next_event()=>if let NetworkEvent::PartialDataColumnSidecar {column,..}=event {let c=observe(&v,*column,"receiver_dropped_first_row_0");assert_eq!(c.sidecar.as_ssz_bytes(),bytes(&v.partials["row_0"]));dropped=true;},
    }
   }
  }).await.expect("actual complementary cells were not exchanged");
  // Deliberately lose this application delivery. No acknowledgement/available-bit update
  // is published, so the receiver still lacks row 0 after its real TCP link is closed.
  let remote=b.node.local_peer_id;a.node.__hard_disconnect_testing_only(remote);
  timeout(Duration::from_secs(10),async {loop {
   tokio::select! {e=a.node.next_event()=>if matches!(e,NetworkEvent::PeerDisconnected(_)){break},_=b.node.next_event()=>{}}
  }}).await.expect("real TCP disconnect did not finish");
  connect(&mut a,&mut b,b_addr).await;
  publish(&mut b,&v,"row_1");publish(&mut a,&v,"full");
  timeout(Duration::from_secs(20),async {loop {
   tokio::select! {
    _=a.node.next_event()=>{},
    event=b.node.next_event()=>if let NetworkEvent::PartialDataColumnSidecar {column,..}=event {
     let c=observe(&v,*column,"receiver_recovered_row_0_after_reconnect");
     assert_eq!(c.sidecar.as_ssz_bytes(),bytes(&v.partials["row_0"]));break;
    }
   }
  }}).await.expect("lost row was not recovered over the reconnected real peer");
  publish(&mut b,&v,"full");publish(&mut a,&v,"full");publish(&mut a,&v,"full");
  for (name,p) in [("sender",&a),("receiver",&b)] {
   println!("PARTIAL_PEER {}",serde_json::json!({"name":name,"peer_id":p.node.local_peer_id.to_string(),"node_id":hex::encode(p.node.local_enr().node_id().raw()),"metadata":serde_json::to_value(p.globals.local_metadata.read().clone()).unwrap()}));
  }
 });
}

#[test]
fn actual_partial_group_metadata_and_sidecars_match_independent_wire_vectors() {
    let v = vectors();
    let mut group = vec![1];
    group.extend(
        types::PartialDataColumnGroupId {
            beacon_block_root: v.beacon_root,
            slot: Slot::new(v.slot),
        }
        .as_ssz_bytes(),
    );
    assert_eq!(group, bytes(&v.group_id));
    for name in ["row_0", "row_1", "full"] {
        let partial = column(&v, name);
        let metadata = types::PartialDataColumnPartsMetadata::<E> {
            available: partial.sidecar.cells_present_bitmap.clone(),
            requests: column(&v, "full").sidecar.cells_present_bitmap,
        };
        assert_eq!(
            metadata.as_ssz_bytes(),
            bytes(&v.metadata_all_requested[name])
        );
        assert_eq!(partial.sidecar.as_ssz_bytes(), bytes(&v.partials[name]));
    }
}
