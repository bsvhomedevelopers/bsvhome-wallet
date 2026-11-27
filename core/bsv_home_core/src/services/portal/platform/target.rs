use crate::alien::AlienHttpMethod;

use super::BsvHomePlatformAccountError;

macro_rules! platform_targets {
    ($($variant:ident => ($path:literal, $method:ident)),+ $(,)?) => {
        pub(super) enum PlatformTarget { $($variant),+ }

        impl TryFrom<&str> for PlatformTarget {
            type Error = BsvHomePlatformAccountError;
            fn try_from(path: &str) -> Result<Self, Self::Error> {
                match path { $($path => Ok(Self::$variant),)+ _ => Err(BsvHomePlatformAccountError::InvalidInput) }
            }
        }

        impl PlatformTarget {
            pub fn path(&self) -> &'static str { match self { $(Self::$variant => $path),+ } }
            pub fn method(&self) -> AlienHttpMethod { match self { $(Self::$variant => AlienHttpMethod::$method),+ } }
        }
    };
}

platform_targets! {
    AccountActionsChallenge => ("portal/account/actions/challenge", Post),
    AccountClosureCancel => ("portal/account/closure/cancel", Post),
    AccountClosureChallenge => ("portal/account/closure/challenge", Post),
    AccountClosureFinish => ("portal/account/closure/finish", Post),
    AccountClosureReopenEmailFinish => ("portal/account/closure/reopen/email/finish", Post),
    AccountClosureReopenPasskeyFinish => ("portal/account/closure/reopen/passkey/finish", Post),
    AccountClosureReopenPasskeyStart => ("portal/account/closure/reopen/passkey/start", Post),
    AccountClosureRequest => ("portal/account/closure/request", Post),
    AccountClosureStatus => ("portal/account/closure/status", Post),
    AccountDevicesPage => ("portal/account/devices/page", Post),
    AccountDevicesRevokeSessions => ("portal/account/devices/revoke-sessions", Post),
    AccountEmailChangeFinish => ("portal/account/email/change/finish", Post),
    AccountEmailChangeStart => ("portal/account/email/change/start", Post),
    AccountPasskeysRevoke => ("portal/account/passkeys/revoke", Post),
    AccountProfile => ("portal/account/profile", Post),
    AccountProfileUpdate => ("portal/account/profile/update", Post),
    AccountSessionRenew => ("portal/account/session/renew", Post),
    AuthPaymailConfig => ("portal/auth/paymail/config", Get),
    AuthPaymailMe => ("portal/auth/paymail/me", Get),
    ChainTransactionsPage => ("portal/chain/transactions/page", Post),
    ChainTransfersBroadcast => ("portal/chain/transfers/broadcast", Post),
    ChainTransfersCancel => ("portal/chain/transfers/cancel", Post),
    ChainTransfersOne => ("portal/chain/transfers/one", Post),
    ChainTransfersPrepare => ("portal/chain/transfers/prepare", Post),
    ChainTransfersResume => ("portal/chain/transfers/resume", Post),
    ChainTransfersResumeChallenge => ("portal/chain/transfers/resume/challenge", Post),
    ChainTransfersRetryBroadcast => ("portal/chain/transfers/retry-broadcast", Post),
    ChainWalletsBalance => ("portal/chain/wallets/balance", Post),
    ChainWalletsSync => ("portal/chain/wallets/sync", Post),
    HdAccounts => ("portal/hd/accounts", Post),
    HdAddresses => ("portal/hd/addresses", Post),
    HdAddressesAllocate => ("portal/hd/addresses/allocate", Post),
    HdBalance => ("portal/hd/balance", Post),
    HdEnroll => ("portal/hd/enroll", Post),
    HdEnrollChallenge => ("portal/hd/enroll/challenge", Post),
    HdScansCancel => ("portal/hd/scans/cancel", Post),
    HdScansOne => ("portal/hd/scans/one", Post),
    HdScansResume => ("portal/hd/scans/resume", Post),
    HdScansStart => ("portal/hd/scans/start", Post),
    IdentityAlternativeKeys => ("portal/identity/alternative/keys", Post),
    IdentityAlternativeKeysEnroll => ("portal/identity/alternative/keys/enroll", Post),
    IdentityAlternativeKeysEnrollChallenge => ("portal/identity/alternative/keys/enroll/challenge", Post),
    IdentityAlternativeKeysRevoke => ("portal/identity/alternative/keys/revoke", Post),
    IdentityAlternativeKeysRevokeChallenge => ("portal/identity/alternative/keys/revoke/challenge", Post),
    IdentityAlternativeRecoveryCancel => ("portal/identity/alternative/recovery/cancel", Post),
    IdentityAlternativeRecoveryChallenge => ("portal/identity/alternative/recovery/challenge", Post),
    IdentityAlternativeRecoveryRegistrationChallenge => ("portal/identity/alternative/recovery/registration/challenge", Post),
    IdentityAlternativeRecoveryRegistrationStart => ("portal/identity/alternative/recovery/registration/start", Post),
    IdentityAlternativeRecoveryRequest => ("portal/identity/alternative/recovery/request", Post),
    IdentityAlternativeRecoveryStatus => ("portal/identity/alternative/recovery/status", Post),
    IdentityEmailFinish => ("portal/identity/email/finish", Post),
    IdentityEmailStart => ("portal/identity/email/start", Post),
    IdentityPasskeyList => ("portal/identity/passkey/list", Post),
    IdentityPasskeyLoginFinish => ("portal/identity/passkey/login/finish", Post),
    IdentityPasskeyLoginStart => ("portal/identity/passkey/login/start", Post),
    IdentityPasskeyRegisterFinish => ("portal/identity/passkey/register/finish", Post),
    IdentityPasskeyRegisterStart => ("portal/identity/passkey/register/start", Post),
    IdentityPasskeyStepUpFinish => ("portal/identity/passkey/step-up/finish", Post),
    IdentityPasskeyStepUpStart => ("portal/identity/passkey/step-up/start", Post),
    MpcPaymailChallenge => ("portal/mpc/paymail/challenge", Post),
    MpcPaymailConfirm => ("portal/mpc/paymail/confirm", Post),
    MpcRecoveryCancel => ("portal/mpc/recovery/cancel", Post),
    MpcRecoveryChallenge => ("portal/mpc/recovery/challenge", Post),
    MpcRecoveryInformationChallenge => ("portal/mpc/recovery/information/challenge", Post),
    MpcRecoveryInformationSubmit => ("portal/mpc/recovery/information/submit", Post),
    MpcRecoveryStart => ("portal/mpc/recovery/start", Post),
    MpcRecoveryStatus => ("portal/mpc/recovery/status", Post),
    MpcRecoveryTimeline => ("portal/mpc/recovery/timeline", Post),
    MpcTasksCancel => ("portal/mpc/tasks/cancel", Post),
    MpcTasksExchange => ("portal/mpc/tasks/exchange", Post),
    MpcTasksPrepare => ("portal/mpc/tasks/prepare", Post),
    MpcTasksStart => ("portal/mpc/tasks/start", Post),
    MpcTasksStatus => ("portal/mpc/tasks/status", Post),
    MpcWalletsOne => ("portal/mpc/wallets/one", Post),
    WalletAuthLogout => ("portal/wallet-auth/logout", Post),
}
