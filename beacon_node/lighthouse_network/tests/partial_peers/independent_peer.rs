//! Actual encrypted transport between Rust partial-capable gossip and independent JS full gossip.
use super::*;
use serde_json::{Value, json};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
fn retain_wire(name: &str, bytes: &[u8]) {
    if let Ok(directory) = std::env::var("ETH2030_INDEPENDENT_PEER_OUTPUT") {
        let directory = std::path::Path::new(&directory);
        std::fs::create_dir_all(directory).unwrap();
        std::fs::write(directory.join(name), bytes).unwrap();
    }
}
fn retain_event(direction: &str, value: &Value) {
    use std::io::Write;
    if let Ok(directory) = std::env::var("ETH2030_INDEPENDENT_PEER_OUTPUT") {
        let directory = std::path::Path::new(&directory);
        std::fs::create_dir_all(directory).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("events.jsonl"))
            .unwrap();
        writeln!(file, "{}", json!({"direction":direction,"value":value})).unwrap();
    }
}
struct IndependentPeer {
    child: Child,
    input: ChildStdin,
    output: Lines<BufReader<ChildStdout>>,
}
impl IndependentPeer {
    async fn start() -> Self {
        let root = std::env::var("ETH2030_LODESTAR_ROOT")
            .expect("independent Lodestar dependency checkout required");
        let node = std::env::var("ETH2030_NODE_BINARY").unwrap_or_else(|_| "node".into());
        let mut child = Command::new(node)
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/partial_peers/independent_full_peer.mjs"
            ))
            .arg(root)
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/testdata/gloas-multi-row-partials.json"
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap()).lines();
        Self {
            child,
            input,
            output,
        }
    }
    async fn next(&mut self) -> Value {
        let line = self
            .output
            .next_line()
            .await
            .unwrap()
            .expect("independent peer exited");
        println!("INDEPENDENT_PEER {line}");
        let value: Value = serde_json::from_str(&line).unwrap();
        retain_event("independent_to_native", &value);
        assert_ne!(
            value["event"], "failure",
            "independent decode failed: {value}"
        );
        value
    }
    async fn command(&mut self, value: Value) {
        retain_event("native_to_independent", &value);
        let mut line = serde_json::to_vec(&value).unwrap();
        line.push(b'\n');
        self.input.write_all(&line).await.unwrap();
        self.input.flush().await.unwrap();
    }
}
async fn connected(native: &mut Peer, independent: &mut IndependentPeer, remote: PeerId) {
    timeout(Duration::from_secs(30),async {loop {
  if subscribed(native,remote){return;}
  tokio::select! {event=native.node.next_event()=>connection_event(native,"rust",event),_=independent.next()=>{},_=tokio::time::sleep(Duration::from_millis(10))=>{}}
 }}).await.expect("independent full peer never joined the native topic mesh");
}
async fn metadata(native: &mut Peer, independent: &mut IndependentPeer) -> Value {
    independent.command(json!({"command":"metadata"})).await;
    timeout(Duration::from_secs(15), async {
        loop {
            tokio::select! {
             event=native.node.next_event()=>connection_event(native,"metadata",event),
             event=independent.next()=>if event["event"]=="metadata" {break event;}
            }
        }
    })
    .await
    .expect("independent metadata RPC did not complete")
}

#[test]
#[ignore = "requires pinned independent Lodestar JS dependencies; run with ETH2030_LODESTAR_ROOT"]
fn actual_independent_full_peer_fallback_reverse_delivery_and_reconnect() {
    let rt = Arc::new(Runtime::new().unwrap());
    let weak = Arc::downgrade(&rt);
    rt.block_on(async {
  for reverse in [false,true] {
   let vectors=vectors();let mut native=peer(weak.clone(),spec(),8).await;
   let address=listen(&mut native).await;
   native.node.subscribe_kind(GossipKind::DataColumnSidecar(DataColumnSubnetId::new(0)));
   let mut independent=IndependentPeer::start().await;
   let ready=timeout(Duration::from_secs(30),independent.next()).await.unwrap();assert_eq!(ready["event"],"ready");
   let remote:PeerId=ready["peer"].as_str().unwrap().parse().unwrap();
   let dial=format!("{address}/p2p/{}",native.node.local_peer_id);
   independent.command(json!({"command":"dial","address":dial})).await;
   connected(&mut native,&mut independent,remote).await;
   let initial=metadata(&mut native,&mut independent).await;
   assert_eq!(initial["cgc"],8);let mut seq=initial["seq"].as_u64().unwrap();
   for (count,sampling) in [(16,16),(4,8)] {
    // These are the exact subscription and advertisement operations used by
    // NetworkMessage::CustodyCountChanged; peer status re-announcement belongs
    // to the enclosing chain service and is qualified separately.
    native.node.subscribe_new_data_column_subnets(sampling);
    native.node.update_enr_cgc(count);
    // Decreasing duty does not require dropping extra sampling subscriptions;
    // the runtime intentionally retains its previous subscriptions.
    assert_eq!(native.globals.sampling_subnets().len(),16);
    assert!(native.globals.sampling_subnets().len()>=sampling as usize);
    let observed=metadata(&mut native,&mut independent).await;
    assert_eq!(observed["cgc"],count);let next=observed["seq"].as_u64().unwrap();assert!(next>seq);seq=next;
   }
   // Reset the actual encrypted socket and reconnect the same JS identity before delivery.
   independent.command(json!({"command":"disconnect"})).await;
   timeout(Duration::from_secs(15),async {loop {tokio::select! {
    event=native.node.next_event()=>{if matches!(event,NetworkEvent::PeerDisconnected(id) if id==remote){break;}connection_event(&mut native,"rust",event);},
    _=independent.next()=>{}
   }}}).await.expect("independent TCP disconnect was not observed");
   independent.command(json!({"command":"dial","address":dial})).await;
   connected(&mut native,&mut independent,remote).await;
   let after_reconnect=metadata(&mut native,&mut independent).await;assert_eq!(after_reconnect["cgc"],4);assert!(after_reconnect["seq"].as_u64().unwrap()>=seq);
   if reverse {
    let score_topic=libp2p::gossipsub::IdentTopic::new("/eth2/d65ba395/data_column_sidecar_0/ssz_snappy");
    native.node.gossipsub_mut().set_topic_params(score_topic,libp2p::gossipsub::TopicScoreParams{topic_weight:1.0,invalid_message_deliveries_weight:-100.0,invalid_message_deliveries_decay:0.9,..Default::default()}).unwrap();
    independent.command(json!({"command":"publish"})).await;
    timeout(Duration::from_secs(20),async {loop {tokio::select! {
     event=native.node.next_event()=>match event {
      NetworkEvent::PubsubMessage{id,source,message:lighthouse_network::PubsubMessage::DataColumnSidecar(data),..}=>{
       assert_eq!(data.1.as_ssz_bytes(),bytes(&vectors.full_column_ssz));
       retain_wire("js-to-rust.ssz", &data.1.as_ssz_bytes());
       native.node.gossipsub_mut().report_message_validation_result(&id,&source,libp2p::gossipsub::MessageAcceptance::Accept);
       println!("INDEPENDENT_RESULT {}",json!({"direction":"js_full_to_rust","reconnected":true,"bytes_match":true}));break;
      },_=>connection_event(&mut native,"rust",event)
     },_=independent.next()=>{}
    }}}).await.expect("independent full column was not delivered to native decode");
    let before=native.node.gossipsub_mut().peer_score(&remote).unwrap();
    independent.command(json!({"command":"invalid"})).await;
    timeout(Duration::from_secs(5),async {loop {
     if native.node.gossipsub_mut().peer_score(&remote).unwrap()<before-0.1 {break;}
     tokio::select! {event=native.node.next_event()=>connection_event(&mut native,"invalid",event),_=independent.next()=>{},_=tokio::time::sleep(Duration::from_millis(10))=>{}}
    }}).await.expect("independent malformed full gossip was not rejected/scored");
    println!("INDEPENDENT_RESULT {}",json!({"case":"independent_malformed_full_rejected","score_before":before,"score_after":native.node.gossipsub_mut().peer_score(&remote)}));
   } else {
    // Completed columns use the ordinary full publication emitted by
    // NetworkBeaconProcessor after partial assembly. The partial-only extension
    // deliberately does not send cells to peers that lack that extension.
    publish(&mut native,&vectors,"full");
    let full=DataColumnSidecarGloas::<E>::from_ssz_bytes(&bytes(&vectors.full_column_ssz)).unwrap();
    native.node.publish(vec![lighthouse_network::PubsubMessage::DataColumnSidecar(Box::new((DataColumnSubnetId::new(0),Arc::new(types::DataColumnSidecar::Gloas(full)))))]);
    timeout(Duration::from_secs(20),async {loop {tokio::select! {
     event=native.node.next_event()=>connection_event(&mut native,"rust",event),
     event=independent.next()=>if event["event"]=="received" {assert_eq!(event["received"],1);assert_eq!(event["ssz"],hex::encode(bytes(&vectors.full_column_ssz)));retain_wire("rust-to-js.ssz", &hex::decode(event["ssz"].as_str().unwrap()).unwrap());println!("INDEPENDENT_RESULT {}",json!({"direction":"rust_partial_to_js_full_fallback","reconnected":true,"bytes_match":true}));break;}
    }}}).await.expect("native partial publication did not fall back to independent full gossip");
   }
   independent.command(json!({"command":"stop"})).await;
   independent.input.shutdown().await.unwrap();
   drop(independent.input);
   // Graceful mplex shutdown needs the remote swarm to keep acknowledging
   // stream closes. Waiting only for the child would stop the native reactor.
   let stopped=independent.child.wait();tokio::pin!(stopped);
   let status=timeout(Duration::from_secs(15),async {loop {
    tokio::select! {
     status=&mut stopped=>break status.unwrap(),
     event=native.node.next_event()=>connection_event(&mut native,"shutdown",event),
     line=independent.output.next_line()=>if let Some(line)=line.unwrap(){println!("INDEPENDENT_PEER {line}");},
    }
   }}).await.expect("independent peer shutdown stalled");
   assert!(status.success(),"independent peer failed");
  }
 });
}
