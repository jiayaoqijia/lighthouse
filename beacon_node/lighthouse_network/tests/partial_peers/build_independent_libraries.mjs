// Build unmodified pinned component libraries in an isolated Lodestar checkout.
// This is a transport/codec peer, not a full Lodestar CLI build qualification.
import {createRequire} from "node:module";
import {execFileSync} from "node:child_process";
import path from "node:path";
import {readFileSync} from "node:fs";
const root=path.resolve(process.argv[2]);
const git=(...args)=>execFileSync("git",args,{cwd:root,encoding:"utf8"}).trim();
if(git("rev-parse","HEAD")!=="4e224659179dc8050344b48a397f1791450bf24c"||git("status","--porcelain","--untracked-files=no")!=="")throw Error("independent source differs from pin");
const require=createRequire(root+"/package.json");
const manifest = require.resolve("typescript/package.json");
const compiler = JSON.parse(readFileSync(manifest, "utf8"));
const tsc = path.resolve(path.dirname(manifest), compiler.bin.tsc);
for(const name of ["params","utils","types","config","reqresp"]){
 execFileSync(process.execPath,[tsc,"-p","tsconfig.build.json"],{cwd:root+"/packages/"+name,stdio:"inherit"});
}
// Only the production beacon-node codec/RPC modules are used by the peer.
// Emit their unchanged source without claiming native bindings or the full
// beacon-node type-check/build have passed.
execFileSync(process.execPath,[tsc,"-p","tsconfig.build.json","--noCheck"],{cwd:root+"/packages/beacon-node",stdio:"inherit"});
if(git("status","--porcelain","--untracked-files=no")!=="")throw Error("component build changed independent source");
