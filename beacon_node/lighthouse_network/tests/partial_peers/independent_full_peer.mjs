import {pathToFileURL} from "node:url";
import {readFileSync,existsSync} from "node:fs";
import path from "node:path";
import {createInterface} from "node:readline";
import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import {execFileSync} from "node:child_process";
const [root,fixturePath]=process.argv.slice(2);
const revision=execFileSync("git",["rev-parse","HEAD"],{cwd:root,encoding:"utf8"}).trim();
assert.equal(revision,"4e224659179dc8050344b48a397f1791450bf24c");
assert.equal(execFileSync("git",["status","--porcelain","--untracked-files=no"],{cwd:root,encoding:"utf8"}).trim(),"");
const dependencies=new Map();
const digest=(data)=>createHash("sha256").update(data).digest("hex");
const dep=async(name)=>{
 let manifest=root+"/packages/beacon-node/node_modules/"+name+"/package.json";
 if(!existsSync(manifest))manifest=root+"/node_modules/"+name+"/package.json";
 const pkg=JSON.parse(readFileSync(manifest,"utf8"));
 const entry=pkg.exports?.["."]?.import;
 assert.equal(typeof entry,"string",`missing ESM export: ${name}`);
 const filename=path.resolve(path.dirname(manifest),entry);
 dependencies.set(name,{name,version:pkg.version,entry_sha256:digest(readFileSync(filename))});
 return import(pathToFileURL(filename).href);
};
const [{createLibp2p},{tcp},{noise},{mplex},{identify},{gossipsub,StrictNoSign}]=await Promise.all([dep("libp2p"),dep("@libp2p/tcp"),dep("@chainsafe/libp2p-noise"),dep("@libp2p/mplex"),dep("@libp2p/identify"),dep("@libp2p/gossipsub")]);
const {msgIdFn,fastMsgIdFn,msgIdToStrFn,DataTransformSnappy}=await import(pathToFileURL(root+"/packages/beacon-node/lib/network/gossip/encoding.js").href);
const {ssz}=await dep("@lodestar/types");
const {multiaddr}=await dep("@multiformats/multiaddr");
const fixture=JSON.parse(readFileSync(fixturePath,"utf8"));
const full=Buffer.from(fixture.full_column_ssz.slice(2),"hex");
const topic="/eth2/d65ba395/data_column_sidecar_0/ssz_snappy";
const topicCache={getTopic:()=>({boundary:{fork:"gloas"},type:"data_column_sidecar"}),getTypeSizes:()=>({minSize:0,maxSize:10*1024*1024})};
const node=await createLibp2p({addresses:{listen:["/ip4/127.0.0.1/tcp/0"]},transports:[tcp()],connectionEncrypters:[noise()],streamMuxers:[mplex()],services:{identify:identify(),pubsub:gossipsub({globalSignaturePolicy:StrictNoSign,allowPublishToZeroTopicPeers:true,emitSelf:false,msgIdFn:(message)=>msgIdFn(topicCache,message),fastMsgIdFn,msgIdToStrFn,dataTransform:new DataTransformSnappy(topicCache,null)})}});
const print=(value)=>process.stdout.write(JSON.stringify(value)+"\n");
const {ReqResp,ReqRespEncoding,collectExactOne}=await dep("@lodestar/reqresp");
const {createBeaconConfig,defaultChainConfig}=await dep("@lodestar/config");
const {MetadataV3}=await import(pathToFileURL(root+"/packages/beacon-node/lib/network/reqresp/protocols.js").href);
const metadataConfig=createBeaconConfig(defaultChainConfig,Buffer.from("0888cde6ee0c51578046793b377d6f6474908f7812b8db4d87a736d73e539d2c","hex"));
const logger={debug(){},verbose(){},info(){},warn(){},error(){}};
const reqresp=new ReqResp({libp2p:node,logger,metricsRegister:null});
reqresp.registerDialOnlyProtocol(MetadataV3("gloas",metadataConfig));await reqresp.start();

let received=0;
node.services.pubsub.addEventListener("message",({detail})=>{try {assert.equal(detail.topic,topic);assert.deepEqual(Buffer.from(detail.data),full);const decoded=ssz.gloas.DataColumnSidecar.deserialize(detail.data);assert.deepEqual(Buffer.from(ssz.gloas.DataColumnSidecar.serialize(decoded)),full);assert.equal(decoded.slot,fixture.slot);assert.equal(decoded.index,fixture.column_index);assert.equal(decoded.column.length,2);assert.equal(decoded.kzgProofs.length,2);received++;print({event:"received",received,ssz:Buffer.from(detail.data).toString("hex")});}catch(error){print({event:"failure",error:String(error)});process.exitCode=1;}});
node.services.pubsub.subscribe(topic);
await node.start();print({event:"ready",peer:node.peerId.toString(),addresses:node.getMultiaddrs().map(a=>a.toString()),source_revision:revision,protocols:node.getProtocols(),node:process.version,dependencies:[...dependencies.values()],lockfile_sha256:digest(readFileSync(root+"/pnpm-lock.yaml")),encoding_sha256:digest(readFileSync(root+"/packages/beacon-node/lib/network/gossip/encoding.js")),ssz_gloas_sha256:digest(readFileSync(root+"/packages/types/lib/gloas/sszTypes.js"))});
const timer=setInterval(()=>{const peers=node.services.pubsub.getMeshPeers(topic);if(peers.length)print({event:"mesh",peers});},500);
for await (const line of createInterface({input:process.stdin})) {
 const command=JSON.parse(line);
 if(command.command==="dial") {await node.dial(multiaddr(command.address));print({event:"dialed"});}
 else if(command.command==="metadata") {const peers=node.getPeers();assert.equal(peers.length,1);const response=await collectExactOne(reqresp.sendRequest(peers[0],"metadata",[3],ReqRespEncoding.SSZ_SNAPPY,new Uint8Array()));const metadata=ssz.fulu.Metadata.deserialize(response.data);print({event:"metadata",cgc:Number(metadata.custodyGroupCount),seq:Number(metadata.seqNumber),ssz:Buffer.from(response.data).toString("hex")});}
 else if(command.command==="publish") {const result=await node.services.pubsub.publish(topic,full);print({event:"published",recipients:result.recipients.map(p=>p.toString())});}
 else if(command.command==="invalid") {await node.services.pubsub.publish(topic,Uint8Array.of(0));print({event:"invalid_published"});}
 else if(command.command==="disconnect") {await Promise.all(node.getPeers().map(p=>node.hangUp(p)));print({event:"disconnected"});}
 else if(command.command==="stop")break;
 else throw Error("unknown control command");
}
clearInterval(timer);print({event:"stopping",resources:process.getActiveResourcesInfo()});await reqresp.stop();await node.stop();process.stdin.pause();print({event:"stopped",received,resources:process.getActiveResourcesInfo()});
