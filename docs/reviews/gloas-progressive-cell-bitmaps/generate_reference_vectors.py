# Generate independent unchanged-reference vectors; never derive roots from the client.
import json,hashlib
from pathlib import Path
import subprocess,sys
reference=Path(sys.argv[1]).resolve()
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=reference,text=True).strip()=='d2021d2719235f0df4726a1d330502cb15aa8ea3'
subprocess.run(['git','diff','--exit-code','HEAD','--'],cwd=reference,check=True,stdout=subprocess.DEVNULL)
sys.path.insert(0,str(reference/'tests/core/pyspec'))
from eth_consensus_specs.gloas import minimal as gloas
from eth_consensus_specs.fulu import minimal as fulu
out=Path(sys.argv[2]);out.parent.mkdir(parents=True,exist_ok=True)
rows=[]
for fork,spec in [('fulu',fulu),('gloas',gloas)]:
 for n,indices in [(0,[]),(2,[0,1]),(260,[0,257])]:
  raw=bytearray(n//8+1)
  for index in indices:raw[index//8]|=1<<(index%8)
  raw[n//8]|=1<<(n%8)
  bitmap=spec.CellsBitList.decode_bytes(bytes(raw))
  cells=[spec.Cell(bytes([1+i])*2048) for i in range(len(indices))]
  proofs=[spec.KZGProof(bytes([0x42+i])*48) for i in range(len(indices))]
  sidecar=spec.PartialDataColumnSidecar(cells_present_bitmap=bitmap,partial_column=spec.DataColumn(data=cells),kzg_proofs=spec.KZGProofs(data=proofs))
  metadata=spec.PartialDataColumnPartsMetadata(available=bitmap,requests=bitmap)
  rows.append({'fork':fork,'bits':n,'bitmap_ssz':'0x'+bitmap.encode_bytes().hex(),'bitmap_root':'0x'+bitmap.hash_tree_root().hex(),'sidecar_ssz':'0x'+sidecar.encode_bytes().hex(),'sidecar_root':'0x'+sidecar.hash_tree_root().hex(),'metadata_ssz':'0x'+metadata.encode_bytes().hex(),'metadata_root':'0x'+metadata.hash_tree_root().hex()})
record={'reference':'d2021d2719235f0df4726a1d330502cb15aa8ea3','reference_module_sha256':{name:hashlib.sha256(Path(module.__file__).read_bytes()).hexdigest() for name,module in [('fulu_minimal',fulu),('gloas_minimal',gloas)]},'scope':'Independent current Fulu bounded and Gloas progressive bitmaps, partial sidecar and metadata, including empty and multi-chunk bitmaps','vectors':rows};out.write_text(json.dumps(record,indent=2)+'\n');print('generated',len(rows))
