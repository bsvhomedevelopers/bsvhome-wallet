use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use crate::backup::{
    bsv_address, decrypt_backup, encrypt_backup, public_key, random, BackupContext,
    EncryptedBackup, PROTOCOL,
};
use dkls23_secp256k1::{
    protocols::{
        dkg_session::DkgSession, sign_session::SignSession, signing::SignData, Parameters,
        PartyIndex,
    },
    Party,
};
use elliptic_curve::sec1::ToSec1Point;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, Write},
    path::{Path, PathBuf},
};
use zeroize::{Zeroize, Zeroizing};

use crate::identity;

use dkls23_secp256k1::protocols::dkg::{
    BroadcastDerivationPhase2to4, BroadcastDerivationPhase3to4, ProofCommitment,
    TransmitInitMulPhase3to4, TransmitInitZeroSharePhase2to4, TransmitInitZeroSharePhase3to4,
};
use dkls23_secp256k1::protocols::signing::{Broadcast3to4, TransmitPhase1to2, TransmitPhase2to3};
type C = k256::Secp256k1;
type DkgFinal = (
    Vec<ProofCommitment<C>>,
    Vec<TransmitInitZeroSharePhase2to4>,
    Vec<TransmitInitZeroSharePhase3to4>,
    Vec<TransmitInitMulPhase3to4<C>>,
    BTreeMap<PartyIndex, BroadcastDerivationPhase2to4>,
    BTreeMap<PartyIndex, BroadcastDerivationPhase3to4>,
);
type RefreshFinal = (
    Vec<ProofCommitment<C>>,
    Vec<TransmitInitZeroSharePhase2to4>,
    Vec<TransmitInitZeroSharePhase3to4>,
    Vec<TransmitInitMulPhase3to4<C>>,
);

type Result<T> = std::result::Result<T, String>;

fn receive<T: DeserializeOwned>(input: &mut impl BufRead) -> Result<T> {
    let mut line = Zeroizing::new(Vec::new());
    loop {
        let buf = input.fill_buf().map_err(|_| "IO_ERROR")?;
        if buf.is_empty() {
            return Err("CHANNEL_CLOSED".into());
        }
        let n = buf
            .iter()
            .position(|b| *b == b'\n')
            .map(|x| x + 1)
            .unwrap_or(buf.len());
        if line.len() + n > 8_000_000 {
            return Err("MESSAGE_TOO_LARGE".into());
        }
        let done = buf[n - 1] == b'\n';
        line.extend_from_slice(&buf[..n]);
        input.consume(n);
        if done {
            break;
        }
    }
    serde_json::from_slice(&line).map_err(|_| "INVALID_MESSAGE".into())
}

fn send(out: &mut impl Write, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer(&mut *out, value).map_err(|_| "IO_ERROR")?;
    out.write_all(b"\n").map_err(|_| "IO_ERROR")?;
    out.flush().map_err(|_| "IO_ERROR".into())
}

fn save(path: &Path, data: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!("pending-{}", hex::encode(random::<12>()?)));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let mut file = options.open(&temp).map_err(|_| "STORAGE_ERROR")?;
    file.write_all(data).map_err(|_| "STORAGE_ERROR")?;
    file.sync_all().map_err(|_| "STORAGE_ERROR")?;
    std::fs::rename(&temp, path).map_err(|_| "STORAGE_ERROR")?;
    std::fs::File::open(path.parent().ok_or("STORAGE_ERROR")?)
        .and_then(|f| f.sync_all())
        .map_err(|_| "STORAGE_ERROR".into())
}

#[derive(Serialize, Deserialize)]
struct State {
    party: Party,
    wallet_id: String,
    epoch: u64,
    mode: String,
    candidate: Option<Party>,
    used: BTreeSet<String>,
    backup_confirmed: bool,
    restored: bool,
    identity_secret: [u8; 32],
}

impl Drop for State {
    fn drop(&mut self) {
        self.identity_secret.zeroize();
    }
}

fn context(state: &State, pending: bool) -> Result<BackupContext> {
    let p = if pending {
        state.candidate.as_ref().ok_or("NO_PENDING_EPOCH")?
    } else {
        &state.party
    };
    Ok(BackupContext {
        wallet_id: state.wallet_id.clone(),
        epoch: state.epoch + u64::from(pending),
        public_key: public_key(p),
        protocol: PROTOCOL.into(),
        format: 1,
    })
}

fn dkg(input: &mut impl BufRead, output: &mut impl Write, role: u8, sid: Vec<u8>) -> Result<Party> {
    let mut session = DkgSession::<k256::Secp256k1>::new(
        Parameters::new(2, 2).map_err(|_| "PARAMETERS_INVALID")?,
        PartyIndex::new(role).map_err(|_| "ROLE_INVALID")?,
        sid,
    );
    send(output, &session.phase1())?;
    let fragments: Vec<k256::Scalar> = receive(input)?;
    send(
        output,
        &session.phase2(&fragments).map_err(|_| "DKG_PHASE2")?,
    )?;
    let _: Value = receive(input)?;
    send(output, &session.phase3().map_err(|_| "DKG_PHASE3")?)?;
    let (proofs, z2, z3, mul, b2, b3): DkgFinal = receive(input)?;
    let (mut party, _) = session
        .phase4(&proofs, &z2, &z3, &mul, &b2, &b3, |_| String::new())
        .map_err(|_| "DKG_PHASE4")?;
    party.address = bsv_address(&party);
    Ok(party)
}

fn sign(
    input: &mut impl BufRead,
    output: &mut impl Write,
    party: &Party,
    sid: Vec<u8>,
    digest: [u8; 32],
) -> Result<Value> {
    let data = SignData {
        sign_id: sid,
        counterparties: vec![
            PartyIndex::new(if party.party_index.as_u8() == 1 { 2 } else { 1 })
                .map_err(|_| "ROLE_INVALID")?,
        ],
        message_hash: digest,
    };
    let (mut session, round1) = SignSession::new(party, data).map_err(|_| "SIGN_PHASE1")?;
    send(output, &round1)?;
    send(
        output,
        &session
            .phase2(&receive::<Vec<TransmitPhase1to2>>(input)?)
            .map_err(|_| "SIGN_PHASE2")?,
    )?;
    send(
        output,
        &session
            .phase3(&receive::<Vec<TransmitPhase2to3<C>>>(input)?)
            .map_err(|_| "SIGN_PHASE3")?,
    )?;
    let sig = session
        .phase4(&receive::<Vec<Broadcast3to4<C>>>(input)?, true)
        .map_err(|_| "SIGN_PHASE4")?;
    let compact = sig.to_bytes();
    let independent =
        secp256k1::ecdsa::Signature::from_compact(&compact).map_err(|_| "SIGNATURE_INVALID")?;
    let key = secp256k1::PublicKey::from_slice(party.pk.to_sec1_point(true).as_bytes())
        .map_err(|_| "PUBLIC_KEY_INVALID")?;
    secp256k1::Secp256k1::verification_only()
        .verify_ecdsa(secp256k1::Message::from_digest(digest), &independent, &key)
        .map_err(|_| "SIGNATURE_INVALID")?;
    Ok(
        json!({"signature":hex::encode(compact),"der":hex::encode(independent.serialize_der()),"public_key":public_key(party),"address":party.address}),
    )
}

fn refresh(
    input: &mut impl BufRead,
    output: &mut impl Write,
    party: &Party,
    sid: &[u8],
) -> Result<Party> {
    send(output, &party.refresh_complete_phase1())?;
    let (correction, proof, keep2, z2) =
        party.refresh_complete_phase2(sid, &receive::<Vec<k256::Scalar>>(input)?);
    send(output, &(proof, z2))?;
    let _: Value = receive(input)?;
    let (keep3, z3, mulkeep, mul) = party.refresh_complete_phase3(sid, &keep2);
    send(output, &(z3, mul))?;
    let (proofs, z2, z3, mul): RefreshFinal = receive(input)?;
    let updated = party
        .refresh_complete_phase4(sid, &correction, &proofs, &keep3, &z2, &z3, &mulkeep, &mul)
        .map_err(|_| "REFRESH_PHASE4")?;
    if updated.pk != party.pk || updated.address != party.address {
        return Err("WALLET_IDENTITY_CHANGED".into());
    }
    Ok(updated)
}

fn persist(root: &Path, key: &[u8; 32], state: &State) -> Result<()> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "STORAGE_KEY_INVALID")?;
    let nonce = random::<12>()?;
    let plaintext = Zeroizing::new(serde_json::to_vec(state).map_err(|_| "STORAGE_ERROR")?);
    let mut bytes = nonce.to_vec();
    bytes.extend(
        cipher
            .encrypt(
                &Nonce::try_from(nonce.as_slice()).map_err(|_| "NONCE_INVALID")?,
                plaintext.as_ref(),
            )
            .map_err(|_| "STORAGE_ERROR")?,
    );
    save(&root.join("participant.share"), &bytes)
}

fn load(root: &Path, key: &[u8; 32]) -> Result<Option<State>> {
    if !root.join("participant.share").exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(root.join("participant.share")).map_err(|_| "STORAGE_ERROR")?;
    if bytes.len() < 28 {
        return Err("STORAGE_ERROR".into());
    }
    let plain = Zeroizing::new(
        Aes256Gcm::new_from_slice(key)
            .map_err(|_| "STORAGE_ERROR")?
            .decrypt(
                &Nonce::try_from(&bytes[..12]).map_err(|_| "NONCE_INVALID")?,
                &bytes[12..],
            )
            .map_err(|_| "STORAGE_ERROR")?,
    );
    serde_json::from_slice(&plain)
        .map(Some)
        .map_err(|_| "STORAGE_ERROR".into())
}

pub fn run(
    root: PathBuf,
    wallet_id: String,
    key: Zeroizing<[u8; 32]>,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<()> {
    let role = 1;
    std::fs::create_dir_all(&root).map_err(|_| "STORAGE_ERROR")?;
    #[cfg(unix)]
    {
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "STORAGE_ERROR")?;
    }
    let mut state = load(&root, &key)?;
    if state.as_ref().is_some_and(|s| s.wallet_id != wallet_id) {
        return Err("WALLET_IDENTITY_MISMATCH".into());
    }
    send(
        output,
        &json!({"ready":true,"role":role,"protocol":PROTOCOL,"core_commit":"c9c407eae3447bcbfb539d77bb910ceb2d3a005e","api_version":1}),
    )?;
    loop {
        let c: Value = match receive(input) {
            Ok(v) => v,
            Err(e) if e == "CHANNEL_CLOSED" => return Ok(()),
            Err(e) => return Err(e),
        };
        let op = c["op"].as_str().ok_or("COMMAND_INVALID")?;
        if op == "dkg" {
            if state.is_some() {
                return Err("ALREADY_INITIALIZED".into());
            }
            let sid = hex::decode(c["session_id"].as_str().ok_or("SESSION_INVALID")?)
                .map_err(|_| "SESSION_INVALID")?;
            if sid.len() != 32 {
                return Err("SESSION_INVALID".into());
            }
            let wallet = c["wallet_id"]
                .as_str()
                .filter(|x| x.len() <= 64 && !x.is_empty())
                .ok_or("WALLET_INVALID")?
                .to_string();
            if wallet != wallet_id {
                return Err("WALLET_IDENTITY_MISMATCH".into());
            }
            let party = dkg(input, output, role, sid)?;
            let s = State {
                party,
                wallet_id: wallet,
                epoch: 0,
                mode: "ACTIVE".into(),
                candidate: None,
                used: BTreeSet::new(),
                backup_confirmed: false,
                restored: false,
                identity_secret: identity::new_secret()?,
            };
            persist(&root, &key, &s)?;
            send(
                output,
                &json!({"public_key":public_key(&s.party),"address":s.party.address,"epoch":0}),
            )?;
            state = Some(s);
            continue;
        }
        if op == "restore" {
            if role != 1 || state.is_some() {
                return Err("RESTORE_STATE_INVALID".into());
            }
            let backup: EncryptedBackup =
                serde_json::from_value(c["backup"].clone()).map_err(|_| "BACKUP_INVALID")?;
            let recovery_key = Zeroizing::new(
                hex::decode(c["recovery_key"].as_str().ok_or("RECOVERY_KEY_INVALID")?)
                    .map_err(|_| "RECOVERY_KEY_INVALID")?,
            );
            let recovery_key: &[u8; 32] = recovery_key
                .as_slice()
                .try_into()
                .map_err(|_| "RECOVERY_KEY_INVALID")?;
            let expected: BackupContext =
                serde_json::from_value(c["expected"].clone()).map_err(|_| "CONTEXT_INVALID")?;
            if expected.wallet_id != wallet_id {
                return Err("WALLET_IDENTITY_MISMATCH".into());
            }
            let party = decrypt_backup(&backup, recovery_key, &expected)?;
            let s = State {
                party,
                wallet_id: expected.wallet_id,
                epoch: expected.epoch,
                mode: "RESTORED_REFRESH_REQUIRED".into(),
                candidate: None,
                used: BTreeSet::new(),
                backup_confirmed: false,
                restored: true,
                identity_secret: identity::new_secret()?,
            };
            persist(&root, &key, &s)?;
            send(
                output,
                &json!({"status":s.mode,"public_key":public_key(&s.party),"epoch":s.epoch}),
            )?;
            state = Some(s);
            continue;
        }
        let s = state.as_mut().ok_or("NOT_INITIALIZED")?;
        if s.party.party_index.as_u8() != role {
            return Err("ROLE_MISMATCH".into());
        }
        match op {
            "verify_backup" => {
                let backup: EncryptedBackup =
                    serde_json::from_value(c["backup"].clone()).map_err(|_| "BACKUP_INVALID")?;
                let recovery_key = Zeroizing::new(
                    hex::decode(c["recovery_key"].as_str().ok_or("RECOVERY_KEY_INVALID")?)
                        .map_err(|_| "RECOVERY_KEY_INVALID")?,
                );
                let recovery_key: &[u8; 32] = recovery_key
                    .as_slice()
                    .try_into()
                    .map_err(|_| "RECOVERY_KEY_INVALID")?;
                let expected: BackupContext =
                    serde_json::from_value(c["expected"].clone()).map_err(|_| "CONTEXT_INVALID")?;
                if expected != context(s, s.candidate.is_some())? {
                    return Err("BACKUP_CONTEXT_MISMATCH".into());
                }
                let recovered = decrypt_backup(&backup, recovery_key, &expected)?;
                if public_key(&recovered) != expected.public_key {
                    return Err("BACKUP_VERIFY_FAILED".into());
                }
                send(output, &json!({"verified":true,"context":expected}))?;
            }
            "identity_public_key" => send(
                output,
                &json!({"public_key":identity::public_key(&s.identity_secret)?}),
            )?,
            "identity_sign" => {
                let message = c["message"].as_str().ok_or("PAYMAIL_MESSAGE_INVALID")?;
                let proof = identity::sign_binding(
                    &s.identity_secret,
                    message,
                    &s.wallet_id,
                    s.epoch,
                    &s.party.address,
                )?;
                send(output, &proof)?;
            }
            "status" => send(
                output,
                &json!({"context":context(s,false)?,"status":s.mode,"address":s.party.address}),
            )?,
            "backup" => {
                if role != 1 {
                    return Err("CLIENT_ONLY".into());
                }
                let pending = s.candidate.is_some();
                let p = if pending {
                    s.candidate.as_ref().ok_or("NO_PENDING_EPOCH")?
                } else {
                    &s.party
                };
                let recovery_key = Zeroizing::new(random::<32>()?);
                let ctx = context(s, pending)?;
                let backup = encrypt_backup(p, &recovery_key, ctx.clone())?;
                let recovered = decrypt_backup(&backup, &recovery_key, &ctx)?;
                if public_key(&recovered) != public_key(p) {
                    return Err("BACKUP_VERIFY_FAILED".into());
                }
                s.backup_confirmed = true;
                persist(&root, &key, s)?;
                send(
                    output,
                    &json!({"context":ctx,"backup_verified":true,"backup":backup,"recovery_key":hex::encode(recovery_key.as_ref())}),
                )?;
            }
            "sign" | "proof" => {
                let proof = op == "proof";
                if !proof && s.mode != "ACTIVE" {
                    return Err("SIGNING_PAUSED".into());
                }
                if proof && s.candidate.is_none() {
                    return Err("NO_PENDING_EPOCH".into());
                }
                if c["epoch"].as_u64() != Some(s.epoch + u64::from(proof)) {
                    return Err("EPOCH_MISMATCH".into());
                }
                let id = c["session_id"]
                    .as_str()
                    .ok_or("SESSION_INVALID")?
                    .to_string();
                let sid = hex::decode(&id).map_err(|_| "SESSION_INVALID")?;
                if sid.len() != 32 || !s.used.insert(id) {
                    return Err("SESSION_REPLAY".into());
                }
                let digest: [u8; 32] = hex::decode(c["digest"].as_str().ok_or("DIGEST_INVALID")?)
                    .map_err(|_| "DIGEST_INVALID")?
                    .try_into()
                    .map_err(|_| "DIGEST_INVALID")?;
                persist(&root, &key, s)?;
                let signature = sign(
                    input,
                    output,
                    if proof {
                        s.candidate.as_ref().ok_or("NO_PENDING_EPOCH")?
                    } else {
                        &s.party
                    },
                    sid,
                    digest,
                )?;
                send(output, &signature)?;
            }
            "refresh" => {
                if s.candidate.is_some() || c["epoch"].as_u64() != Some(s.epoch) {
                    return Err("EPOCH_MISMATCH".into());
                }
                let id = c["session_id"]
                    .as_str()
                    .ok_or("SESSION_INVALID")?
                    .to_string();
                let sid = hex::decode(&id).map_err(|_| "SESSION_INVALID")?;
                if sid.len() != 32 || !s.used.insert(id) {
                    return Err("SESSION_REPLAY".into());
                }
                s.mode = "REFRESH_PENDING".into();
                s.backup_confirmed = false;
                persist(&root, &key, s)?;
                s.candidate = Some(refresh(input, output, &s.party, &sid)?);
                persist(&root, &key, s)?;
                send(
                    output,
                    &json!({"public_key":public_key(s.candidate.as_ref().ok_or("NO_PENDING_EPOCH")?),"pending_epoch":s.epoch+1}),
                )?;
            }
            "abort_refresh" => {
                if s.mode != "REFRESH_PENDING" || c["epoch"].as_u64() != Some(s.epoch) {
                    return Err("REFRESH_ABORT_INVALID".into());
                }
                s.candidate = None;
                s.mode = if s.restored {
                    "RESTORED_REFRESH_REQUIRED"
                } else {
                    "ACTIVE"
                }
                .into();
                s.backup_confirmed = false;
                persist(&root, &key, s)?;
                send(output, &json!({"epoch":s.epoch,"status":s.mode}))?;
            }
            "commit" => {
                if s.candidate.is_none() || c["epoch"].as_u64() != Some(s.epoch + 1) {
                    return Err("EPOCH_MISMATCH".into());
                }
                if role == 1 && !s.backup_confirmed {
                    return Err("BACKUP_CONFIRMATION_REQUIRED".into());
                }
                if c["backup_confirmed"] != true {
                    return Err("BACKUP_CONFIRMATION_REQUIRED".into());
                }
                s.party = s.candidate.take().ok_or("NO_PENDING_EPOCH")?;
                s.restored = false;
                s.epoch += 1;
                s.mode = "ACTIVE".into();
                persist(&root, &key, s)?;
                send(output, &json!({"epoch":s.epoch,"status":s.mode}))?;
            }
            _ => return Err("COMMAND_UNKNOWN".into()),
        }
    }
}

#[cfg(test)]
#[path = "participant_tests.rs"]
mod tests;
