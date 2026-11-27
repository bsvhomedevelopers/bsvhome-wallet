const fs = require('fs');
const path = require('path');
const {PrivateKey,Transaction,TransactionSignature,LockingScript,UnlockingScript,Hash,BigNumber,ECDSA,Spend,OP}=require('@bsv/sdk');
const vectors=[];
for(const [threshold,count] of [[2,2],[2,3],[3,3]]) {
 const keys=Array.from({length:count},(_,i)=>PrivateKey.fromString(Buffer.alloc(32,i+1).toString('hex')));
 const publicKeys=keys.map(key=>key.toPublicKey().toString());
 const sorted=[...publicKeys].sort();
 const locking=new LockingScript().writeOpCode(80+threshold);
 sorted.forEach(publicKey=>locking.writeBin(Array.from(Buffer.from(publicKey,'hex'))));
 locking.writeOpCode(80+count).writeOpCode(OP.OP_CHECKMULTISIG);
 const source=new Transaction(1,[{sourceTXID:'04'.repeat(32),sourceOutputIndex:0,unlockingScript:new UnlockingScript(),sequence:0xffffffff}], [{satoshis:10000,lockingScript:locking},{satoshis:20000,lockingScript:locking}],0);
 const fee=Math.ceil((10+2*(41+1+threshold*73)+34+8+1+locking.toBinary().length)*100/1000);
 const transaction=new Transaction(1,[0,1].map(i=>({sourceTXID:source.id('hex'),sourceOutputIndex:i,unlockingScript:new UnlockingScript(),sequence:0xffffffff})),[{satoshis:25000,lockingScript:LockingScript.fromHex('76a91479b000887626b294a914501a4cd226b58b23598388ac')},{satoshis:5000-fee,lockingScript:locking}],0);
 const unsigned=transaction.toHex();
 const params=index=>({sourceTXID:source.id('hex'),sourceOutputIndex:index,sourceSatoshis:(index+1)*10000,transactionVersion:1,otherInputs:transaction.inputs.filter((_,j)=>j!==index),outputs:transaction.outputs,inputIndex:index,subscript:locking,inputSequence:0xffffffff,lockTime:0,scope:0x41});
 const digests=[0,1].map(i=>Buffer.from(Hash.hash256(TransactionSignature.format(params(i)))).toString('hex'));
 const signatures=keys.map((key,k)=>({publicKey:publicKeys[k],signatures:digests.map((digest,inputIndex)=>{const signature=ECDSA.sign(new BigNumber(digest,'hex'),key,true);return {inputIndex,signature:Buffer.from(new TransactionSignature(signature.r,signature.s,0x41).toChecksigFormat()).toString('hex')}})}));
 const selected=signatures.sort((a,b)=>a.publicKey.localeCompare(b.publicKey)).slice(0,threshold);
 const validation=[];
 for(let i=0;i<2;i++) {
  const unlocking=new UnlockingScript().writeOpCode(OP.OP_0);
  selected.forEach(member=>unlocking.writeBin(Array.from(Buffer.from(member.signatures[i].signature,'hex'))));
  transaction.inputs[i].unlockingScript=unlocking;
  const spend=new Spend({...params(i),lockingScript:locking,unlockingScript:unlocking});
  validation.push(spend.validate());
 }
 const subsets = count === 3 && threshold === 2 ? [[0,1],[0,2],[1,2]] : [Array.from({length:threshold},(_,i)=>i)];
 const subsetValidation = subsets.map(subset => [0,1].map(inputIndex => {
  const unlocking = new UnlockingScript().writeOpCode(OP.OP_0);
  subset.map(index=>signatures[index]).forEach(member=>unlocking.writeBin(Array.from(Buffer.from(member.signatures[inputIndex].signature,'hex'))));
  return new Spend({...params(inputIndex), lockingScript:locking, unlockingScript:unlocking}).validate();
 }));
 const baseSignatures = selected.map(member=>Array.from(Buffer.from(member.signatures[0].signature,'hex')));
 const validateMutation = (dummy, reversed, version, scope) => {
  const unlocking = new UnlockingScript();
  if (dummy !== null) unlocking.writeOpCode(dummy);
  const bytes = reversed ? [...baseSignatures].reverse() : baseSignatures;
  bytes.forEach(signature=>unlocking.writeBin([...signature.slice(0,-1),scope]));
  try { return new Spend({...params(0),transactionVersion:version,lockingScript:locking,unlockingScript:unlocking}).validate(); }
  catch { return false; }
 };
 const mutationValidation = [validateMutation(null,false,1,0x41),validateMutation(OP.OP_1,false,1,0x41),validateMutation(OP.OP_0,true,1,0x41),validateMutation(OP.OP_0,false,2,0x41),validateMutation(OP.OP_0,false,1,0x01)];
 const finalized=new Transaction(transaction.version, transaction.inputs, transaction.outputs, transaction.lockTime);
 vectors.push({threshold,count,publicKeys,lockingScript:locking.toHex(),previous:source.toHex(),unsigned,fee,digests,signatures,finalized:finalized.toHex(),transactionId:finalized.id('hex'),validation,subsetValidation,mutationValidation});
}
fs.writeFileSync(path.join(__dirname, 'multisig_sdk_vectors.json'),JSON.stringify({sdk:'@bsv/sdk@2.0.0',vectors},null,2)+'\n');
console.log(vectors.map(v=>({threshold:v.threshold,count:v.count,fee:v.fee,validation:v.validation,transactionId:v.transactionId})));
