const fs = require('fs');
const crypto = require('crypto');
const path = require('path');
const sdkVersion = JSON.parse(fs.readFileSync(path.resolve(path.dirname(require.resolve('@bsv/sdk')), '../../package.json'), 'utf8')).version;
if (sdkVersion !== '2.6.0') throw new Error('Install @bsv/sdk@2.6.0 to regenerate this fixture');
const {HD, Transaction, LockingScript, UnlockingScript, P2PKH, Spend, TransactionSignature, Hash} = require('@bsv/sdk');
const sha256 = input => crypto.createHash('sha256').update(input).digest('hex');
(async () => {
  const vectors = [];
  for (const accountPath of ["m/44'/236'/0'", "m/44'/145'/0'", "m/44'/236'/7'/3/4"]) {
    const account = HD.fromSeed(Array(64).fill(7)).derive(accountPath);
    const child = (branch, index) => account.derive(`m/${branch}/${index}`);
    const firstAddress = child(0,0).privKey.toAddress();
    const xpub = account.toPublic().toString();
    const pairing = {version:1,kind:'wowpay-bsv-pairing',network:'mainnet',accountPath,xpub,walletId:sha256(`WowPay/BSV/Cold/v1:mainnet:${accountPath}:${xpub}`),firstAddress};
    const p2pkh = new P2PKH();
    const previous = new Transaction(2, [{sourceTXID:'00'.repeat(32),sourceOutputIndex:0xffffffff,unlockingScript:new UnlockingScript().writeBin([7]),sequence:0xffffffff}], [0,1].map(index => ({satoshis:50000,lockingScript:p2pkh.lock(child(0,index).privKey.toAddress())})),0);
    const recipient = child(0,8).privKey.toAddress();
    const changeAddress = child(1,0).privKey.toAddress();
    const fee = Math.ceil((10 + 2*148 + 2*34) * 1000/1000);
    const transaction = new Transaction(2,[0,1].map(index=>({sourceTXID:previous.id('hex'),sourceOutputIndex:index,unlockingScript:new UnlockingScript(),sequence:0xfffffffd})),[{satoshis:75000,lockingScript:p2pkh.lock(recipient)},{satoshis:25000-fee,lockingScript:p2pkh.lock(changeAddress)}],0);
    const request = {version:1,kind:'wowpay-bsv-request',pairing,unsignedTransaction:transaction.toHex(),inputs:[0,1].map(index=>({previousTransaction:previous.toHex(),branch:0,index})),recipient,amount:75000,changeIndex:0};
    const review = {commitment:sha256('WowPay/BSV/Cold/Request/v1:'+JSON.stringify(request)),walletId:pairing.walletId,recipient,amount:75000,fee,change:25000-fee,changeAddress,inputCount:2};
    const digests = [];
    const validation = [];
    for (const index of [0,1]) {
      const params={sourceTXID:previous.id('hex'),sourceOutputIndex:index,sourceSatoshis:50000,transactionVersion:2,otherInputs:transaction.inputs.filter((_,i)=>i!==index),outputs:transaction.outputs,inputIndex:index,subscript:previous.outputs[index].lockingScript,inputSequence:0xfffffffd,lockTime:0,scope:0x41};
      digests.push(Buffer.from(Hash.hash256(TransactionSignature.format(params))).toString('hex'));
      transaction.inputs[index].unlockingScript=await p2pkh.unlock(child(0,index).privKey,'all',false,50000,previous.outputs[index].lockingScript).sign(transaction,index);
      validation.push(new Spend({...params,lockingScript:previous.outputs[index].lockingScript,unlockingScript:transaction.inputs[index].unlockingScript}).validate());
    }
    const signed={version:1,kind:'wowpay-bsv-signed',commitment:review.commitment,transactionId:transaction.id('hex'),rawTransaction:transaction.toHex()};
    const payload=JSON.stringify(request);const checksum=sha256(payload);const encoded=Buffer.from(payload).toString('base64');const total=Math.ceil(encoded.length/800);
    const qrFrames=Array.from({length:total},(_,index)=>JSON.stringify({version:1,kind:'wowpay-bsv-qr',checksum,total,index,data:encoded.slice(index*800,(index+1)*800)}));
    vectors.push({accountPath,pairing,request,review,signed,digests,qrFrames,validation});
  }
  fs.writeFileSync(path.join(__dirname, 'bsv-cold-sdk-vectors.json'), JSON.stringify({sdk:'@bsv/sdk@2.6.0',testSeedConstruction:'64 bytes of 0x07; public test fixture, never fund',vectors},null,2)+'\n');
  console.log(vectors.map(v=>({accountPath:v.accountPath,transactionId:v.signed.transactionId,validation:v.validation,commitment:v.review.commitment})));
})().catch(error => {console.error(error);process.exit(1);});
