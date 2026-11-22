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
    pending: Vec<PartialDataColumn<E>>,
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
        pending: vec![],
    }
}
async fn listen(peer: &mut Peer) -> libp2p::Multiaddr {
    timeout(Duration::from_secs(10), async {
        loop {
            if let NetworkEvent::NewListenAddr(addr) = peer.node.next_event().await
                && addr.iter().any(|p| matches!(p, Protocol::Tcp(_)))
            {
                return addr;
            }
        }
    })
    .await
    .unwrap()
}
fn subscribed(a: &mut Peer, b: PeerId) -> bool {
    let subscribed = a.node.gossipsub_mut().all_peers().any(|(id, topics)| {
        *id == b
            && topics
                .iter()
                .any(|t| t.as_str().contains("data_column_sidecar_0"))
    });
    let topic =
        libp2p::gossipsub::TopicHash::from_raw("/eth2/d65ba395/data_column_sidecar_0/ssz_snappy");
    subscribed
        && a.node
            .gossipsub_mut()
            .mesh_peers(&topic)
            .any(|peer| *peer == b)
}

fn connection_event(peer: &mut Peer, label: &str, event: NetworkEvent<E>) {
    match event {
        NetworkEvent::PartialDataColumnSidecar { column, .. } => peer.pending.push(*column),
        NetworkEvent::PeerConnectedOutgoing(id) => println!("PEER_EVENT {label} outgoing {id}"),
        NetworkEvent::PeerConnectedIncoming(id) => println!("PEER_EVENT {label} incoming {id}"),
        NetworkEvent::PeerDisconnected(id) => println!("PEER_EVENT {label} disconnected {id}"),
        NetworkEvent::StatusPeer(id) => println!("PEER_EVENT {label} status_requested {id}"),
        NetworkEvent::RPCFailed { error, .. } => {
            println!("PEER_EVENT {label} rpc_failed {error:?}")
        }
        _ => {}
    }
}
async fn connect(a: &mut Peer, b: &mut Peer, address: libp2p::Multiaddr) {
    a.node.testing_dial(address).unwrap();
    let result = timeout(Duration::from_secs(20), async {
        loop {
            if subscribed(a, b.node.local_peer_id) && subscribed(b, a.node.local_peer_id) {
                break;
            }
            tokio::select! {
                e=a.node.next_event()=>connection_event(a,"sender",e),
                e=b.node.next_event()=>connection_event(b,"receiver",e),
                // Subscription changes are internal events; next_event need not yield.
                _=tokio::time::sleep(Duration::from_millis(10))=>{}
            }
        }
    })
    .await;
    if result.is_err() {
        for (label, p) in [("sender", a), ("receiver", b)] {
            let topics: Vec<_> = p
                .node
                .gossipsub_mut()
                .all_peers()
                .map(|(id, topics)| {
                    (
                        id.to_string(),
                        topics
                            .iter()
                            .map(|x| x.as_str().to_owned())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
            println!(
                "PEER_DIAGNOSTIC {label} connected={} topics={topics:?}",
                p.globals.connected_peers()
            );
        }
    }
    result.expect("real TCP peers did not negotiate the actual partial column topic");
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
fn actual_gloas_tcp_complementary_rows_match_reference_and_duplicates() {
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
  let mut receiver_learned=false;let mut source_learned=false;
  timeout(Duration::from_secs(20),async {
   while !(receiver_learned&&source_learned) {
    tokio::select! {
     event=a.node.next_event()=>if let NetworkEvent::PartialDataColumnSidecar {column,..}=event {let c=observe(&v,*column,"sender_received_row_1");assert_eq!(c.sidecar.as_ssz_bytes(),bytes(&v.partials["row_1"]));source_learned=true;},
     event=b.node.next_event()=>if let NetworkEvent::PartialDataColumnSidecar {column,..}=event {let c=observe(&v,*column,"receiver_received_row_0");assert_eq!(c.sidecar.as_ssz_bytes(),bytes(&v.partials["row_0"]));receiver_learned=true;},
    }
   }
  }).await.expect("actual complementary cells were not exchanged");
  publish(&mut b,&v,"full");publish(&mut a,&v,"full");publish(&mut a,&v,"full");
  for (name,p) in [("sender",&a),("receiver",&b)] {
   println!("PARTIAL_PEER {}",serde_json::json!({"name":name,"peer_id":p.node.local_peer_id.to_string(),"node_id":hex::encode(p.node.local_enr().node_id().raw()),"metadata":serde_json::to_value(p.globals.local_metadata.read().clone()).unwrap(),"custody_columns":({let mut columns=p.globals.sampling_subnets().into_iter().map(|id|*id).collect::<Vec<_>>();columns.sort_unstable();columns})}));
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

struct FaultProxy {
    address: libp2p::Multiaddr,
    reset: tokio::sync::watch::Sender<u64>,
    worker: tokio::task::JoinHandle<()>,
}
impl Drop for FaultProxy {
    fn drop(&mut self) {
        self.worker.abort();
    }
}
async fn fault_proxy(destination: &libp2p::Multiaddr) -> FaultProxy {
    let port = destination
        .iter()
        .find_map(|p| {
            if let Protocol::Tcp(port) = p {
                Some(port)
            } else {
                None
            }
        })
        .unwrap();
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let local = listener.local_addr().unwrap();
    let (reset, _) = tokio::sync::watch::channel(0u64);
    let notifications = reset.clone();
    let worker = tokio::spawn(async move {
        loop {
            let (mut incoming, _) = listener.accept().await.unwrap();
            let mut changes = notifications.subscribe();
            tokio::spawn(async move {
                let mut outgoing = tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port))
                    .await
                    .unwrap();
                // Opaque encrypted libp2p traffic: the test only resets the physical link.
                tokio::select! {
                 _=tokio::io::copy_bidirectional(&mut incoming,&mut outgoing)=>{},
                 _=changes.changed()=>{},
                }
            });
        }
    });
    FaultProxy {
        address: format!("/ip4/127.0.0.1/tcp/{}", local.port())
            .parse()
            .unwrap(),
        reset,
        worker,
    }
}
#[test]
fn actual_gloas_partial_network_loss_and_reconnect_recover_missing_row() {
    let rt = Arc::new(Runtime::new().unwrap());
    let weak = Arc::downgrade(&rt);
    rt.block_on(async move {
  let v=vectors();let config=spec();let mut a=peer(weak.clone(),config.clone(),8).await;let mut b=peer(weak,config,16).await;
  listen(&mut a).await;let b_addr=listen(&mut b).await;let proxy=fault_proxy(&b_addr).await;
  a.node.subscribe_kind(GossipKind::DataColumnSidecar(DataColumnSubnetId::new(v.column_index)));
  b.node.subscribe_kind(GossipKind::DataColumnSidecar(DataColumnSubnetId::new(v.column_index)));
  connect(&mut a,&mut b,proxy.address.clone()).await;
  // Queue real metadata/cells, then cut before polling the swarms can deliver them.
  // Unlike dropping an already decoded application event, no cell was accepted.
  publish(&mut a,&v,"full");publish(&mut b,&v,"row_1");
  proxy.reset.send_modify(|version|*version+=1);
  timeout(Duration::from_secs(10),async {
   let mut left=false;let mut right=false;
   while !(left&&right) {
    tokio::select! {
     e=a.node.next_event()=>match e {NetworkEvent::PeerDisconnected(_)=>left=true,NetworkEvent::PartialDataColumnSidecar {..}=>panic!("fault occurred after cell delivery"),_=>{}},
     e=b.node.next_event()=>match e {NetworkEvent::PeerDisconnected(_)=>right=true,NetworkEvent::PartialDataColumnSidecar {..}=>panic!("fault occurred after cell delivery"),_=>{}},
    }
   }
  }).await.expect("reset did not close both actual TCP connections");
  connect(&mut a,&mut b,proxy.address.clone()).await;
  publish(&mut b,&v,"row_1");publish(&mut a,&v,"full");
  let mut recovered=false;
  for pending in b.pending.drain(..) {let c=observe(&v,pending,"wire_loss_recovery_during_reconnect");assert_eq!(c.sidecar.as_ssz_bytes(),bytes(&v.partials["row_0"]));recovered=true;}
  if !recovered {
   timeout(Duration::from_secs(20),async {loop {
    tokio::select! {
     _=a.node.next_event()=>{},
     e=b.node.next_event()=>if let NetworkEvent::PartialDataColumnSidecar {column,..}=e {
      let c=observe(&v,*column,"wire_loss_recovered_missing_row_0");assert_eq!(c.sidecar.as_ssz_bytes(),bytes(&v.partials["row_0"]));break;
     }
    }
   }}).await.expect("missing wire delivery was not recovered over the real reconnected peer");
  }
 });
}

struct FutureVersionPartial {
    native: lighthouse_network::types::OutgoingPartialColumnGloas<E>,
    group: Vec<u8>,
    body: Vec<u8>,
}
impl libp2p::gossipsub::partial_messages::Partial for FutureVersionPartial {
    fn group_id(&self) -> Vec<u8> {
        self.group.clone()
    }
    fn metadata(&self) -> Box<dyn libp2p::gossipsub::partial_messages::Metadata> {
        self.native.metadata()
    }
    fn partial_action_from_metadata(
        &self,
        _peer: PeerId,
        _metadata: Option<&[u8]>,
    ) -> Result<
        libp2p::gossipsub::partial_messages::PartialAction,
        libp2p::gossipsub::partial_messages::PartialError,
    > {
        Ok(libp2p::gossipsub::partial_messages::PartialAction {
            need: false,
            send: Some((self.body.clone(), self.native.metadata())),
        })
    }
}
#[test]
fn actual_unknown_partial_version_is_ignored_without_peer_penalty() {
    let rt = Arc::new(Runtime::new().unwrap());
    let weak = Arc::downgrade(&rt);
    rt.block_on(async move {
  let v=vectors();let config=spec();let mut a=peer(weak.clone(),config.clone(),8).await;let mut b=peer(weak,config,16).await;
  listen(&mut a).await;let destination=listen(&mut b).await;
  a.node.subscribe_kind(GossipKind::DataColumnSidecar(DataColumnSubnetId::new(v.column_index)));
  b.node.subscribe_kind(GossipKind::DataColumnSidecar(DataColumnSubnetId::new(v.column_index)));
  connect(&mut a,&mut b,destination).await;
  let remote=a.node.local_peer_id;
  let score_topic=libp2p::gossipsub::IdentTopic::new("/eth2/d65ba395/data_column_sidecar_0/ssz_snappy");
  // Make the classification observable independently of the deployment's topic weights.
  b.node.gossipsub_mut().set_topic_params(score_topic,libp2p::gossipsub::TopicScoreParams {
   topic_weight:1.0,invalid_message_deliveries_weight:-100.0,invalid_message_deliveries_decay:0.9,..Default::default()
  }).unwrap();
  let before=b.node.gossipsub_mut().peer_score(&remote).unwrap();
  let full=column(&v,"full");let requests=full.sidecar.cells_present_bitmap.clone();
  let native=lighthouse_network::types::OutgoingPartialColumnGloas::new(Arc::new(full),requests);
  let mut group=bytes(&v.group_id);group[0]=0x7f;
  let body=bytes(&v.partials["full"]);
  let topic=libp2p::gossipsub::TopicHash::from_raw("/eth2/d65ba395/data_column_sidecar_0/ssz_snappy");
  a.node.gossipsub_mut().publish_partial(topic,FutureVersionPartial {native,group,body}).unwrap();
  let deadline=tokio::time::sleep(Duration::from_secs(2));tokio::pin!(deadline);
  loop {
   tokio::select! {
    _=a.node.next_event()=>{},
    event=b.node.next_event()=>assert!(!matches!(event,NetworkEvent::PartialDataColumnSidecar {..}),"unknown future version was delivered"),
    _=&mut deadline=>break,
   }
  }
  let after=b.node.gossipsub_mut().peer_score(&remote).unwrap();
  assert!(after>=before-0.001,"unknown version was rejected/penalized instead of ignored: before={before}, after={after}");
  let gossip=lighthouse_network::types::GossipTopic::new(GossipKind::DataColumnSidecar(DataColumnSubnetId::new(0)),lighthouse_network::types::GossipEncoding::default(),[0xd6,0x5b,0xa3,0x95]);
  b.node.report_partial_message_validation_failure(remote,gossip);
  let deadline=tokio::time::sleep(Duration::from_secs(2));tokio::pin!(deadline);
  loop {
   if b.node.gossipsub_mut().peer_score(&remote).unwrap()<after-0.1 {break}
   tokio::select! {_=a.node.next_event()=>{},_=b.node.next_event()=>{},_=tokio::time::sleep(Duration::from_millis(10))=>{},_=&mut deadline=>panic!("known-invalid penalty control did not execute")}
  }
 });
}
