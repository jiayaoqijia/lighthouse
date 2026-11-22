import importlib,json,hashlib
from pathlib import Path
from eth_consensus_specs.test.helpers.genesis import create_genesis_state
from eth_consensus_specs.test.helpers.attester_slashings import get_valid_attester_slashing_by_indices
out=Path(__file__).parent
out.mkdir(parents=True,exist_ok=True)
EXPECTED_GENERATED_SHAS={'minimal': 'aa50441092c6599819bbb2d987ed9c801203daa24fa40f4816c0e8665f82cde4', 'mainnet': '533f62dc20e90272a314365ce900e0bef0a70769dc71a5eba97d2fae262a3cf3'}
results=[]
for preset in ['minimal','mainnet']:
 spec=importlib.import_module('eth_consensus_specs.gloas.'+preset)
 if hashlib.sha256(Path(spec.__file__).read_bytes()).hexdigest()!=EXPECTED_GENERATED_SHAS[preset]:raise RuntimeError('reference source mismatch')
 state=create_genesis_state(spec,[32_000_000_000]*64,32_000_000_000)
 anchor=spec.BeaconBlock(slot=state.slot,state_root=state.hash_tree_root())
 store=spec.get_forkchoice_store(state,anchor)
 root=spec.hash_tree_root(anchor)
 members=sorted(int(i) for committee in range(int(spec.get_committee_count_per_slot(state,spec.get_current_epoch(state)))) for i in spec.get_beacon_committee(state,state.slot,spec.CommitteeIndex(committee)))
 threshold=int(spec.calculate_committee_fraction(state,spec.config.REORG_HEAD_WEIGHT_THRESHOLD))
 eq=[];weight=0
 for i in members:
  eq.append(i);weight+=int(state.validators[i].effective_balance)
  if weight>=threshold:break
 signing_state=state.copy()
 spec.process_slots(signing_state,spec.Slot(1))
 slashing=get_valid_attester_slashing_by_indices(spec,signing_state,eq,slot=spec.Slot(0),signed_1=True,signed_2=True)
 if not spec.is_valid_indexed_attestation(state,slashing.attestation_1) or not spec.is_valid_indexed_attestation(state,slashing.attestation_2):raise RuntimeError('real BLS slashing signatures failed')
 spec.on_attester_slashing(store,slashing)
 if set(int(i) for i in store.equivocating_indices)!=set(eq) or store.latest_messages:raise RuntimeError('fork-choice slashing setup failed')
 (out/(preset+'-slashing.ssz')).write_bytes(slashing.encode_bytes())
 competing=slashing.attestation_2.data.target.root
 score=int(spec.get_attestation_score(store,spec.ForkChoiceNode(root=root,payload_status=spec.PAYLOAD_STATUS_PENDING),state))
 weak=bool(spec.is_head_weak(store,root))
 if score!=0 or weak or weight<threshold:raise RuntimeError('counterexample setup failed')
 body=state.encode_bytes();(out/(preset+'-head-state.ssz')).write_bytes(body)
 results.append({'preset':preset,'head_root':'0x'+bytes(root).hex(),'head_slot':int(state.slot),'committee_members':members,'equivocating_indices':eq,'raw_effective_balances':[int(v.effective_balance) for v in state.validators],'active_voting_balances':[int(v.effective_balance) if spec.is_active_validator(v,spec.get_current_epoch(state)) and not v.slashed else 0 for v in state.validators],'total_voting_balance':sum(int(v.effective_balance) for v in state.validators if spec.is_active_validator(v,spec.get_current_epoch(state)) and not v.slashed),'head_attestation_score':score,'equivocation_committee_weight':weight,'head_threshold':threshold,'head_weak':weak,'conflicting_signed_target_root':'0x'+bytes(competing).hex(),'slashing_bls_verified':True,'latest_message_count':len(store.latest_messages),'head_state_sha256':hashlib.sha256(body).hexdigest(),'generated_spec_sha256':hashlib.sha256(Path(spec.__file__).read_bytes()).hexdigest()})
(out/'cases.json').write_text(json.dumps({'source':'2ea9d96a420927eb1a35d7c9399c8b3691c69f6a','scope':'Unchanged reference is_head_weak, real shuffled committee membership and justified balances; isolated algorithm counterexample, not full block admission. A real BLS-verified attester slashing populated equivocating_indices before any latest message for the head; no synthetic latest message is inserted.','cases':results},indent=2)+'\n')
print(json.dumps(results,indent=2))
